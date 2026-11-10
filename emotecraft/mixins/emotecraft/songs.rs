// The built-in emotes' songs (Caramelldansen, Let's Groove, Take the L), as Emotecraft ships them
// beside the emotes (`<name>.opus`). To leave them out, delete this list's entries and the three
// .opus files in data/emotes/: the emotes then play without music.
//
// This Source Code Form is a port of Emotecraft (https://github.com/KosmX/emotes), GPL-3.0.

/// (emote, Ogg Opus file).
pub(super) const SONGS: &[(&str, &[u8])] = &[
    ("take_the_l", horizon_client::native_data!("emotecraft", "emotes/take_the_l.opus")),
    ("caramell_dance", horizon_client::native_data!("emotecraft", "emotes/caramell_dance.opus")),
    ("lets_groove", horizon_client::native_data!("emotecraft", "emotes/lets_groove.opus")),
];
