// Roughly Enough Items for Horizon: the mod's logic, with no game types, so
// the same file builds and tests on its own (`cargo test` at the mod's root)
// and is what the bundle adds to the game as `crate::game::roughlyenoughitems`.
//
// A port of Roughly Enough Items (REI) by shedaniel (MIT,
// https://github.com/shedaniel/RoughlyEnoughItems): its search grammar
// (`Argument`, the argument types), the entry list's pages, favorites, the
// default plugin's categories and the recipe screen's paging.
// `roughlyenoughitems_glue` reads the game's items and recipes into this
// model and `roughlyenoughitems_view` draws it.
use std::collections::HashMap;

pub const ID: &str = "roughlyenoughitems";
pub const NAME: &str = "Roughly Enough Items";
pub const VERSION: &str = "1.1.0";

/// Russian labels when the game's language is Russian, English otherwise.
pub fn russian(language: &str) -> bool {
    language.starts_with("ru")
}

// ---- search (`Argument`, `ArgumentTypesRegistry`) ---------------------------------

/// What a search term reads.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    /// `@`: the mod's id or name.
    Mod,
    /// `#`: the tooltip's text.
    Tooltip,
    /// `$`: an item tag.
    Tag,
    /// `*`: the id's path (`oak_log`).
    Identifier,
    /// `r/…/`: the whole name, case and all.
    Regex,
    /// A plain word: the name.
    Text,
}

impl Kind {
    /// The colour the search field gives its terms (`getHighlightedStyle`).
    pub fn colour(self) -> Option<u32> {
        match self {
            Kind::Mod => Some(0xffff_a8f3),
            Kind::Tooltip => Some(0xffff_e0ad),
            Kind::Tag => Some(0xff9e_fff4),
            Kind::Identifier => Some(0xff8d_7eed),
            Kind::Regex => Some(0xffbf_ffa8),
            Kind::Text => None,
        }
    }
}

/// The prefixed types in REI's order: the first whose prefix a term has wins.
const PREFIXED: [(Kind, char); 4] = [
    (Kind::Mod, '@'),
    (Kind::Tooltip, '#'),
    (Kind::Tag, '$'),
    (Kind::Identifier, '*'),
];

thread_local! {
    /// How `@`, `#`, `$` and `*` take part (`search.*_search`): 0 always (plain
    /// words read it too), 1 with the prefix only, 2 never (the prefix is text).
    static SEARCH_MODE_STATE: std::cell::Cell<[u8; 4]> = const { std::cell::Cell::new([1, 0, 1, 0]) };
}

/// The search modes from the settings.
pub fn set_search_modes(modes: [u8; 4]) {
    SEARCH_MODE_STATE.with(|m| m.set(modes));
}

fn search_mode(kind: Kind) -> u8 {
    let i = match kind {
        Kind::Mod => 0,
        Kind::Tooltip => 1,
        Kind::Tag => 2,
        Kind::Identifier => 3,
        _ => return 1,
    };
    SEARCH_MODE_STATE.with(|m| m.get()[i])
}

/// What a search reads of one entry: everything lowercased but `name_case`.
pub struct Searchable<'a> {
    pub name: &'a str,
    /// The name as shown, for `r/…/`.
    pub name_case: &'a str,
    pub mod_id: &'a str,
    pub mod_name: &'a str,
    /// Every tooltip line, the name first; None while not worked out.
    pub tooltip: Option<&'a str>,
    /// `minecraft:logs`.
    pub tags: &'a [String],
    /// `oak_log`.
    pub path: &'a str,
}

#[derive(Clone, PartialEq, Eq, Debug)]
struct Term {
    kind: Kind,
    negate: bool,
    text: String,
}

/// A parsed search: groups split by `|`, any of which may match; every word
/// of a group must, and a word matches when any of its terms does.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Query {
    /// None: everything matches (empty text, or an empty `|` group).
    groups: Option<Vec<Vec<Vec<Term>>>>,
}

/// How each character of the search text is shown.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mark {
    Plain,
    /// A term's prefix (`@`, `-@`, `r/` and the closing `/`) or body.
    Prefix(Kind),
    Body(Kind),
    /// `|`, gray.
    Bar,
    /// `"`, gold.
    Quote,
}

/// The words of one group with where each starts (in chars): a quoted word
/// is what is between its quotes (`(?:"([^"]*)")|([^\s]+)`).
fn tokens(group: &[char]) -> Vec<(usize, String, bool)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < group.len() {
        if group[i].is_whitespace() {
            i += 1;
            continue;
        }
        if group[i] == '"'
            && let Some(end) = group[i + 1..].iter().position(|&c| c == '"')
        {
            out.push((i + 1, group[i + 1..i + 1 + end].iter().collect(), true));
            i += end + 2;
            continue;
        }
        let start = i;
        while i < group.len() && !group[i].is_whitespace() {
            i += 1;
        }
        out.push((start, group[start..i].iter().collect(), false));
    }
    out
}

/// A term with a prefix: its kind, negation, text and the prefix's length in
/// chars at the front (and back, for a regex).
fn prefixed(word: &str) -> Option<(Kind, bool, String, usize, usize)> {
    let chars: Vec<char> = word.chars().collect();
    for (kind, p) in PREFIXED {
        if search_mode(kind) == 2 {
            continue;
        }
        if chars.first() == Some(&p) {
            if chars.get(1) == Some(&'-') {
                return Some((kind, true, chars[2..].iter().collect(), 2, 0));
            }
            return Some((kind, false, chars[1..].iter().collect(), 1, 0));
        }
        if chars.first() == Some(&'-') && chars.get(1) == Some(&p) {
            return Some((kind, true, chars[2..].iter().collect(), 2, 0));
        }
    }
    let (negate, rest) = match chars.first() {
        Some('-') => (true, &chars[1..]),
        _ => (false, &chars[..]),
    };
    if rest.len() >= 3 && rest[0] == 'r' && rest[1] == '/' && rest[rest.len() - 1] == '/' {
        let body: String = rest[2..rest.len() - 1].iter().collect();
        return Some((Kind::Regex, negate, body, 2 + usize::from(negate), 1));
    }
    None
}

impl Query {
    /// Parses the search field's text as REI does.
    pub fn parse(text: &str) -> Query {
        if text.is_empty() {
            return Query::default();
        }
        let mut groups = Vec::new();
        for group in text.split('|') {
            let chars: Vec<char> = group.chars().collect();
            let words = tokens(&chars);
            if words.is_empty() {
                // An empty group matches everything, so the whole search does.
                return Query::default();
            }
            let mut parsed = Vec::new();
            for (_, word, _) in words {
                if let Some((kind, negate, body, _, _)) = prefixed(&word) {
                    let text = if kind == Kind::Regex { body } else { body.to_lowercase() };
                    parsed.push(vec![Term { kind, negate, text }]);
                    continue;
                }
                let lower = word.to_lowercase();
                let (negate, text) = match lower.strip_prefix('-') {
                    Some(rest) => (true, rest.to_owned()),
                    None => (false, lower),
                };
                // Plain words: the name, and each type that is always on (by
                // default the tooltip and the id's path).
                parsed.push(
                    std::iter::once(Kind::Text)
                        .chain(PREFIXED.iter().map(|(k, _)| *k).filter(|k| search_mode(*k) == 0))
                        .map(|kind| Term { kind, negate, text: text.clone() })
                        .collect(),
                );
            }
            groups.push(parsed);
        }
        Query { groups: Some(groups) }
    }


    /// The search reads tooltips.
    pub fn needs_tooltips(&self) -> bool {
        self.groups
            .iter()
            .flatten()
            .flatten()
            .flatten()
            .any(|t| t.kind == Kind::Tooltip)
    }

    pub fn matches(&self, s: &Searchable<'_>) -> bool {
        let Some(groups) = &self.groups else {
            return true;
        };
        groups
            .iter()
            .any(|group| group.iter().all(|word| word.iter().any(|t| term_hits(t, s) != t.negate)))
    }
}

fn term_hits(t: &Term, s: &Searchable<'_>) -> bool {
    let text = t.text.as_str();
    match t.kind {
        Kind::Text => s.name.contains(text),
        Kind::Mod => s.mod_id.contains(text) || s.mod_name.contains(text),
        Kind::Tooltip => s.tooltip.is_some_and(|tip| tip.contains(text)),
        Kind::Tag => s.tags.iter().any(|tag| tag.contains(text)),
        Kind::Identifier => s.path.contains(text),
        Kind::Regex => Regex::parse(text).is_some_and(|r| r.full_match(s.name_case)),
    }
}

/// How each character of `text` shows in the search field
/// (`OverlaySearchFieldSyntaxHighlighter`).
pub fn marks(text: &str) -> Vec<Mark> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = vec![Mark::Plain; chars.len()];
    let mut start = 0;
    for (i, c) in chars.iter().copied().chain(std::iter::once('|')).enumerate() {
        if c != '|' {
            continue;
        }
        let group = &chars[start..i];
        for (at, word, quoted) in tokens(group) {
            let at = start + at;
            if quoted {
                out[at - 1] = Mark::Quote;
                if let Some(m) = out.get_mut(at + word.chars().count()) {
                    *m = Mark::Quote;
                }
            }
            if let Some((kind, _, _, front, back)) = prefixed(&word) {
                let n = word.chars().count();
                for (k, m) in out[at..at + n].iter_mut().enumerate() {
                    *m = if k < front || k >= n - back { Mark::Prefix(kind) } else { Mark::Body(kind) };
                }
            }
        }
        if i < chars.len() {
            out[i] = Mark::Bar;
        }
        start = i + 1;
    }
    out
}

// ---- a small regex for `r/…/` -------------------------------------------------------

/// Java's `Pattern.matcher(name).matches()` for the patterns people write
/// in a search: literals, `.`, classes (`[a-z]`, `[^…]`, `\d \w \s`),
/// groups and `*`, `+`, `?` (greedy, backtracking).
#[derive(Clone, Debug)]
pub struct Regex {
    seq: Vec<Piece>,
}

#[derive(Clone, Debug)]
struct Piece {
    atom: Atom,
    min: usize,
    max: usize,
}

#[derive(Clone, Debug)]
enum Atom {
    Char(char),
    Any,
    Class(Vec<(char, char)>, bool),
    Group(Vec<Vec<Piece>>),
}

impl Regex {
    pub fn parse(pattern: &str) -> Option<Regex> {
        let chars: Vec<char> = pattern.chars().collect();
        let mut at = 0;
        let alts = Self::alternatives(&chars, &mut at)?;
        if at != chars.len() {
            return None;
        }
        Some(Regex {
            seq: vec![Piece { atom: Atom::Group(alts), min: 1, max: 1 }],
        })
    }

    fn alternatives(chars: &[char], at: &mut usize) -> Option<Vec<Vec<Piece>>> {
        let mut alts = vec![Vec::new()];
        while *at < chars.len() && chars[*at] != ')' {
            if chars[*at] == '|' {
                *at += 1;
                alts.push(Vec::new());
                continue;
            }
            let atom = Self::atom(chars, at)?;
            let (mut min, mut max) = (1, 1);
            match chars.get(*at) {
                Some('*') => (min, max) = (0, usize::MAX),
                Some('+') => (min, max) = (1, usize::MAX),
                Some('?') => (min, max) = (0, 1),
                _ => {}
            }
            if min != 1 || max != 1 {
                *at += 1;
            }
            alts.last_mut()?.push(Piece { atom, min, max });
        }
        Some(alts)
    }

    fn atom(chars: &[char], at: &mut usize) -> Option<Atom> {
        let c = chars[*at];
        *at += 1;
        Some(match c {
            '.' => Atom::Any,
            '^' | '$' => Atom::Group(vec![Vec::new()]),
            '(' => {
                if chars.get(*at) == Some(&'?') && chars.get(*at + 1) == Some(&':') {
                    *at += 2;
                }
                let alts = Self::alternatives(chars, at)?;
                if chars.get(*at) != Some(&')') {
                    return None;
                }
                *at += 1;
                Atom::Group(alts)
            }
            '[' => {
                let negate = chars.get(*at) == Some(&'^');
                if negate {
                    *at += 1;
                }
                let mut ranges = Vec::new();
                let mut first = true;
                loop {
                    let c = *chars.get(*at)?;
                    *at += 1;
                    if c == ']' && !first {
                        break;
                    }
                    first = false;
                    let lo = if c == '\\' {
                        let e = *chars.get(*at)?;
                        *at += 1;
                        if let Some(class) = Self::escape_class(e) {
                            ranges.extend(class);
                            continue;
                        }
                        e
                    } else {
                        c
                    };
                    if chars.get(*at) == Some(&'-') && chars.get(*at + 1).is_some_and(|&c| c != ']') {
                        let hi = chars[*at + 1];
                        *at += 2;
                        ranges.push((lo, hi));
                    } else {
                        ranges.push((lo, lo));
                    }
                }
                Atom::Class(ranges, negate)
            }
            '\\' => {
                let e = *chars.get(*at)?;
                *at += 1;
                match Self::escape_class(e) {
                    Some(class) => Atom::Class(class, e.is_ascii_uppercase()),
                    None => Atom::Char(e),
                }
            }
            '*' | '+' | '?' | ')' => return None,
            c => Atom::Char(c),
        })
    }

