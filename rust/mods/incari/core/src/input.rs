//! Input as Incari hears it (Horizon's `NativeInput`), and keys as the Java client's profiles
//! store them: USB HID scancodes, as SDL numbers them (`keyboard:229` is Right Shift).

/// One input event, positions in GUI units.
#[derive(Clone, Debug, PartialEq)]
pub enum Input {
    Move { at: [f32; 2] },
    /// GLFW's button numbers: 0 left, 1 right, 2 middle.
    Button { button: i32, pressed: bool, at: [f32; 2], modifiers: u32 },
    Scroll { dx: f32, dy: f32, at: [f32; 2] },
    /// A key by the game's name (`key.keyboard.right.shift`).
    Key { key: Key, name: String, pressed: bool, repeat: bool, modifiers: u32 },
    Text { text: String },
}

/// Modifier bits, GLFW's.
pub const SHIFT: u32 = 1;
pub const CONTROL: u32 = 2;
pub const ALT: u32 = 4;
pub const SUPER: u32 = 8;

/// A key or a mouse button, in the Java client's codes (framework's `Key`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum Key {
    #[default]
    None,
    Keyboard(u16),
    Mouse(u8),
}

impl Key {
    /// How the profiles write it: `keyboard:224`, `mouse:3` or `none`.
    pub fn encode(self) -> String {
        match self {
            Key::None => "none".into(),
            Key::Keyboard(code) => format!("keyboard:{code}"),
            Key::Mouse(button) => format!("mouse:{button}"),
        }
    }

    pub fn decode(text: &str) -> Option<Self> {
        if text == "none" {
            return Some(Key::None);
        }
        let (device, code) = text.split_once(':')?;
        match device {
            "keyboard" => code.parse().ok().map(|c| if c == 0 { Key::None } else { Key::Keyboard(c) }),
            "mouse" => code.parse().ok().map(Key::Mouse),
            _ => None,
        }
    }

    pub fn is_bound(self) -> bool {
        self != Key::None
    }

    /// The key the game names `name` (`key.keyboard.c`).
    pub fn from_name(name: &str) -> Self {
        if let Some(button) = name.strip_prefix("key.mouse.") {
            return match button {
                "left" => Key::Mouse(0),
                "right" => Key::Mouse(1),
                "middle" => Key::Mouse(2),
                n => n.parse::<u8>().map_or(Key::None, |n| Key::Mouse(n.saturating_sub(1))),
            };
        }
        let Some(key) = name.strip_prefix("key.keyboard.") else {
            return Key::None;
        };
        NAMES
            .iter()
            .find(|(_, n, _)| *n == key)
            .map_or(Key::None, |(code, _, _)| Key::Keyboard(*code))
    }

    /// What a settings row shows for it ("Right Shift", "C", "Mouse 4").
    pub fn label(self) -> String {
        match self {
            Key::None => "None".into(),
            Key::Mouse(0) => "Left Button".into(),
            Key::Mouse(1) => "Right Button".into(),
            Key::Mouse(2) => "Middle Button".into(),
            Key::Mouse(n) => format!("Mouse {}", n + 1),
            Key::Keyboard(code) => NAMES
                .iter()
                .find(|(c, _, _)| *c == code)
                .map_or_else(|| format!("Key {code}"), |(_, _, label)| (*label).to_owned()),
        }
    }
}

