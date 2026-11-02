//! The service against the fake (OnlineServiceTest, SocialTest and PresenceTest in Java).
use super::fake::{Client, Fake};
use super::service::{Answer, Effect, Failure, Online, Outcome, ReportReason, Status};
use super::social::{Delivery, MessagesFrom, Peer};
use super::world::Uuid;
use serde_json::json;
use std::collections::HashMap;

const PASSWORD: &str = "correct horse battery";

fn started(fake: &mut Fake) -> Client {
    let mut c = Client::new(None);
    c.online.start();
    c.settle(fake);
    c
}

fn result(c: &mut Client, fake: &mut Fake, ticket: u32) -> Outcome {
    c.settle(fake);
    c.online.take_result(ticket).expect("an outcome")
}

fn signed_up(fake: &mut Fake, name: &str) -> Client {
    let mut c = started(fake);
    let t = c.online.sign_up(name, PASSWORD);
    assert!(matches!(result(&mut c, fake, t), Ok(Answer::Codes(_))));
    c
}

#[test]
fn an_empty_url_is_off_and_never_connects() {
    let mut off = Online::new("", "incari/26.3", None, 1);
    off.start();
    off.tick(0.0, 0);
    assert_eq!(off.status(), Status::Off);
    assert!(off.take_effects().is_empty());
}

#[test]
fn nothing_connects_before_start() {
    let mut fake = Fake::new();
    let mut c = Client::new(None);
    c.settle(&mut fake);
    assert!(fake.sockets.is_empty());
    assert_eq!(c.online.status(), Status::Connecting);
}

#[test]
fn hello_leaves_the_client_signed_out() {
    let mut fake = Fake::new();
    let c = started(&mut fake);
    assert_eq!(c.online.status(), Status::SignedOut);
    assert_eq!(fake.received[0]["t"], "hello");
    assert_eq!(fake.received[0]["v"], 1);
    assert_eq!(fake.received[0]["client"], "incari/26.3");
    assert_eq!(fake.received[0]["id"], 1, "ids count from 1 on each connection");
}

#[test]
fn hello_names_the_release_its_notes_and_featured_servers() {
    let mut fake = Fake::new();
    let mut notes: Vec<String> = vec!["hidden\u{200B}space".into(), "  A changelog  ".into()];
    notes.extend((0..14).map(|i| format!("Change {i}")));
    fake.release = Some(json!({"version": "0.3.1", "notes": notes}));
    fake.featured = Some(json!([{"name": "Carrot", "address": "wss://a"}, {"name": "", "address": "b"}]));
    let c = started(&mut fake);
    assert_eq!(c.online.latest_release(), Some("0.3.1"));
    assert_eq!(c.online.release_notes().len(), 12);
    assert_eq!(c.online.release_notes()[0], "A changelog");
    assert_eq!(c.online.featured_servers().unwrap().len(), 1);

    let mut fake = Fake::new();
    fake.release = Some(json!({"version": "0.3.1 <script>"}));
    assert_eq!(started(&mut fake).online.latest_release(), None);
}

#[test]
fn sign_up_solves_the_challenge_signs_in_and_keeps_only_the_token() {
    let mut fake = Fake::new();
    let mut c = started(&mut fake);
    let t = c.online.sign_up("Frost_1", PASSWORD);
    let Ok(Answer::Codes(codes)) = result(&mut c, &mut fake, t) else { panic!() };
    assert_eq!(codes.len(), 10);
    assert!(codes.iter().all(|c| super::rules::normalize_code(c).as_deref() == Some(c.as_str())));
    assert_eq!(c.online.status(), Status::SignedIn);
    assert_eq!(c.online.account_name(), Some("Frost_1"));
    let saved = c.session.clone().unwrap();
    assert!(saved.contains("token-") && !saved.contains(PASSWORD));
    // After signing in: friends and the wardrobe asked for.
    let kinds: Vec<&str> = fake.received.iter().map(|m| m["t"].as_str().unwrap()).collect();
    assert!(kinds.contains(&"friends") && kinds.contains(&"wardrobe"));
}

