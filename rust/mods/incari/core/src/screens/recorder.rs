//! The screen recorder's screen (RecorderScreen.java, ADR 0062): start and stop, and how it
//! records (frame rate, quality, the game's sound and the microphone). Recording goes on with the
//! screen closed; the key does the same as the button. Its settings are saved into
//! `settings.txt` with the Java client's keys when it closes.
use super::list::List;
use crate::recorder::{self, Recorder, Settings, State};
use crate::screen::{Cx, Go, Screen};
use crate::theme::CONTROL_WIDTH;
use crate::ui;

/// Quality presets, kilobits a second: what a Chromebook encodes without dropping frames, up.
const QUALITIES: [u32; 4] = [1500, 2500, 5000, 8000];
const RATES: [u32; 2] = [30, 60];

pub struct RecorderScreen {
    list: List,
    recorder: Box<dyn Recorder>,
    settings: Settings,
    /// The record key's name ("F8").
    key: String,
    /// The local time for a recording's name, from the game side.
    local: fn() -> [u32; 6],
    changed: bool,
}

impl RecorderScreen {
    /// `settings_txt`: `incari/settings.txt` as it is now.
    pub fn new(
        recorder: Box<dyn Recorder>,
        settings_txt: &str,
        key: &str,
        local: fn() -> [u32; 6],
    ) -> Self {
        Self {
            list: List::default(),
            recorder,
            settings: Settings::read(settings_txt),
            key: key.to_owned(),
            local,
            changed: false,
        }
    }
}

/// The next of `values` after `current` (the first when it isn't one).
fn cycle(values: &[u32], current: u32) -> u32 {
    let i = values
        .iter()
        .position(|v| *v == current)
        .map_or(0, |i| (i + 1) % values.len());
    values[i]
}

impl Screen for RecorderScreen {
    fn draw(&mut self, cx: &mut Cx) {
        let caption = match self.recorder.error() {
            Some(e) if !e.is_empty() => e,
            _ => format!(
                "Records the game and its sound. The video downloads when you stop. {} also starts and stops it.",
                self.key
            ),
        };
        let toggle = if self.recorder.state() != State::Idle {
            "Stop and Download"
        } else {
            "Start Recording"
        };
        let done = cx.host.translate("gui.done");
        let s = &mut self.settings;
        let mut changed = false;
        let pressed = self.list.draw(
            cx,
            "Record Screen",
            &caption,
            false,
            |cx, rows| {
                if let Some(r) = rows.row(
                    cx,
                    "Frame rate",
                    "Frames per second in the video.",
                    &[CONTROL_WIDTH],
                ) {
                    let fps = if RATES.contains(&s.fps) { s.fps } else { 30 };
                    if ui::button(cx.p, cx.ui, r[0], &format!("{fps} fps"), true) {
                        s.fps = cycle(&RATES, fps);
                        changed = true;
                    }
                }
                let tip = format!(
                    "Higher is sharper but uses more memory while recording, about {} MB a minute.",
                    s.kbps * 60 / 8 / 1000
                );
                if let Some(r) = rows.row(cx, "Quality", &tip, &[CONTROL_WIDTH]) {
                    let kbps = if QUALITIES.contains(&s.kbps) {
                        s.kbps
                    } else {
                        2500
                    };
                    if ui::button(
                        cx.p,
                        cx.ui,
                        r[0],
                        &format!("{:.1} Mbit/s", kbps as f32 / 1000.0),
                        true,
                    ) {
                        s.kbps = cycle(&QUALITIES, kbps);
                        changed = true;
                    }
                }
                let volume =
                    |cx: &mut Cx, r: [f32; 4], id: usize, value: &mut u32, changed: &mut bool| {
                        let label = format!("{value}%");
                        if let Some(v) = ui::slider(
                            cx.p,
                            cx.ui,
                            ui::id("record-volume", id),
                            r,
                            *value as f32 / 100.0,
                            &label,
                        ) {
                            let v = (v * 100.0).round() as u32;
                            *changed |= v != *value;
                            *value = v;
                        }
                    };
                if let Some(r) = rows.row(
                    cx,
                    "Game sound",
                    "How loud the game is in the video compared to what you hear.",
                    &[CONTROL_WIDTH],
                ) {
                    volume(cx, r[0], 0, &mut s.game_volume, &mut changed);
                }
                if let Some(r) = rows.row(
                    cx,
                    "Microphone",
                    "Records your microphone too. Your browser asks for permission the first time.",
                    &[CONTROL_WIDTH],
                ) && ui::switch(cx.p, cx.ui, r[0], s.microphone)
                {
                    s.microphone = !s.microphone;
                    changed = true;
                }
                if s.microphone
                    && let Some(r) = rows.row(cx, "Microphone volume", "", &[CONTROL_WIDTH])
                {
                    volume(cx, r[0], 1, &mut s.microphone_volume, &mut changed);
                }
            },
            &[toggle, &done],
            None,
        );
        self.changed |= changed;
        match pressed {
            Some(0) => recorder::toggle(self.recorder.as_mut(), &self.settings, (self.local)()),
            Some(_) => cx.go(Go::Back),
            None => {}
        }
    }

    fn closed(&mut self, cx: &mut Cx) {
        if self.changed {
            let text = cx
                .host
                .read("settings.txt")
                .and_then(|b| String::from_utf8(b).ok())
                .unwrap_or_default();
            cx.host
                .write("settings.txt", self.settings.merged_into(&text).as_bytes());
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn cycles_through_the_presets() {
        assert_eq!(super::cycle(&super::QUALITIES, 8000), 1500);
        assert_eq!(super::cycle(&super::QUALITIES, 2500), 5000);
        assert_eq!(super::cycle(&super::RATES, 45), 30);
    }
}
