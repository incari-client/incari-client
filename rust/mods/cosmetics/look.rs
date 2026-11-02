// Before a player is drawn: what they wear and the emote they play (ADR 0061, `Cosmetics.extract`
// and `dress`): the cape as the player's own, a backpack hiding it, the emote's pose, and the
// hat and back cosmetic for `layer` right after. Per player per frame: nothing is made.
fn incari_cosmetics_look(
    game: &crate::game::Game<'_>,
    player: &crate::game::native_mob_draw::NativeMobRender<'_>,
    look: &mut crate::game::native_mob_draw::NativePlayerLook<'_>,
) {
    use crate::game::incari_cosmetics as m;
    use crate::game::native_mob_draw::NativeCape;
    use horizon_client::mobs::anim::{PartBlend, PoseBlend};
    use incari_core::cosmetics::{Slot, catalog, wardrobe, world};
    let _ = game;
    // The player's own is kept under 0: a world without a server has no UUIDs.
    let key = if look.local { 0 } else { match look.uuid { Some(u) => u, None => return } };
    let (own, show_others) = wardrobe::with(|w| (w.worn(), w.show_others()));
    let (worn, now) = incari_core::online::with(|o| {
        let ids = if look.local { None } else { o.player(key).map(|p| p.cosmetics.as_slice()) };
        world::with(|w| {
            if !look.local {
                w.walking(key, player.walk_speed);
            }
            (w.worn_by(key, look.local, own, ids, show_others), w.now(player.partial))
        })
    });
    if worn.is_empty() {
        return;
    }
    // The emote's pose over the model's: the legs still walk.
    if let Some(p) = worn.pose(now) {
        let part = |r: Option<[f32; 3]>| r.map_or(PartBlend::default(), |rotation| PartBlend { weight: p.weight, rotation });
        look.pose = PoseBlend { head: part(p.head), right_arm: part(p.right_arm), left_arm: part(p.left_arm), ..PoseBlend::default() };
    }
    let backpack = worn.loadout.in_slot(Slot::Back).is_some_and(|b| b.id == "back.backpack");
    let cape = worn.loadout.in_slot(Slot::Cape);
    if backpack {
        // A backpack and a cape would go through each other.
        look.hide_cape = true;
    } else if let Some(c) = cape {
        let n = catalog::index(c);
        let [x, y] = m::cape_slot(n);
        look.cape = Some(NativeCape {
            sprite: horizon_client::hud::MOD_ENTITY_CANVAS_SPRITE,
            rect: [x as f32, y as f32, m::CAPE[0] as f32, m::CAPE[1] as f32],
        });
        m::with(|s| s.cape_drawn(n));
    }
    // Others' trails go by where they're drawn; the player's own by where they are (`tick`).
    let mob = player.id;
    if !look.local {
        let trail = worn.loadout.in_slot(Slot::Trail).map(catalog::index);
        let position = look.position;
        m::with(|s| s.saw(mob, trail, position));
    }
    if look.invisible || look.distance_sq > world::MAX_DISTANCE * world::MAX_DISTANCE {
        return;
    }
    // A hat hides under a helmet (the halo floats above one); a back cosmetic under an elytra.
    let hat = worn
        .loadout
        .in_slot(Slot::Hat)
        .filter(|h| look.head_item.is_none() || h.id == "hat.halo")
        .map(catalog::index);
    let back = worn.loadout.in_slot(Slot::Back).filter(|_| !look.chest_wings).map(catalog::index);
    if hat.is_none() && back.is_none() {
        return;
    }
    let cape_shown = !backpack && !look.chest_wings && (cape.is_some() || look.has_cape);
    let [flap, lean, _] = look.cape_angles;
    let job = m::Job {
        mob,
        hat,
        back,
        age: player.age,
        crouching: look.crouching,
        fall_flying: look.fall_flying,
        chest_armor: look.chest_armor,
        cape_swing: cape_shown.then(|| (6.0 + lean / 2.0 + flap).to_radians()),
    };
    m::with(|s| s.set_job(job));
    look.parts = true;
}
