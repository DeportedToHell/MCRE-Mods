// BetterF3's modules' `update`s: what each line shows, read from the game
// (the window's world, its renderer and sound engine, and the integrated
// server's half of the debug entries). Ported from BetterF3 by cominixo and
// TreyRuffy (MIT, https://github.com/TreyRuffy/BetterF3).
use super::Game;
use super::betterf3::{self as b, Kind, Module, Val, styled, tr, trf};
use super::debug_lines::ServerDebug;
use horizon_client::java_text::format_fixed;
use horizon_core::random::RandomSource as _;
use horizon_client::text::Text;
use horizon_world::heightmap::HeightmapType;
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

/// What every module's update reads this frame.
pub(super) struct Shared {
    /// The server's half of the debug entries (`ServerLevel` and its chunk),
    /// in single player.
    pub(super) server: Option<ServerDebug>,
    pub(super) framebuffer: [i32; 2],
}

/// The camera's feet (`Entity.blockPosition`).
pub(super) fn feet(g: &Game<'_>) -> Option<[i32; 3]> {
    let p = g.player.as_ref()?;
    Some([p.position.x, p.position.y, p.position.z].map(|v| v.floor() as i32))
}

/// The server's lines for the feet, asked of the integrated server for its
/// next tick (or of this game, when it runs the world itself).
pub(super) fn server_debug(g: &Game<'_>, wanted: bool) -> Option<ServerDebug> {
    let at = feet(g).filter(|_| wanted);
    match &g.integrated {
        Some(server) => {
            server.request_debug(at.map(|f| (g.dim, f)));
            at.map(|_| server.debug())
        }
        None if g.remote.is_none() => at.map(|f| g.server_debug_lines(f)),
        None => None,
    }
}

/// Stops the integrated server computing its lines.
pub(super) fn stop_server_debug(g: &Game<'_>) {
    if let Some(server) = &g.integrated {
        server.request_debug(None);
    }
}

/// `String.format("%.Nf", v)`.
fn fixed(v: f64, decimals: usize) -> String {
    format_fixed(v, decimals)
}

/// `Mth.wrapDegrees(float)`.
fn wrap_degrees(v: f32) -> f32 {
    let mut f = v % 360.0;
    if f >= 180.0 {
        f -= 360.0;
    }
    if f < -180.0 {
        f += 360.0;
    }
    f
}

/// A number after `label` in a vanilla debug line (`SC: 12, MO: 3, ...`).
fn number_after(line: &str, label: &str) -> Option<i64> {
    let at = line.find(label)? + label.len();
    let rest = line[at..].trim_start();
    let end = rest.find(|c: char| !(c.is_ascii_digit() || c == '-')).unwrap_or(rest.len());
    rest[..end].parse().ok()
}

/// `Utils.propertyToString`: `key: value`, a boolean green or red.
fn property(key: &str, value: &str) -> String {
    let color = match value {
        "true" => "§a",
        "false" => "§c",
        _ => "",
    };
    format!("{key}: {color}{value}")
}

pub(super) fn update(m: &mut Module, g: &Game<'_>, s: &Shared) {
    match m.kind {
        Kind::Minecraft => minecraft(m),
        Kind::Fps => fps(m, g),
        Kind::Graphics => graphics(m, g),
        Kind::Server => server(m, g),
        Kind::Coords => coords(m, g),
        Kind::Chunks => chunks(m, g, s),
        Kind::Location => location(m, g, s),
        Kind::Entity => entity(m, g, s),
        Kind::Sound => sound(m, g),
        Kind::Help => help(m, g),
        Kind::MiscLeft => {
            // The vanilla lines BetterF3 doesn't show itself: none are left
            // while its own entry is the only one enabled.
            m.lines[0].set(Val::Strings(Vec::new()));
        }
        Kind::MiscRight => {
            m.lines[0].set(Val::Strings(vec![format!("BetterF3 Version: {}", b::VERSION)]));
        }
        Kind::System => system(m, g, s),
        Kind::Target => target(m, g),
        Kind::Empty => empty(m),
    }
}

/// `MinecraftModule`: the version, the launched version and the client's brand.
fn minecraft(m: &mut Module) {
    let v = horizon_client::save::VERSION_NAME;
    m.lines[0].str(format!("{v} ({v}/horizon)"));
}

