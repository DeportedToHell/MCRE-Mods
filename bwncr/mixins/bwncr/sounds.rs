// Bad Wither No Cookie's check of every sound the client starts
// (`SoundEngineMixin` and `SoundHandler.onPlaySound`). It sits beside the
// game's sound hook in the client library, which can't reach the game's
// modules: the game keeps the settings current (`set`) and takes the debug
// lines and the names heard for `/bwncr listen` at each frame (`take`).
// Ported from Bad Wither No Cookie - Reloaded by Kreezxil and its
// contributors (MIT, https://gitlab.com/kreezxil/BadWitherNoCookie).
use std::sync::Mutex;

/// What the mod silences, as `SoundEventHandler` copies it from the config
/// before each sound.
#[derive(Clone, Debug, PartialEq)]
pub struct Filter {
    /// The mod is on in Options > Mods.
    pub on: bool,
    pub wither: bool,
    pub trader: bool,
    pub dragon: bool,
    pub lightning: bool,
    /// `silenceUs`.
    pub sounds: Vec<String>,
    /// `debugMode`: each sound's lines go to the log.
    pub debug: bool,
    /// `/bwncr listen` (`CommonClass.whatWasThat`): each sound's name goes to
    /// the chat.
    pub listen: bool,
}

const OFF: Filter = Filter {
    on: false,
    wither: false,
    trader: false,
    dragon: false,
    lightning: false,
    sounds: Vec::new(),
    debug: false,
    listen: false,
};

/// At most this many lines and names wait for the game's next frame.
const KEEP: usize = 512;
/// The instances remembered as started.
const SEEN: usize = 256;

struct State {
    filter: Filter,
    log: Vec<String>,
    heard: Vec<String>,
    /// The instances already seen starting: Horizon starts a looping sound
    /// again through this hook, where Java's `SoundEngine.play` runs once, so
    /// the handler's lines and the name heard come once per sound.
    seen: Vec<u64>,
}

static STATE: Mutex<State> = Mutex::new(State { filter: OFF, log: Vec::new(), heard: Vec::new(), seen: Vec::new() });

fn state() -> std::sync::MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(|e| e.into_inner())
}

/// The settings from now on.
pub fn set(filter: Filter) {
    let mut s = state();
    if s.filter != filter {
        s.filter = filter;
    }
}

/// The debug lines and the names heard since the last call.
pub fn take() -> (Vec<String>, Vec<String>) {
    let mut s = state();
    (std::mem::take(&mut s.log), std::mem::take(&mut s.heard))
}

/// The sound event's path, as `SoundInstance.getLocation().getPath()` gives
/// it (`minecraft:entity.wither.spawn` is `entity.wither.spawn`).
pub fn path(identifier: &str) -> &str {
    identifier.split_once(':').map_or(identifier, |(_, path)| path)
}

/// `SoundHandler.onPlaySound` for the sound `name`: whether it is silenced,
/// its debug lines into `log` when there is one.
fn judge(f: &Filter, name: &str, mut log: Option<&mut Vec<String>>) -> bool {
    // The lines are written out only when they are kept.
    let mut say = |line: std::fmt::Arguments<'_>| {
        if let Some(log) = log.as_deref_mut() {
            log.push(line.to_string());
        }
    };
    say(format_args!("Intercepted {name}"));
    let mut silenced = false;
    if (name == "entity.wither.spawn" || name.eq_ignore_ascii_case("entity.wither.death")) && f.wither {
        say(format_args!("Silencing the wither's death."));
        silenced = true;
    }
    if (name.eq_ignore_ascii_case("entity.wandering_trader.ambient") || name.eq_ignore_ascii_case("entity.llama.ambient"))
        && f.trader
    {
        say(format_args!("Silencing wandering trader and llama ambient sounds."));
        silenced = true;
    }
    if name == "entity.ender_dragon.death" && f.dragon {
        say(format_args!("Silencing the ender dragon death"));
        silenced = true;
    }
    if name == "entity.lightning_bolt.thunder" && f.lightning {
        say(format_args!("Silencing thunder"));
        silenced = true;
    }
    for sound in &f.sounds {
        if name == sound {
            say(format_args!("Silencing {sound}"));
            silenced = true;
        }
    }
    silenced
}

/// A sound about to start (`starting`) or to change while it plays: true
/// when the mod silences it, so it doesn't start.
pub fn play(instance: u64, identifier: &str, starting: bool) -> bool {
    if !starting {
        return false;
    }
    let mut s = state();
    if !s.filter.on {
        return false;
    }
    let name = path(identifier);
    let first = !s.seen.contains(&instance);
    if first {
        if s.seen.len() >= SEEN {
            s.seen.remove(0);
        }
        s.seen.push(instance);
    }
    let State { filter, log, heard, .. } = &mut *s;
    let lines = (first && filter.debug && log.len() < KEEP).then_some(log);
    let silenced = judge(filter, name, lines);
    if first && filter.listen && heard.len() < KEEP {
        heard.push(name.to_owned());
    }
    silenced
}

#[cfg(test)]
mod tests {
    use super::*;

    fn on() -> Filter {
        Filter { on: true, wither: true, trader: true, dragon: true, lightning: true, ..OFF }
    }

    #[test]
    fn judges_as_sound_handler() {
        let mut f = on();
        assert!(judge(&f, "entity.wither.spawn", None));
        assert!(judge(&f, "ENTITY.WITHER.DEATH", None));
        assert!(!judge(&f, "ENTITY.WITHER.SPAWN", None));
        assert!(judge(&f, "entity.wandering_trader.ambient", None));
        assert!(judge(&f, "entity.llama.ambient", None));
        assert!(judge(&f, "entity.ender_dragon.death", None));
        assert!(judge(&f, "entity.lightning_bolt.thunder", None));
        assert!(!judge(&f, "entity.cow.ambient", None));
        f.sounds.push("entity.cow.ambient".into());
        let mut log = Vec::new();
        assert!(judge(&f, "entity.cow.ambient", Some(&mut log)));
        assert_eq!(log, ["Intercepted entity.cow.ambient", "Silencing entity.cow.ambient"]);
        f.wither = false;
        assert!(!judge(&f, "entity.wither.spawn", None));
    }

    #[test]
    fn paths_lose_their_namespace() {
        assert_eq!(path("minecraft:entity.wither.spawn"), "entity.wither.spawn");
        assert_eq!(path("entity.wither.spawn"), "entity.wither.spawn");
        assert_eq!(path("mymod:machine.hum"), "machine.hum");
    }

    // One test for the shared state, so no other test runs beside it.
    #[test]
    fn plays_through_the_filter() {
        set(OFF);
        assert!(!play(1, "minecraft:entity.wither.spawn", true));
        set(Filter { debug: true, listen: true, ..on() });
        assert!(play(2, "minecraft:entity.wither.spawn", true));
        assert!(!play(2, "minecraft:entity.wither.spawn", false));
        assert!(!play(3, "minecraft:entity.cow.ambient", true));
        // A loop's next start: silenced still, heard once.
        assert!(!play(3, "minecraft:entity.cow.ambient", true));
        let (log, heard) = take();
        assert_eq!(
            log,
            ["Intercepted entity.wither.spawn", "Silencing the wither's death.", "Intercepted entity.cow.ambient"]
        );
        assert_eq!(heard, ["entity.wither.spawn", "entity.cow.ambient"]);
        assert_eq!(take(), (Vec::new(), Vec::new()));
        set(OFF);
    }
}
