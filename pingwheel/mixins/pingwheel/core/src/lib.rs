//! Ping Wheel for Horizon: what doesn't touch the game. A port of Ping Wheel by Luken
//! (https://github.com/LukenSkyne/Minecraft-Ping-Wheel, MIT), branch 26.3 (v1.12.2):
//!
//! - `protocol`: the three plugin channels' packets on `FriendlyByteBuf`'s byte layout
//!   (`PingLocationC2SPacket`, `UpdateChannelC2SPacket`, `PingLocationS2CPacket`), and Java's
//!   `String.hashCode` the packets carry the dimension as (`GameContext.updateDimension`);
//! - `server`: `ServerCore` (the relay, the channels, the team rule) and `RateLimiter`;
//! - `config`: `ClientConfig` and `ServerConfig` in the files Gson writes
//!   (`config/pingwheel.json`, `config/pingwheel.server.json`);
//! - `math`: `MathUtils.calculateAngleRectIntersection` and the ping's scale.

pub mod config;
pub mod math;
pub mod protocol;
pub mod server;

/// The Ping Wheel version this port speaks and reports where the original reports its own
/// (`Global.MOD_VERSION` in the server's "make sure your version matches" messages).
pub const ORIGINAL_VERSION: &str = "1.12.2";

/// `ClientConfig.MAX_CHANNEL_LENGTH`.
pub const MAX_CHANNEL_LENGTH: usize = 128;