    fn escape_class(e: char) -> Option<Vec<(char, char)>> {
        Some(match e.to_ascii_lowercase() {
            'd' => vec![('0', '9')],
            'w' => vec![('a', 'z'), ('A', 'Z'), ('0', '9'), ('_', '_')],
            's' => vec![(' ', ' '), ('\t', '\t'), ('\n', '\n'), ('\r', '\r')],
            _ => return None,
        })
    }

    pub fn full_match(&self, text: &str) -> bool {
        let chars: Vec<char> = text.chars().collect();
        Self::seq(&self.seq, &chars, 0, &mut |end| end == chars.len())
    }

    /// Matches `seq` at `at`, calling `k` with every end it reaches.
    fn seq(seq: &[Piece], chars: &[char], at: usize, k: &mut dyn FnMut(usize) -> bool) -> bool {
        let Some((first, rest)) = seq.split_first() else {
            return k(at);
        };
        Self::repeat(first, 0, chars, at, &mut |end| Self::seq(rest, chars, end, k))
    }

    fn repeat(p: &Piece, done: usize, chars: &[char], at: usize, k: &mut dyn FnMut(usize) -> bool) -> bool {
        // Greedy: one more first, then stop here.
        if done < p.max
            && Self::atom_at(&p.atom, chars, at, &mut |end| {
                end != at && Self::repeat(p, done + 1, chars, end, k)
            })
        {
            return true;
        }
        done >= p.min && k(at)
    }

    fn atom_at(atom: &Atom, chars: &[char], at: usize, k: &mut dyn FnMut(usize) -> bool) -> bool {
        match atom {
            Atom::Char(c) => chars.get(at) == Some(c) && k(at + 1),
            Atom::Any => at < chars.len() && k(at + 1),
            Atom::Class(ranges, negate) => chars
                .get(at)
                .is_some_and(|c| ranges.iter().any(|(lo, hi)| (lo..=hi).contains(&c)) != *negate)
                && k(at + 1),
            Atom::Group(alts) => alts.iter().any(|alt| Self::seq(alt, chars, at, k)),
        }
    }
}

// ---- grids and pages -----------------------------------------------------------

/// A rectangle in GUI pixels: x, y, width, height.
pub type Rect = [f32; 4];

pub fn inside(p: (f32, f32), r: Rect) -> bool {
    p.0 >= r[0] && p.1 >= r[1] && p.0 < r[0] + r[2] && p.1 < r[1] + r[3]
}

/// How many pages `len` entries take, `per_page` a page: at least one.
pub fn pages(len: usize, per_page: usize) -> usize {
    if per_page == 0 {
        return 1;
    }
    len.div_ceil(per_page).max(1)
}

/// The page after `page` turning by `delta`, wrapping round as REI's do.
pub fn turn(page: usize, delta: i32, pages: usize) -> usize {
    let pages = pages.max(1) as i64;
    (page as i64 + i64::from(delta)).rem_euclid(pages) as usize
}

/// Which of `len` entries a slot shows at `millis`: one a second, every
/// slot in step (REI cycles slots that share entries together).
pub fn cycle(len: usize, millis: u64) -> usize {
    if len == 0 {
        return 0;
    }
    (millis / 1000) as usize % len
}

// ---- favorites -------------------------------------------------------------------

/// A favorite that is no item: the game mode, weather and time buttons of the
/// favorites panel.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Shortcut {
    GameMode(u8),
    Weather(u8),
    Time(u8),
}

impl Shortcut {
    pub const GAME_MODES: [&'static str; 4] = ["survival", "creative", "adventure", "spectator"];
    pub const WEATHERS: [&'static str; 3] = ["clear", "rain", "thunder"];
    pub const TIMES: [(&'static str, u32); 4] = [("morn", 1000), ("noon", 6000), ("evening", 12000), ("night", 18000)];

    /// The favorites file's key: `rei:gamemode/creative`.
    pub fn key(self) -> String {
        match self {
            Shortcut::GameMode(i) => format!("rei:gamemode/{}", Self::GAME_MODES[usize::from(i) % 4]),
            Shortcut::Weather(i) => format!("rei:weather/{}", Self::WEATHERS[usize::from(i) % 3]),
            Shortcut::Time(i) => format!("rei:time/{}", Self::TIMES[usize::from(i) % 4].0),
        }
    }

    pub fn from_key(key: &str) -> Option<Shortcut> {
        let rest = key.strip_prefix("rei:")?;
        let (kind, name) = rest.split_once('/')?;
        let find = |list: &[&str]| list.iter().position(|n| *n == name).map(|i| i as u8);
        match kind {
            "gamemode" => find(&Self::GAME_MODES).map(Shortcut::GameMode),
            "weather" => find(&Self::WEATHERS).map(Shortcut::Weather),
            "time" => Self::TIMES.iter().position(|(n, _)| *n == name).map(|i| Shortcut::Time(i as u8)),
            _ => None,
        }
    }

    /// The command it runs (REI's default commands, without the slash).
    pub fn command(self) -> String {
        match self {
            Shortcut::GameMode(i) => format!("gamemode {}", Self::GAME_MODES[usize::from(i) % 4]),
            Shortcut::Weather(i) => format!("weather {}", Self::WEATHERS[usize::from(i) % 3]),
            Shortcut::Time(i) => format!("time set {}", Self::TIMES[usize::from(i) % 4].1),
        }
    }
}

/// The favorites, by stack key (`minecraft:potion[...]`) or shortcut key.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Favorites {
    pub keys: Vec<String>,
}

impl Favorites {
    /// Adds `key` at `at` (the end when None); one already there moves.
    pub fn add(&mut self, key: &str, at: Option<usize>) {
        if let Some(i) = self.keys.iter().position(|k| k == key) {
            self.keys.remove(i);
        }
        let at = at.unwrap_or(self.keys.len()).min(self.keys.len());
        self.keys.insert(at, key.to_owned());
    }
    pub fn remove(&mut self, key: &str) -> bool {
        let before = self.keys.len();
        self.keys.retain(|k| k != key);
        self.keys.len() != before
    }
    /// One key a line; `#` starts a comment.
    pub fn parse(text: &str) -> Favorites {
        let mut keys: Vec<String> = Vec::new();
        for line in text.lines().map(str::trim) {
            if line.is_empty() || line.starts_with('#') || keys.iter().any(|k| k == line) {
                continue;
            }
            keys.push(line.to_owned());
        }
        Favorites { keys }
    }
    pub fn write(&self) -> String {
        let mut out = String::from("# Roughly Enough Items favorites: one a line, items as /give writes them\n");
        for key in &self.keys {
            out.push_str(key);
            out.push('\n');
        }
        out
    }
}

// ---- recipes ---------------------------------------------------------------------

/// The default plugin's categories that have displays, in REI's order
/// (`DefaultClientPlugin.registerCategories`). Fuel, information and tag
/// categories show nothing for vanilla and are left out.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub enum Category {
    Crafting,
    Smelting,
    Smoking,
    Blasting,
    Campfire,
    StoneCutting,
    Brewing,
    Composting,
    Stripping,
    Smithing,
    Anvil,
    BeaconBase,
    BeaconPayment,
    Tilling,
    Pathing,
    Waxing,
    WaxScraping,
    Oxidizing,
    OxidationScraping,
    /// Every item and block tag, as a tree (`DefaultTagCategory`).
    Tag,
}

impl Category {
    pub const ALL: [Category; 20] = [
        Category::Crafting,
        Category::Smelting,
        Category::Smoking,
        Category::Blasting,
        Category::Campfire,
        Category::StoneCutting,
        Category::Brewing,
        Category::Composting,
        Category::Stripping,
        Category::Smithing,
        Category::Anvil,
        Category::BeaconBase,
        Category::BeaconPayment,
        Category::Tilling,
        Category::Pathing,
        Category::Waxing,
        Category::WaxScraping,
        Category::Oxidizing,
        Category::OxidationScraping,
        Category::Tag,
    ];
    pub fn index(self) -> usize {
        Category::ALL.iter().position(|c| *c == self).unwrap_or(0)
    }
    /// A display's height (`getDisplayHeight`); every one is 150 wide.
    pub fn height(self) -> f32 {
        match self {
            Category::Crafting | Category::Brewing | Category::Tag => 66.0,
            Category::Smelting | Category::Smoking | Category::Blasting | Category::Campfire => 49.0,
            Category::Anvil => 48.0,
            Category::Composting | Category::BeaconBase | Category::BeaconPayment => 140.0,
            _ => 36.0,
        }
    }
    /// One display a page.
    pub fn one_per_page(self) -> bool {
        matches!(self, Category::Composting | Category::BeaconBase | Category::BeaconPayment)
    }
    /// The tab's icon.
    pub fn icon(self) -> &'static str {
        match self {
            Category::Crafting => "minecraft:crafting_table",
            Category::Smelting => "minecraft:furnace",
            Category::Smoking => "minecraft:smoker",
            Category::Blasting => "minecraft:blast_furnace",
            Category::Campfire => "minecraft:campfire",
            Category::StoneCutting => "minecraft:stonecutter",
            Category::Brewing => "minecraft:brewing_stand",
            Category::Composting => "minecraft:composter",
            Category::Stripping => "minecraft:iron_axe",
            Category::Smithing => "minecraft:smithing_table",
            Category::Anvil => "minecraft:anvil",
            Category::BeaconBase | Category::BeaconPayment => "minecraft:beacon",
            Category::Tilling => "minecraft:iron_hoe",
            Category::Pathing => "minecraft:iron_shovel",
            Category::Waxing => "minecraft:honeycomb",
            Category::WaxScraping => "minecraft:golden_axe",
            Category::Oxidizing => "minecraft:clock",
            Category::OxidationScraping => "minecraft:stone_axe",
            Category::Tag => "minecraft:name_tag",
        }
    }
    /// Its workstations (`addWorkstations`); the tools' categories take every
    /// item of a tag instead (`tool_tag`).
    pub fn workstations(self) -> &'static [&'static str] {
        match self {
            Category::Crafting => &["minecraft:crafting_table"],
            Category::Smelting => &["minecraft:furnace"],
            Category::Smoking => &["minecraft:smoker"],
            Category::Blasting => &["minecraft:blast_furnace"],
            Category::Campfire => &["minecraft:campfire", "minecraft:soul_campfire"],
            Category::StoneCutting => &["minecraft:stonecutter"],
            Category::Brewing => &["minecraft:brewing_stand"],
            Category::Composting => &["minecraft:composter"],
            Category::Smithing => &["minecraft:smithing_table"],
            Category::Anvil => &["minecraft:anvil"],
            Category::BeaconBase | Category::BeaconPayment => &["minecraft:beacon"],
            Category::Waxing => &["minecraft:honeycomb"],
            _ => &[],
        }
    }
    /// The item tag whose tools are its workstations.
    pub fn tool_tag(self) -> Option<&'static str> {
        match self {
            Category::Stripping | Category::WaxScraping | Category::OxidationScraping => Some("minecraft:axes"),
            Category::Tilling => Some("minecraft:hoes"),
            Category::Pathing => Some("minecraft:shovels"),
            _ => None,
        }
    }
}

/// An entry of a display: `id` is the entry's (one per item and components,
/// damage and count aside), with its count and damage here.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Stack {
    pub id: u32,
    pub count: u32,
    pub damage: u32,
}

impl Stack {
    /// A slot's empty turn (the firework rocket's optional gunpowder).
    pub const EMPTY: u32 = u32::MAX;
    pub fn one(id: u32) -> Stack {
        Stack { id, count: 1, damage: 0 }
    }
    pub fn is_empty(self) -> bool {
        self.id == Stack::EMPTY
    }
}

/// One display as REI shows it.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Recipe {
    pub category: Option<Category>,
    /// `minecraft:oak_planks`, or "" for one REI makes up.
    pub id: String,
    /// Each input slot's entries, any one of which fits; empty for an empty
    /// slot. Crafting: the 3x3 grid row by row.
    pub inputs: Vec<Vec<Stack>>,
    pub outputs: Vec<Vec<Stack>>,
    /// Crafting: the recipe's own size (shapeless: 2x2 for up to four
    /// entries, else 3x3), for the inventory's 2x2 grid.
    pub width: u32,
    pub height: u32,
    pub shapeless: bool,
    /// Cooking: ticks and experience (0 when unknown).
    pub ticks: u32,
    pub experience: f32,
    /// Anvil: the level cost.
    pub cost: u32,
    /// Composting: each input's chance.
    pub chances: Vec<f32>,
    /// Tag: the tree's place in the data's list.
    pub tag: Option<usize>,
}

impl Recipe {
    pub fn category(&self) -> Category {
        self.category.unwrap_or(Category::Crafting)
    }
}

// ---- tags (`TagNode`, `TagTreeWidget`) ------------------------------------------------

