// A chunk Horizon's minimap scanned: remembered for the world map.
fn incari_waypoints_map_tile(dimension: &str, chunk: [i32; 2], colors: &[u32; 256]) {
    crate::game::incari_waypoints_world::map_tile(dimension, chunk, colors);
}