#[test]
fn a_taken_username_says_so() {
    let mut fake = Fake::new();
    signed_up(&mut fake, "Frost");
    let mut other = started(&mut fake);
    let t = other.online.sign_up("frost", PASSWORD);
    assert_eq!(result(&mut other, &mut fake, t), Err(Failure::Taken));
    assert_eq!(other.online.status(), Status::SignedOut);
}

#[test]
fn a_sign_up_cancelled_before_its_form_makes_nothing() {
    let mut fake = Fake::new();
    let mut c = started(&mut fake);
    c.online.pow_tries_per_tick = 1;
    let t = c.online.sign_up("Frost", PASSWORD);
    c.settle(&mut fake);
    assert!(c.online.cancel_sign_up());
    for _ in 0..2000 {
        c.settle(&mut fake);
    }
    assert!(c.online.take_result(t).is_none());
    assert!(fake.accounts.is_empty());
}

#[test]
fn codes_of_a_sign_up_whose_screen_closed_wait_for_the_account_screen() {
    let mut fake = Fake::new();
    let mut c = started(&mut fake);
    let t = c.online.sign_up("Frost", PASSWORD);
    c.online.keep_codes(t);
    c.settle(&mut fake);
    assert!(c.online.take_result(t).is_none());
    assert_eq!(c.online.take_unsaved_codes().unwrap().len(), 10);
    assert_eq!(c.online.take_notices()[0].title, "Account created");
}

#[test]
fn a_saved_session_resumes_and_an_ended_one_is_forgotten() {
    let mut fake = Fake::new();
    let first = signed_up(&mut fake, "Frost");
    let mut second = Client::new(first.session.clone());
    assert_eq!(second.online.account_name(), Some("Frost"));
    assert!(second.online.has_saved_session());
    second.online.start();
    second.settle(&mut fake);
    assert_eq!(second.online.status(), Status::SignedIn);
    assert_eq!(fake.received.iter().filter(|m| m["t"] == "resume").count(), 1);

    fake.accounts.get_mut("frost").unwrap().tokens.clear();
    let mut third = Client::new(first.session.clone());
    third.online.start();
    third.settle(&mut fake);
    assert_eq!(third.online.status(), Status::SignedOut);
    assert_eq!(third.online.account_name(), None);
    assert_eq!(third.session, None);
}

#[test]
fn a_wrong_or_limited_sign_in_reads_as_wrong_credentials() {
    let mut fake = Fake::new();
    signed_up(&mut fake, "Frost");
    let mut c = started(&mut fake);
    let t = c.online.sign_in("Frost", "wrong password!");
    assert_eq!(result(&mut c, &mut fake, t), Err(Failure::WrongCredentials));
    fake.fail_next = Some(("signin".into(), "rate_limited".into()));
    let t = c.online.sign_in("Frost", PASSWORD);
    assert_eq!(result(&mut c, &mut fake, t), Err(Failure::WrongCredentials));
    let t = c.online.sign_in("frost", PASSWORD);
    assert_eq!(result(&mut c, &mut fake, t), Ok(Answer::Done));
    assert_eq!(c.online.account_name(), Some("Frost"), "the account's own spelling");
}

#[test]
fn signed_in_requests_fail_at_once_when_signed_out_or_offline() {
    let mut fake = Fake::new();
    let mut c = started(&mut fake);
    let t = c.online.change_password("a", "b");
    assert_eq!(c.online.take_result(t), Some(Err(Failure::SignedOut)));
    let mut off = Client::new(None);
    let t = off.online.sign_in("a", "b");
    assert_eq!(off.online.take_result(t), Some(Err(Failure::Offline)));
}

#[test]
fn recover_normalizes_the_code_and_ends_other_sessions() {
    let mut fake = Fake::new();
    let mut first = signed_up(&mut fake, "Frost");
    let mut c = started(&mut fake);
    let t = c.online.recover("Frost", "not a code", "a new password 1");
    assert_eq!(c.online.take_result(t), Some(Err(Failure::WrongCode)));
    let t = c.online.recover("Frost", "abcd efgh 0003", "a new password 1");
    assert_eq!(result(&mut c, &mut fake, t), Ok(Answer::Done));
    assert_eq!(fake.received.iter().rfind(|m| m["t"] == "recover").unwrap()["code"], "ABCD-EFGH-0003");
    first.settle(&mut fake);
    assert_eq!(first.online.status(), Status::SignedOut, "signedout ended the first session");
    assert_eq!(first.session, None);
}