/// A tag as REI draws it: the tag, the entries it names itself (a missing
/// item keeps its id, drawn as "?"), then the tags it includes.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct TagTree {
    /// `minecraft:shovels`.
    pub id: String,
    pub values: Vec<Result<u32, String>>,
    pub children: Vec<TagTree>,
    /// Every entry, the included tags' too: what the tag's frame cycles.
    pub all: Vec<u32>,
}

/// One box of a tag tree: a tag's frame, or a panel of entries.
#[derive(Clone, Copy, Debug)]
pub enum TagNode<'a> {
    Reference(&'a TagTree),
    Values(&'a [Result<u32, String>]),
}

impl<'a> TagNode<'a> {
    /// The box's size (`ReferenceTagNodeWidget` 24x23, `ValueTagNodeWidget`
    /// four entries a row at 16 px, 6 px round).
    pub fn size(self) -> (f32, f32) {
        match self {
            TagNode::Reference(_) => (24.0, 23.0),
            TagNode::Values(v) => {
                let w = v.len().clamp(1, 4);
                let h = v.len().div_ceil(w).max(1);
                (16.0 * w as f32 + 12.0, 16.0 * h as f32 + 12.0)
            }
        }
    }
    /// The boxes under it: a tag's own entries first, then its tags.
    pub fn children(self) -> Vec<TagNode<'a>> {
        match self {
            TagNode::Reference(t) => {
                let mut out = Vec::new();
                if !t.values.is_empty() {
                    out.push(TagNode::Values(&t.values));
                }
                out.extend(t.children.iter().map(TagNode::Reference));
                out
            }
            TagNode::Values(_) => Vec::new(),
        }
    }
    /// The whole subtree's size (`TagTreeWidget`): children 16 px below,
    /// 6 px apart, centred under the box.
    pub fn tree_size(self) -> (f32, f32) {
        let (rw, rh) = self.size();
        let children = self.children();
        if children.is_empty() {
            return (rw, rh);
        }
        let sizes: Vec<(f32, f32)> = children.iter().map(|c| c.tree_size()).collect();
        let total = sizes.iter().map(|s| s.0 + 6.0).sum::<f32>() - 6.0;
        let cx = (rw / 2.0).floor();
        let left = (cx - (total / 2.0).floor()).min(0.0);
        let right = (cx - (total / 2.0).floor() + total).max(rw);
        let tallest = sizes.iter().map(|s| s.1).fold(0.0, f32::max);
        (right - left, rh + 16.0 + tallest)
    }
}

impl TagTree {
    /// The tree as text (`TagNode.asTree`), for the clipboard; `name` gives an
    /// entry's id.
    pub fn text(&self, name: &dyn Fn(u32) -> String) -> String {
        let mut out = String::new();
        self.print(&mut out, "", "", name);
        out
    }
    fn print(&self, out: &mut String, prefix: &str, children_prefix: &str, name: &dyn Fn(u32) -> String) {
        out.push_str(prefix);
        out.push_str(&self.id);
        out.push('\n');
        let n = usize::from(!self.values.is_empty()) + self.children.len();
        let mut i = 0;
        let branch = |i: usize| {
            if i + 1 < n {
                (format!("{children_prefix}├── "), format!("{children_prefix}│   "))
            } else {
                (format!("{children_prefix}└── "), format!("{children_prefix}    "))
            }
        };
        if !self.values.is_empty() {
            let (p, _) = branch(i);
            for v in &self.values {
                out.push_str(&p);
                match v {
                    Ok(id) => out.push_str(&name(*id)),
                    Err(id) => out.push_str(id),
                }
                out.push('\n');
            }
            i += 1;
        }
        for child in &self.children {
            let (p, c) = branch(i);
            child.print(out, &p, &c, name);
            i += 1;
        }
    }
}

/// The tag view's zoom range (`OverflowWidget`): REI's scale runs from 0.78
/// (drawn 1.28 times) to the one that fits the whole tree; drawn size is
/// its inverse.
pub fn tag_scale_range(content: (f32, f32), view: (f32, f32)) -> (f32, f32) {
    let fit = (content.0 / view.0).min(content.1 / view.1);
    (0.78, fit.max(0.78))
}

/// Where content `size` drawn `scale` times sits in a view `len` long:
/// against both edges when larger (`handleBounceBack`), centred when not.
pub fn tag_offset(offset: f32, size: f32, len: f32) -> f32 {
    if size <= len {
        ((len - size) / 2.0).floor()
    } else {
        offset.clamp(len - size, 0.0)
    }
}

/// What the recipe screen shows: an entry's recipes or usages, or every
/// category (the title's "View All Categories").
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Lookup {
    Recipes(u32),
    Usages(u32),
    All(Category),
    /// One display alone (a recipe of the display history).
    Display(usize),
    /// One category's displays (a click area: the furnace's arrow).
    Category(Category),
}

impl Lookup {
    pub fn focus(self) -> Option<u32> {
        match self {
            Lookup::Recipes(id) | Lookup::Usages(id) => Some(id),
            Lookup::All(_) | Lookup::Display(_) | Lookup::Category(_) => None,
        }
    }
}

/// Which displays make and use each entry, and each category's.
#[derive(Clone, Debug, Default)]
pub struct Index {
    pub recipes: Vec<Recipe>,
    by_output: HashMap<u32, Vec<usize>>,
    by_input: HashMap<u32, Vec<usize>>,
    by_category: HashMap<Category, Vec<usize>>,
    workstations: HashMap<Category, Vec<u32>>,
}

impl Index {
    pub fn new(recipes: Vec<Recipe>, workstations: HashMap<Category, Vec<u32>>) -> Index {
        let mut index = Index { workstations, ..Index::default() };
        for (i, r) in recipes.iter().enumerate() {
            let mut seen = Vec::new();
            for s in r.outputs.iter().flatten().filter(|s| !s.is_empty()) {
                if !seen.contains(&s.id) {
                    seen.push(s.id);
                    index.by_output.entry(s.id).or_default().push(i);
                }
            }
            seen.clear();
            for s in r.inputs.iter().flatten().filter(|s| !s.is_empty()) {
                if !seen.contains(&s.id) {
                    seen.push(s.id);
                    index.by_input.entry(s.id).or_default().push(i);
                }
            }
            index.by_category.entry(r.category()).or_default().push(i);
        }
        index.recipes = recipes;
        index
    }
    pub fn workstations(&self, c: Category) -> &[u32] {
        self.workstations.get(&c).map_or(&[], Vec::as_slice)
    }
    pub fn of_category(&self, c: Category) -> &[usize] {
        self.by_category.get(&c).map_or(&[], Vec::as_slice)
    }
    /// The tabs `lookup` shows, in REI's category order: usages of a
    /// workstation hold its categories' every display.
    pub fn lookup(&self, lookup: Lookup) -> Vec<(Category, Vec<usize>)> {
        let mut tabs: Vec<(Category, Vec<usize>)> = Vec::new();
        let add = |c: Category, i: usize, tabs: &mut Vec<(Category, Vec<usize>)>| match tabs.iter_mut().find(|(t, _)| *t == c) {
            Some((_, list)) => {
                if !list.contains(&i) {
                    list.push(i);
                }
            }
            None => tabs.push((c, vec![i])),
        };
        match lookup {
            Lookup::Recipes(id) => {
                for &i in self.by_output.get(&id).map_or(&[][..], Vec::as_slice) {
                    add(self.recipes[i].category(), i, &mut tabs);
                }
            }
            Lookup::Usages(id) => {
                for &i in self.by_input.get(&id).map_or(&[][..], Vec::as_slice) {
                    add(self.recipes[i].category(), i, &mut tabs);
                }
                for c in Category::ALL {
                    if self.workstations(c).contains(&id) {
                        for &i in self.of_category(c) {
                            add(c, i, &mut tabs);
                        }
                    }
                }
            }
            Lookup::Display(i) => {
                if let Some(recipe) = self.recipes.get(i) {
                    add(recipe.category(), i, &mut tabs);
                }
            }
            Lookup::All(_) => {
                for c in Category::ALL {
                    for &i in self.of_category(c) {
                        add(c, i, &mut tabs);
                    }
                }
            }
            Lookup::Category(c) => {
                for &i in self.of_category(c) {
                    add(c, i, &mut tabs);
                }
            }
        }
        tabs.sort_by_key(|(c, _)| c.index());
        tabs
    }
}

// ---- the recipe screen's state ------------------------------------------------

/// The recipe screen: what it shows, the tab and the page, and the screens
/// before it (Backspace goes back; REI keeps ten).
#[derive(Clone, Debug, Default)]
pub struct Screen {
    pub lookup: Option<Lookup>,
    pub tabs: Vec<(Category, Vec<usize>)>,
    pub tab: usize,
    pub page: usize,
    pub history: Vec<(Lookup, Category, usize)>,
}

impl Screen {
    pub fn is_open(&self) -> bool {
        self.lookup.is_some()
    }
    /// Shows `lookup`; false (and nothing changes) when it has no displays.
    pub fn open(&mut self, index: &Index, lookup: Lookup) -> bool {
        let tabs = index.lookup(lookup);
        if tabs.is_empty() {
            return false;
        }
        if let (Some(old), Some((category, _))) = (self.lookup, self.tabs.get(self.tab)) {
            self.history.push((old, *category, self.page));
            if self.history.len() > 10 {
                self.history.remove(0);
            }
        }
        self.lookup = Some(lookup);
        self.tab = match lookup {
            Lookup::All(c) => tabs.iter().position(|(t, _)| *t == c).unwrap_or(0),
            _ => 0,
        };
        self.tabs = tabs;
        self.page = 0;
        true
    }
    /// The screen before: false when there was none (back to the container).
    pub fn back(&mut self, index: &Index) -> bool {
        while let Some((lookup, category, page)) = self.history.pop() {
            let tabs = index.lookup(lookup);
            if tabs.is_empty() {
                continue;
            }
            self.tab = tabs.iter().position(|(c, _)| *c == category).unwrap_or(0);
            self.tabs = tabs;
            self.lookup = Some(lookup);
            self.page = page;
            return true;
        }
        false
    }
    pub fn close(&mut self) {
        *self = Screen::default();
    }
    pub fn category(&self) -> Option<Category> {
        self.tabs.get(self.tab).map(|(c, _)| *c)
    }
    pub fn recipes(&self) -> &[usize] {
        self.tabs.get(self.tab).map_or(&[], |(_, r)| r.as_slice())
    }
    pub fn select_tab(&mut self, tab: usize) {
        if tab < self.tabs.len() && tab != self.tab {
            self.tab = tab;
            self.page = 0;
        }
    }
    pub fn turn_tab(&mut self, delta: i32) {
        let tab = turn(self.tab, delta, self.tabs.len());
        self.select_tab(tab);
    }
    pub fn turn_page(&mut self, delta: i32, per_page: usize) {
        self.page = turn(self.page, delta, pages(self.recipes().len(), per_page));
    }
}

/// The recipe window's height and each category's displays a page
/// (`DefaultDisplayViewingScreen.init`): `room` is the screen's height less
/// the tab and search rows, `counts` each shown category with its displays.
pub fn window_height(room: f32, counts: &[(Category, usize)]) -> f32 {
    let largest = room.max(100.0).min(300.0);
    let mut height: f32 = 0.0;
    for &(c, n) in counts {
        let shown = (per_page(largest, c)).min(n.max(8)).max(1);
        height = height.max(36.0 + (c.height() + 4.0) * shown as f32);
    }
    height.min(largest)
}

/// The displays of `c` a page in a window `height` tall.
pub fn per_page(height: f32, c: Category) -> usize {
    if c.one_per_page() {
        return 1;
    }
    let fit = ((height - 36.0) / (c.height() + 4.0)).floor() as i64 - 1;
    (fit.clamp(0, 7) + 1) as usize
}

// ---- settings ----------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Settings {
    /// The overlay beside container screens (Ctrl+O).
    pub overlay: bool,
    /// Cheats mode (`cheats.mode`): on, or on in creative only.
    pub cheat: bool,
    pub cheat_creative: bool,
    /// `cheats.amount` Reversed (JEI-like): a click gives a stack, right click one.
    pub cheat_reversed: bool,
    pub favorites: bool,
    /// The favorites' "+" button: 0 never, 1 when active, 2 always.
    pub favorites_button: u8,
    /// The craftable filter's button beside the search field.
    pub craftable_button: bool,
    /// The mod's name at the foot of item tooltips, blue and italic.
    pub mod_name: bool,
    /// "Press A to add this to favorites." under entry tooltips.
    pub favorites_hint: bool,
    /// The page labels in rainbow colours.
    pub rainbow: bool,
    /// The search field colours its terms, and underlines them.
    pub syntax: bool,
    pub underline: bool,
    /// The entry list shows nothing until something is searched.
    pub hide_idle: bool,
    /// REI's dark theme.
    pub dark: bool,
    /// Animations end at once.
    pub reduced_motion: bool,
    /// Each recipe's panel: 0 default, 1 lighter, 2 none.
    pub recipe_border: u8,
    /// The entry list on the left, the favorites on the right.
    pub left_panel: bool,
    /// The entry list scrolls instead of turning pages.
    pub scrolling: bool,
    /// Where the search field sits: 0 centred under the screen, 1 under the
    /// entry list, 2 over it (`SearchFieldLocation`).
    pub search_position: u8,
    /// The config button in the top corner instead of beside the search field.
    pub config_upper: bool,
    /// The recipe screen's larger tabs and its larger tab arrows.
    pub larger_tabs: bool,
    pub larger_arrows: bool,
    /// A click on a screen's own recipe arrow shows its category.
    pub clickable_arrows: bool,
    /// The entry list's order: registry, name, groups, then those descending.
    pub ordering: u8,
    /// The entry list's zoom (an entry is `ceil(18 * zoom)` pixels).
    pub zoom: f32,
    /// The entry under the pointer grows instead of being lit.
    pub focus_zoomed: bool,
    /// How `@mod`, `#tooltip`, `$tag` and `*id` take part: 0 always, 1 with
    /// their prefix only, 2 never.
    pub search_modes: [u8; 4],
}

