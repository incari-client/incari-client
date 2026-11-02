// What a section's ghosts were made from: meshed again when it changes.
fn incari_schematics_ghost_revision(section: [i32; 3], revision: &mut u64) {
    crate::game::incari_schematics_world::ghost_revision(section, revision);
}
