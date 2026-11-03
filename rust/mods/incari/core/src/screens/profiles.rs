//! The module profiles (ProfilesScreen.java): switch to one, make a new one, delete one.
use super::list::List;
use crate::screen::{Cx, Go, Screen};
use crate::theme::{CONTROL_WIDTH, SMALL_BUTTON_WIDTH};
use crate::ui;

#[derive(Default)]
pub struct Profiles {
    list: List,
    name: String,
    /// Why the last change didn't happen, shown as the caption.
    problem: Option<String>,
}

impl Screen for Profiles {
    fn draw(&mut self, cx: &mut Cx) {
        let names: Vec<String> = cx.modules.profiles().to_vec();
        let active = cx.modules.active_profile().to_owned();
        let caption = self
            .problem
            .clone()
            .unwrap_or_else(|| format!("Active: {active}. Each keeps its own modules and HUD."));
        let done = cx.host.translate("gui.done");
        let mut switch = None;
        let mut delete = None;
        let mut create = false;
        let name = &mut self.name;
        let pressed = self.list.draw(
            cx,
            "Profiles",
            &caption,
            false,
            |cx, rows| {
                for (i, n) in names.iter().enumerate() {
                    let Some(r) = rows.row(cx, n, "", &[CONTROL_WIDTH, SMALL_BUTTON_WIDTH]) else {
                        continue;
                    };
                    let chosen = *n == active;
                    if ui::button(
                        cx.p,
                        cx.ui,
                        r[0],
                        if chosen { "Active" } else { "Switch" },
                        !chosen,
                    ) {
                        switch = Some(i);
                    }
                    if ui::button(cx.p, cx.ui, r[1], "Delete", names.len() > 1) {
                        delete = Some(i);
                    }
                }
                rows.heading(cx, "New profile");
                if let Some(r) = rows.row(cx, "Name", "", &[CONTROL_WIDTH, SMALL_BUTTON_WIDTH]) {
                    ui::text_field(
                        cx.p,
                        cx.ui,
                        ui::id("profile-name", 0),
                        r[0],
                        name,
                        "Name",
                        24,
                    );
                    create = ui::button(cx.p, cx.ui, r[1], "Create", !name.trim().is_empty());
                }
            },
            &[&done],
            None,
        );
        let mut files = crate::client::HostFiles(&mut *cx.host);
        if let Some(i) = switch {
            cx.modules.switch_profile(&names[i], &mut files);
        }
        if let Some(i) = delete {
            cx.modules.delete_profile(&names[i], &mut files);
        }
        if create {
            let wanted = self.name.trim().to_owned();
            if cx.modules.create_profile(&wanted, &mut files) {
                self.name.clear();
                self.problem = None;
            } else {
                self.problem = Some(format!(
                    "Can't make \"{wanted}\": taken, or not a name a profile can have."
                ));
            }
        }
        if pressed.is_some() {
            cx.go(Go::Back);
        }
    }
}
