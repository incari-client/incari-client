// The zoom keys, while playing.
fn incari_minimap_key(game: &mut crate::game::Game<'_>, key: &str, pressed: bool) {
    use crate::game::{incari_client as c, incari_minimap as m};
    use incari_core::input::Key;
    if !pressed || !game.native_playing() {
        return;
    }
    let key = Key::from_name(key);
    if key == Key::None {
        return;
    }
    let Some((zoom_in, zoom_out, zoom, world_map)) =
        c::settings(m::ID, |s| (s.key("zoom-in"), s.key("zoom-out"), s.int("zoom"), s.key("world-map")))
    else {
        return;
    };
    // The zoom is the module's setting, kept in the profile as the Java module keeps it.
    let set = |steps: i32| {
        c::set_setting(m::ID, "zoom", incari_core::framework::setting::Val::Int(m::zoomed(zoom, steps)))
    };
    if key == zoom_in {
        set(1);
    } else if key == zoom_out {
        set(-1);
    } else if key == world_map {
        c::open(game, Box::new(incari_core::screens::world_map::WorldMap::new()));
    }
}