#[test]
fn account_changes() {
    let mut fake = Fake::new();
    let mut c = signed_up(&mut fake, "Frost");
    let t = c.online.change_password("wrong", "another password 2");
    assert_eq!(result(&mut c, &mut fake, t), Err(Failure::WrongPassword));
    let t = c.online.change_password(PASSWORD, "another password 2");
    assert_eq!(result(&mut c, &mut fake, t), Ok(Answer::Done));
    let t = c.online.new_recovery_codes("another password 2");
    assert!(matches!(result(&mut c, &mut fake, t), Ok(Answer::Codes(c)) if c.len() == 10));
    let t = c.online.set_share_server(false);
    assert_eq!(result(&mut c, &mut fake, t), Ok(Answer::Done));
    assert!(!c.online.shares_server());
    let t = c.online.set_show_badge(false);
    assert_eq!(result(&mut c, &mut fake, t), Ok(Answer::Done));
    assert!(!c.online.shows_badge());
    let t = c.online.delete_account("another password 2");
    assert_eq!(result(&mut c, &mut fake, t), Ok(Answer::Done));
    assert_eq!(c.online.status(), Status::SignedOut);
    assert!(fake.accounts.is_empty());
    assert!(c.online.shows_badge() && c.online.shares_server(), "back to the defaults");
}

#[test]
fn sign_out_everywhere_signs_out_the_other_device() {
    let mut fake = Fake::new();
    let mut first = signed_up(&mut fake, "Frost");
    let mut second = started(&mut fake);
    let t = second.online.sign_in("Frost", PASSWORD);
    result(&mut second, &mut fake, t).unwrap();
    let t = second.online.sign_out(true);
    assert_eq!(result(&mut second, &mut fake, t), Ok(Answer::Done));
    first.settle(&mut fake);
    assert_eq!(first.online.status(), Status::SignedOut);
    assert_eq!(second.online.status(), Status::SignedOut);
}

#[test]
fn signing_out_offline_forgets_the_token_here() {
    let mut fake = Fake::new();
    let mut c = signed_up(&mut fake, "Frost");
    c.reachable = false;
    c.drop_socket(&mut fake);
    c.settle(&mut fake);
    assert_eq!(c.online.status(), Status::Connecting);
    assert_eq!(c.online.account_name(), Some("Frost"), "shown while reconnecting");
    let t = c.online.sign_out(true);
    assert_eq!(c.online.take_result(t), Some(Err(Failure::Offline)));
    let t = c.online.sign_out(false);
    assert_eq!(c.online.take_result(t), Some(Ok(Answer::Done)));
    c.settle(&mut fake);
    assert_eq!(c.session, None);
    assert_eq!(c.online.account_name(), None);
}

#[test]
fn a_dropped_connection_fails_what_waits_and_reconnects_with_back_off() {
    let mut fake = Fake::new();
    let mut c = signed_up(&mut fake, "Frost");
    fake.unanswered.insert("blocks".into());
    let t = c.online.blocks();
    c.settle(&mut fake);
    c.drop_socket(&mut fake);
    assert_eq!(c.online.take_result(t), Some(Err(Failure::Offline)));
    assert_eq!(c.online.status(), Status::Connecting);
    c.settle(&mut fake);
    assert!(c.socket.is_none(), "waits before trying again");
    c.now += 1000.0;
    c.settle(&mut fake);
    assert_eq!(c.online.status(), Status::SignedIn, "resumed on the new connection");
}

#[test]
fn a_request_without_an_answer_times_out_and_a_sign_in_reconnects() {
    let mut fake = Fake::new();
    let mut c = signed_up(&mut fake, "Frost");
    fake.unanswered.insert("blocks".into());
    let t = c.online.blocks();
    c.now += 20_000.0;
    assert_eq!(result(&mut c, &mut fake, t), Err(Failure::Offline));
    assert_eq!(c.online.status(), Status::SignedIn, "an ordinary timeout keeps the connection");

    let mut d = started(&mut fake);
    fake.unanswered.insert("signin".into());
    let t = d.online.sign_in("Frost", PASSWORD);
    d.settle(&mut fake);
    let socket = d.socket.unwrap().0;
    d.now += 20_000.0;
    assert_eq!(result(&mut d, &mut fake, t), Err(Failure::Offline));
    assert_ne!(d.socket.map(|s| s.0), Some(socket), "a fresh connection starts both signed out");
}

