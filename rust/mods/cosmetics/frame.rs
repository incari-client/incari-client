// Once a frame: the wardrobe read the first time and saved when it changed, kept in step with the
// online service, and the cosmetics' pictures painted into the entity canvas: the still ones once
// (again after a new atlas), moving capes ten times a second while one was drawn lately.
fn incari_cosmetics_frame(game: &mut crate::game::Game<'_>) {
    use crate::game::incari_cosmetics as m;
    use incari_core::cosmetics::{Slot, art, catalog, models, wardrobe};
    thread_local! {
        static START: std::cell::Cell<Option<horizon_os::time::Instant>> = const { std::cell::Cell::new(None) };
    }
    let now = START.with(|s| {
        let start = s.get().unwrap_or_else(|| {
            let now = horizon_os::time::Instant::now();
            s.set(Some(now));
            now
        });
        start.elapsed().as_secs_f64() * 1000.0
    });
    m::with(|s| s.now_ms = now);
    let folder = horizon_os::env::current_dir().unwrap_or_default().join("incari");
    wardrobe::ensure_loaded(|| horizon_os::fs::read(folder.join(wardrobe::FILE)).ok());
    wardrobe::with(|w| incari_core::online::with(|o| w.sync(o, now)));
    if let Some(text) = wardrobe::with(|w| w.take_unsaved()) {
        let path = folder.join(wardrobe::FILE);
        if let Some(dir) = path.parent() {
            let _ = horizon_os::fs::create_dir_all(dir);
        }
        if let Err(e) = horizon_os::fs::write(&path, text.as_bytes()) {
            log::warn!("Incari: couldn't save the wardrobe: {e}");
        }
    }

    let (still, moving) = m::with(|s| (!s.painted, s.capes_due()));
    if !still && !moving {
        return;
    }
    let t = (now / 1000.0) as f32;
    let mut pixels = vec![0u32; art::CAPE_WIDTH * art::CAPE_HEIGHT];
    let mut all = true;
    for (n, cape) in catalog::in_slot(Slot::Cape).enumerate().take(m::CAPES) {
        let due = still || (art::animated(cape.id) && m::with(|s| s.cape_moving(n)));
        if !due {
            continue;
        }
        art::cape(cape.id, t, &mut pixels);
        all &= game.native_entity_canvas_upload(m::cape_slot(n), m::CAPE, art::rgba(&pixels));
    }
    if still {
        let mut texture = vec![0u32; models::TEXTURE * models::TEXTURE];
        for cosmetic in catalog::all() {
            let (Some(n), Some(spec)) = (models::number(cosmetic), models::spec(cosmetic.id)) else {
                continue;
            };
            if n >= m::MODELS {
                continue;
            }
            models::texture(cosmetic.id, &spec, &mut texture);
            all &= game.native_entity_canvas_upload(m::model_slot(n), [m::MODEL, m::MODEL], art::rgba(&texture));
        }
        // No canvas yet (the boot screen's atlas): again next frame.
        m::with(|s| s.painted = all);
    }
}
