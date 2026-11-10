//! Loading emotes: the built-in ones (with their icons and songs), the `emotes/` folder and its
//! sub-folders (`UniversalEmoteSerializer.loadEmotes`, `EmoteSerializer.serializeEmotes`), each
//! file by its reader (`.json` and `.blockyanim`, `.emotecraft`; Quark's `.emote` is refused as
//! in the original), with the `<name>.png` icon, `<name>.opus` song and `<name>.nbs` song beside
//! it; and writing an emote as a `.emotecraft` file (`BinaryFormat`, the Export Menu).
//!
//! This Source Code Form is a port of Emotecraft (https://github.com/KosmX/emotes), GPL-3.0.

use crate::anim::{Animation, Extra};
use crate::opus::OpusSound;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// The built-in emotes, in the order they load.
pub const BUILT_IN: [&str; 14] = [
    "waving",
    "clap",
    "crying",
    "point",
    "here",
    "palm",
    "backflip",
    "roblox_potion_dance",
    "kazotsky_kick",
    "twerk",
    "club_penguin_dance",
    "take_the_l",
    "caramell_dance",
    "lets_groove",
];

/// A built-in emote's files.
pub struct BuiltIn {
    pub name: &'static str,
    pub json: &'static str,
    pub icon: &'static [u8],
    pub song: Option<&'static [u8]>,
}

/// `serializeInternalJson`.
pub fn load_built_in(b: &BuiltIn) -> Vec<Arc<Animation>> {
    let list = match crate::loader::load_json(b.json) {
        Ok(l) => l,
        Err(e) => {
            log::warn!("[Emotecraft] Failed to load built-in emote! {}: {e}", b.name);
            return Vec::new();
        }
    };
    let song = b.song.and_then(|s| match OpusSound::read(s) {
        Ok(s) => Some(Arc::new(s)),
        Err(e) => {
            log::warn!("[Emotecraft] Error while reading built-in sound: {}: {e}", b.name);
            None
        }
    });
    list.into_iter()
        .map(|mut a| {
            a.extra.insert("isBuiltin".into(), Extra::Bool(true));
            a.icon = Some(Arc::new(b.icon.to_vec()));
            a.sound = song.clone();
            Arc::new(a)
        })
        .collect()
}

/// A reader for the file's name (`findReader`): its extension.
pub fn readable(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".json") || lower.ends_with(".blockyanim") || lower.ends_with(".emotecraft") || lower.ends_with(".emote")
}

fn base_name(name: &str) -> &str {
    match name.rfind('.') {
        Some(i) if i > 0 => &name[..i],
        _ => name,
    }
}

/// `UniversalEmoteSerializer.readData`: the emotes in a file's bytes.
pub fn read_data(bytes: &[u8], file_name: &str) -> Result<Vec<Animation>, String> {
    let lower = file_name.to_ascii_lowercase();
    if lower.ends_with(".emotecraft") {
        let d = crate::packet::NetData::read(bytes, crate::packet::Bound::Client, false).map_err(|e| e.to_string())?;
        if d.purpose != crate::packet::Task::File {
            return Err("Binary emote is invalid".into());
        }
        let a = d.emote.ok_or("Binary emote is invalid")?;
        return Ok(vec![(*a).clone()]);
    }
    if lower.ends_with(".emote") {
        return Err("Quark emotes are not supported".into());
    }
    let text = std::str::from_utf8(bytes).map_err(|e| e.to_string())?;
    let text = text.trim_start_matches('\u{feff}');
    crate::loader::load_json(text)
}

/// `EmoteSerializer.serializeExternalEmote`: a file's emotes with what lies beside them.
pub fn load_file(file: &Path, folder: Option<&str>) -> Vec<Arc<Animation>> {
    let Some(file_name) = file.file_name().map(|n| n.to_string_lossy().into_owned()) else { return Vec::new() };
    let bytes = match std::fs::read(file) {
        Ok(b) => b,
        Err(e) => {
            log::warn!("[Emotecraft] Error while importing external emote: {file_name}: {e}");
            return Vec::new();
        }
    };
    let mut list = match read_data(&bytes, &file_name) {
        Ok(l) => l,
        Err(e) => {
            log::warn!("[Emotecraft] Error while importing external emote: {file_name}: {e}");
            return Vec::new();
        }
    };
    let base = base_name(&file_name).to_owned();
    let dir = file.parent().map(Path::to_path_buf).unwrap_or_default();
    let icon = std::fs::read(dir.join(format!("{base}.png"))).ok().map(Arc::new);
    let sound = dir.join(format!("{base}.opus"));
    let sound = if sound.is_file() {
        match std::fs::read(&sound).map_err(|e| e.to_string()).and_then(|b| OpusSound::read(&b)) {
            Ok(s) => Some(Arc::new(s)),
            Err(e) => {
                log::warn!("[Emotecraft] Error while reading sound: {base}.opus: {e}");
                None
            }
        }
    } else {
        None
    };
    let nbs = std::fs::read(dir.join(format!("{base}.nbs"))).ok().filter(|b| b.len() <= crate::packet::MAX_PACKET_SIZE).map(Arc::new);
    for a in &mut list {
        if let Some(f) = folder.filter(|f| !f.trim().is_empty()) {
            a.extra.insert("folderpath".into(), Extra::Str(f.to_owned()));
        }
        a.extra.insert("fileName".into(), Extra::Str(file_name.clone()));
        a.extra.remove("isBuiltin");
        if icon.is_some() {
            a.icon = icon.clone();
        }
        if sound.is_some() {
            a.sound = sound.clone();
        }
        if nbs.is_some() {
            a.nbs = nbs.clone();
        }
    }
    list.into_iter().map(Arc::new).collect()
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>, depth: u32) {
    if depth > 32 {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<PathBuf> = rd.filter_map(|e| e.ok().map(|e| e.path())).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            walk(&p, out, depth + 1);
        } else if p.is_file() {
            out.push(p);
        }
    }
}

