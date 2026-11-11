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

/// `ClientConfig.MAX_CHANNEL_LENGTH`: in UTF-16 code units, as Java counts a `String`'s length
/// (and as `writeUtf(channel, 128)` takes it), not in characters.
pub const MAX_CHANNEL_LENGTH: usize = 128;

/// A string's length as Java's `String.length()`: its UTF-16 code units (a character outside the
/// Basic Multilingual Plane, such as an emoji, counts as two).
pub fn java_length(s: &str) -> usize {
    s.encode_utf16().count()
}

/// The channel fits `MAX_CHANNEL_LENGTH`, and so the packets (`channel.length() <= 128`).
pub fn channel_fits(channel: &str) -> bool {
    java_length(channel) <= MAX_CHANNEL_LENGTH
}

/// `s.substring(0, max)` in UTF-16 code units: the longest start of `s` of at most `max` units. A
/// character the cut would split in half (a surrogate pair) is left out whole, as
/// `EditBox.insertText` does; Java's `substring` would keep the lone half, which a Rust string
/// can't hold and `writeUtf` would send as `?`.
pub fn cut_java(s: &str, max: usize) -> &str {
    let mut units = 0;
    for (i, c) in s.char_indices() {
        units += c.len_utf16();
        if units > max {
            return &s[..i];
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lengths_in_utf16_units() {
        assert_eq!(java_length(""), 0);
        assert_eq!(java_length("red"), 3);
        assert_eq!(java_length("\u{e9}\u{4e2d}"), 2); // é 中: one unit each
        assert_eq!(java_length("\u{1F600}"), 2); // 😀: a surrogate pair
        assert!(channel_fits(&"x".repeat(128)));
        assert!(!channel_fits(&"x".repeat(129)));
        assert!(channel_fits(&"\u{4e2d}".repeat(128)));
        assert!(channel_fits(&"\u{1F600}".repeat(64)));
        // 65 characters but 130 units: Java's `length()` is 130 and `writeUtf(channel, 128)` fails.
        assert!(!channel_fits(&"\u{1F600}".repeat(65)));
    }

    #[test]
    fn cuts_at_utf16_units_without_splitting_a_pair() {
        assert_eq!(cut_java("abc", 128), "abc");
        assert_eq!(cut_java("abcdef", 3), "abc");
        assert_eq!(cut_java("abc", 0), "");
        assert_eq!(cut_java(&"\u{1F600}".repeat(65), 128), "\u{1F600}".repeat(64));
        // "x" and 64 emoji is 129 units: the pair across the cut goes whole, leaving 127.
        let odd = format!("x{}", "\u{1F600}".repeat(64));
        assert_eq!(cut_java(&odd, 128), format!("x{}", "\u{1F600}".repeat(63)));
        assert_eq!(java_length(cut_java(&odd, 128)), 127);
    }
}