/// `FpsModule`: fps and the limit, coloured by how high the fps is.
fn fps(m: &mut Module, g: &Game<'_>) {
    let fps = g.fps.round() as i32;
    let limit = g.framerate_limit();
    let limit = if limit >= 260 { tr("text.betterf3.line.fps.unlimited") } else { limit.to_string() };
    let vsync = if g.settings.vsync { tr("text.betterf3.line.fps.vsync") } else { String::new() };
    let text = trf("format.betterf3.fps", &[&fps, &limit, &vsync]).trim().to_owned();
    let color = if fps >= 60 {
        m.extra_color(0)
    } else if fps >= 20 {
        m.extra_color(1)
    } else {
        m.extra_color(2)
    };
    m.lines[0].set(Val::List(vec![styled(text, color)]));
}

/// `GraphicsModule`.
fn graphics(m: &mut Module, g: &Game<'_>) {
    let st = &g.settings;
    let clouds = match st.clouds.as_str() {
        "false" => tr("text.betterf3.line.off"),
        "fast" => tr("text.betterf3.line.fast"),
        _ => tr("text.betterf3.line.fancy"),
    };
    let distance = match &g.remote {
        Some(r) => st.render_distance.min(r.view_distance),
        None => st.render_distance,
    };
    let mut preset = st.graphics_preset.clone();
    if let Some(first) = preset.get(..1) {
        preset = first.to_uppercase() + &preset[1..];
    }
    m.lines[0].str(distance.to_string());
    m.lines[1].str(preset);
    m.lines[2].str(clouds);
    m.lines[3].str(st.biome_blend.to_string());
    let effects = g.requested_post_effects();
    if effects.is_empty() {
        m.lines[4].active = false;
    } else {
        m.lines[4].str(effects.join(", "));
    }
    m.lines[0].reduced = true;
    m.lines[3].reduced = true;
}

/// `ServerModule`: the server (and the integrated server's tick time), the
/// packets per second and the tick manager.
fn server(m: &mut Module, g: &Game<'_>) {
    let integrated = g.has_singleplayer_server();
    let name = if integrated {
        tr("text.betterf3.line.integrated_server")
    } else if g.player.is_some() {
        g.remote.as_ref().map(|r| r.brand.clone()).unwrap_or_default()
    } else {
        String::new()
    };
    let [sent, received] = g.debug_charts.packet_rates;
    m.lines[1].str((sent.round() as i64).to_string());
    m.lines[2].str((received.round() as i64).to_string());
    let tick = if integrated {
        let ms = g.integrated.as_ref().map_or(g.debug_charts.smoothed_tick_ms, |s| s.smoothed_tick_ms());
        (ms.round() as i64).to_string()
    } else {
        String::new()
    };
    let color = m.name_color;
    let mut values = vec![styled(name, color)];
    if tick.is_empty() {
        m.lines[0].format = Some("format.betterf3.no_format");
    } else {
        values.push(styled(tick, color));
    }
    m.lines[0].set(Val::List(values));
    let status = if g.clock.sprinting() {
        "sprinting"
    } else if g.clock.stepping() {
        "stepping"
    } else if g.clock.frozen() {
        "frozen"
    } else {
        "normal"
    };
    m.lines[3].str(tr(&format!("text.betterf3.line.tick_manager_status.{status}")));
}