#[test]
fn no_hello_in_time_tries_again() {
    let mut fake = Fake::new();
    fake.unanswered.insert("hello".into());
    let mut c = started(&mut fake);
    c.now += 15_000.0;
    c.settle(&mut fake);
    assert!(c.socket.is_none());
    assert_eq!(c.online.status(), Status::Connecting);
}

#[test]
fn a_retired_protocol_stops_trying() {
    let mut fake = Fake::new();
    fake.protocol = 2;
    let mut c = started(&mut fake);
    assert_eq!(c.online.status(), Status::UpdateNeeded);
    c.now += 600_000.0;
    c.settle(&mut fake);
    assert_eq!(fake.sockets.len(), 1);
    let t = c.online.sign_in("a", "b");
    assert_eq!(c.online.take_result(t), Some(Err(Failure::Update)));
}

#[test]
fn no_session_signs_out() {
    let mut fake = Fake::new();
    let mut c = signed_up(&mut fake, "Frost");
    fake.fail_next = Some(("blocks".into(), "no_session".into()));
    let t = c.online.blocks();
    assert_eq!(result(&mut c, &mut fake, t), Err(Failure::SignedOut));
    assert_eq!(c.online.status(), Status::SignedOut);
}

#[test]
fn messages_too_long_for_a_frame_never_go() {
    let mut fake = Fake::new();
    let mut c = signed_up(&mut fake, "Frost");
    let t = c.online.report_username("Steve", ReportReason::Spam, &"x".repeat(20_000));
    assert_eq!(result(&mut c, &mut fake, t), Ok(Answer::Done));
    assert_eq!(fake.reports[0]["note"].as_str().unwrap().len(), 500, "the note is cut to 500");
}

// -------------------------------------------------------------------------------------------------
// Friends and messages
// -------------------------------------------------------------------------------------------------

fn friends(fake: &mut Fake) -> (Client, Client) {
    let mut alex = signed_up(fake, "Alex");
    let mut steve = signed_up(fake, "Steve");
    let t = alex.online.befriend("steve");
    assert_eq!(result(&mut alex, fake, t), Ok(Answer::Done));
    steve.settle(fake);
    let request = steve.online.social().requests()[0].clone();
    assert!(request.incoming);
    assert_eq!(request.label, "Alex");
    let t = steve.online.accept(&request.rid);
    assert_eq!(result(&mut steve, fake, t), Ok(Answer::Done));
    alex.settle(fake);
    (alex, steve)
}

#[test]
fn a_request_by_username_accepted_makes_friends_both_ways() {
    let mut fake = Fake::new();
    let (mut alex, steve) = friends(&mut fake);
    assert_eq!(steve.online.social().friends()[0].username, "Alex");
    assert_eq!(alex.online.social().friends()[0].username, "Steve");
    assert!(alex.online.social().requests().is_empty());
    let notices = alex.online.take_notices();
    assert!(notices.iter().any(|n| n.title == "New friend"), "{notices:?}");
}

#[test]
fn an_unknown_username_is_turned_down() {
    let mut fake = Fake::new();
    let mut alex = signed_up(&mut fake, "Alex");
    let t = alex.online.befriend("Nobody");
    assert_eq!(result(&mut alex, &mut fake, t), Err(Failure::Rejected));
}

#[test]
fn messages_between_friends_count_unread_until_viewed() {
    let mut fake = Fake::new();
    let (mut alex, mut steve) = friends(&mut fake);
    let fid = alex.online.social().friends()[0].fid.clone();
    let to_steve = Peer::new(true, &fid, "Steve");
    let t = alex.online.send_message(&to_steve, "  hello there  ");
    let sending = alex.online.social().existing(&to_steve).unwrap().messages()[0].clone();
    assert_eq!((sending.delivery, sending.text.as_str()), (Delivery::Sending, "hello there"));
    assert_eq!(result(&mut alex, &mut fake, t), Ok(Answer::Done));
    let sent = alex.online.social().existing(&to_steve).unwrap().messages()[0].clone();
    assert_eq!(sent.delivery, Delivery::Sent);
    assert!(sent.mid.starts_with('m'));

    steve.settle(&mut fake);
    assert_eq!(steve.online.social().unread(), 1);
    assert!(steve.online.take_notices().iter().any(|n| n.text == "hello there"));
    let to_alex = Peer::new(true, &fid, "Alex");
    steve.online.set_viewing(Some(&to_alex));
    steve.settle(&mut fake);
    assert_eq!(steve.online.social().unread(), 0);
    assert_eq!(fake.received.last().unwrap()["t"], "read");
    assert_eq!(fake.received.last().unwrap()["mid"], sent.mid.as_str());
}

