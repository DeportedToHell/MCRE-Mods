// Bad Wither No Cookie - Reloaded for Horizon: the mod's settings
// (`BWNCR_Config`), kept in `config/bwncr.toml` as AutoConfig's
// `Toml4jConfigSerializer` keeps them on Fabric, and the mod's texts. No game
// types here, so `cargo test` runs it alone.
//
// Ported from Bad Wither No Cookie - Reloaded by Kreezxil and its
// contributors (MIT, https://gitlab.com/kreezxil/BadWitherNoCookie, branch
// 1.21.1, 3.20.4).

pub const ID: &str = "bwncr";
/// `fabric.mod.json`'s name.
pub const NAME: &str = "Bad Wither No Cookie Reloaded";
pub const VERSION: &str = "1.0.0";
/// AutoConfig's file in the game's `config` folder (`@Config(name = "bwncr")`).
pub const CONFIG_FILE: &str = "bwncr.toml";
/// The client command's root (`/bwncr listen`).
pub const COMMAND: &str = "bwncr";

/// `BWNCR_Config.General`, with its defaults.
#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub silence_wither: bool,
    pub silence_trader: bool,
    pub silence_dragon: bool,
    pub silence_lightning: bool,
    pub silence_us: Vec<String>,
    pub debug_mode: bool,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            silence_wither: true,
            silence_trader: true,
            silence_dragon: true,
            silence_lightning: true,
            silence_us: Vec::new(),
            debug_mode: false,
        }
    }
}

// ---- the config file ----------------------------------------------------------------------

/// Reads `bwncr.toml` as Toml4j reads it into `BWNCR_Config`: the `[general]`
/// table's keys, each missing or unreadable one at its default.
pub fn parse(text: &str) -> Config {
    let mut c = Config::default();
    let mut table = String::new();
    let mut lines = text.lines();
    while let Some(line) = lines.next() {
        let line = strip_comment(line).trim();
        if line.is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix('[') {
            table = unquote_key(rest.trim_end_matches(']'));
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = unquote_key(key);
        let mut value = value.trim().to_owned();
        // An array may go on over the next lines until its bracket closes.
        if value.starts_with('[') {
            while !array_closed(&value) {
                let Some(next) = lines.next() else {
                    break;
                };
                value.push(' ');
                value.push_str(strip_comment(next).trim());
            }
        }
        if table != "general" {
            continue;
        }
        let flag = match value.as_str() {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        };
        match (key.as_str(), flag) {
            ("silenceWither", Some(b)) => c.silence_wither = b,
            ("silenceTrader", Some(b)) => c.silence_trader = b,
            ("silenceDragon", Some(b)) => c.silence_dragon = b,
            ("silenceLightning", Some(b)) => c.silence_lightning = b,
            ("debugMode", Some(b)) => c.debug_mode = b,
            ("silenceUs", None) => {
                if let Some(list) = parse_strings(&value) {
                    c.silence_us = list;
                }
            }
            _ => {}
        }
    }
    c
}

/// `bwncr.toml` as Toml4j's `TomlWriter` writes `BWNCR_Config`.
pub fn write(c: &Config) -> String {
    let list: Vec<String> = c.silence_us.iter().map(|s| quote(s)).collect();
    format!(
        "[general]\nsilenceWither = {}\nsilenceTrader = {}\nsilenceDragon = {}\nsilenceLightning = {}\nsilenceUs = [{}]\ndebugMode = {}\n",
        c.silence_wither,
        c.silence_trader,
        c.silence_dragon,
        c.silence_lightning,
        list.join(", "),
        c.debug_mode,
    )
}

/// The line up to a `#` that is not inside a string.
fn strip_comment(line: &str) -> &str {
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for (i, ch) in line.char_indices() {
        match quote {
            Some('"') if escaped => escaped = false,
            Some('"') if ch == '\\' => escaped = true,
            Some(q) if ch == q => quote = None,
            Some(_) => {}
            None if ch == '"' || ch == '\'' => quote = Some(ch),
            None if ch == '#' => return &line[..i],
            None => {}
        }
    }
    line
}

/// An array's text has its closing bracket (outside the strings).
fn array_closed(value: &str) -> bool {
    let mut depth = 0i32;
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for ch in value.chars() {
        match quote {
            Some('"') if escaped => escaped = false,
            Some('"') if ch == '\\' => escaped = true,
            Some(q) if ch == q => quote = None,
            Some(_) => {}
            None => match ch {
                '"' | '\'' => quote = Some(ch),
                '[' => depth += 1,
                ']' => {
                    depth -= 1;
                    if depth == 0 {
                        return true;
                    }
                }
                _ => {}
            },
        }
    }
    false
}

/// A bare or quoted key (`silenceUs`, `"silenceUs"`).
fn unquote_key(key: &str) -> String {
    let k = key.trim();
    if let Some(q) = k.chars().next().filter(|c| *c == '"' || *c == '\'')
        && let Some(s) = read_string(&mut k.chars().peekable(), q)
    {
        return s;
    }
    k.to_owned()
}

/// `["a", 'b']`: the strings, or None when the text is no array of strings.
fn parse_strings(value: &str) -> Option<Vec<String>> {
    let inner = value.trim().strip_prefix('[')?;
    let mut chars = inner.chars().peekable();
    let mut out = Vec::new();
    loop {
        while chars.peek().is_some_and(|c| c.is_whitespace() || *c == ',') {
            chars.next();
        }
        match chars.peek().copied() {
            Some(']') => return Some(out),
            Some(q @ ('"' | '\'')) => out.push(read_string(&mut chars, q)?),
            _ => return None,
        }
    }
}

/// One basic (`"`, with escapes) or literal (`'`) string from `chars`, whose
/// next character is its opening quote `q`.
fn read_string(chars: &mut std::iter::Peekable<std::str::Chars<'_>>, q: char) -> Option<String> {
    if chars.next()? != q {
        return None;
    }
    let mut s = String::new();
    loop {
        let ch = chars.next()?;
        if ch == q {
            return Some(s);
        }
        if q == '"' && ch == '\\' {
            match chars.next()? {
                'b' => s.push('\u{8}'),
                't' => s.push('\t'),
                'n' => s.push('\n'),
                'f' => s.push('\u{c}'),
                'r' => s.push('\r'),
                '"' => s.push('"'),
                '\\' => s.push('\\'),
                u @ ('u' | 'U') => {
                    let n = if u == 'u' { 4 } else { 8 };
                    let hex: String = (0..n).filter_map(|_| chars.next()).collect();
                    s.push(u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32)?);
                }
                _ => return None,
            }
        } else {
            s.push(ch);
        }
    }
}

