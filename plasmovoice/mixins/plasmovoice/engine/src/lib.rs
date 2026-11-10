//! Plasmo Voice for Horizon: the voice chat engine. A port of Plasmo Voice by Plasmo
//! (https://github.com/plasmoapp/plasmo-voice), LGPL-3.0: its protocol (`tcp`, `udp`), the
//! audio encryption (`crypto`) and the ids it derives (`md5`), on Java's byte layout (`wire`).
//
// This library is free software; you can redistribute it and/or modify it under the terms of the
// GNU Lesser General Public License version 3, as published by the Free Software Foundation.

pub mod audio;
pub mod client;
pub mod crypto;
pub mod dsp;
pub mod md5;
pub mod mutes;
pub mod server;
pub mod server_commands;
pub mod server_config;
pub mod server_lang;
pub mod tcp;
pub mod udp;
pub mod wire;

/// The Plasmo Voice version the port speaks (the server checks the major, 2, and `>= 2.0.0`).
pub const PROTOCOL_VERSION: &str = "2.2.0";

/// The log target of the engine's lines: Horizon's logger shows the informational lines of
/// `horizon*` targets only (the server's "UDP server is started", "connected", mutes).
pub(crate) const LOG: &str = "horizon_plasmovoice";