/// `CoordsModule`: the position, the block, the chunk and the velocity.
fn coords(m: &mut Module, g: &Game<'_>) {
    let Some(p) = g.player.as_ref() else {
        return;
    };
    let (cx, cy, cz) = (m.extra_color(0), m.extra_color(1), m.extra_color(2));
    let xyz = || b::append(b::append(styled("X", cx), styled("Y", cy)), styled("Z", cz));
    let pos = [p.position.x, p.position.y, p.position.z];
    m.lines[0].set(Val::List(vec![
        xyz(),
        styled(fixed(pos[0], 3), cx),
        styled(fixed(pos[1], 5), cy),
        styled(fixed(pos[2], 3), cz),
    ]));
    let block = pos.map(|v| v.floor() as i32);
    let three = |v: [i32; 3]| Val::List(vec![styled(v[0].to_string(), cx), styled(v[1].to_string(), cy), styled(v[2].to_string(), cz)]);
    m.lines[1].set(three(block));
    m.lines[2].set(three(block.map(|v| v & 15)));
    m.lines[3].set(three(block.map(|v| v >> 4)));
    // The root vehicle's position against the last one, per game tick
    // (prevPos - currentPos, as BetterF3 subtracts).
    let multiplier = f64::from((g.fps.round() as i32).min(20));
    let time = g.world.game_time;
    let mem = &mut m.memory;
    let since = time - mem.pos_time;
    if mem.prev_pos != pos {
        mem.velocity = [mem.prev_pos[0] - pos[0], mem.prev_pos[1] - pos[1], mem.prev_pos[2] - pos[2]];
        mem.pos_time = time;
    } else if since > 1 {
        mem.velocity = [0.0; 3];
    }
    mem.prev_pos = pos;
    let v = mem.velocity;
    m.lines[4].set(Val::List(vec![
        styled(fixed(v[0] * multiplier, 3), cx),
        styled(fixed(v[1] * multiplier, 3), cy),
        styled(fixed(v[2] * multiplier, 3), cz),
    ]));
    let (default_name, _) = Kind::Coords.colors();
    let length = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    let horizontal = (v[0] * v[0] + v[2] * v[2]).sqrt();
    m.lines[5].set(Val::Comp(styled(fixed(length * multiplier, 3), default_name)));
    m.lines[6].set(Val::Comp(styled(fixed(horizontal * multiplier, 3), default_name)));
}

/// `Level.getEntityCount` over the entities Horizon keeps.
fn entity_count(g: &Game<'_>) -> usize {
    g.mobs.iter().filter(|m| !m.removed).count()
        + g.dropped.len()
        + g.orbs.len()
        + g.paintings.len()
        + g.item_frames.len()
        + g.armor_stands.len()
        + g.boats.len()
        + g.carts.len()
        + g.crystals.len()
        + g.creatures.projectiles.len()
}

/// `ChunksModule`.
fn chunks(m: &mut Module, g: &Game<'_>, s: &Shared) {
    let distance = g.settings.render_distance;
    let side = (distance * 2 + 1) as usize;
    let total = side * side * (g.world.layout().height() / 16) as usize;
    let rendered = g.renderer.as_ref().map_or(0, |r| r.drawn_sections);
    if g.player.is_none() {
        return;
    }
    // ClientChunkCache.Storage: a square of (max(2, distance) + 3) chunks
    // about the centre.
    let range = g.remote.as_ref().map_or(g.settings.render_distance, |r| r.view_distance);
    let storage = 2 * (range.max(2) + 3) + 1;
    m.lines[5].str((storage * storage).to_string());
    m.lines[6].str(g.world.chunks.len().to_string());
    let (value, total_c, on) = (m.value_color, m.extra_color(2), m.extra_color(0));
    m.lines[0].set(Val::List(vec![
        styled(tr("text.betterf3.line.rendered"), value),
        styled(tr("text.betterf3.line.total"), total_c),
        styled(rendered.to_string(), value),
        styled(total.to_string(), total_c),
    ]));
    // Smart culling is always on here.
    m.lines[1].set(Val::Comp(styled(tr("text.betterf3.line.enabled"), on)));
    let pending = g.streaming.pending();
    m.lines[2].str(pending.to_string());
    m.lines[3].str(pending.to_string());
    // Horizon keeps no pool of free buffers.
    m.lines[4].str("0");
    if let Some(n) = s.server.as_ref().and_then(|d| d.chunks.as_deref()).and_then(|l| number_after(l, "W:")) {
        m.lines[7].str(n.to_string());
    }
    let forced = if g.has_singleplayer_server() { g.streaming.forced.len() } else { 0 };
    m.lines[8].str(forced.to_string());
    if let Some(n) = s.server.as_ref().and_then(|d| d.spawn_counts.as_deref()).and_then(|l| number_after(l, "SC:")) {
        m.lines[9].str(n.to_string());
    }
    if let Some([x, _, z]) = feet(g) {
        let (cx, cz) = (x >> 4, z >> 4);
        m.lines[10].str(format!("r.{}.{}.mca ({}, {})", cx >> 5, cz >> 5, cx & 31, cz & 31));
    }
}

/// `Heightmap.Types` with `DebugEntryHeightmap.HEIGHTMAP_NAMES`.
fn heightmap_name(kind: HeightmapType) -> &'static str {
    match kind {
        HeightmapType::WorldSurfaceWg => "SW",
        HeightmapType::WorldSurface => "S",
        HeightmapType::OceanFloorWg => "OW",
        HeightmapType::OceanFloor => "O",
        HeightmapType::MotionBlocking => "M",
        HeightmapType::MotionBlockingNoLeaves => "ML",
    }
}

