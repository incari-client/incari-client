// Each frame: the layers read and their textures loaded while the module is on, freed when off
// (Java's ModuleSkyLayers.extract: a pack's large sky costs nothing until it's wanted).
fn incari_customsky_frame(game: &mut crate::game::Game<'_>) {
    use crate::game::{incari_client as c, incari_customsky as m};
    let brightness = c::settings(m::ID, |s| m::level(s.int("brightness"))).unwrap_or(0.0);
    // Read in a world, as the sky is drawn (the title keeps what's loaded).
    if brightness > 0.0 && game.native_player().is_none() {
        return;
    }
    match m::step(brightness, |id| game.native_has_sky_texture(id)) {
        m::Step::Keep => {}
        m::Step::Release => {
            for id in m::take_textures() {
                game.native_drop_sky_texture(id);
            }
        }
        m::Step::Load => {
            for id in m::take_textures() {
                game.native_drop_sky_texture(id);
            }
            // Every pack's layers in the first folder that has any, in their files' order; a
            // texture two layers name is loaded once.
            let mut layers = Vec::new();
            let mut textures: Vec<(String, u64)> = Vec::new();
            for folder in m::FOLDERS {
                for file in m::layer_files(folder, &game.native_resource_files(folder)) {
                    let Some(bytes) = game.native_resource(&format!("assets/minecraft/{file}"))
                    else {
                        continue;
                    };
                    let Some(layer) = m::SkyLayer::parse(&String::from_utf8_lossy(&bytes), &file)
                    else {
                        log::warn!(
                            "Incari: custom sky {file}: can't be drawn (fade times or source)"
                        );
                        continue;
                    };
                    let texture = match textures.iter().find(|(s, _)| s == layer.source()) {
                        Some((_, id)) => Some(*id),
                        None => m::resource_path(layer.source())
                            .and_then(|path| game.native_resource(&path))
                            .and_then(|png| game.native_decode_png(&png))
                            .and_then(|(w, h, rgba)| game.native_sky_texture(w, h, &rgba)),
                    };
                    let Some(id) = texture else {
                        log::warn!("Incari: custom sky {file}: no texture {}", layer.source());
                        continue;
                    };
                    if !textures.iter().any(|(_, t)| *t == id) {
                        textures.push((layer.source().to_owned(), id));
                    }
                    layers.push((layer, id));
                }
                if !layers.is_empty() {
                    log::info!("Incari: custom sky: {} layers from {folder}", layers.len());
                    break;
                }
            }
            m::finish_load(layers);
        }
    }
}
