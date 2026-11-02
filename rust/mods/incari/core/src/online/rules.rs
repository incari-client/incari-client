//! The account rules and formats checked on this side (Credentials, RecoveryCodes, ProofOfWork,
//! ServerHash and OnlineService.Featured in Java; Releases is `crate::release`): what the form
//! says before anything is sent, and what the service's answers are read with. The service checks
//! them again.
use super::sha256;

pub const MIN_USERNAME: usize = 3;
pub const MAX_USERNAME: usize = 16;
pub const MIN_PASSWORD: usize = 10;
pub const MAX_PASSWORD: usize = 128;

/// Passwords long enough for the length rule that people still pick most. The service holds the
/// 10,000 most common; this short list catches the obvious ones without a round trip.
const COMMON: &[&str] = &[
    "1234567890", "0987654321", "1234567891", "12345678910", "123456789a", "123456789q", "a123456789",
    "qwertyuiop", "qwerty1234", "qwerty123456", "1q2w3e4r5t", "1qaz2wsx3edc", "asdfghjkl1", "zxcvbnm123",
    "abcdefghij", "abc1234567", "password12", "password123", "password1234", "password1!", "p@ssw0rd12",
    "passw0rd123", "iloveyou12", "iloveyou123", "letmein123", "welcome123", "football12", "football123",
    "baseball12", "princess12", "sunshine12", "monkey1234", "dragon1234", "trustno1234", "minecraft1",
    "minecraft12", "minecraft123", "minecraft1234", "eaglercraft", "eaglercraft1", "eaglercraft123",
    "incariclient", "incari1234", "chromebook1", "chromebook123", "fortnite123", "roblox12345",
];