/// `LocationModule`.
fn location(m: &mut Module, g: &Game<'_>, s: &Shared) {
    let Some([x, y, z]) = feet(g) else {
        return;
    };
    let w = &g.world;
    // Biome.
    if let Some(chunk) = w.chunks.get(&(x >> 4, z >> 4))
        && let Some(biome) = g.res.biomes().list.get(chunk.biome(x, y, z) as usize)
    {
        m.lines[7].str(biome.name.clone());
    }
    let light;
    let mut light_server = String::new();
    let mut difficulty = String::new();
    let mut slime = String::new();
    let mut highest = String::new();
    let mut highest_server = String::new();
    if !w.loaded(x, z) {
        light = tr("text.betterf3.line.waiting_chunk");
    } else {
        let packed = w.light(x, y, z);
        let (sky, block) = (packed >> 4, packed & 15);
        light = trf("format.betterf3.chunklight", &[&sky.max(block), &sky, &block]);
        light_server = trf("format.betterf3.chunklight_server", &[&sky, &block]);
        let server = s.server.as_ref();
        let server_heights = server.and_then(|d| d.heights);
        for kind in HeightmapType::ALL {
            if kind.send_to_client() {
                let h = g.column_height(kind, x, z);
                if h > -1 {
                    highest.push_str(&format!("  {}: {h}", heightmap_name(kind)));
                }
            }
            // The server chunk's, in single player (the client chunk's until
            // the server has answered).
            if kind.keep_after_worldgen() && g.has_singleplayer_server() {
                let i = [
                    HeightmapType::WorldSurface,
                    HeightmapType::OceanFloor,
                    HeightmapType::MotionBlocking,
                    HeightmapType::MotionBlockingNoLeaves,
                ]
                .iter()
                .position(|k| *k == kind)
                .unwrap_or(0);
                let h = server_heights.map_or_else(|| g.column_height(kind, x, z), |hs| hs[i]);
                if h > -1 {
                    highest_server.push_str(&format!("  {}: {h}", heightmap_name(kind)));
                }
            }
        }
        // "Local Difficulty: 1.50 // 0.00 (Day 3)" from the server.
        if let Some(line) = server.and_then(|d| d.difficulty.as_deref()) {
            let numbers: Vec<f64> = line
                .split(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-'))
                .filter_map(|t| t.parse().ok())
                .collect();
            if numbers.len() >= 2 {
                difficulty = format!(
                    "{}  {}: {}",
                    fixed(numbers[0], 2),
                    tr("text.betterf3.line.clamped"),
                    fixed(numbers[1], 2)
                );
            }
        }
        slime = if g.has_singleplayer_server() {
            let chunk = horizon_core::random::slime_chunk_random(x >> 4, z >> 4, g.level.seed, 0x3ad8025f)
                .next_int_bound(10)
                .unwrap_or(1)
                == 0;
            tr(if chunk { "text.betterf3.line.slime_chunk.true" } else { "text.betterf3.line.slime_chunk.false" })
        } else {
            tr("text.betterf3.line.slime_chunk.unknown")
        };
    }
    m.lines[0].str(g.dim.id().to_string());
    // Direction.fromYRot.
    let (facing, towards) = match ((g.yaw / 90.0 + 0.5).floor() as i32) & 3 {
        0 => ("south", "positive_z"),
        1 => ("west", "negative_x"),
        2 => ("north", "negative_z"),
        _ => ("east", "positive_x"),
    };
    m.lines[1].str(format!(
        "{} ({})",
        tr(&format!("text.betterf3.line.{facing}")),
        tr(&format!("text.betterf3.line.{towards}"))
    ));
    let yaw = fixed(f64::from(wrap_degrees(g.yaw)), 1);
    let pitch = fixed(f64::from(wrap_degrees(g.pitch)), 1);
    m.lines[2].str(trf("format.betterf3.rotation", &[&yaw, &pitch]));
    m.lines[3].str(light);
    m.lines[4].str(light_server);
    m.lines[5].str(highest.trim().to_owned());
    m.lines[6].str(highest_server.trim().to_owned());
    m.lines[8].str(difficulty);
    m.lines[9].str((w.day_time % 24000).to_string());
    m.lines[10].str((w.day_time / 24000).to_string());
    m.lines[11].str(slime.trim().to_owned());
}

/// `EntityModule`: particles, the entities drawn of the level's, and the
/// last spawn state's count of each category.
fn entity(m: &mut Module, g: &Game<'_>, s: &Shared) {
    if g.player.is_none() {
        return;
    }
    let (value, total) = (m.value_color, m.extra_color(0));
    let values = vec![
        styled(tr("text.betterf3.line.rendered"), value),
        styled(tr("text.betterf3.line.total"), total),
        styled(g.rendered_mobs.get().to_string(), value),
        styled(entity_count(g).to_string(), total),
    ];
    if let Some(line) = s.server.as_ref().and_then(|d| d.spawn_counts.as_deref()) {
        const ABBREVIATIONS: [&str; 8] = ["MO:", "C:", "AM:", "AX:", "UWC:", "WC:", "WA:", "MI:"];
        // After "SC: n, ": each category's count.
        let rest = line.split_once(", ").map_or("", |(_, r)| r);
        let parts: Vec<&str> = rest.split(", ").collect();
        for (i, label) in ABBREVIATIONS.iter().enumerate() {
            let n = parts.iter().find_map(|p| p.strip_prefix(label)).and_then(|n| n.trim().parse::<i64>().ok()).unwrap_or(0);
            m.lines[i + 2].str(n.to_string());
        }
    }
    m.lines[0].str(g.particles.len().to_string());
    m.lines[1].set(Val::List(values));
}

/// `SoundModule`: `getChannelDebugString`'s channels, and the mood.
fn sound(m: &mut Module, g: &Game<'_>) {
    let used = g.audio.engine.used_channels();
    let max = [horizon_client::sound_engine::STATIC_CHANNELS, horizon_client::sound_engine::STREAMING_CHANNELS];
    let (value, maximum) = (m.value_color, m.extra_color(0));
    for i in 0..2 {
        m.lines[i].set(Val::List(vec![
            styled(tr("text.betterf3.line.playing"), value),
            styled(tr("text.betterf3.line.maximum"), maximum),
            styled(used[i].to_string(), value),
            styled(max[i].to_string(), maximum),
        ]));
    }
    if g.player.is_some() {
        m.lines[2].str(format!("{}%", (g.ambience.moodiness * 100.0).round() as i32));
    }
}

/// `HelpModule`: which charts show, and the help key.
fn help(m: &mut Module, g: &Game<'_>) {
    let (on, off) = (m.extra_color(0), m.extra_color(1));
    let state = |shown: bool| {
        Val::Comp(if shown {
            styled(tr("text.betterf3.line.visible"), on)
        } else {
            styled(tr("text.betterf3.line.hidden"), off)
        })
    };
    // Horizon draws no profiler chart.
    m.lines[0].set(state(false));
    m.lines[1].set(state(g.debug_charts.fps));
    m.lines[2].set(state(g.debug && g.debug_charts.network));
    m.lines[3].str(tr("text.betterf3.line.help_press"));
}

/// `EmptyModule`: as many blank lines as it is set to.
fn empty(m: &mut Module) {
    let n = m.empty_lines.min(20).max(0) as usize;
    for (i, l) in m.lines.iter_mut().enumerate() {
        l.str("");
        l.active = i < n;
    }
}

/// The CPU's name, read on a thread of its own (it can start a process).
fn cpu() -> String {
    static CPU: OnceLock<String> = OnceLock::new();
    static STARTED: std::sync::Once = std::sync::Once::new();
    STARTED.call_once(|| {
        let _ = std::thread::Builder::new().name("betterf3-cpu".into()).spawn(|| {
            CPU.get_or_init(horizon_client::crash::cpu);
        });
    });
    CPU.get().cloned().unwrap_or_default()
}

/// `SystemModule`.
fn system(m: &mut Module, g: &Game<'_>, s: &Shared) {
    let time = local_time(&m.time_format).unwrap_or_else(|| {
        m.time_format = b::DEFAULT_TIME_FORMAT.to_owned();
        local_time(b::DEFAULT_TIME_FORMAT).unwrap_or_default()
    });
    m.lines[0].str(time);
    // No JVM here: the line stays empty and hidden.
    m.lines[1].active = false;
    if let Some(mem) = horizon_client::debug_overlay::ProcessMemory::read() {
        let mib = |v: u64| v / 1024 / 1024;
        let percent = mem.used * 100 / mem.max;
        let usage = format!("{}% {:03}/{:03} MB", space2(percent as i64), mib(mem.used), mib(mem.max));
        let usage_color = if percent >= 90 {
            "§c"
        } else if percent >= 60 {
            "§e"
        } else {
            "§a"
        };
        m.lines[2].str(if m.memory_color { format!("{usage_color}{usage}") } else { usage });
        // `allocationRate`: twice a second; a drop (a collection) keeps the last.
        let now = std::time::Instant::now();
        let mem_state = &mut m.memory;
        if mem_state.alloc_at.is_none_or(|at| now.duration_since(at).as_millis() >= 500) {
            if let Some(at) = mem_state.alloc_at
                && mem.used >= mem_state.alloc_bytes
            {
                let seconds = now.duration_since(at).as_secs_f64();
                mem_state.alloc_rate = ((mem.used - mem_state.alloc_bytes) as f64 / seconds).round() as i64;
            }
            mem_state.alloc_at = Some(now);
            mem_state.alloc_bytes = mem.used;
        }
        let rate = mem_state.alloc_rate / 1024 / 1024;
        m.lines[3].str(format!("{} MB/s", space2(rate)));
        m.lines[4].str(format!("{}% {:03}MB", space2((mem.allocated * 100 / mem.max) as i64), mib(mem.allocated)));
    }
    m.lines[5].str(cpu());
    if let Some(r) = g.renderer.as_ref() {
        let info = &r.info;
        let vendor = if info.vendor == 0 && info.backend == wgpu::Backend::Metal {
            "Apple".to_owned()
        } else {
            horizon_client::debug_overlay::vendor_name(info.vendor)
        };
        m.lines[6].str(format!("{} x {} ({vendor})", s.framebuffer[0], s.framebuffer[1]));
        m.lines[7].str(info.name.clone());
        let u = g.gpu_utilization;
        m.lines[8].str(if u > 0.0 {
            if u > 100.0 { "§c100%".to_owned() } else { format!("{}%", u.round() as i64) }
        } else {
            "N/A".to_owned()
        });
        m.lines[9].str(format!("{:?}", info.backend));
        let driver = format!("{} {}", info.driver, info.driver_info);
        m.lines[10].str(driver.trim().lines().next().unwrap_or("").to_owned());
    }
}

/// `String.format("% 2d", n)`: a space for the sign, at least two wide.
fn space2(n: i64) -> String {
    let s = if n < 0 { n.to_string() } else { format!(" {n}") };
    format!("{s:>2}")
}

/// `LocalDateTime.now().format(DateTimeFormatter.ofPattern(pattern))` for
/// the pattern letters a time format uses; None for a pattern Java refuses.
pub(super) fn local_time(pattern: &str) -> Option<String> {
    use chrono::{Datelike, Timelike};
    let now = chrono::Local::now();
    let chars: Vec<char> = pattern.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\'' {
            // Quoted text; '' is a quote.
            let mut j = i + 1;
            if chars.get(j) == Some(&'\'') {
                out.push('\'');
                i += 2;
                continue;
            }
            while j < chars.len() {
                if chars[j] == '\'' {
                    if chars.get(j + 1) == Some(&'\'') {
                        out.push('\'');
                        j += 2;
                        continue;
                    }
                    break;
                }
                out.push(chars[j]);
                j += 1;
            }
            if j >= chars.len() {
                return None;
            }
            i = j + 1;
            continue;
        }
        if !c.is_ascii_alphabetic() {
            if matches!(c, '[' | ']' | '{' | '}' | '#') {
                return None;
            }
            out.push(c);
            i += 1;
            continue;
        }
        let mut n = 1;
        while chars.get(i + n) == Some(&c) {
            n += 1;
        }
        let pad = |v: u32, n: usize| format!("{v:0n$}");
        let piece = match c {
            'H' if n <= 2 => pad(now.hour(), n),
            'h' if n <= 2 => pad(now.hour12().1, n),
            'k' if n <= 2 => pad(if now.hour() == 0 { 24 } else { now.hour() }, n),
            'K' if n <= 2 => pad(now.hour() % 12, n),
            'm' if n <= 2 => pad(now.minute(), n),
            's' if n <= 2 => pad(now.second(), n),
            'S' if n <= 9 => {
                let nanos = format!("{:09}", now.nanosecond() % 1_000_000_000);
                nanos[..n].to_owned()
            }
            'a' if n == 1 => (if now.hour() < 12 { "AM" } else { "PM" }).to_owned(),
            'd' if n <= 2 => pad(now.day(), n),
            'D' if n <= 3 => pad(now.ordinal(), n),
            'M' | 'L' if n <= 2 => pad(now.month(), n),
            'M' | 'L' if n == 3 => now.format("%b").to_string(),
            'M' | 'L' if n == 4 => now.format("%B").to_string(),
            'y' | 'u' if n == 2 => format!("{:02}", now.year().rem_euclid(100)),
            'y' | 'u' if n != 2 && n <= 19 => format!("{:0n$}", now.year()),
            'E' if n <= 3 => now.format("%a").to_string(),
            'E' if n == 4 => now.format("%A").to_string(),
            _ => return None,
        };
        out.push_str(&piece);
        i += n;
    }
    Some(out)
}