#[test]
fn history_loads_a_page_and_a_refused_message_shows_failed() {
    let mut fake = Fake::new();
    let (mut alex, mut steve) = friends(&mut fake);
    let fid = alex.online.social().friends()[0].fid.clone();
    let to_steve = Peer::new(true, &fid, "Steve");
    for i in 0..3 {
        let t = alex.online.send_message(&to_steve, &format!("m{i}"));
        result(&mut alex, &mut fake, t).unwrap();
    }
    let to_alex = Peer::new(true, &fid, "Alex");
    let mut fresh = Client::new(steve.session.clone());
    fresh.online.start();
    fresh.settle(&mut fake);
    fresh.online.load_older(&to_alex);
    fresh.settle(&mut fake);
    let c = fresh.online.social().existing(&to_alex).unwrap();
    assert_eq!(c.messages().len(), 3);
    assert!(!c.has_more());
    assert!(c.messages().iter().all(|m| !m.mine));

    fake.accounts.get_mut("steve").unwrap().messages = "nobody".into();
    let t = alex.online.send_message(&to_steve, "anyone?");
    assert_eq!(result(&mut alex, &mut fake, t), Err(Failure::Closed));
    let last = alex.online.social().existing(&to_steve).unwrap().messages().last().unwrap().clone();
    assert_eq!(last.delivery, Delivery::Failed);
    steve.settle(&mut fake);
}

#[test]
fn blocking_a_friend_ends_the_friendship_both_sides() {
    let mut fake = Fake::new();
    let (mut alex, mut steve) = friends(&mut fake);
    let friend = alex.online.social().friends()[0].clone();
    let t = alex.online.block_friend(&friend.fid, &friend.username);
    assert_eq!(result(&mut alex, &mut fake, t), Ok(Answer::Done));
    assert!(alex.online.social().friends().is_empty());
    steve.settle(&mut fake);
    assert!(steve.online.social().friends().is_empty());
    let t = alex.online.blocks();
    let Ok(Answer::Blocks(blocks)) = result(&mut alex, &mut fake, t) else { panic!() };
    assert_eq!(blocks[0].label, "Steve");
    let t = alex.online.unblock(&blocks[0].id);
    assert_eq!(result(&mut alex, &mut fake, t), Ok(Answer::Done));
    assert!(fake.blocks.is_empty());
}

#[test]
fn chat_settings_are_kept_by_the_service() {
    let mut fake = Fake::new();
    let mut alex = signed_up(&mut fake, "Alex");
    let t = alex.online.set_messages_from(MessagesFrom::Nobody);
    assert_eq!(result(&mut alex, &mut fake, t), Ok(Answer::Done));
    let t = alex.online.set_friend_requests(false);
    assert_eq!(result(&mut alex, &mut fake, t), Ok(Answer::Done));
    assert_eq!(alex.online.social().messages_from(), MessagesFrom::Nobody);
    let mut again = Client::new(alex.session.clone());
    again.online.start();
    again.settle(&mut fake);
    assert_eq!(again.online.social().messages_from(), MessagesFrom::Nobody);
    assert!(!again.online.social().friend_requests());
}

#[test]
fn a_service_without_friends_says_so() {
    let mut fake = Fake::new();
    fake.fail_next = Some(("friends".into(), "bad_request".into()));
    let alex = signed_up(&mut fake, "Alex");
    assert_eq!(alex.online.social().available(), Some(false));
}

// -------------------------------------------------------------------------------------------------
// Presence and lookups
// -------------------------------------------------------------------------------------------------

const ALEX: Uuid = 4 << 76 | 1;
const STEVE: Uuid = 4 << 76 | 2;