/// HID usage, the game's key name, and its label.
const NAMES: &[(u16, &str, &str)] = &[
    (4, "a", "A"),
    (5, "b", "B"),
    (6, "c", "C"),
    (7, "d", "D"),
    (8, "e", "E"),
    (9, "f", "F"),
    (10, "g", "G"),
    (11, "h", "H"),
    (12, "i", "I"),
    (13, "j", "J"),
    (14, "k", "K"),
    (15, "l", "L"),
    (16, "m", "M"),
    (17, "n", "N"),
    (18, "o", "O"),
    (19, "p", "P"),
    (20, "q", "Q"),
    (21, "r", "R"),
    (22, "s", "S"),
    (23, "t", "T"),
    (24, "u", "U"),
    (25, "v", "V"),
    (26, "w", "W"),
    (27, "x", "X"),
    (28, "y", "Y"),
    (29, "z", "Z"),
    (30, "1", "1"),
    (31, "2", "2"),
    (32, "3", "3"),
    (33, "4", "4"),
    (34, "5", "5"),
    (35, "6", "6"),
    (36, "7", "7"),
    (37, "8", "8"),
    (38, "9", "9"),
    (39, "0", "0"),
    (40, "enter", "Enter"),
    (41, "escape", "Escape"),
    (42, "backspace", "Backspace"),
    (43, "tab", "Tab"),
    (44, "space", "Space"),
    (45, "minus", "-"),
    (46, "equal", "="),
    (47, "left.bracket", "["),
    (48, "right.bracket", "]"),
    (49, "backslash", "\\"),
    (51, "semicolon", ";"),
    (52, "apostrophe", "'"),
    (53, "grave.accent", "`"),
    (54, "comma", ","),
    (55, "period", "."),
    (56, "slash", "/"),
    (57, "caps.lock", "Caps Lock"),
    (58, "f1", "F1"),
    (59, "f2", "F2"),
    (60, "f3", "F3"),
    (61, "f4", "F4"),
    (62, "f5", "F5"),
    (63, "f6", "F6"),
    (64, "f7", "F7"),
    (65, "f8", "F8"),
    (66, "f9", "F9"),
    (67, "f10", "F10"),
    (68, "f11", "F11"),
    (69, "f12", "F12"),
    (70, "print.screen", "Print Screen"),
    (71, "scroll.lock", "Scroll Lock"),
    (72, "pause", "Pause"),
    (73, "insert", "Insert"),
    (74, "home", "Home"),
    (75, "page.up", "Page Up"),
    (76, "delete", "Delete"),
    (77, "end", "End"),
    (78, "page.down", "Page Down"),
    (79, "right", "Right"),
    (80, "left", "Left"),
    (81, "down", "Down"),
    (82, "up", "Up"),
    (83, "num.lock", "Num Lock"),
    (84, "keypad.divide", "Keypad /"),
    (85, "keypad.multiply", "Keypad *"),
    (86, "keypad.subtract", "Keypad -"),
    (87, "keypad.add", "Keypad +"),
    (88, "keypad.enter", "Keypad Enter"),
    (89, "keypad.1", "Keypad 1"),
    (90, "keypad.2", "Keypad 2"),
    (91, "keypad.3", "Keypad 3"),
    (92, "keypad.4", "Keypad 4"),
    (93, "keypad.5", "Keypad 5"),
    (94, "keypad.6", "Keypad 6"),
    (95, "keypad.7", "Keypad 7"),
    (96, "keypad.8", "Keypad 8"),
    (97, "keypad.9", "Keypad 9"),
    (98, "keypad.0", "Keypad 0"),
    (99, "keypad.decimal", "Keypad ."),
    (101, "menu", "Menu"),
    (224, "left.control", "Left Control"),
    (225, "left.shift", "Left Shift"),
    (226, "left.alt", "Left Alt"),
    (227, "left.win", "Left Super"),
    (228, "right.control", "Right Control"),
    (229, "right.shift", "Right Shift"),
    (230, "right.alt", "Right Alt"),
    (231, "right.win", "Right Super"),
];

/// Keyboard keys by HID usage, for code that names them.
pub mod keys {
    pub const ESCAPE: u16 = 41;
    pub const ENTER: u16 = 40;
    pub const KEYPAD_ENTER: u16 = 88;
    pub const BACKSPACE: u16 = 42;
    pub const DELETE: u16 = 76;
    pub const TAB: u16 = 43;
    pub const LEFT: u16 = 80;
    pub const RIGHT: u16 = 79;
    pub const UP: u16 = 82;
    pub const DOWN: u16 = 81;
    pub const HOME: u16 = 74;
    pub const END: u16 = 77;
    pub const A: u16 = 4;
    pub const C: u16 = 6;
    pub const V: u16 = 25;
    pub const X: u16 = 27;
    pub const Z: u16 = 29;
    pub const F: u16 = 9;
    pub const RIGHT_SHIFT: u16 = 229;
    pub const F1: u16 = 58;
    pub const F2: u16 = 59;
    pub const F3: u16 = 60;
    pub const F11: u16 = 68;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_java_profiles_keys() {
        assert_eq!(Key::decode("keyboard:229"), Some(Key::Keyboard(229)));
        assert_eq!(Key::decode("mouse:3"), Some(Key::Mouse(3)));
        assert_eq!(Key::decode("none"), Some(Key::None));
        assert_eq!(Key::decode("keyboard:0"), Some(Key::None));
        assert_eq!(Key::decode("pad:1"), None);
        assert_eq!(Key::Keyboard(6).encode(), "keyboard:6");
    }

    #[test]
    fn names_keys_as_the_game_does() {
        assert_eq!(Key::from_name("key.keyboard.right.shift"), Key::Keyboard(229));
        assert_eq!(Key::from_name("key.keyboard.c"), Key::Keyboard(6));
        assert_eq!(Key::from_name("key.mouse.4"), Key::Mouse(3));
        assert_eq!(Key::Keyboard(229).label(), "Right Shift");
        assert_eq!(Key::Mouse(3).label(), "Mouse 4");
    }
}