impl Default for Settings {
    fn default() -> Self {
        Settings::read(|_| None)
    }
}

impl Settings {
    /// The toggles the game's own settings page shows for the mod.
    pub const KEYS: [&'static str; 7] = ["overlay", "favorites", "craftable_button", "mod_name", "dark", "left_panel", "scrolling"];

    /// Reads the settings, `get` giving an option's stored value (by its REI
    /// option id, `appearance.theme`), REI's defaults where none is stored.
    pub fn read(get: impl Fn(&str) -> Option<String>) -> Settings {
        let value = |id: &str| get(id).map(|v| v.trim().to_owned()).unwrap_or_else(|| option_default(id).to_owned());
        let is = |id: &str, token: &str| value(id) == token;
        let index = |id: &str| option_index(id, &value(id)).unwrap_or(0) as u8;
        let syntax = value("search.syntax_highlighting");
        Settings {
            overlay: !is("overlay", "false"),
            cheat: is("cheats.mode", "on"),
            cheat_creative: is("cheats.mode", "when_creative"),
            cheat_reversed: is("cheats.amount", "jei_like"),
            favorites: is("favorites.mode", "true"),
            favorites_button: index("favorites.new_favorites_button_visibility"),
            craftable_button: is("layout.craftable_filter", "true"),
            mod_name: is("appearance.append_mod_names", "true"),
            favorites_hint: is("appearance.append_favorites_hint", "true"),
            rainbow: is("appearance.rainbow", "true"),
            syntax: syntax.starts_with("colorful"),
            underline: syntax.ends_with("underscored"),
            hide_idle: is("layout.hide_when_idle", "true"),
            dark: is("appearance.theme", "dark"),
            reduced_motion: is("appearance.reduced_motion", "true"),
            recipe_border: index("appearance.recipe_border"),
            left_panel: is("layout.location", "left"),
            scrolling: is("list.display_mode", "scrolled"),
            search_position: index("layout.search_field_location"),
            config_upper: is("layout.config_button_location", "upper"),
            larger_tabs: is("accessibility.larger_tabs", "true"),
            larger_arrows: is("accessibility.larger_arrow_buttons", "true"),
            clickable_arrows: is("accessibility.clickable_recipe_arrows", "true"),
            ordering: index("list.ordering"),
            zoom: value("list.zoom").parse::<f32>().unwrap_or(1.0).clamp(0.25, 4.0),
            focus_zoomed: is("list.focus_mode", "zoomed"),
            search_modes: [index("search.mod_search"), index("search.tooltip_search"), index("search.tag_search"), index("search.identifier_search")],
        }
    }
    /// Cheating now: on, or on in creative.
    pub fn cheating(&self, creative: bool) -> bool {
        self.cheat || (self.cheat_creative && creative)
    }
    /// One of the menu's and the settings page's switches.
    pub fn get(&self, key: &str) -> bool {
        match key {
            "overlay" => self.overlay,
            "cheat" => self.cheat,
            "favorites" => self.favorites,
            "craftable_button" => self.craftable_button,
            "mod_name" => self.mod_name,
            "syntax" => self.syntax,
            "hide_idle" => self.hide_idle,
            "dark" => self.dark,
            "reduced_motion" => self.reduced_motion,
            "left_panel" => self.left_panel,
            "scrolling" => self.scrolling,
            _ => false,
        }
    }
    /// The option and value a switch stands for (the gear menu's toggles).
    pub fn token(key: &str, on: bool) -> (&'static str, &'static str) {
        let b = if on { "true" } else { "false" };
        match key {
            "cheat" => ("cheats.mode", if on { "on" } else { "off" }),
            "favorites" => ("favorites.mode", b),
            "craftable_button" => ("layout.craftable_filter", b),
            "mod_name" => ("appearance.append_mod_names", b),
            "syntax" => ("search.syntax_highlighting", if on { "colorful" } else { "plain_underscored" }),
            "hide_idle" => ("layout.hide_when_idle", b),
            "dark" => ("appearance.theme", if on { "dark" } else { "light" }),
            "reduced_motion" => ("appearance.reduced_motion", b),
            "left_panel" => ("layout.location", if on { "left" } else { "right" }),
            "scrolling" => ("list.display_mode", if on { "scrolled" } else { "paginated" }),
            _ => ("overlay", b),
        }
    }
}

// ---- the config screen's options (`AllREIConfigCategories`, `AllREIConfigGroups`,
// `AllREIConfigOptions`) -----------------------------------------------------------------

/// How an option's value is shown and changed.
#[derive(Clone, Copy, Debug)]
pub enum OptKind {
    /// Its values: stored token, English, Russian. Two values switch on a
    /// click; more open a menu.
    Choice(&'static [(&'static str, &'static str, &'static str)]),
    /// A key of the mod's (`KEYS` name).
    Key(&'static str),
    /// A link to a sub-screen or an action: "Configure...", "Details...",
    /// "Reload..." (the text key).
    Link(&'static str),
    /// A value shown, never changed here (REI's command strings).
    Text,
}

#[derive(Clone, Copy, Debug)]
pub struct Opt {
    pub id: &'static str,
    pub name: (&'static str, &'static str),
    pub desc: Option<(&'static str, &'static str)>,
    pub kind: OptKind,
    pub default: &'static str,
    /// Shown as REI shows it but not changeable in Horizon: what it changes is
    /// the game's own, or a part of REI not ported.
    pub fixed: bool,
    /// Shows "Load Level First!" outside a world.
    pub level: bool,
}

/// A group's inline preview (`ConfigGroupWidget`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Preview {
    None,
    Interface,
    Tooltip,
    Accessibility,
}

#[derive(Clone, Copy, Debug)]
pub struct Group {
    pub id: &'static str,
    pub name: (&'static str, &'static str),
    pub preview: Preview,
    pub opts: &'static [Opt],
}

#[derive(Clone, Copy, Debug)]
pub struct Cat {
    pub key: &'static str,
    pub name: (&'static str, &'static str),
    pub desc: Option<(&'static str, &'static str)>,
    pub groups: &'static [Group],
}

const BOOL: &[(&str, &str, &str)] = &[("false", "Disabled", "Выключено"), ("true", "Enabled", "Включено")];
const SEARCH_MODES: &[(&str, &str, &str)] = &[
    ("always", "Always Active", "Always Active"),
    ("prefix", "Activate with Prefix", "Activate with Prefix"),
    ("never", "Always Inactive", "Always Inactive"),
];
const PERCENT: &[(&str, &str, &str)] = &[
    ("0.0", "0%", "0%"), ("0.05", "5%", "5%"), ("0.1", "10%", "10%"), ("0.15", "15%", "15%"), ("0.2", "20%", "20%"),
    ("0.25", "25%", "25%"), ("0.3", "30%", "30%"), ("0.35", "35%", "35%"), ("0.4", "40%", "40%"), ("0.45", "45%", "45%"),
    ("0.5", "50%", "50%"), ("0.55", "55%", "55%"), ("0.6", "60%", "60%"), ("0.65", "65%", "65%"), ("0.7", "70%", "70%"),
    ("0.75", "75%", "75%"), ("0.8", "80%", "80%"), ("0.85", "85%", "85%"), ("0.9", "90%", "90%"), ("0.95", "95%", "95%"),
    ("1.0", "100%", "100%"),
];
const ZOOMS: &[(&str, &str, &str)] = &[
    ("0.25", "25%", "25%"), ("0.5", "50%", "50%"), ("0.75", "75%", "75%"), ("1.0", "100%", "100%"),
    ("1.25", "125%", "125%"), ("1.5", "150%", "150%"), ("1.75", "175%", "175%"), ("2.0", "200%", "200%"),
    ("2.25", "225%", "225%"), ("2.5", "250%", "250%"), ("2.75", "275%", "275%"), ("3.0", "300%", "300%"),
    ("3.25", "325%", "325%"), ("3.5", "350%", "350%"), ("3.75", "375%", "375%"), ("4.0", "400%", "400%"),
];

const fn opt(id: &'static str, name: (&'static str, &'static str), desc: Option<(&'static str, &'static str)>, kind: OptKind, default: &'static str) -> Opt {
    Opt { id, name, desc, kind, default, fixed: false, level: false }
}
const fn fixed(mut o: Opt) -> Opt {
    o.fixed = true;
    o
}
const fn level(mut o: Opt) -> Opt {
    o.level = true;
    o
}
/// A keybind: its stored value is its modifiers ("ctrl", "alt+shift"), the
/// key itself is the game's mod key `keybind`.
const fn key(id: &'static str, name: &'static str, desc: Option<(&'static str, &'static str)>, keybind: &'static str) -> Opt {
    opt(id, (name, name), desc, OptKind::Key(keybind), "")
}
const fn key_with(id: &'static str, name: &'static str, keybind: &'static str, modifiers: &'static str) -> Opt {
    opt(id, (name, name), None, OptKind::Key(keybind), modifiers)
}