/// `Entity.pick(20, 0, false)` from the camera: the block outline hit.
fn block_hit(g: &Game<'_>) -> Option<[i32; 3]> {
    let (origin, dir) = g.aim()?;
    Some(g.clip_ray(origin, dir, 20.0)?.pos)
}

/// `Entity.pick(20, 0, true)`: the first fluid along the view, under its
/// surface, before the block outline hit; else that block.
fn fluid_hit(g: &Game<'_>) -> Option<[i32; 3]> {
    let (origin, dir) = g.aim()?;
    let block = g.clip_ray(origin, dir, 20.0);
    let reach = block.as_ref().map_or(20.0, |h| (0..3).map(|i| (h.point[i] - origin[i]).powi(2)).sum::<f64>().sqrt());
    let mut t = 0.0;
    while t <= reach {
        let p: [f64; 3] = std::array::from_fn(|i| origin[i] + dir[i] * t);
        let bp = p.map(|v| v.floor() as i32);
        if let Some((height, _)) = g.fluid_height_of(bp[0], bp[1], bp[2], None)
            && p[1] - f64::from(bp[1]) <= f64::from(height)
        {
            return Some(bp);
        }
        t += 0.05;
    }
    block.map(|h| h.pos)
}

/// A `FluidState`'s name and properties.
fn fluid_state(g: &Game<'_>, pos: [i32; 3]) -> (String, Vec<(&'static str, String)>) {
    use horizon_client::blocks::Fluid;
    let f = g.res.facts().get(g.world.block(pos[0], pos[1], pos[2]));
    let kind = match f.fluid {
        Fluid::None => return ("minecraft:empty".into(), Vec::new()),
        Fluid::Water => "water",
        Fluid::Lava => "lava",
    };
    let falling = ("falling", f.fluid_falling.to_string());
    if f.fluid_level == 8 && !f.fluid_falling {
        (format!("minecraft:{kind}"), vec![falling])
    } else {
        (format!("minecraft:flowing_{kind}"), vec![falling, ("level", f.fluid_level.to_string())])
    }
}