/// `EmoteSerializer.serializeEmotes`: every emote file under `root`, with its folder path;
/// `server/` and `*_export` folders are left out unless `root` is `server/` itself.
pub fn load_dir(root: &Path, skip_server: bool) -> Vec<Arc<Animation>> {
    let mut files = Vec::new();
    walk(root, &mut files, 0);
    let mut out = Vec::new();
    for f in files {
        let Some(name) = f.file_name().map(|n| n.to_string_lossy().into_owned()) else { continue };
        if !readable(&name) {
            continue;
        }
        let folder = f
            .parent()
            .and_then(|p| p.strip_prefix(root).ok())
            .map(|p| p.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/"))
            .unwrap_or_default();
        if (skip_server && folder.starts_with("server")) || folder.contains("_export") {
            continue;
        }
        out.extend(load_file(&f, Some(&folder)));
    }
    out
}

/// What `loadEmotes` loaded: the emotes the server streams to its clients, and the ones every
/// client has (the built-in ones, and a client's or single player's own folder).
#[derive(Default, Clone)]
pub struct Loaded {
    pub server: Vec<Arc<Animation>>,
    pub hidden: Vec<Arc<Animation>>,
}

impl Loaded {
    /// `getLoadedEmotes`: the hidden ones, then the server's (a server one replaces a hidden one
    /// with its id).
    pub fn all(&self) -> Vec<Arc<Animation>> {
        let mut out: Vec<Arc<Animation>> = Vec::new();
        for a in self.hidden.iter().chain(&self.server) {
            if let Some(i) = out.iter().position(|o| o.uuid == a.uuid) {
                out[i] = a.clone();
            } else {
                out.push(a.clone());
            }
        }
        out
    }
}

fn add(list: &mut Vec<Arc<Animation>>, a: Arc<Animation>) {
    if let Some(i) = list.iter().position(|o| o.uuid == a.uuid) {
        list[i] = a;
    } else {
        list.push(a);
    }
}

/// `UniversalEmoteSerializer.loadEmotes`.
pub fn load(built_in: &[BuiltIn], load_built_in_emotes: bool, emotes_dir: &Path, folder_on_server: bool) -> Loaded {
    let mut l = Loaded::default();
    if load_built_in_emotes {
        for b in built_in {
            for a in load_built_in(b) {
                add(&mut l.hidden, a);
            }
        }
    }
    if !emotes_dir.is_dir() {
        let _ = std::fs::create_dir_all(emotes_dir);
    }
    for a in load_dir(emotes_dir, true) {
        add(if folder_on_server { &mut l.server } else { &mut l.hidden }, a);
    }
    let server_dir = emotes_dir.join("server");
    if server_dir.is_dir() {
        for a in load_dir(&server_dir, false) {
            add(&mut l.server, a);
        }
    }
    l
}

/// `EmoteWriter.writeAnimationInFormat` with the binary writer: `<dir>/<name>.emotecraft`, the
/// name made safe as a file name, numbered if taken.
pub fn export(a: &Arc<Animation>, dir: &Path) -> std::io::Result<PathBuf> {
    let raw = a.str("fileName").map(|f| base_name(f).to_owned()).unwrap_or_else(|| a.name_or_id());
    let safe: String = raw.chars().map(|c| if c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | ' ') { c } else { '_' }).collect();
    let safe = if safe.trim().is_empty() { crate::buf::uuid_string(a.uuid) } else { safe };
    let mut path = dir.join(format!("{safe}.emotecraft"));
    let mut n = 1;
    while path.exists() {
        path = dir.join(format!("{safe}_{n}.emotecraft"));
        n += 1;
    }
    std::fs::write(&path, crate::packet::emote_to_bytes(a))?;
    Ok(path)
}
