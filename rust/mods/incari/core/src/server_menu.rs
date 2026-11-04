//! An Eaglercraft server's own pause menu (ServerPauseMenu.java): its `pause_menu.yml`'s server
//! info and Discord buttons, which take the place of the game's Server Links button, and small
//! pictures beside the menu's buttons and title, as Eagler's 1.8 client shows them.
use crate::canvas::{Image, Key};
use crate::paint::Painter;
use std::collections::HashMap;

/// The pictures' side in GUI units, and their gap to the text.
pub const ICON_SIZE: f32 = 16.0;
pub const ICON_GAP: f32 = 4.0;

/// The canvas ids of the menus' pictures: a block of 256 a menu, by its serial.
const PICTURES: u32 = 0xC05_4000;

/// What the server sent.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ServerMenu {
    /// New with each menu the server sends.
    pub serial: u32,
    pub server_info: Option<ServerButton>,
    pub discord: Option<ServerButton>,
    /// Each picture key (`icon_discord_L`, `icon_title_R`...) to an index into `icons`.
    pub icon_keys: HashMap<String, usize>,
    pub icons: Vec<Image>,
}

/// One of the server's buttons: its text, and the link it opens. A server info button that
/// shows markup in a web view has none, and isn't shown: there's no web view.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerButton {
    pub text: String,
    pub url: Option<String>,
}

impl ServerButton {
    /// Its link if it's a web address: a server mustn't make the page open a `javascript:` one.
    pub fn safe_url(&self) -> Option<&str> {
        let url = self.url.as_deref()?.trim();
        let lower = url.get(..8).unwrap_or(url).to_ascii_lowercase();
        (lower.starts_with("https://") || lower.starts_with("http://")).then_some(url)
    }
}

impl ServerMenu {
    /// Whether it has a button of its own, which replaces the game's Server Links.
    pub fn active(&self) -> bool {
        self.server_info.is_some() || self.discord.is_some()
    }

    /// The buttons shown, with the name their pictures are keyed by: those that open a link.
    pub fn buttons(&self) -> Vec<(&ServerButton, &'static str)> {
        [
            (self.server_info.as_ref(), "serverInfo"),
            (self.discord.as_ref(), "discord"),
        ]
        .into_iter()
        .filter_map(|(b, key)| Some((b?, key)))
        .filter(|(b, _)| b.url.is_some())
        .collect()
    }

    /// Whether `key`'s picture on `side` (`L` or `R`) was sent.
    pub fn has(&self, key: &str, side: char) -> bool {
        self.icon_keys.contains_key(&format!("icon_{key}_{side}"))
    }

    /// The width of a button's text with its pictures beside it.
    pub fn content_width(&self, text_width: f32, key: &str) -> f32 {
        let sides = ['L', 'R'].iter().filter(|s| self.has(key, **s)).count();
        text_width + sides as f32 * (ICON_SIZE + ICON_GAP)
    }

    /// Draws `key`'s picture on `side` at (`x`, `y`), `ICON_SIZE` square; nothing if none came.
    pub fn draw_icon(&self, p: &mut Painter, key: &str, side: char, x: f32, y: f32) {
        let Some(&index) = self.icon_keys.get(&format!("icon_{key}_{side}")) else {
            return;
        };
        let Some(image) = self.icons.get(index) else {
            return;
        };
        let id = PICTURES + (self.serial & 0x3F) * 256 + index as u32;
        let Some(region) = p
            .canvas
            .get(Key::Picture(id, image.width as u16), || image.clone())
        else {
            return;
        };
        let dst = p.place(x, y, ICON_SIZE, ICON_SIZE);
        let argb = p.faded(0xFFFF_FFFF);
        p.list.image(region.src(), dst, argb);
    }

    /// The pictures beside a label of `text_width` centred on `middle`, its top at `top` of a
    /// row `height` tall: as Eagler's client draws them by the game's own buttons and title.
    pub fn draw_beside(
        &self,
        p: &mut Painter,
        key: &str,
        middle: f32,
        text_width: f32,
        top: f32,
        height: f32,
    ) {
        let y = top + ((height - ICON_SIZE) / 2.0).floor();
        let left = middle - (text_width / 2.0).floor() - ICON_GAP - ICON_SIZE;
        let right = middle + ((text_width + 1.0) / 2.0).floor() + ICON_GAP;
        self.draw_icon(p, key, 'L', left, y);
        self.draw_icon(p, key, 'R', right, y);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn button(url: Option<&str>) -> ServerButton {
        ServerButton {
            text: "Store".into(),
            url: url.map(str::to_owned),
        }
    }

    #[test]
    fn only_web_links_open() {
        assert_eq!(
            button(Some(" HTTPS://shop.example ")).safe_url(),
            Some("HTTPS://shop.example")
        );
        assert_eq!(button(Some("javascript:alert(1)")).safe_url(), None);
        assert_eq!(button(None).safe_url(), None);
    }

    #[test]
    fn markup_buttons_are_left_out() {
        let menu = ServerMenu {
            server_info: Some(button(None)),
            discord: Some(button(Some("https://discord.gg/x"))),
            ..Default::default()
        };
        assert!(menu.active());
        let keys: Vec<_> = menu.buttons().iter().map(|(_, k)| *k).collect();
        assert_eq!(keys, ["discord"]);
    }

    #[test]
    fn pictures_widen_the_content() {
        let mut menu = ServerMenu::default();
        menu.icon_keys.insert("icon_discord_L".into(), 0);
        assert_eq!(menu.content_width(50.0, "discord"), 70.0);
        assert_eq!(menu.content_width(50.0, "serverInfo"), 50.0);
    }
}