fn fluid_tags(root: &std::path::Path) -> &'static HashMap<String, HashSet<String>> {
    static TAGS: OnceLock<HashMap<String, HashSet<String>>> = OnceLock::new();
    TAGS.get_or_init(|| {
        horizon_client::items::load_tags(root, "fluid", &horizon_world::loot::PackOverlay::NONE).unwrap_or_default()
    })
}

/// `Minecraft.crosshairPickEntity`'s type.
fn crosshair_entity(g: &Game<'_>) -> Option<String> {
    let mut best: Option<(f64, String)> = None;
    let mut offer = |hit: Option<(f64, String)>| {
        if let Some((distance, kind)) = hit
            && g.before_block(distance)
            && best.as_ref().is_none_or(|b| distance < b.0)
        {
            best = Some((distance, kind));
        }
    };
    offer(g.mob_hit().map(|(i, d, _)| (d, g.mobs[i].kind.id.to_string())));
    offer(g.boat_hit().map(|(i, d)| (d, g.boats[i].entity_type())));
    offer(g.cart_hit().map(|(i, d)| (d, g.carts[i].kind.id().to_owned())));
    offer(g.armor_stand_hit().map(|(_, d, _)| (d, "minecraft:armor_stand".to_owned())));
    offer(g.frame_hit().map(|(i, d)| (d, g.item_frames[i].kind().to_owned())));
    offer(g.painting_hit().map(|(_, d)| (d, "minecraft:painting".to_owned())));
    offer(g.dragon_hit().map(|(_, d)| (d, "minecraft:ender_dragon".to_owned())));
    best.map(|b| b.1)
}