pub const CATEGORIES: &[Cat] = &[
    Cat {
        key: "appearance",
        name: ("Appearance", "Внешний вид"),
        desc: Some(("Interface theming, style, tooltips", "Тематика интерфейса, стиль, всплывающие подсказки")),
        groups: &[
            Group {
                id: "appearance.interface",
                name: ("Interface", "Интерфейс"),
                preview: Preview::Interface,
                opts: &[
                    opt("appearance.theme", ("Theme", "Тема"), Some(("The global theme of Roughly Enough Items. This will be applied everywhere, including background, buttons, etc.", "Глобальная тема Roughly Enough Items. Будет применена везде, включая фон, кнопки и т. д.")), OptKind::Choice(&[("light", "Light", "Светлый"), ("dark", "Dark", "Темный")]), "light"),
                    opt("appearance.recipe_border", ("Recipe Border", "Граница рецепта"), Some(("The border of each recipe. Default is the preferred REI look.", "Граница каждого рецепта.  По умолчанию выбран предпочтительный вид REI.")), OptKind::Choice(&[("default", "Default", "По умолчанию"), ("lighter", "Lighter", "Светлее"), ("none", "Invisible", "Невидимый")]), "default"),
                    opt("appearance.reduced_motion", ("Reduced Motion", "Уменьшенное Движение"), Some(("Disables animation for transitions, menus, hover states, to minimize discomfort caused by vestibular motion triggers.", "Disables animation for transitions, menus, hover states, to minimize discomfort caused by vestibular motion triggers.")), OptKind::Choice(BOOL), "false"),
                    fixed(opt("appearance.recipe_lookup_style", ("Recipe Lookup Style", "Recipe Lookup Style"), Some(("The style of how recipes are displayed. Classic is the default look, while Composite is for the more informative, villger-like look.", "The style of how recipes are displayed. Classic is the default look, while Composite is for the more informative, villger-like look.")), OptKind::Choice(&[("original", "Classic", "Classic"), ("composite", "Composite", "Composite")]), "original")),
                ],
            },
            Group {
                id: "appearance.tooltips",
                name: ("Tooltips", "Tooltips"),
                preview: Preview::Tooltip,
                opts: &[
                    opt("appearance.append_mod_names", ("Append Mod Names", "Append Mod Names"), Some(("Appends the containing namespace for entries. The appended line will be in italisised light blue.", "Appends the containing namespace for entries. The appended line will be in italisised light blue.")), OptKind::Choice(BOOL), "true"),
                    opt("appearance.append_favorites_hint", ("Append Favorites Hint", "Append Favorites Hint"), Some(("Shows a hint on how to favorite an entry, or a recipe.", "Shows a hint on how to favorite an entry, or a recipe.")), OptKind::Choice(BOOL), "false"),
                ],
            },
            Group {
                id: "appearance.advanced",
                name: ("Advanced", "Advanced"),
                preview: Preview::None,
                opts: &[opt("appearance.rainbow", ("Rainbow o(〒﹏〒)o", "Rainbow o(〒﹏〒)o"), None, OptKind::Choice(BOOL), "false")],
            },
        ],
    },
    Cat {
        key: "input",
        name: ("Input", "Вход"),
        desc: Some(("Keybindings, input methods", "Сочетания клавиш, методы ввода")),
        groups: &[Group {
            id: "input.keybinds",
            name: ("Keybinds", "Keybinds"),
            preview: Preview::None,
            opts: &[
                key("input.recipe", "View Recipes", None, "recipe"),
                key("input.usage", "View Usages", None, "usage"),
                key_with("input.hide", "Show / Hide REI", "hide", "ctrl"),
                key("input.previous_page", "Navigate to Previous Page", None, "previous_page"),
                key("input.next_page", "Navigate to Next Page", None, "next_page"),
                key("input.focus_search", "Focus Search Field", None, "focus_search"),
                key("input.copy_recipe_id", "Copy Recipe Identifier", Some(("Copy a recipe's identifier by pressing the keybind, while hovering over a recipe's info button (shown with \"+\", or \"?\").", "Copy a recipe's identifier by pressing the keybind, while hovering over a recipe's info button (shown with \"+\", or \"?\").")), "copy_recipe_id"),
                key("input.favorite", "Favorite Entry", None, "favorite"),
                key("input.back", "Back", Some(("Quickly navigate back to the previous recipes by pressing the keybind.", "Quickly navigate back to the previous recipes by pressing the keybind.")), "back"),
            ],
        }],
    },
    Cat {
        key: "layout",
        name: ("Layout", "Разметка"),
        desc: Some(("Widgets, boundaries", "Виджеты, границы")),
        groups: &[
            Group {
                id: "layout.widgets",
                name: ("Widgets", "Widgets"),
                preview: Preview::None,
                opts: &[
                    opt("layout.search_field_location", ("Search Field Location", "Search Field Location"), Some(("The location of the search field. By default, the search field is placed at the bottom center, which automatically moves to the side to make way for tall container screens.", "The location of the search field. By default, the search field is placed at the bottom center, which automatically moves to the side to make way for tall container screens.")), OptKind::Choice(&[("center", "Center", "Center"), ("bottom_side", "Bottom Right", "Bottom Right"), ("top_side", "Top Right", "Top Right")]), "center"),
                    opt("layout.config_button_location", ("Config Button Location", "Config Button Location"), Some(("The location of the configuration button. By default, it is placed next to the search field. However, this can be changed to look like the legacy versions of REI, where it is placed on the top side of the screen.", "The location of the configuration button. By default, it is placed next to the search field. However, this can be changed to look like the legacy versions of REI, where it is placed on the top side of the screen.")), OptKind::Choice(&[("upper", "Top Left", "Top Left"), ("lower", "Next to Search Field", "Next to Search Field")]), "lower"),
                    opt("layout.craftable_filter", ("Craftable Filter", "Craftable Filter"), Some(("Craftable Filter allows you to view stacks currently craftable with the ingredients in your inventory. This can be useful alongside the Ctrl+Click quick craft feature to craft quickly.", "Craftable Filter allows you to view stacks currently craftable with the ingredients in your inventory. This can be useful alongside the Ctrl+Click quick craft feature to craft quickly.")), OptKind::Choice(BOOL), "true"),
                ],
            },
            Group {
                id: "layout.panel",
                name: ("Panel", "Panel"),
                preview: Preview::None,
                opts: &[
                    fixed(opt("layout.boundaries", ("Boundaries", "Boundaries"), Some(("Declare the boundaries of the panel. Useful to limit the columns / rows displayed on screen.", "Declare the boundaries of the panel. Useful to limit the columns / rows displayed on screen.")), OptKind::Link("configure"), "")),
                    opt("layout.location", ("Location", "Location"), Some(("Change the location of the panel. The entire REI setup may be mirrored horizontally, including the list and all widgets.", "Change the location of the panel. The entire REI setup may be mirrored horizontally, including the list and all widgets.")), OptKind::Choice(&[("left", "Left", "Left"), ("right", "Right", "Right")]), "right"),
                    opt("layout.hide_when_idle", ("Hide When Idle", "Hide When Idle"), Some(("Hide the panel when search is not active.", "Hide the panel when search is not active.")), OptKind::Choice(BOOL), "false"),
                ],
            },
        ],
    },
    Cat {
        key: "accessibility",
        name: ("Accessibility", "Доступность"),
        desc: Some(("Vision, features", "Зрение, особенности")),
        groups: &[
            Group {
                id: "accessibility.display",
                name: ("Display", "Display"),
                preview: Preview::Accessibility,
                opts: &[
                    opt("accessibility.larger_tabs", ("Larger Tabs", "Larger Tabs"), Some(("Increase the size of tabs in recipe viewing screens.", "Increase the size of tabs in recipe viewing screens.")), OptKind::Choice(BOOL), "false"),
                    opt("accessibility.larger_arrow_buttons", ("Larger Arrow Buttons", "Larger Arrow Buttons"), Some(("Increase the size of pages switching buttons next to tabs in recipe viewing screens.", "Increase the size of pages switching buttons next to tabs in recipe viewing screens.")), OptKind::Choice(BOOL), "true"),
                ],
            },
            Group {
                id: "accessibility.widgets",
                name: ("Widgets", "Widgets"),
                preview: Preview::None,
                opts: &[
                    fixed(opt("accessibility.scrollbar_visibility", ("Scrollbar Visibility", "Scrollbar Visibility"), None, OptKind::Choice(&[("false", "When Scrolling", "When Scrolling"), ("true", "Always Visible", "Always Visible")]), "false")),
                    opt("accessibility.clickable_recipe_arrows", ("Clickable Recipe Arrows", "Clickable Recipe Arrows"), Some(("Click on recipe arrows to see all recipes in the target category.", "Click on recipe arrows to see all recipes in the target category.")), OptKind::Choice(BOOL), "true"),
                ],
            },
            Group {
                id: "accessibility.inventory_search",
                name: ("Inventory Search", "Inventory Search"),
                preview: Preview::None,
                opts: &[
                    fixed(opt("accessibility.inventory_search_mode", ("Mode", "Mode"), Some(("Inventory Search allows you to search for items in your inventory. This can be useful for finding items in your inventory quickly.\nDouble click the search bar to toggle this feature.", "Inventory Search allows you to search for items in your inventory. This can be useful for finding items in your inventory quickly.\nDouble click the search bar to toggle this feature.")), OptKind::Choice(BOOL), "true")),
                    fixed(opt("accessibility.inventory_search_darken_opacity", ("Dim Overlay Opacity", "Dim Overlay Opacity"), Some(("The opacity of the dark overlay for items not matching the search filter.", "The opacity of the dark overlay for items not matching the search filter.")), OptKind::Choice(PERCENT), "0.85")),
                    fixed(opt("accessibility.inventory_search_opacity", ("Highlight Opacity", "Highlight Opacity"), Some(("The opacity of the green overlay for items matching the search filter.", "The opacity of the green overlay for items matching the search filter.")), OptKind::Choice(PERCENT), "1.0")),
                ],
            },
            Group {
                id: "accessibility.features",
                name: ("Features", "Features"),
                preview: Preview::None,
                opts: &[
                    fixed(opt("accessibility.vanilla_recipe_book", ("Vanilla Recipe Book", "Vanilla Recipe Book"), Some(("Toggle the vanilla recipe book. This option is not designed for Modpack developers, as users should be the one to decide whether to use the vanilla recipe book.", "Toggle the vanilla recipe book. This option is not designed for Modpack developers, as users should be the one to decide whether to use the vanilla recipe book.")), OptKind::Choice(BOOL), "true")),
                    fixed(opt("accessibility.status_effects_location", ("Status Effects Location", "Status Effects Location"), Some(("The location of the status effects. By vanilla, the status effects are placed on the right, but this can be changed to the left.", "The location of the status effects. By vanilla, the status effects are placed on the right, but this can be changed to the left.")), OptKind::Choice(&[("false", "Right", "Right"), ("true", "Left", "Left")]), "false")),
                ],
            },
        ],
    },
    Cat {
        key: "filtering",
        name: ("Filtering", "Фильтрация"),
        desc: Some(("Categories, entries filtering", "Категории, фильтрация записей")),
        groups: &[
            Group {
                id: "filtering.filtering",
                name: ("Filtering", "Filtering"),
                preview: Preview::None,
                opts: &[
                    fixed(level(opt("filtering.categories", ("Manage Categories", "Manage Categories"), Some(("Reorder, toggle, and configure categories. This is useful for organizing entries, and hiding entries you do not need.", "Reorder, toggle, and configure categories. This is useful for organizing entries, and hiding entries you do not need.")), OptKind::Link("configure"), ""))),
                    fixed(level(opt("filtering.customized_filtering", ("Customized Filtering", "Customized Filtering"), Some(("Create filtering rules to hide / show specific entries from the entry list.", "Create filtering rules to hide / show specific entries from the entry list.")), OptKind::Link("configure"), ""))),
                ],
            },
            Group {
                id: "filtering.advanced",
                name: ("Advanced", "Advanced"),
                preview: Preview::None,
                opts: &[
                    fixed(opt("filtering.filter_displays", ("Filter Displays", "Filter Displays"), Some(("Apply filtering rules to determine the visibility of displays. Displays with all entries filtered will be hidden.", "Apply filtering rules to determine the visibility of displays. Displays with all entries filtered will be hidden.")), OptKind::Choice(BOOL), "true")),
                    fixed(opt("filtering.merge_displays", ("Merge Displays", "Merge Displays"), Some(("Merge displays with the same recipe. This is useful for unifying recipes.", "Merge displays with the same recipe. This is useful for unifying recipes.")), OptKind::Choice(BOOL), "true")),
                    fixed(opt("filtering.force_local_recipes", ("Local Recipes Fallback", "Local Recipes Fallback"), Some(("Synthesizes recipe displays from your client's own data packs, for multiplayer servers (e.g. 2b2t) that do not send recipe data. §eAuto: loads local recipes as a fallback until the server syncs its own; §eAlways: loads local recipes even when the server provides them; §eNever: off. §eWarning: Some recipes may not exist on the server.", "Synthesizes recipe displays from your client's own data packs, for multiplayer servers (e.g. 2b2t) that do not send recipe data. §eAuto: loads local recipes as a fallback until the server syncs its own; §eAlways: loads local recipes even when the server provides them; §eNever: off. §eWarning: Some recipes may not exist on the server.")), OptKind::Choice(&[("never", "Never", "Never"), ("auto", "Auto", "Auto"), ("always", "Always", "Always")]), "auto")),
                ],
            },
        ],
    },
    Cat {
        key: "list",
        name: ("List", "Список"),
        desc: Some(("List order, zoom, collapsible groups", "Порядок списка, масштабирование, свертываемые группы")),
        groups: &[
            Group {
                id: "list.entries",
                name: ("Entries", "Entries"),
                preview: Preview::None,
                opts: &[
                    opt("list.display_mode", ("Display Mode", "Display Mode"), Some(("The way entries are laid out. Paginated mode displays entries in pages, where there are buttons to traverse the different pages. Scrolled mode displays entries in a vertical list.", "The way entries are laid out. Paginated mode displays entries in pages, where there are buttons to traverse the different pages. Scrolled mode displays entries in a vertical list.")), OptKind::Choice(&[("paginated", "Paginated", "Paginated"), ("scrolled", "Scrolled", "Scrolled")]), "paginated"),
                    opt("list.ordering", ("Ordering", "Ordering"), Some(("The method the entries are sorted by. By default, REI follows the order in which entries are registered into the game. This can be changed to mimic the creative tabs or the direction of the sorting.", "The method the entries are sorted by. By default, REI follows the order in which entries are registered into the game. This can be changed to mimic the creative tabs or the direction of the sorting.")), OptKind::Choice(&[("registry", "By Registry", "By Registry"), ("name", "By Name", "By Name"), ("groups", "By Groups", "By Groups"), ("registry_desc", "By Registry (Descending)", "By Registry (Descending)"), ("name_desc", "By Name (Descending)", "By Name (Descending)"), ("groups_desc", "By Groups (Descending)", "By Groups (Descending)")]), "registry"),
                    opt("list.zoom", ("Zoom", "Zoom"), Some(("The size of the entries. Increasing the zoom amount may also improve performance as less entries are on the screen at the same time.", "The size of the entries. Increasing the zoom amount may also improve performance as less entries are on the screen at the same time.")), OptKind::Choice(ZOOMS), "1.0"),
                    opt("list.focus_mode", ("Focus Mode", "Focus Mode"), Some(("How the focus state of the entries will be presented. By default, REI follows vanilla and highlights the slot.", "How the focus state of the entries will be presented. By default, REI follows vanilla and highlights the slot.")), OptKind::Choice(&[("highlighted", "Highlighted", "Highlighted"), ("zoomed", "Zoomed", "Zoomed")]), "highlighted"),
                ],
            },
            Group {
                id: "list.collapsible_groups",
                name: ("Collapsible Groups", "Collapsible Groups"),
                preview: Preview::None,
                opts: &[fixed(level(opt("list.collapsible_entries", ("Collapsible Entries", "Collapsible Entries"), Some(("Collapsible Entries allows you to group entries together, and expand / collapse them. This is useful for organizing entries, and hiding entries you do not need.", "Collapsible Entries allows you to group entries together, and expand / collapse them. This is useful for organizing entries, and hiding entries you do not need.")), OptKind::Link("configure"), "")))],
            },
        ],
    },
    Cat {
        key: "favorites",
        name: ("Favorites", "Избранное"),
        desc: None,
        groups: &[
            Group {
                id: "favorites.favorites",
                name: ("Favorites", "Favorites"),
                preview: Preview::None,
                opts: &[
                    opt("favorites.mode", ("Mode", "Mode"), Some(("Favorite different entries on the opposite side of the entry panel. The favorites are persistent across reloads, and act as a quick access to things you are working on.\nSimply press \"A\", or drag and drop entries to add / remove an entry from favorites.", "Favorite different entries on the opposite side of the entry panel. The favorites are persistent across reloads, and act as a quick access to things you are working on.\nSimply press \"A\", or drag and drop entries to add / remove an entry from favorites.")), OptKind::Choice(BOOL), "true"),
                    opt("favorites.new_favorites_button_visibility", ("New Favorites Button Visibility", "New Favorites Button Visibility"), Some(("The visibility of the new favorites button on the bottom side of the panel.", "The visibility of the new favorites button on the bottom side of the panel.")), OptKind::Choice(&[("invisible", "Always Invisible", "Always Invisible"), ("auto_hide", "When Active", "When Active"), ("visible", "Always Visible", "Always Visible")]), "visible"),
                ],
            },
            Group {
                id: "favorites.advanced",
                name: ("Advanced", "Advanced"),
                preview: Preview::None,
                opts: &[
                    opt("favorites.game_mode_command", ("Game Mode Command", "Game Mode Command"), Some(("The command invoked to change the gamemode. This may be useful if the server replaced the default /gamemode command. Available placeholders: {gamemode}.", "The command invoked to change the gamemode. This may be useful if the server replaced the default /gamemode command. Available placeholders: {gamemode}.")), OptKind::Text, "/gamemode {gamemode}"),
                    opt("favorites.time_command", ("Time Command", "Time Command"), Some(("The command invoked to change the time. This may be useful if the server replaced the default /time command. Available placeholders: {time}.", "The command invoked to change the time. This may be useful if the server replaced the default /time command. Available placeholders: {time}.")), OptKind::Text, "/time set {time}"),
                    opt("favorites.weather_command", ("Weather Command", "Weather Command"), Some(("The command invoked to change the weather. This may be useful if the server replaced the default /weather command. Available placeholders: {weather}.", "The command invoked to change the weather. This may be useful if the server replaced the default /weather command. Available placeholders: {weather}.")), OptKind::Text, "/weather {weather}"),
                ],
            },
        ],
    },
    Cat {
        key: "search",
        name: ("Search", "Поиск"),
        desc: None,
        groups: &[
            Group {
                id: "search.appearance",
                name: ("Appearance", "Appearance"),
                preview: Preview::None,
                opts: &[opt("search.syntax_highlighting", ("Syntax Highlighting", "Syntax Highlighting"), Some(("Displays different search filters in different colors for easier identification.", "Displays different search filters in different colors for easier identification.")), OptKind::Choice(&[("plain", "Plain", "Plain"), ("plain_underscored", "Underscored", "Underscored"), ("colorful", "Colorful", "Colorful"), ("colorful_underscored", "Colorful & Underscored", "Colorful & Underscored")]), "colorful")],
            },
            Group {
                id: "search.filters",
                name: ("Filters", "Filters"),
                preview: Preview::None,
                opts: &[
                    opt("search.mod_search", ("Mod Search", "Mod Search"), Some(("Filter entries by its containing mod §l%s§r.", "Filter entries by its containing mod §l%s§r.")), OptKind::Choice(SEARCH_MODES), "prefix"),
                    opt("search.tooltip_search", ("Tooltip Search", "Tooltip Search"), Some(("Filter entries by contents of its tooltip §l%s§r. This is useful for searching through details of the entry, for example, enchantments.", "Filter entries by contents of its tooltip §l%s§r. This is useful for searching through details of the entry, for example, enchantments.")), OptKind::Choice(SEARCH_MODES), "always"),
                    opt("search.tag_search", ("Tag Search", "Tag Search"), Some(("Filter entries by the tags it is contained in §l%s§r.", "Filter entries by the tags it is contained in §l%s§r.")), OptKind::Choice(SEARCH_MODES), "prefix"),
                    opt("search.identifier_search", ("Identifier Search", "Identifier Search"), Some(("Filter entries by its identifier §l%s§r.", "Filter entries by its identifier §l%s§r.")), OptKind::Choice(SEARCH_MODES), "always"),
                ],
            },
            Group {
                id: "search.advanced",
                name: ("Advanced", "Advanced"),
                preview: Preview::None,
                opts: &[fixed(opt("search.async_search", ("Async Search", "Async Search"), Some(("Parallelize search with multiple threads. This usually improves performance and \"snappiness\" of search.", "Parallelize search with multiple threads. This usually improves performance and \"snappiness\" of search.")), OptKind::Choice(BOOL), "true"))],
            },
        ],
    },
    Cat {
        key: "cheats",
        name: ("Cheats", "Читы"),
        desc: None,
        groups: &[
            Group {
                id: "cheats.cheats",
                name: ("Cheats", "Cheats"),
                preview: Preview::None,
                opts: &[
                    opt("cheats.mode", ("Mode", "Mode"), Some(("Cheats mode allows you to grab items from the side. This requires operator permission. The /give command would be used as a fallback if REI is not installed on the server.", "Cheats mode allows you to grab items from the side. This requires operator permission. The /give command would be used as a fallback if REI is not installed on the server.")), OptKind::Choice(&[("off", "Disabled", "Disabled"), ("on", "Enabled", "Enabled"), ("when_creative", "When Creative", "When Creative")]), "off"),
                    fixed(opt("cheats.method", ("Method", "Method"), Some(("The way how cheating is achieved. Grab mode places the cheated item at your cursor, while give mode places the cheated item in your inventory.", "The way how cheating is achieved. Grab mode places the cheated item at your cursor, while give mode places the cheated item in your inventory.")), OptKind::Choice(&[("grab", "Grab", "Grab"), ("give", "Give", "Give")]), "give")),
                    opt("cheats.amount", ("Amount", "Amount"), Some(("The amount of items cheated. By default, REI cheats 1 item and a stack when clicked with shift.", "Количество обманутых предметов. По умолчанию REI выдаёт 1 предмет и стак при нажатии на shift.")), OptKind::Choice(&[("rei_like", "Default", "Default"), ("jei_like", "Reversed", "Reversed")]), "rei_like"),
                ],
            },
            Group {
                id: "cheats.advanced",
                name: ("Advanced", "Advanced"),
                preview: Preview::None,
                opts: &[opt("cheats.give_command", ("Give Command", "Give Command"), Some(("The command invoked to cheat items when REI is not installed on the server. This may be useful if the server replaced the default /give command. Available placeholders: {item_name}, {item_identifier}, {count} and {player_name}.", "The command invoked to cheat items when REI is not installed on the server. This may be useful if the server replaced the default /give command. Available placeholders: {item_name}, {item_identifier}, {count} and {player_name}.")), OptKind::Text, "/give {player_name} {item_identifier}{nbt} {count}")],
            },
        ],
    },
    Cat {
        key: "performance",
        name: ("Performance", "Производительность"),
        desc: None,
        groups: &[Group {
            id: "performance.reload",
            name: ("Reload", "Reload"),
            preview: Preview::None,
            opts: &[
                fixed(opt("performance.reload_thread", ("Reload Thread", "Reload Thread"), Some(("By using its own thread by default for reloading, this prevents soft-locking the game until everything is loaded in.", "By using its own thread by default for reloading, this prevents soft-locking the game until everything is loaded in.")), OptKind::Choice(&[("false", "Main Thread (Blocking)", "Main Thread (Blocking)"), ("true", "REI Thread", "REI Thread")]), "true")),
                fixed(opt("performance.cached_display_lookup", ("Cached Display Lookup", "Cached Display Lookup"), Some(("Cached Display Lookup reduces the time when looking up a display. This stores the inputs and outputs of a display ahead of time.", "Cached Display Lookup reduces the time when looking up a display. This stores the inputs and outputs of a display ahead of time.")), OptKind::Choice(BOOL), "true")),
            ],
        }],
    },
    Cat {
        key: "debug",
        name: ("Debug", "Отладка"),
        desc: None,
        groups: &[Group {
            id: "debug.performance",
            name: ("Performance", "Performance"),
            preview: Preview::None,
            opts: &[
                fixed(level(opt("debug.plugins_performance", ("Plugins Performance", "Plugins Performance"), Some(("Detailed break-down of plugins' reload times. This is very useful in debugging the performance after loaded into a world.", "Detailed break-down of plugins' reload times. This is very useful in debugging the performance after loaded into a world.")), OptKind::Link("details"), ""))),
                fixed(opt("debug.search_performance", ("Search Performance", "Search Performance"), Some(("Verbose console print out for analysing search performance.", "Verbose console print out for analysing search performance.")), OptKind::Choice(BOOL), "false")),
                fixed(opt("debug.entry_list_performance", ("Entry List Performance", "Entry List Performance"), Some(("Display the time used to render entries on the entry list.", "Display the time used to render entries on the entry list.")), OptKind::Choice(BOOL), "false")),
                fixed(level(opt("debug.display_registry_analysis", ("Display Registry Analysis", "Display Registry Analysis"), Some(("Break-down in registration of displays in each category.", "Break-down in registration of displays in each category.")), OptKind::Link("details"), ""))),
            ],
        }],
    },
    Cat {
        key: "reset",
        name: ("Reload & Reset", "Перезагрузить и сбросить"),
        desc: None,
        groups: &[
            Group {
                id: "reset.reload",
                name: ("Reload", "Reload"),
                preview: Preview::None,
                opts: &[
                    level(opt("reset.reload_plugins", ("Reload Plugins", "Reload Plugins"), Some(("Resets the current loaded data and reload all data and plugins.", "Resets the current loaded data and reload all data and plugins.")), OptKind::Link("reload"), "")),
                    level(opt("reset.reload_search", ("Reload Search", "Reload Search"), Some(("Resets all cache around search. The search cache will rebuild before the first search after reloading.", "Resets all cache around search. The search cache will rebuild before the first search after reloading.")), OptKind::Link("reload"), "")),
                ],
            },
            Group { id: "reset.reset", name: ("Reset", "Reset"), preview: Preview::None, opts: &[] },
        ],
    },
];

