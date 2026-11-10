//! Playing emotes' songs (`EmoteSoundInstance`, `PcmAudioStream`): each one at its avatar,
//! linear attenuation over 16 blocks, in the Players volume, joined where the animation is,
//! looping at its loop point, normalized to -14 LUFS when the option asks, and the other
//! players' songs stepping down to their volume while the player's own plays. Mixed on a
//! speaker stream of the mod's own (the game's sound engine plays only its sound events), on
//! the device the game uses.

use crate::opus::{Decoded, NO_LOOP, OpusSound, SAMPLE_RATE};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SizedSample};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

/// One song playing (the stream of an `EmoteSoundInstance`).
struct Voice {
    pcm: Arc<Decoded>,
    loop_start: i32,
    cursor: usize,
    /// Left and right gains now and what they move to.
    gain: [f32; 2],
    target: [f32; 2],
}

impl Voice {
    fn exhausted(&self) -> bool {
        self.loop_start < 0 && self.cursor >= self.pcm.samples.len()
    }
    fn next(&mut self) -> f32 {
        if self.cursor >= self.pcm.samples.len() {
            if self.loop_start < 0 {
                return 0.0;
            }
            self.cursor = self.loop_start as usize;
        }
        let v = f32::from(self.pcm.samples[self.cursor]) / 32768.0;
        self.cursor += 1;
        v
    }
}

#[derive(Default)]
struct Mix {
    voices: BTreeMap<u128, Voice>,
}

impl Mix {
    fn frame(&mut self) -> (f32, f32) {
        let (mut l, mut r) = (0.0, 0.0);
        for v in self.voices.values_mut() {
            // About 20 ms to reach a new gain, so a moving player doesn't click.
            for c in 0..2 {
                v.gain[c] += (v.target[c] - v.gain[c]) * 0.001;
            }
            let s = v.next();
            l += s * v.gain[0];
            r += s * v.gain[1];
        }
        (l.clamp(-1.0, 1.0), r.clamp(-1.0, 1.0))
    }
}

/// The speakers, on a thread of their own.
struct Output {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    mix: Arc<Mutex<Mix>>,
    device: String,
}

impl Drop for Output {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            t.thread().unpark();
            let _ = t.join();
        }
    }
}

fn open(device: &str) -> Result<Output, String> {
    let mix = Arc::new(Mutex::new(Mix::default()));
    let mix2 = mix.clone();
    let stop = Arc::new(AtomicBool::new(false));
    let stop2 = stop.clone();
    let wanted = device.to_owned();
    let (tx, rx) = std::sync::mpsc::channel::<Result<(), String>>();
    let thread = std::thread::Builder::new()
        .name("Emotecraft songs".into())
        .spawn(move || {
            let host = cpal::default_host();
            let dev = (!wanted.is_empty())
                .then(|| host.output_devices().ok()?.find(|d| d.name().ok().as_deref() == Some(&wanted)))
                .flatten()
                .or_else(|| host.default_output_device());
            let Some(dev) = dev else {
                let _ = tx.send(Err("no speakers".into()));
                return;
            };
            let stream = match stream(&dev, mix2) {
                Ok(s) => s,
                Err(e) => {
                    let _ = tx.send(Err(e));
                    return;
                }
            };
            if let Err(e) = stream.play() {
                let _ = tx.send(Err(e.to_string()));
                return;
            }
            let _ = tx.send(Ok(()));
            while !stop2.load(Ordering::Relaxed) {
                std::thread::park_timeout(std::time::Duration::from_millis(100));
            }
            drop(stream);
        })
        .map_err(|e| e.to_string())?;
    match rx.recv() {
        Ok(Ok(())) => Ok(Output { stop, thread: Some(thread), mix, device: device.to_owned() }),
        Ok(Err(e)) => Err(e),
        Err(_) => Err("speaker thread ended".into()),
    }
}

fn stream(dev: &cpal::Device, mix: Arc<Mutex<Mix>>) -> Result<cpal::Stream, String> {
    let config = dev.default_output_config().map_err(|e| e.to_string())?;
    let format = config.sample_format();
    let cfg: cpal::StreamConfig = config.into();
    match format {
        cpal::SampleFormat::F32 => typed::<f32>(dev, &cfg, mix),
        cpal::SampleFormat::I16 => typed::<i16>(dev, &cfg, mix),
        cpal::SampleFormat::U16 => typed::<u16>(dev, &cfg, mix),
        cpal::SampleFormat::I32 => typed::<i32>(dev, &cfg, mix),
        f => Err(format!("unsupported speaker format {f:?}")),
    }
}

fn typed<T>(dev: &cpal::Device, cfg: &cpal::StreamConfig, mix: Arc<Mutex<Mix>>) -> Result<cpal::Stream, String>
where
    T: SizedSample + FromSample<f32>,
{
    let channels = usize::from(cfg.channels.max(1));
    let step = f64::from(SAMPLE_RATE) / f64::from(cfg.sample_rate.0);
    let mut pos = 0.0f64;
    let mut cur = (0.0f32, 0.0f32);
    let mut next = (0.0f32, 0.0f32);
    dev.build_output_stream(
        cfg,
        move |data: &mut [T], _| {
            let Ok(mut m) = mix.lock() else { return };
            for frame in data.chunks_mut(channels) {
                while pos >= 1.0 {
                    cur = next;
                    next = m.frame();
                    pos -= 1.0;
                }
                let t = pos as f32;
                let (l, r) = (cur.0 + (next.0 - cur.0) * t, cur.1 + (next.1 - cur.1) * t);
                pos += step;
                for (c, s) in frame.iter_mut().enumerate() {
                    let v = match (channels, c) {
                        (1, _) => (l + r) * 0.5,
                        (_, 0) => l,
                        (_, 1) => r,
                        _ => 0.0,
                    };
                    *s = T::from_sample(v);
                }
            }
        },
        |e| log::warn!("[Emotecraft] speaker error: {e}"),
        None,
    )
    .map_err(|e| e.to_string())
}