/// `TargetModule`: the block, the fluid and the entity under the crosshair.
fn target(m: &mut Module, g: &Game<'_>) {
    if g.player.is_none() {
        return;
    }
    let pos_text = |p: [i32; 3]| format!("{}, {}, {}", p[0], p[1], p[2]);
    let tags = |set: &HashMap<String, HashSet<String>>, id: &str| -> Vec<String> {
        horizon_client::debug_overlay::tag_lines(set, id)
    };
    match block_hit(g).and_then(|p| Some((p, g.pack.assets.registry.state(g.world.block(p[0], p[1], p[2]))?))) {
        Some((pos, state)) => {
            m.lines[0].str(pos_text(pos));
            m.lines[1].str(state.name.clone());
            m.lines[2].set(Val::Strings(state.properties.iter().map(|(k, v)| property(k, v)).collect()));
            m.lines[3].set(Val::Strings(tags(&g.items().block_tags, &state.name)));
        }
        None => {
            for l in &mut m.lines[..5] {
                l.active = false;
            }
        }
    }
    match fluid_hit(g) {
        Some(pos) => {
            let (name, props) = fluid_state(g, pos);
            m.lines[5].str(pos_text(pos));
            m.lines[6].str(name.clone());
            m.lines[7].set(Val::Strings(props.iter().map(|(k, v)| property(k, v)).collect()));
            m.lines[8].set(Val::Strings(tags(fluid_tags(&g.options.resources), &name)));
        }
        None => {
            for l in &mut m.lines[5..10] {
                l.active = false;
            }
        }
    }
    match crosshair_entity(g) {
        Some(kind) => m.lines[10].str(kind),
        None => m.lines[10].active = false,
    }
}