/// The recipe screen's tabs a page: as many of 24 (or 28) px as fit in the
/// window less 6, an odd number (`TabContainerWidget.initTabsSize`).
pub fn tabs_per_page(larger: bool) -> usize {
    let n = ((190 - 6) / if larger { 28 } else { 24 }) as usize;
    if n % 2 == 0 { n - 1 } else { n }
}

/// An option by its id.
pub fn find_option(id: &str) -> Option<&'static Opt> {
    CATEGORIES.iter().flat_map(|c| c.groups.iter()).flat_map(|g| g.opts.iter()).find(|o| o.id == id)
}

/// An option's default token (`"true"` for the overlay, which has no row).
pub fn option_default(id: &str) -> &'static str {
    find_option(id).map_or("true", |o| o.default)
}

/// The place of `token` among an option's values.
pub fn option_index(id: &str, token: &str) -> Option<usize> {
    match find_option(id)?.kind {
        OptKind::Choice(values) => values.iter().position(|(t, _, _)| *t == token),
        _ => None,
    }
}

// ---- display makers ---------------------------------------------------------------------

/// A crafting recipe's inputs on REI's 3x3 grid (`CraftingDisplay`): entry
/// `i` of a list `w` wide at row `i / w`, column `i % w`, from the top left;
/// a shapeless recipe is `min(n, 3)` wide. Also its size for the 2x2 grid.
pub fn crafting_grid(inputs: Vec<Vec<Stack>>, width: u32, shapeless: bool) -> (Vec<Vec<Stack>>, u32, u32) {
    let n = inputs.len();
    let w = if shapeless || width == 0 {
        if n >= 9 { 3 } else { n.clamp(1, 3) }
    } else {
        (width as usize).clamp(1, 3)
    };
    let mut grid = vec![Vec::new(); 9];
    for (i, slot) in inputs.into_iter().enumerate() {
        let (row, col) = (i / w, i % w);
        if row < 3 {
            grid[row * 3 + col] = slot;
        }
    }
    let (gw, gh) = if shapeless || width == 0 {
        let side = if n > 4 { 3 } else { 2 };
        (side, side)
    } else {
        (w as u32, n.div_ceil(w) as u32)
    };
    (grid, gw, gh)
}

