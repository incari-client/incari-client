//! Cosmetics (ADR 0061): the catalog, what's worn, the device's wardrobe and its sync with the
//! service, the cosmetics' own art and models (painted in code), name tag styles, emotes, Incari's
//! badge, and what the world shows of them, with no game types. The Wardrobe and the emote wheel
//! are `screens::wardrobe` and `screens::emote_wheel`; the game side (`rust/mods/cosmetics`) draws
//! what's worn on players through the fork's player hooks.
pub mod art;
pub mod badges;
pub mod catalog;
pub mod emotes;
pub mod loadout;
pub mod models;
pub mod nametags;
pub mod preview;
pub mod textures;
pub mod wardrobe;
pub mod world;

pub use catalog::{Cosmetic, Slot};
pub use loadout::Loadout;
pub use world::Worn;
