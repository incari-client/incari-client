//! Incari Client on Horizon (ADR 0068, 0069): the client's look (Frost), its screens, the module
//! framework and the profiles, written once with no game types. The bundle's helpers in
//! `rust/mods/incari/` are its game side: they hand it input, draw what it lists, and do what it
//! asks of the game.
#![allow(clippy::too_many_arguments)] // drawing calls name every part of a shape, as the Java ones do

pub mod canvas;
pub mod client;
pub mod color;
pub mod cosmetics;
pub mod credits;
pub mod crosshair;
pub mod draw;
pub mod featured;
pub mod framework;
pub mod host_extras;
pub mod hud;
pub mod icons;
pub mod input;
pub mod map_memory;
pub mod modules;
pub mod motion;
pub mod online;
pub mod paint;
pub mod panels;
pub mod player;
pub mod recorder;
pub mod release;
pub mod schematic;
pub mod screen;
pub mod screens;
pub mod server_menu;
pub mod settings_skin;
pub mod skin;
pub mod theme;
pub mod ui;
pub mod waypoint;
pub mod waypoint_overlay;