/// An anvil repairing an item `damage` worn of `max` with `count` of its
/// material (`AnvilMenu.createResult`): the damage left and the cost, None
/// when nothing is mended.
pub fn anvil_repair(max: u32, damage: u32, count: u32) -> Option<(u32, u32)> {
    let quarter = max / 4;
    let (mut left, mut used) = (damage, 0);
    while used < count {
        let mend = left.min(quarter);
        if mend == 0 {
            break;
        }
        left -= mend;
        used += 1;
    }
    (used > 0).then_some((left, used))
}

/// The chance vanilla's named compost providers add a layer
/// (`DefaultCompostingCategory`: low 30%, low-medium 50%, medium 65%,
/// medium-high 85%, anything else 100%).
pub fn compost_chance_named(name: &str) -> Option<f32> {
    let name = name.strip_prefix("minecraft:").unwrap_or(name);
    Some(match name.strip_prefix("compostable/")? {
        "low" => 0.3,
        "low_medium" => 0.5,
        "medium" => 0.65,
        "high" | "medium_high" => 0.85,
        _ => 1.0,
    })
}

/// Java's `DecimalFormat("###.##")`: two decimals at most, no trailing
/// zeros, no zero before the point (`.35`), the system's separator.
pub fn decimal(v: f64, ru: bool) -> String {
    let text = format!("{v:.2}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    let text = match text.strip_prefix("0.") {
        Some(rest) => format!(".{rest}"),
        None => text.to_owned(),
    };
    let text = if text.is_empty() || text == "-0" { "0".to_owned() } else { text };
    if ru { text.replace('.', ",") } else { text }
}

// ---- keys and labels ------------------------------------------------------------------

/// The mod's keys on Options > Mods: name, label, default key (REI's
/// keybinds; "hide" goes with Ctrl).
pub const KEYS: [(&str, &str, &str); 9] = [
    ("recipe", "key_recipe", "key.keyboard.r"),
    ("usage", "key_usage", "key.keyboard.u"),
    ("hide", "key_hide", "key.keyboard.o"),
    ("previous_page", "key_previous_page", "key.keyboard.unknown"),
    ("next_page", "key_next_page", "key.keyboard.unknown"),
    ("focus_search", "key_focus_search", "key.keyboard.unknown"),
    ("copy_recipe_id", "key_copy_recipe_id", "key.mouse.middle"),
    ("favorite", "key_favorite", "key.keyboard.a"),
    ("back", "key_back", "key.keyboard.backspace"),
];

/// A label in English or Russian (REI's `en_us` and `ru_ru`, English where
/// REI has no Russian; the mod's own settings where REI has none).
pub fn tr(ru: bool, key: &str) -> &'static str {
    let (en, ru_text) = match key {
        "description" => (
            "Item and recipe viewer: every item beside container screens, its recipes and usages, favorites.",
            "Просмотр предметов и рецептов: все предметы рядом с инвентарём, их рецепты и применение, избранное.",
        ),
        "on" => ("ON", "ВКЛ"),
        "off" => ("OFF", "ВЫКЛ"),
        "overlay" => ("Show REI", "Показывать REI"),
        "cheat" => ("Cheating", "Читы"),
        "favorites" => ("Favorites", "Избранное"),
        "craftable_button" => ("Craftable Filter", "Фильтр по возможности создания"),
        "mod_name" => ("Append Mod Names", "Названия модов в подсказках"),
        "syntax" => ("Syntax Highlighting", "Подсветка синтаксиса"),
        "hide_idle" => ("Hide Entries when not Searching", "Скрыть записи, когда вы не ищете"),
        "dark" => ("Dark Theme", "Тёмная тема"),
        "reduced_motion" => ("Reduced Motion", "Уменьшить движение"),
        "left_panel" => ("Left Side Panel", "Боковая панель слева"),
        "scrolling" => ("Scrolling Side Panel", "Прокрутка боковой панели"),
        "menu_display" => ("Display Settings...", "Настройки отображения..."),
        "menu_remove_recipe_book" => ("Remove Recipe Book", "Убрать книгу рецептов"),
        "menu_left_effects" => ("Left Side Mob Effects", "Эффекты мобов слева"),
        "menu_search_position" => ("Search Field Position...", "Положение поля поиска..."),
        "clear_favorites" => ("Clear favorites", "Очистить избранное"),
        "key_recipe" => ("View Recipes", "Показать рецепты"),
        "key_usage" => ("View Usages", "Показать применение"),
        "key_hide" => ("Show / Hide REI (Ctrl+)", "Показать / скрыть REI (Ctrl+)"),
        "key_previous_page" => ("Navigate to Previous Page", "Предыдущая страница"),
        "key_next_page" => ("Navigate to Next Page", "Следующая страница"),
        "key_focus_search" => ("Focus Search Field", "Выбрать поле поиска"),
        "key_favorite" => ("Favorite Entry", "Добавить в избранное"),
        "key_copy_recipe_id" => ("Copy Recipe Identifier", "Copy Recipe Identifier"),
        "config_title" => ("Roughly Enough Items Config", "Конфигурация Roughly Enough Items"),
        "cancel" => ("Cancel", "Отмена"),
        "back_arrow" => ("↩ Back", "↩ Назад"),
        "search_options" => ("Search options...", "Search options..."),
        "value_default" => ("%s (Default)", "%s (Default)"),
        "configure" => ("Configure...", "Configure..."),
        // Mod Menu's `modmenu.configure`: the page's button to the config screen.
        "modmenu_configure" => ("Configure...", "Настройки..."),
        "details" => ("Details...", "Details..."),
        "reload" => ("Reload...", "Reload..."),
        "requires_level" => ("Load Level First!", "Load Level First!"),
        "not_bound" => ("Not Bound", "Не назначена"),
        "favorites_tooltip" => ("Press %s to add this to favorites.", "Нажмите %s чтобы добавить в избранное."),
        "remove_favorites_tooltip" => ("Press %s to remove this from favorites.", "Нажмите %s чтобы убрать из избранного."),
        "oak_planks" => ("Oak Planks", "Дубовые доски"),
        "not_ported" => ("Not available in Horizon", "Недоступно в Horizon"),
        "view_recipes_for" => ("View Recipes for %s", "Посмотреть рецепты для %s"),
        "key_back" => ("Back", "Назад"),
        // The overlay.
        "search" => ("Search...", "Поиск..."),
        "previous_page" => ("Previous Page", "Предыдущая страница"),
        "next_page" => ("Next Page", "Следующая страница"),
        "go_back_first_page" => ("Back to Page 1", "Вернуться в начало"),
        "choose_page" => ("Choose Page", "выбрать страницу"),
        "shift_click_choose" => ("Shift-Click to Choose Page", "Shift+ЛКМ для выбрать страницу"),
        "showing_craftable" => ("Showing Craftable", "Показать создаваемое"),
        "showing_all" => ("Showing All", "Показать всё"),
        "cheating" => ("Cheating", "Читы"),
        "cheating_disabled" => ("§7Cheating Disabled", "§7Читы выключены"),
        "cheating_commands" => ("§bCheating Enabled (Using Commands)", "§bЧиты включены (Используя команды)"),
        "cheating_creative" => ("§aCheating Enabled (Using Creative)", "§aЧиты включены (Включён Творческий режим)"),
        "menu_craftable_filter" => ("Craftable Filter", "Фильтр по возможности создания"),
        "menu_syntax" => ("Syntax Highlighting", "Подсветка синтаксиса"),
        "menu_config" => ("More Options...", "Дополнительные настройки..."),
        "menu_hide_idle" => ("Hide Entries when not Searching", "Скрыть записи, когда вы не ищете"),
        "move_items_tooltip" => ("Ctrl-Click to Quick Craft", "Ctrl+ЛКМ для помещения предметов"),
        "collapsed" => ("Collapsed Entry: ", "Скрытый предмет: "),
        "collapsed_expand" => ("Alt-Click to expand all ", "Нажмите Alt, чтобы расширить все "),
        "collapsed_collapse" => ("Alt-Click to collapse all ", "Нажмите Alt, чтобы свернуть все "),
        "entries" => ("entries", "предметов"),
        "spawn_egg" => ("Spawn Egg", "Яйца призыва"),
        "music_disc" => ("Music Disc", "Музыкальный диск"),
        // Favorites.
        "add_favorite_widget" => ("Add...", "Добавить..."),
        "dispose_here" => ("Dispose Here", "Убирать сюда"),
        "section_gamemode" => ("Game Mode", "Игровой режим"),
        "section_weather" => ("Weather", "Погода"),
        "section_time" => ("Time", "Время"),
        "switch_to" => ("Switch to ", "Сменить на "),
        "drag_to_add" => ("§7Drag this up to add this to your favorites!", "§7Перетащите, чтобы добавить в избранное!"),
        "weather_clear" => ("Clear", "Ясная погода"),
        "weather_rain" => ("Rain", "Дождь"),
        "weather_thunder" => ("Thunder", "Гроза"),
        "time_morn" => ("Morning", "Утро"),
        "time_noon" => ("Noon", "Полдень"),
        "time_evening" => ("Evening", "Вечер"),
        "time_night" => ("Night", "Ночь"),
        "short_survival" => ("S", "В"),
        "short_creative" => ("C", "Т"),
        "short_adventure" => ("A", "П"),
        "short_spectator" => ("SP", "Н"),
        // The recipe screen.
        "previous_category" => ("Previous Category", "Предыдущая категория"),
        "next_category" => ("Next Category", "Следующая категория"),
        "view_all_categories" => ("View All Categories", "Показать все категории"),
        "categories" => ("categories", "категорий"),
        "shapeless" => ("Shapeless", "Бесформенные"),
        "move_items" => ("Move Items", "Переместить предметы"),
        "not_supported" => (
            "Move Items is not supported with this recipe and this container.",
            "Помещение предметов не поддерживается этим рецептом и этим контейнером.",
        ),
        "not_enough" => ("Not enough materials.", "Недостаточно материалов."),
        "missing" => ("Missing following:", "Пропущено следующее:"),
        "too_small" => ("Unable to move items to a 2x2 grid.", "Невозможно поместить предметы в сетку 2x2."),
        "no_room" => ("Can't find any space for item in the inventory", "В инвентаре недостаточно места для этого предмета"),
        "accepts" => ("Accepts following:", "Принимаются следующие варианты:"),
        "recipe_id" => ("Recipe Id: ", "ID рецепта: "),
        "cooking_time_xp" => ("%s XP in %s sec", "%s опыта за %s сек"),
        "campfire_time" => ("%s sec", "%s секунд"),
        "brewing_input" => ("Original Potion", "Оригинальное зелье"),
        "brewing_reactant" => ("Ingredient", "Ингредиент"),
        "brewing_result" => ("Resulted Potion", "Получаемое зелье"),
        "composting_chance" => ("§e%d%% Chance", "§e%d%% шанс"),
        "composting_page" => ("Page %d", "%d страница"),
        "expand_view" => ("Open Expanded View", "Открыть расширенный просмотр"),
        "expand_view_close" => ("Close Expanded View", "Закрыть расширенный просмотр"),
        "tag_copy" => ("Save Tag Tree to clipboard", "Сохранить теги в буфер обмена"),
        "tag_accept" => ("Tag: #", "Теги: #"),
        "save_recipes" => ("Press %s on this recipe to save it for later.", "Нажмите %s на рецепт, чтобы сохранить его на попозже."),
        _ => ("", ""),
    };
    if ru { ru_text } else { en }
}

/// A category's title (`category.rei.*`); the anvil's is the game's own block
/// name, which the glue looks up.
pub fn category_title(ru: bool, c: Category) -> &'static str {
    let (en, ru_text) = match c {
        Category::Crafting => ("Crafting", "Создание"),
        Category::Smelting => ("Smelting", "Плавление"),
        Category::Smoking => ("Smoking", "Копчение"),
        Category::Blasting => ("Blasting", "Выплавка"),
        Category::Campfire => ("Campfire", "Жарка на костре"),
        Category::StoneCutting => ("Stone Cutting", "Резка"),
        Category::Brewing => ("Brewing", "Зельеварение"),
        Category::Composting => ("Composting", "Компостирование"),
        Category::Stripping => ("Stripping", "Обтёсывание"),
        Category::Smithing => ("Smithing", "Ковка"),
        Category::Anvil => ("Anvil", "Наковальня"),
        Category::BeaconBase => ("Beacon Base", "Основа для маяка"),
        Category::BeaconPayment => ("Beacon Payment", "Оплата маяка"),
        Category::Tilling => ("Tilling", "Вспашка"),
        Category::Pathing => ("Pathing", "Создание троп"),
        Category::Waxing => ("Waxing", "Вощение"),
        Category::WaxScraping => ("Wax Scraping", "Соскаб. воска"),
        Category::Oxidizing => ("Oxidizing", "Окисление"),
        Category::OxidationScraping => ("Oxidation Scraping", "Соскаб. окисла"),
        Category::Tag => ("Tag", "Тэг"),
    };
    if ru { ru_text } else { en }
}

