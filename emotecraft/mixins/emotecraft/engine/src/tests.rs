//! The built-in emotes, for the tests.

pub const BUILT_IN_JSON: [&str; 14] = [
    include_str!("../../data/emotes/waving.json"),
    include_str!("../../data/emotes/clap.json"),
    include_str!("../../data/emotes/crying.json"),
    include_str!("../../data/emotes/point.json"),
    include_str!("../../data/emotes/here.json"),
    include_str!("../../data/emotes/palm.json"),
    include_str!("../../data/emotes/backflip.json"),
    include_str!("../../data/emotes/roblox_potion_dance.json"),
    include_str!("../../data/emotes/kazotsky_kick.json"),
    include_str!("../../data/emotes/twerk.json"),
    include_str!("../../data/emotes/club_penguin_dance.json"),
    include_str!("../../data/emotes/take_the_l.json"),
    include_str!("../../data/emotes/caramell_dance.json"),
    include_str!("../../data/emotes/lets_groove.json"),
];

pub const BUILT_IN_SONGS: [(&str, &[u8]); 3] = [
    ("take_the_l", include_bytes!("../../data/emotes/take_the_l.opus")),
    ("caramell_dance", include_bytes!("../../data/emotes/caramell_dance.opus")),
    ("lets_groove", include_bytes!("../../data/emotes/lets_groove.opus")),
];