/// A line's text as `DebugLine.toText` / `toTextCustom` /
/// `DebugLineList.toTexts` build it.
pub(super) fn format_lines(m: &Module, reduced_debug: bool, out: &mut Vec<Text>) {
    for l in &m.lines {
        if reduced_debug && !l.reduced {
            continue;
        }
        if !l.active || !l.enabled {
            continue;
        }
        let format = l.format.unwrap_or(l.def.format);
        match &l.value {
            Val::Unset => {}
            Val::Strings(values) => {
                for v in values {
                    out.push(formatted_from_string(v, m.name_color, m.value_color));
                }
            }
            v if !l.def.custom => {
                let name = line_name(l.def.id);
                let value = match v {
                    Val::Comp(t) => t.clone(),
                    Val::Str(s) => styled(s.clone(), m.value_color),
                    _ => Text::default(),
                };
                if name.is_empty() {
                    // An unnamed line: "%s%s".
                    out.push(b::append(Text::default(), value));
                } else {
                    out.push(b::translatable(format, &[styled(name, m.name_color), value]));
                }
            }
            Val::List(values) => {
                let name = line_name(l.def.id);
                let mut args = Vec::new();
                if !name.is_empty() {
                    args.push(styled(name, m.name_color));
                }
                args.extend(values.iter().cloned());
                out.push(b::with_color(b::translatable(format, &args), m.name_color));
            }
            Val::Str(s) => out.push(b::translatable(format, &[b::plain(line_name(l.def.id)), b::plain(s.clone())])),
            Val::Comp(t) => out.push(b::translatable(format, &[b::plain(line_name(l.def.id)), t.clone()])),
        }
    }
}

/// `DebugLine.name`: `text.betterf3.line.<id>`.
fn line_name(id: &str) -> String {
    if id.is_empty() {
        return String::new();
    }
    tr(&format!("text.betterf3.line.{id}"))
}

/// `Utils.formattedFromString`: before the first `:` in the name colour,
/// the rest in the value colour.
fn formatted_from_string(s: &str, name: u32, value: u32) -> Text {
    match s.split_once(':') {
        Some((n, v)) => b::append(b::append(styled(n, name), b::plain(":")), styled(v, value)),
        None => b::plain(s),
    }
}