/// A mod's display name from its id when it has no other: `minecraft` as
/// `Minecraft`, `my_mod` as `My Mod`.
pub fn mod_display_name(id: &str) -> String {
    id.split(['_', '-'])
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(f) => f.to_uppercase().chain(c).collect::<String>(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry<'a>(name: &'a str, path: &'a str, tags: &'a [String], tooltip: Option<&'a str>) -> Searchable<'a> {
        Searchable {
            name,
            name_case: name,
            mod_id: "minecraft",
            mod_name: "minecraft",
            tooltip,
            tags,
            path,
        }
    }

    #[test]
    fn plain_words_search_the_name_tooltip_and_path() {
        let tags = vec!["minecraft:logs".to_owned()];
        let log = entry("oak log", "oak_log", &tags, Some("oak log\nbuilding blocks"));
        assert!(Query::parse("oak").matches(&log));
        assert!(Query::parse("OAK LOG").matches(&log));
        assert!(Query::parse("building").matches(&log));
        assert!(Query::parse("oak_log").matches(&log));
        assert!(!Query::parse("oak plank").matches(&log));
        assert!(Query::parse("oak").needs_tooltips());
        assert!(!Query::parse("@x").needs_tooltips());
    }

    #[test]
    fn prefixes_negation_alternatives_and_quotes() {
        let tags = vec!["minecraft:logs".to_owned()];
        let log = entry("oak log", "oak_log", &tags, Some("oak log"));
        let none: Vec<String> = Vec::new();
        let stone = entry("stone", "stone", &none, Some("stone"));
        assert!(Query::parse("$logs").matches(&log));
        assert!(!Query::parse("$logs").matches(&stone));
        // Tags need their prefix.
        assert!(!Query::parse("logs").matches(&stone));
        assert!(Query::parse("@minecraft").matches(&stone));
        assert!(!Query::parse("-@minecraft").matches(&stone));
        assert!(!Query::parse("@-minecraft").matches(&stone));
        assert!(Query::parse("*oak").matches(&log));
        assert!(Query::parse("-oak").matches(&stone));
        assert!(!Query::parse("-oak").matches(&log));
        assert!(Query::parse("stone | oak").matches(&log));
        assert!(Query::parse("\"oak log\"").matches(&log));
        assert!(!Query::parse("\"log oak\"").matches(&log));
        // An empty group matches everything; so does an empty search.
        assert!(Query::parse("zzz|").matches(&stone));
        assert!(Query::parse("").matches(&stone));
        // A lone dash excludes everything.
        assert!(!Query::parse("-").matches(&stone));
    }

    #[test]
    fn regex_terms_match_the_whole_name() {
        let none: Vec<String> = Vec::new();
        let log = entry("oak log", "oak_log", &none, None);
        let named = Searchable { name_case: "Oak Log", ..log };
        assert!(Query::parse("r/Oak.*/").matches(&named));
        assert!(!Query::parse("r/oak.*/").matches(&named));
        assert!(!Query::parse("r/Oak/").matches(&named));
        assert!(Query::parse("-r/Stone/").matches(&named));
        let r = Regex::parse("[A-Z][a-z]+ (Log|Wood)").unwrap();
        assert!(r.full_match("Oak Log"));
        assert!(!r.full_match("oak Log"));
        assert!(Regex::parse("a\\d?b+").unwrap().full_match("a7bbb"));
        assert!(Regex::parse("(").is_none());
    }

    #[test]
    fn the_field_colours_each_term() {
        let m = marks("@mc |\"a b\" -#x");
        assert_eq!(m[0], Mark::Prefix(Kind::Mod));
        assert_eq!(m[1], Mark::Body(Kind::Mod));
        assert_eq!(m[3], Mark::Plain);
        assert_eq!(m[4], Mark::Bar);
        assert_eq!(m[5], Mark::Quote);
        assert_eq!(m[6], Mark::Plain);
        assert_eq!(m[9], Mark::Quote);
        assert_eq!(m[11], Mark::Prefix(Kind::Tooltip));
        assert_eq!(m[12], Mark::Prefix(Kind::Tooltip));
        assert_eq!(m[13], Mark::Body(Kind::Tooltip));
        let r = marks("r/a/");
        assert_eq!(r, vec![Mark::Prefix(Kind::Regex), Mark::Prefix(Kind::Regex), Mark::Body(Kind::Regex), Mark::Prefix(Kind::Regex)]);
    }

    #[test]
    fn pages_wrap_and_slots_cycle_together() {
        assert!(inside((5.0, 5.0), [0.0, 0.0, 10.0, 10.0]));
        assert!(!inside((10.0, 5.0), [0.0, 0.0, 10.0, 10.0]));
        assert_eq!(pages(0, 10), 1);
        assert_eq!(pages(21, 10), 3);
        assert_eq!(turn(0, -1, 3), 2);
        assert_eq!(turn(2, 1, 3), 0);
        assert_eq!(cycle(3, 2500), 2);
    }

    #[test]
    fn favorites_and_shortcuts_survive_a_round_trip() {
        let mut f = Favorites::default();
        f.add("minecraft:stone", None);
        f.add("minecraft:dirt", None);
        f.add("minecraft:stone", None);
        assert_eq!(f.keys, ["minecraft:dirt", "minecraft:stone"]);
        f.add("rei:time/noon", Some(0));
        assert!(f.remove("minecraft:dirt"));
        assert_eq!(Favorites::parse(&f.write()), f);
        assert_eq!(Shortcut::from_key("rei:time/noon"), Some(Shortcut::Time(1)));
        assert_eq!(Shortcut::Time(1).command(), "time set 6000");
        assert_eq!(Shortcut::GameMode(1).key(), "rei:gamemode/creative");
        assert_eq!(Shortcut::from_key("rei:weather/rain").map(Shortcut::command).as_deref(), Some("weather rain"));
    }

    fn recipe(category: Category, inputs: &[u32], output: u32) -> Recipe {
        Recipe {
            category: Some(category),
            inputs: inputs.iter().map(|&i| vec![Stack::one(i)]).collect(),
            outputs: vec![vec![Stack::one(output)]],
            ..Recipe::default()
        }
    }

    #[test]
    fn lookups_follow_reis_order_and_workstations() {
        // 1 log, 2 planks, 3 sticks, 4 crafting table, 5 furnace, 6 charcoal.
        let recipes = vec![
            recipe(Category::Crafting, &[1], 2),
            recipe(Category::Crafting, &[2, 2], 3),
            recipe(Category::Smelting, &[1], 6),
            recipe(Category::Crafting, &[2, 2, 2, 2], 4),
        ];
        let mut ws = HashMap::new();
        ws.insert(Category::Crafting, vec![4]);
        ws.insert(Category::Smelting, vec![5]);
        let index = Index::new(recipes, ws);
        assert_eq!(index.lookup(Lookup::Usages(1)), vec![(Category::Crafting, vec![0]), (Category::Smelting, vec![2])]);
        assert_eq!(index.lookup(Lookup::Usages(4)), vec![(Category::Crafting, vec![0, 1, 3])]);
        assert_eq!(index.lookup(Lookup::Recipes(3)), vec![(Category::Crafting, vec![1])]);
        assert!(index.lookup(Lookup::Recipes(1)).is_empty());

        let mut screen = Screen::default();
        assert!(!screen.open(&index, Lookup::Recipes(1)));
        assert!(screen.open(&index, Lookup::Usages(1)));
        screen.select_tab(1);
        assert_eq!(screen.category(), Some(Category::Smelting));
        assert!(screen.open(&index, Lookup::Recipes(3)));
        assert!(screen.back(&index));
        assert_eq!((screen.lookup, screen.category()), (Some(Lookup::Usages(1)), Some(Category::Smelting)));
        assert!(!screen.back(&index));
        assert!(screen.open(&index, Lookup::All(Category::Smelting)));
        assert_eq!(screen.recipes(), &[2]);
    }

    #[test]
    fn the_window_grows_with_its_categories() {
        // REI's worked example: 270 high, crafting only.
        let room = 270.0 - 24.0 - 24.0;
        assert_eq!(per_page(room, Category::Crafting), 2);
        assert_eq!(window_height(room, &[(Category::Crafting, 3)]), 176.0);
        assert_eq!(window_height(room, &[(Category::Crafting, 3), (Category::Smelting, 1)]), 195.0);
        assert_eq!(per_page(195.0, Category::Smelting), 3);
        assert_eq!(per_page(195.0, Category::Composting), 1);
    }

    #[test]
    fn crafting_sits_top_left_as_in_rei() {
        let s = |i: u32| vec![Stack::one(i)];
        // A stick (1x2): the left column.
        let (grid, w, h) = crafting_grid(vec![s(1), s(1)], 1, false);
        assert_eq!((w, h), (1, 2));
        assert_eq!((&grid[0], &grid[3]), (&s(1), &s(1)));
        // Shapeless: three in a row, four as three and one; size 2x2.
        let (grid, w, h) = crafting_grid(vec![s(1), s(2), s(3), s(4)], 0, true);
        assert_eq!((&grid[2], &grid[3], w, h), (&s(3), &s(4), 2, 2));
        let (_, w, _) = crafting_grid(vec![s(1); 5], 0, true);
        assert_eq!(w, 3);
    }

    #[test]
    fn tag_trees_lay_out_and_print_as_in_rei() {
        // #mining: shears, then #shovels (seven) and #hoes (two).
        let shovels = TagTree { id: "minecraft:shovels".into(), values: (1..=7).map(Ok).collect(), children: vec![], all: (1..=7).collect() };
        let hoes = TagTree { id: "minecraft:hoes".into(), values: vec![Ok(8), Err("minecraft:missing".into())], children: vec![], all: vec![8] };
        let mining = TagTree { id: "minecraft:mining".into(), values: vec![Ok(9)], children: vec![shovels, hoes], all: vec![] };
        let root = TagNode::Reference(&mining);
        assert_eq!(root.size(), (24.0, 23.0));
        assert_eq!(TagNode::Values(&mining.children[0].values).size(), (76.0, 44.0));
        assert_eq!(root.children().len(), 3);
        // Children: shears (28 wide), #shovels (76 wide under its frame), #hoes (44).
        let (w, h) = root.tree_size();
        assert_eq!(w, 28.0 + 6.0 + 76.0 + 6.0 + 44.0);
        assert_eq!(h, 23.0 + 16.0 + 23.0 + 16.0 + 44.0);
        let text = mining.text(&|id| format!("minecraft:item{id}"));
        assert!(text.starts_with("minecraft:mining\n├── minecraft:item9\n├── minecraft:shovels\n│   └── minecraft:item1\n"));
        assert!(text.ends_with("└── minecraft:hoes\n    └── minecraft:item8\n    └── minecraft:missing\n"));
        // Small trees are drawn 1.28 times; large ones start at 1.
        assert_eq!(tag_scale_range((40.0, 40.0), (122.0, 52.0)), (0.78, 0.78));
        assert_eq!(tag_scale_range((300.0, 200.0), (122.0, 52.0)).1, 300.0 / 122.0);
        assert_eq!(tag_offset(5.0, 50.0, 122.0), 36.0);
        assert_eq!(tag_offset(5.0, 200.0, 122.0), 0.0);
        assert_eq!(tag_offset(-500.0, 200.0, 122.0), -78.0);
    }

    #[test]
    fn anvil_compost_and_numbers() {
        assert_eq!(anvil_repair(1561, 390, 1), Some((0, 1)));
        assert_eq!(anvil_repair(1561, 1561, 4), Some((1, 4)));
        assert_eq!(anvil_repair(1561, 0, 1), None);
        assert_eq!(compost_chance_named("minecraft:compostable/low"), Some(0.3));
        assert_eq!(compost_chance_named("minecraft:compostable/other"), Some(1.0));
        assert_eq!(compost_chance_named("minecraft:cooking/x"), None);
        assert_eq!(decimal(0.35, false), ".35");
        assert_eq!(decimal(10.0, false), "10");
        assert_eq!(decimal(0.1, true), ",1");
        assert_eq!(decimal(0.0, false), "0");
        assert_eq!(decimal(1.5, false), "1.5");
        assert_eq!(mod_display_name("my_mod"), "My Mod");
    }

    #[test]
    fn settings_start_as_reis_defaults() {
        let s = Settings::default();
        assert!(s.overlay && !s.dark && s.favorites && s.mod_name && !s.cheat && !s.cheat_creative);
        assert!(s.syntax && !s.underline && s.larger_arrows && !s.larger_tabs && s.clickable_arrows);
        assert_eq!((s.zoom, s.search_modes, s.favorites_button, s.recipe_border, s.ordering), (1.0, [1, 0, 1, 0], 2, 0, 0));
        assert_eq!((tabs_per_page(false), tabs_per_page(true)), (7, 5));
        for o in CATEGORIES.iter().flat_map(|c| c.groups.iter()).flat_map(|g| g.opts.iter()) {
            if let OptKind::Choice(values) = o.kind {
                assert!(values.iter().any(|v| v.0 == o.default), "{}", o.id);
            }
        }
        // A stored value wins over the default.
        let dark = Settings::read(|k| (k == "appearance.theme").then(|| "dark".to_owned()));
        assert!(dark.dark);
    }
}