fn tab() -> HashMap<Uuid, String> {
    [(ALEX, "AlexMC".to_owned()), (STEVE, "SteveMC".to_owned())].into()
}

#[test]
fn players_on_the_same_server_see_each_other_and_can_message() {
    let mut fake = Fake::new();
    let mut alex = signed_up(&mut fake, "Alex");
    let mut steve = signed_up(&mut fake, "Steve");
    alex.online.joined("Play.Example.com", "AlexMC");
    alex.online.tab_list(&tab());
    alex.settle(&mut fake);
    let presence = fake.received.iter().rfind(|m| m["t"] == "presence").unwrap().clone();
    assert_eq!(presence["serverHash"], "60c7ef8098b204588d5356f7ed8421d75d6e864581bb492c0ea3acdc41d30c76");
    assert_eq!(presence["server"], "Play.Example.com");
    assert!(alex.online.player(STEVE).is_none(), "Steve isn't there yet");

    steve.online.joined("play.example.com:25565", "SteveMC");
    steve.online.tab_list(&tab());
    steve.settle(&mut fake);
    alex.settle(&mut fake);
    assert!(steve.online.player(ALEX).is_some(), "Steve's lookup found Alex");
    let seen = alex.online.player(STEVE).expect("Alex heard Steve arrive");
    let pid = seen.pid.clone();

    fake.accounts.get_mut("alex").unwrap().messages = "everyone".into();
    let to_alex = Peer::new(false, steve.online.player(ALEX).unwrap().pid.clone(), "AlexMC");
    let t = steve.online.send_message(&to_alex, "hi from here");
    assert_eq!(result(&mut steve, &mut fake, t), Ok(Answer::Done));
    alex.settle(&mut fake);
    let messaged = alex.online.social().players_messaged().to_vec();
    assert_eq!(messaged, [(pid.clone(), "SteveMC".to_owned())]);

    let t = alex.online.befriend_player("SteveMC", &pid);
    assert_eq!(result(&mut alex, &mut fake, t), Ok(Answer::Done));
    let peer = Peer::new(false, &pid, "SteveMC");
    assert!(alex.online.social().sent_to(&peer).is_some());

    alex.online.left();
    assert!(alex.online.social().players_messaged().is_empty());
    alex.now += 5_000.0;
    alex.settle(&mut fake);
    let last = fake.received.iter().rfind(|m| m["t"] == "presence").unwrap();
    assert_ne!(last["serverHash"], json!(null), "kept through the grace");
    alex.now += 6_000.0;
    alex.settle(&mut fake);
    let clear = fake.received.iter().rfind(|m| m["t"] == "presence").unwrap();
    assert_eq!(clear["serverHash"], json!(null));
}

#[test]
fn presence_not_shared_leaves_the_address_out_and_a_limit_retries_later() {
    let mut fake = Fake::new();
    let mut alex = signed_up(&mut fake, "Alex");
    let t = alex.online.set_share_server(false);
    result(&mut alex, &mut fake, t).unwrap();
    fake.fail_next = Some(("presence".into(), "rate_limited".into()));
    alex.online.joined("a.example", "AlexMC");
    alex.settle(&mut fake);
    let count = |fake: &Fake| fake.received.iter().filter(|m| m["t"] == "presence").count();
    assert_eq!(count(&fake), 1);
    alex.now += 59_000.0;
    alex.settle(&mut fake);
    assert_eq!(count(&fake), 1);
    alex.now += 1_000.0;
    alex.settle(&mut fake);
    assert_eq!(count(&fake), 2);
    let last = fake.received.iter().rfind(|m| m["t"] == "presence").unwrap();
    assert!(last.get("server").is_none());
}

#[test]
fn effects_name_the_socket_and_stale_events_are_dropped() {
    let mut o = Online::new("wss://x", "incari/26.3", None, 3);
    o.start();
    let effects = o.take_effects();
    let Effect::Connect { connection, url } = &effects[0] else { panic!() };
    assert_eq!(url, "wss://x");
    o.received(connection + 7, r#"{"t":"ok","id":1}"#);
    o.opened(*connection);
    assert!(matches!(&o.take_effects()[0], Effect::Send { text, .. } if text.contains("\"hello\"")));
}
