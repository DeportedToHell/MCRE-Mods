//! Emotecraft for Horizon: the emote engine. Everything that doesn't touch the game: the
//! animations and their formats (PlayerAnimationLibrary's), Molang, the controller that plays
//! an emote, the network protocol, the songs, the config and the server's logic. The glue in
//! the game's own source (`emotecraft_*` modules) drives it.
//!
//! This Source Code Form is a port of Emotecraft (https://github.com/KosmX/emotes) by KosmX,
//! dima_dencep and contributors, GPL-3.0; with PlayerAnimationLibrary by ZigyTheBird (MIT) and
//! mochafloats by Unnamed Team (MIT). See LICENSE and NOTICE.

pub mod anim;
pub mod audio;
pub mod binary;
pub mod buf;
pub mod config;
pub mod controller;
pub mod easing;
pub mod emotes;
pub mod lang;
pub mod loader;
pub mod molang;
pub mod opus;
pub mod packet;
pub mod server;
pub mod text;

#[cfg(test)]
mod tests;