/// A TOML basic string, as Toml4j's `StringValueWriter` escapes it.
fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\u{c}' => out.push_str("\\f"),
            '\r' => out.push_str("\\r"),
            c if (c as u32) < 0x20 || c as u32 == 0x7f => out.push_str(&format!("\\u{:04X}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

// ---- texts --------------------------------------------------------------------------------

/// The mod's `en_us.json` (the config screen's AutoConfig keys), and the
/// same in Russian (the original has no `ru_ru.json`; translated for the port).
pub fn tr(ru: bool, key: &str) -> &'static str {
    match (key, ru) {
        ("title", _) => "Bad Wither No Cookie - Reloaded",
        ("general", false) => "General",
        ("general", true) => "Общие",
        ("silenceWither", false) => "Silence Wither",
        ("silenceWither", true) => "Заглушить визера",
        ("silenceTrader", false) => "Silence Trader",
        ("silenceTrader", true) => "Заглушить торговца",
        ("silenceDragon", false) => "Silence Dragon",
        ("silenceDragon", true) => "Заглушить дракона",
        ("silenceLightning", false) => "Silence Lightning",
        ("silenceLightning", true) => "Заглушить молнию",
        ("silenceUs", false) => "Silence Us",
        ("silenceUs", true) => "Заглушить эти звуки",
        ("debugMode", false) => "Debug Mode",
        ("debugMode", true) => "Режим отладки",
        _ => "",
    }
}

/// `fabric.mod.json`'s description, on one line (and in Russian).
pub fn description(ru: bool) -> &'static str {
    if ru {
        "Заглушает звуки, которые сервер проигрывает на весь мир, и они становятся слышны только рядом. \
         Можно заглушить и другие звуки: овцу, щиплющую траву, мычание коров и что угодно ещё. \
         Если что-то издаёт звук, его можно заглушить!"
    } else {
        "Silence server level sounds making them local in the process. Other sounds can be silenced even \
         further such as sheep eating grass, cows mooing, you name it. If it makes a sound it can be silenced!"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_without_a_file() {
        assert_eq!(parse(""), Config::default());
    }

    #[test]
    fn writes_what_it_reads() {
        let c = Config {
            silence_wither: false,
            silence_trader: true,
            silence_dragon: false,
            silence_lightning: true,
            silence_us: vec!["entity.sheep.step".into(), "a \"q\" \\ b".into(), "tab\there".into()],
            debug_mode: true,
        };
        assert_eq!(parse(&write(&c)), c);
    }

    #[test]
    fn writes_as_toml4j() {
        let c = Config { silence_us: vec!["entity.cow.ambient".into(), "block.note_block.harp".into()], ..Config::default() };
        assert_eq!(
            write(&c),
            "[general]\nsilenceWither = true\nsilenceTrader = true\nsilenceDragon = true\nsilenceLightning = true\n\
             silenceUs = [\"entity.cow.ambient\", \"block.note_block.harp\"]\ndebugMode = false\n"
        );
        assert!(write(&Config::default()).contains("silenceUs = []\n"));
    }

    #[test]
    fn reads_other_layouts() {
        let text = "# a comment\n\n[general]\n  silenceWither = false # off\n  \"silenceDragon\" = false\n  silenceUs = [\n    'entity.cow.ambient',\n    \"entity.sheep.step\", # here\n  ]\n  debugMode = maybe\n[other]\nsilenceTrader = false\n";
        let c = parse(text);
        assert!(!c.silence_wither);
        assert!(!c.silence_dragon);
        assert!(c.silence_trader);
        assert!(!c.debug_mode);
        assert_eq!(c.silence_us, vec!["entity.cow.ambient".to_owned(), "entity.sheep.step".to_owned()]);
    }

    #[test]
    fn keeps_the_default_list_when_unreadable() {
        assert!(parse("[general]\nsilenceUs = [1, 2]\n").silence_us.is_empty());
        assert!(parse("[general]\nsilenceUs = [\"open\n").silence_us.is_empty());
    }
}