/// Whether `c` may be in a username: ASCII letters, digits and underscores.
pub fn username_character(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// Java's `String.length()`: UTF-16 units, which the service's limits count.
pub(crate) fn java_len(text: &str) -> usize {
    text.encode_utf16().count()
}

/// What's wrong with `username`, or None if it's one the service could take.
pub fn username_problem(username: &str) -> Option<String> {
    let n = java_len(username);
    if !(MIN_USERNAME..=MAX_USERNAME).contains(&n) {
        return Some(format!("A username is {MIN_USERNAME} to {MAX_USERNAME} characters"));
    }
    if !username.chars().all(username_character) {
        return Some("Only letters, digits and underscores".into());
    }
    None
}

/// What's wrong with `password` for `username`'s account, or None if it's fine.
pub fn password_problem(password: &str, username: &str) -> Option<String> {
    let n = java_len(password);
    if n < MIN_PASSWORD {
        return Some(format!("At least {MIN_PASSWORD} characters"));
    }
    if n > MAX_PASSWORD {
        return Some(format!("At most {MAX_PASSWORD} characters"));
    }
    let lower = password.to_lowercase();
    if COMMON.contains(&lower.as_str()) || one_character(&lower) || counting(&lower) {
        return Some("Too common: anyone could guess it".into());
    }
    if !username.is_empty() && lower.contains(&username.to_lowercase()) {
        return Some("Don't put your username in it".into());
    }
    None
}

/// Two characters or fewer, however long.
fn one_character(text: &str) -> bool {
    let mut seen: Vec<u16> = Vec::new();
    for c in text.encode_utf16() {
        if !seen.contains(&c) {
            seen.push(c);
            if seen.len() > 2 {
                return false;
            }
        }
    }
    true
}

/// 0123456789..., abcdef..., or the same backwards.
fn counting(text: &str) -> bool {
    let units: Vec<i32> = text.encode_utf16().map(i32::from).collect();
    let (mut up, mut down) = (0, 0);
    for pair in units.windows(2) {
        let step = pair[1] - pair[0];
        up += i32::from(step == 1);
        down += i32::from(step == -1);
    }
    let steps = units.len() as i32 - 1;
    up == steps || down == steps
}

// -------------------------------------------------------------------------------------------------
// Recovery codes
// -------------------------------------------------------------------------------------------------

const ALPHABET: &str = "0123456789ABCDEFGHJKMNPQRSTVWXYZ";
const CODE_LENGTH: usize = 12;

/// `typed` as a recovery code in its one written form (`XXXX-XXXX-XXXX`), or None: spaces and
/// dashes are ignored, case too, and I and L read as 1, O as 0, as Crockford's base32 says.
pub fn normalize_code(typed: &str) -> Option<String> {
    let mut code = String::with_capacity(CODE_LENGTH + 2);
    let mut count = 0;
    for c in typed.chars() {
        let mut c = c.to_ascii_uppercase();
        if c == ' ' || c == '-' {
            continue;
        }
        if c == 'I' || c == 'L' {
            c = '1';
        } else if c == 'O' {
            c = '0';
        }
        if !ALPHABET.contains(c) || count == CODE_LENGTH {
            return None;
        }
        if count == 4 || count == 8 {
            code.push('-');
        }
        code.push(c);
        count += 1;
    }
    (count == CODE_LENGTH).then_some(code)
}

/// A code as the service sent it, in the written form.
pub(crate) fn display_code(code: &str) -> String {
    normalize_code(code).unwrap_or_else(|| code.to_uppercase())
}

/// Whether a featured server keeps the protocol's bounds: a name of 1 to 32 characters, an address
/// of 1 to 128 without spaces.
pub fn valid_featured(name: &str, address: &str) -> bool {
    !name.trim().is_empty()
        && java_len(name) <= 32
        && !address.trim().is_empty()
        && java_len(address) <= 128
        && !name.chars().any(char::is_control)
        && !address.chars().any(|c| c.is_control() || c.is_whitespace())
}

// -------------------------------------------------------------------------------------------------
// Proof of work
// -------------------------------------------------------------------------------------------------

/// Sign-up's proof of work: SHA-256 of the 16 challenge bytes then the nonce (8 bytes, big-endian)
/// must start with `bits` zero bits. The two fit one block, so a try is one compression with
/// nothing allocated. Solved in steps (`step`) so the caller can do it between frames.
pub struct ProofOfWork {
    bits: u32,
    block: [u32; 16],
    nonce: u64,
    found: Option<u64>,
}

/// The largest nonce tried: JSON numbers are exact up to 2^53.
pub const MAX_NONCE: u64 = (1 << 53) - 1;

impl ProofOfWork {
    /// None for a challenge that isn't 16 bytes or bits past 64.
    pub fn new(challenge: &[u8], bits: u32) -> Option<Self> {
        if challenge.len() != 16 || bits > 64 {
            return None;
        }
        let mut block = [0u32; 16];
        for (i, word) in block.iter_mut().take(4).enumerate() {
            *word = u32::from_be_bytes([challenge[i * 4], challenge[i * 4 + 1], challenge[i * 4 + 2], challenge[i * 4 + 3]]);
        }
        Some(Self { bits, block, nonce: 0, found: None })
    }

    /// Tries up to `tries` more nonces: the nonce once one passes (None if every nonce failed,
    /// never for a fair challenge, or not yet).
    pub fn step(&mut self, tries: u32) -> Option<u64> {
        if self.found.is_some() {
            return self.found;
        }
        for _ in 0..tries {
            if self.nonce > MAX_NONCE {
                return None;
            }
            if self.passes(self.nonce) {
                self.found = Some(self.nonce);
                return self.found;
            }
            self.nonce += 1;
        }
        None
    }

    /// Whether every nonce was tried and failed.
    pub fn exhausted(&self) -> bool {
        self.found.is_none() && self.nonce > MAX_NONCE
    }

    /// Whether `candidate` answers the challenge. Hot path: allocates nothing.
    pub fn passes(&mut self, candidate: u64) -> bool {
        // Words 0-3 the challenge, 4-5 the nonce, then the padding: a 1 bit, zeros, and the
        // message's length in bits (24 bytes = 192).
        self.block[4] = (candidate >> 32) as u32;
        self.block[5] = candidate as u32;
        self.block[6] = 0x8000_0000;
        for w in &mut self.block[7..15] {
            *w = 0;
        }
        self.block[15] = 192;
        let mut state = sha256::INITIAL;
        sha256::compress(&mut state, &self.block);
        let zeros = if state[0] != 0 { state[0].leading_zeros() } else { 32 + state[1].leading_zeros() };
        zeros >= self.bits
    }
}

/// Base64url without padding (RFC 4648 section 5), as the service writes its challenges; padding
/// is accepted too. None for a character outside the alphabet or a bad length.
pub fn base64_url(text: &str) -> Option<Vec<u8>> {
    const ALPHA: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let text = text.trim_end_matches('=');
    if text.len() % 4 == 1 {
        return None;
    }
    let mut out = Vec::with_capacity(text.len() * 3 / 4);
    let (mut bits, mut count) = (0u32, 0u32);
    for b in text.bytes() {
        let value = ALPHA.iter().position(|&a| a == b)? as u32;
        bits = (bits << 6 | value) & 0xFFFF;
        count += 6;
        if count >= 8 {
            count -= 8;
            out.push((bits >> count) as u8);
        }
    }
    Some(out)
}

// -------------------------------------------------------------------------------------------------
// Server hashes
// -------------------------------------------------------------------------------------------------

/// Bumped if the rules below change, so old and new hashes never match by accident.
pub const SERVER_HASH_PREFIX: &str = "incari-server-v1\n";
const GAME_PORT: u16 = 25565;

/// Where the player is, as the service sees it: SHA-256 of the prefix and the normalized address,
/// as 64 lowercase hex digits.
pub fn server_hash(address: &str) -> String {
    sha256::hex(&format!("{SERVER_HASH_PREFIX}{}", normalize_address(address)))
}

/// The address in its one written form: a lowercase host without a trailing dot, the port only if
/// it isn't the default, a URL reduced to host, port and path (no query, no trailing slash); a
/// relay's join code as typed.
pub fn normalize_address(address: &str) -> String {
    let trimmed = address.trim();
    let lower = trimmed.to_lowercase();
    if lower.starts_with("relay:") {
        return format!("relay:{}", trimmed["relay:".len()..].trim());
    }
    if lower.starts_with("ws://") || lower.starts_with("wss://") {
        let secure = lower.starts_with("wss://");
        let mut rest = &trimmed[if secure { 6 } else { 5 }..];
        if let Some(end) = rest.find(['?', '#']) {
            rest = &rest[..end];
        }
        let (authority, path) = match rest.find('/') {
            Some(slash) => (&rest[..slash], &rest[slash..]),
            None => (rest, ""),
        };
        let path = path.trim_end_matches('/');
        let scheme = if secure { "wss://" } else { "ws://" };
        return format!("{scheme}{}{path}", host_and_port(authority, if secure { 443 } else { 80 }));
    }
    host_and_port(trimmed, GAME_PORT)
}

/// `host[:port]` or `[v6]:port`, lowercased, without the default port or a trailing dot.
fn host_and_port(authority: &str, default_port: u16) -> String {
    let mut host = authority;
    if let Some(at) = host.rfind('@') {
        host = &host[at + 1..];
    }
    let mut port = "";
    if host.starts_with('[') {
        if let Some(close) = host.find(']').filter(|&c| c > 0) {
            port = host[close + 1..].strip_prefix(':').unwrap_or("");
            host = &host[..close + 1];
        }
    } else if let Some(colon) = host.rfind(':')
        && host.find(':') == Some(colon)
    {
        port = &host[colon + 1..];
        host = &host[..colon];
    }
    let host = host.to_lowercase();
    let host = host.trim_end_matches('.');
    if port.is_empty() || port == default_port.to_string() {
        host.to_owned()
    } else {
        format!("{host}:{port}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Shared with the service's own tests (ADR 0058), made with Python's hashlib.
    const VECTORS: &str = include_str!("../../../../../../online/src/test/resources/incari/online/vectors.json");

    fn vectors() -> serde_json::Value {
        serde_json::from_str(VECTORS).unwrap()
    }

    #[test]
    fn server_hashes_match_the_vectors() {
        let v = vectors();
        assert_eq!(v["serverHashPrefix"], SERVER_HASH_PREFIX);
        for each in v["serverHashes"].as_array().unwrap() {
            let address = each["address"].as_str().unwrap();
            assert_eq!(normalize_address(address), each["normalized"].as_str().unwrap(), "{address}");
            assert_eq!(server_hash(address), each["hash"].as_str().unwrap(), "{address}");
        }
    }

    #[test]
    fn proof_of_work_finds_the_vectors_first_nonce() {
        for each in vectors()["proofOfWork"].as_array().unwrap() {
            let challenge = base64_url(each["challenge"].as_str().unwrap()).unwrap();
            let mut work = ProofOfWork::new(&challenge, each["bits"].as_u64().unwrap() as u32).unwrap();
            let nonce = loop {
                if let Some(n) = work.step(1000) {
                    break n;
                }
            };
            assert_eq!(nonce, each["nonce"].as_u64().unwrap());
        }
    }

    #[test]
    fn recovery_codes_match_the_vectors() {
        let v = vectors();
        for each in v["recoveryCodes"]["valid"].as_array().unwrap() {
            assert_eq!(normalize_code(each[0].as_str().unwrap()).as_deref(), each[1].as_str());
        }
        for each in v["recoveryCodes"]["invalid"].as_array().unwrap() {
            assert_eq!(normalize_code(each.as_str().unwrap()), None);
        }
    }

    #[test]
    fn base64_url_with_and_without_padding() {
        assert_eq!(base64_url("AAECAwQFBgcICQoLDA0ODw").unwrap(), (0..16).collect::<Vec<u8>>());
        assert_eq!(base64_url("AAECAwQFBgcICQoLDA0ODw==").unwrap().len(), 16);
        assert_eq!(base64_url("-_8").unwrap(), vec![0xFB, 0xFF]);
        assert_eq!(base64_url("A"), None);
        assert_eq!(base64_url("A!AA"), None);
    }

    #[test]
    fn credentials_rules() {
        assert!(username_problem("ab").is_some());
        assert!(username_problem("a_b-c").is_some());
        assert_eq!(username_problem("Frost_1"), None);
        assert_eq!(password_problem("short", "x").as_deref(), Some("At least 10 characters"));
        assert!(password_problem("Minecraft123", "x").unwrap().starts_with("Too common"));
        assert!(password_problem("aaaaaaaaaaab", "x").unwrap().starts_with("Too common"));
        assert!(password_problem("9876543210", "x").unwrap().starts_with("Too common"));
        assert_eq!(password_problem("my frost house 9", "Frost").as_deref(), Some("Don't put your username in it"));
        assert_eq!(password_problem("correct horse battery", "Frost"), None);
        assert!(password_problem(&"x1".repeat(65), "y").unwrap().starts_with("At most"));
    }

    #[test]
    fn featured_bounds() {
        assert!(valid_featured("Carrotcraft", "wss://eagler.carrot-craft.org"));
        assert!(!valid_featured("", "a"));
        assert!(!valid_featured("a", "has space"));
        assert!(!valid_featured(&"n".repeat(33), "a"));
    }
}