/// What the game says about a song this frame.
pub struct Wanted {
    /// The avatar (or the preview) it belongs to.
    pub id: u128,
    pub sound: Arc<OpusSound>,
    /// Where the animation is, in samples (`getAnimationTime() * 48000`).
    pub offset: i32,
    pub position: [f64; 3],
    /// The player's own (a mannequin's too): what the others step down for.
    pub own: bool,
}

/// What the listener hears with.
pub struct Listener {
    pub position: [f64; 3],
    pub yaw: f32,
    /// Master × Players.
    pub volume: f32,
    pub normalize: bool,
    pub other_volume: f32,
    pub device: String,
}

struct Playing {
    sound: Arc<OpusSound>,
    clock: i32,
    finished: bool,
    ducking: f32,
}

/// Every song, kept in step with the emotes once a frame.
#[derive(Default)]
pub struct Songs {
    output: Option<Output>,
    failed_device: Option<String>,
    playing: BTreeMap<u128, Playing>,
    last_tick: Option<std::time::Instant>,
}

impl Songs {
    /// The songs this frame: new ones start where their animation is, a looping animation
    /// that comes round again restarts a finished song, gone ones stop.
    pub fn update(&mut self, listener: &Listener, wanted: Vec<Wanted>) {
        let ticks = {
            let now = std::time::Instant::now();
            let t = self.last_tick.map_or(0.0, |l| now.duration_since(l).as_secs_f32() * 20.0);
            self.last_tick = Some(now);
            t.min(5.0)
        };
        if wanted.is_empty() && self.playing.is_empty() {
            if self.output.is_some() {
                self.output = None;
            }
            return;
        }
        if self.output.as_ref().is_some_and(|o| o.device != listener.device) {
            self.output = None;
        }
        if self.output.is_none() && !wanted.is_empty() && self.failed_device.as_deref() != Some(listener.device.as_str()) {
            match open(&listener.device) {
                Ok(o) => self.output = Some(o),
                Err(e) => {
                    log::warn!("[Emotecraft] Can't open the speakers for emote songs: {e}");
                    self.failed_device = Some(listener.device.clone());
                }
            }
        }
        let Some(out) = self.output.as_ref() else { return };
        let Ok(mut mix) = out.mix.lock() else { return };
        let ids: Vec<u128> = wanted.iter().map(|w| w.id).collect();
        mix.voices.retain(|id, _| ids.contains(id));
        self.playing.retain(|id, _| ids.contains(id));
        let own_playing = wanted.iter().any(|w| w.own && mix.voices.get(&w.id).is_some_and(|v| !v.exhausted()));
        let yaw = f64::from(listener.yaw).to_radians();
        for w in wanted {
            // `startSound`: the clock runs backwards when the emote loops, and a song that
            // ran out starts over with it.
            let restart = match self.playing.get(&w.id) {
                Some(p) => !Arc::ptr_eq(&p.sound, &w.sound) || (w.offset < p.clock && p.finished),
                None => true,
            };
            if restart {
                mix.voices.remove(&w.id);
                self.playing.insert(w.id, Playing { sound: w.sound.clone(), clock: w.offset, finished: false, ducking: 1.0 });
            }
            let Some(p) = self.playing.get_mut(&w.id) else { continue };
            p.clock = w.offset;
            if !mix.voices.contains_key(&w.id) && !p.finished {
                if w.sound.loop_start == NO_LOOP && w.offset >= w.sound.playable() {
                    // A song shorter than its emote has nothing left until the animation loops.
                    p.finished = true;
                    continue;
                }
                let Some(pcm) = w.sound.decoded() else { continue };
                let len = pcm.samples.len();
                let ls = if w.sound.loop_start >= 0 && (w.sound.loop_start as usize) < len { w.sound.loop_start } else { NO_LOOP };
                let off = w.offset.max(0) as usize;
                let cursor = if off < len {
                    off
                } else if ls < 0 {
                    len
                } else {
                    ls as usize + (off - ls as usize) % (len - ls as usize)
                };
                mix.voices.insert(w.id, Voice { pcm, loop_start: ls, cursor, gain: [0.0; 2], target: [0.0; 2] });
            }
            let Some(v) = mix.voices.get_mut(&w.id) else { continue };
            if v.exhausted() {
                p.finished = true;
            }
            // Your own emote has the floor; the others walk down to their level.
            let level = if own_playing && !w.own { listener.other_volume } else { 1.0 };
            let step = 0.05 * ticks;
            p.ducking = if p.ducking < level { (p.ducking + step).min(level) } else { (p.ducking - step).max(level) };
            let normalization = if listener.normalize { v.pcm.normalization } else { 1.0 };
            let d = [w.position[0] - listener.position[0], w.position[1] - listener.position[1], w.position[2] - listener.position[2]];
            let distance = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
            // `Attenuation.LINEAR` over the sound's 16 blocks.
            let attenuation = (1.0 - distance / 16.0).clamp(0.0, 1.0) as f32;
            // Left and right by where it is around the listener's facing.
            let pan = if distance < 1e-3 {
                0.0
            } else {
                let right = [-yaw.cos(), 0.0, -yaw.sin()];
                ((d[0] * right[0] + d[2] * right[2]) / distance) as f32
            };
            let g = listener.volume * normalization * p.ducking * attenuation;
            v.target = [g * (1.0 - pan).min(1.0), g * (1.0 + pan).min(1.0)];
        }
    }

    pub fn stop_all(&mut self) {
        self.playing.clear();
        self.output = None;
    }
}
