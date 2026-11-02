//! The screen recorder (ScreenRecorder.java, ADR 0062), as Eaglercraft's browser builds have it:
//! Record Screen in Incari Settings, or its key (F8), starts recording the game's picture and
//! sound (and the microphone, if set); the same again stops, and the video downloads. While it
//! records, a red "REC" and the time show in the top left corner.
//!
//! The recording itself is the page's (`MediaRecorder` over the canvas's `captureStream` and the
//! game's audio graph, the fork's `native_recorder_*`), behind `Recorder`. The desktop has none:
//! its tile says "Only in the browser build.", as the Java desktop build's does.
use crate::paint::Painter;

/// The recorder's key: F8 (HID 65), as the Java client's.
pub const KEY: u16 = 65;

/// How to record (PlatformRecorder.Options).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    pub frames_per_second: u32,
    pub video_kilobits: u32,
    /// The game's sound in the recording, 0 to 1.
    pub game_volume: f32,
    pub microphone: bool,
    pub microphone_volume: f32,
}

/// Where a recording is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Idle,
    /// Asked to start: waiting on the page (the microphone's permission).
    Starting,
    Recording,
}

/// The platform's recorder (PlatformRecorder).
pub trait Recorder {
    fn supported(&self) -> bool;
    /// Starts recording; when it stops, the video downloads as `file_name` with the format's
    /// extension (`.mp4` or `.webm`, whichever the browser records).
    fn start(&mut self, options: Options, file_name: &str);
    fn stop(&mut self);
    fn state(&self) -> State;
    /// Why the last recording failed or went without the microphone.
    fn error(&self) -> Option<String>;
    fn elapsed_ms(&self) -> u64;
}

/// No recorder: Horizon's builds for now, and the desktop.
#[derive(Default)]
pub struct NoRecorder;

impl Recorder for NoRecorder {
    fn supported(&self) -> bool {
        false
    }
    fn start(&mut self, _options: Options, _file_name: &str) {}
    fn stop(&mut self) {}
    fn state(&self) -> State {
        State::Idle
    }
    fn error(&self) -> Option<String> {
        None
    }
    fn elapsed_ms(&self) -> u64 {
        0
    }
}

/// The recorder's settings, kept in `incari/settings.txt` with the Java client's keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    /// `record-fps`, 15 to 60.
    pub fps: u32,
    /// `record-kbps`, 1000 to 16000.
    pub kbps: u32,
    /// `record-game-volume`, 0 to 100.
    pub game_volume: u32,
    /// `record-microphone`.
    pub microphone: bool,
    /// `record-microphone-volume`, 0 to 100.
    pub microphone_volume: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            fps: 30,
            kbps: 2500,
            game_volume: 100,
            microphone: false,
            microphone_volume: 100,
        }
    }
}

const KEYS: [&str; 5] = [
    "record-fps",
    "record-kbps",
    "record-game-volume",
    "record-microphone",
    "record-microphone-volume",
];

impl Settings {
    /// From `settings.txt`'s `key=value` lines; a value out of range or not a number keeps the
    /// default, as ClientSettings.Whole does.
    pub fn read(text: &str) -> Self {
        let mut s = Self::default();
        for (k, v) in text.lines().filter_map(|l| l.split_once('=')) {
            let (k, v) = (k.trim(), v.trim());
            let whole = |lo: u32, hi: u32, default: u32| {
                v.parse()
                    .ok()
                    .filter(|n| (lo..=hi).contains(n))
                    .unwrap_or(default)
            };
            match k {
                "record-fps" => s.fps = whole(15, 60, 30),
                "record-kbps" => s.kbps = whole(1000, 16000, 2500),
                "record-game-volume" => s.game_volume = whole(0, 100, 100),
                "record-microphone-volume" => s.microphone_volume = whole(0, 100, 100),
                "record-microphone" => s.microphone = v == "true",
                _ => {}
            }
        }
        s
    }

    /// `settings.txt` with these settings in it: its other lines kept, the recorder's replaced.
    pub fn merged_into(&self, text: &str) -> String {
        let mut out: String = text
            .lines()
            .filter(|l| {
                !l.split_once('=')
                    .is_some_and(|(k, _)| KEYS.contains(&k.trim()))
            })
            .flat_map(|l| [l, "\n"])
            .collect();
        let values = [
            self.fps.to_string(),
            self.kbps.to_string(),
            self.game_volume.to_string(),
            self.microphone.to_string(),
            self.microphone_volume.to_string(),
        ];
        for (k, v) in KEYS.iter().zip(values) {
            out.push_str(&format!("{k}={v}\n"));
        }
        out
    }

    pub fn options(&self) -> Options {
        Options {
            frames_per_second: self.fps,
            video_kilobits: self.kbps,
            game_volume: self.game_volume as f32 / 100.0,
            microphone: self.microphone,
            microphone_volume: self.microphone_volume as f32 / 100.0,
        }
    }
}

/// Starts recording, or stops and downloads what was recorded. `local`: the local time
/// (year, month, day, hour, minute, second), for the file's name.
pub fn toggle(recorder: &mut dyn Recorder, settings: &Settings, local: [u32; 6]) {
    if !recorder.supported() {
        return;
    }
    if recorder.state() != State::Idle {
        recorder.stop();
        return;
    }
    recorder.start(settings.options(), &file_name(local));
}

/// `incari-2026-11-01_12.30.05`.
pub fn file_name([y, mo, d, h, mi, s]: [u32; 6]) -> String {
    format!("incari-{y:04}-{mo:02}-{d:02}_{h:02}.{mi:02}.{s:02}")
}

/// Incari Settings' tile line for Record Screen.
pub fn tile_text(recorder: &dyn Recorder) -> &'static str {
    if !recorder.supported() {
        "Only in the browser build."
    } else if recorder.state() != State::Idle {
        "Recording now. Open to stop and save."
    } else {
        "Record the game, its sound and your mic."
    }
}

const RED: u32 = 0xFFFF_4D4D;

/// Over everything while recording: the red dot, "REC" and the time, in the top left corner. The
/// dot blinks once a second, as a camera's does.
pub fn draw_indicator(p: &mut Painter, recorder: &dyn Recorder) {
    if recorder.state() != State::Recording {
        return;
    }
    let ms = recorder.elapsed_ms();
    let seconds = ms / 1000;
    let text = format!("REC {}:{:02}", seconds / 60, seconds % 60);
    let width = p.width(&text) + 16.0;
    p.fill(2.0, 2.0, width, 13.0, 0x9000_0000);
    if ms % 1000 < 600 {
        p.fill(6.0, 6.0, 5.0, 5.0, RED);
    }
    p.text(&text, 14.0, 5.0, 0xFFFF_FFFF, false);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_the_java_clients_keys_and_ranges() {
        let s = Settings::read(
            "show-badges=false\nrecord-fps=60\nrecord-kbps=99\nrecord-microphone=true\n",
        );
        assert_eq!((s.fps, s.kbps, s.microphone), (60, 2500, true));
        let text = s.merged_into("show-badges=false\nrecord-fps=15\n");
        assert!(text.starts_with("show-badges=false\n"));
        assert_eq!(text.matches("record-fps").count(), 1);
        assert_eq!(Settings::read(&text), s);
    }

    #[test]
    fn names_the_video_by_the_time() {
        assert_eq!(
            file_name([2026, 11, 1, 9, 5, 7]),
            "incari-2026-11-01_09.05.07"
        );
        assert_eq!(tile_text(&NoRecorder), "Only in the browser build.");
    }
}
