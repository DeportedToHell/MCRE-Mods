//! Sound in and out, on devices of the mod's own as Plasmo Voice opens its own OpenAL ones:
//! the microphone in 20 ms mono frames at the server's sample rate (`VoiceAudioCapture`), the
//! speakers mixing every source's stereo samples, and the Opus codec (`OpusCodecSupplier`).
//
// This Source Code Form is a port of Plasmo Voice (https://github.com/plasmoapp/plasmo-voice), LGPL-3.0.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SizedSample};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

// ---- Opus -----------------------------------------------------------------------------------

/// `OpusMode`: the encoder's application, from the server's `mode` parameter.
pub fn application(mode: &str) -> rusty_opus::Application {
    match mode {
        "AUDIO" => rusty_opus::Application::Audio,
        "RESTRICTED_LOWDELAY" => rusty_opus::Application::RestrictedLowDelay,
        _ => rusty_opus::Application::Voip,
    }
}

/// libopus's `user_bitrate_to_bitrate`: `OPUS_AUTO` (-1000) and `OPUS_BITRATE_MAX` (-1), which the
/// server may send, as bits per second.
pub fn bitrate(requested: i32, sample_rate: i32, channels: i32, frame_size: i32) -> i32 {
    match requested {
        -1000 => 60 * sample_rate / frame_size.max(1) + sample_rate * channels,
        -1 => 1_500_000,
        b => b.clamp(500, 512_000),
    }
}

/// An Opus encoder for 20 ms frames (`NativeOpusEncoder`).
pub struct Encoder {
    inner: rusty_opus::OpusEncoder,
    channels: usize,
    frame_size: usize,
    mtu: usize,
}

impl Encoder {
    pub fn new(sample_rate: i32, stereo: bool, mode: &str, bitrate_param: i32, mtu: usize) -> Option<Encoder> {
        let channels = if stereo { 2 } else { 1 };
        let mut inner = rusty_opus::OpusEncoder::new(sample_rate, channels, application(mode)).ok()?;
        let frame_size = (sample_rate / 1000 * 20) as usize;
        inner.bitrate_bps = bitrate(bitrate_param, sample_rate, channels as i32, frame_size as i32);
        Some(Encoder { inner, channels, frame_size, mtu: mtu.max(64) })
    }

    /// One frame (`frame_size * channels` samples) to one packet.
    pub fn encode(&mut self, samples: &[i16]) -> Option<Vec<u8>> {
        let input: Vec<f32> = samples.iter().map(|&s| f32::from(s) / 32768.0).collect();
        let mut out = vec![0u8; self.mtu];
        let n = self.inner.encode(&input, self.frame_size, &mut out).ok()?;
        out.truncate(n);
        Some(out)
    }

    pub fn frame_size(&self) -> usize {
        self.frame_size
    }
    pub fn channels(&self) -> usize {
        self.channels
    }
}

/// An Opus decoder (`NativeOpusDecoder`): a lost packet is concealed (`decodePLC`).
pub struct Decoder {
    inner: rusty_opus::OpusDecoder,
    channels: usize,
    frame_size: usize,
}

impl Decoder {
    pub fn new(sample_rate: i32, stereo: bool) -> Option<Decoder> {
        let channels = if stereo { 2 } else { 1 };
        let inner = rusty_opus::OpusDecoder::new(sample_rate, channels).ok()?;
        Some(Decoder { inner, channels, frame_size: (sample_rate / 1000 * 20) as usize })
    }

    /// A packet to interleaved samples; `None` for a lost one.
    pub fn decode(&mut self, packet: Option<&[u8]>) -> Vec<f32> {
        let mut out = vec![0f32; self.frame_size * self.channels];
        let n = self.inner.decode(packet.unwrap_or(&[]), self.frame_size, &mut out).unwrap_or(0);
        out.truncate(n * self.channels);
        out
    }

    pub fn channels(&self) -> usize {
        self.channels
    }
}

// ---- resampling -----------------------------------------------------------------------------

/// A linear resampler for one channel, kept across blocks.
#[derive(Clone, Debug)]
pub struct Resampler {
    ratio: f64,
    pos: f64,
    last: f32,
}

impl Resampler {
    pub fn new(from: u32, to: u32) -> Resampler {
        Resampler { ratio: f64::from(from) / f64::from(to), pos: 0.0, last: 0.0 }
    }
    pub fn process(&mut self, input: &[f32], out: &mut Vec<f32>) {
        if (self.ratio - 1.0).abs() < 1e-9 {
            out.extend_from_slice(input);
            return;
        }
        // The sequence is the previous block's last sample, then `input`: position p lies
        // between sequence[floor(p)] and sequence[floor(p) + 1].
        let at = |k: usize| if k == 0 { self.last } else { input[k - 1] };
        while self.pos < input.len() as f64 {
            let i = self.pos.floor() as usize;
            let t = (self.pos - self.pos.floor()) as f32;
            let (a, b) = (at(i), at(i + 1));
            out.push(a + (b - a) * t);
            self.pos += self.ratio;
        }
        self.pos -= input.len() as f64;
        if let Some(&l) = input.last() {
            self.last = l;
        }
    }
}

// ---- the microphone -------------------------------------------------------------------------

/// The input devices' names (the settings' list).
pub fn input_devices() -> Vec<String> {
    cpal::default_host().input_devices().map(|d| d.filter_map(|d| d.name().ok()).collect()).unwrap_or_default()
}

/// The output devices' names.
pub fn output_devices() -> Vec<String> {
    cpal::default_host().output_devices().map(|d| d.filter_map(|d| d.name().ok()).collect()).unwrap_or_default()
}

/// The system's default input device (`DeviceFactory.getDefaultDeviceName`).
pub fn default_input_device() -> Option<String> {
    cpal::default_host().default_input_device().and_then(|d| d.name().ok())
}

/// The system's default output device.
pub fn default_output_device() -> Option<String> {
    cpal::default_host().default_output_device().and_then(|d| d.name().ok())
}

/// The microphone, open on a thread of its own (a cpal stream lives on the thread that made it):
/// 16-bit frames of `frame_size` samples at `sample_rate` arrive on `frames`, mono (every
/// channel mixed, as OpenAL captures in mono) or, with `stereo` (`stereo_capture`), the first
/// two channels interleaved (a mono microphone on both).
pub struct Capture {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    pub frames: Receiver<Vec<i16>>,
    pub device: String,
    pub channels: usize,
}

impl Capture {
    /// The named device, or the default one (`None` or not found).
    pub fn open(device: Option<&str>, sample_rate: u32, frame_size: usize, stereo: bool) -> Result<Capture, String> {
        let (tx, rx) = channel();
        let (ready_tx, ready_rx) = channel::<Result<String, String>>();
        let stop = Arc::new(AtomicBool::new(false));
        let stop2 = stop.clone();
        let wanted = device.map(str::to_owned);
        let thread = std::thread::Builder::new()
            .name("plasmovoice-capture".into())
            .spawn(move || {
                let host = cpal::default_host();
                let dev = wanted
                    .as_deref()
                    .and_then(|w| host.input_devices().ok()?.find(|d| d.name().ok().as_deref() == Some(w)))
                    .or_else(|| host.default_input_device());
                let Some(dev) = dev else {
                    let _ = ready_tx.send(Err("no microphone".into()));
                    return;
                };
                let name = dev.name().unwrap_or_default();
                let stream = match open_input(&dev, sample_rate, frame_size, stereo, tx) {
                    Ok(s) => s,
                    Err(e) => {
                        let _ = ready_tx.send(Err(e));
                        return;
                    }
                };
                if let Err(e) = stream.play() {
                    let _ = ready_tx.send(Err(e.to_string()));
                    return;
                }
                let _ = ready_tx.send(Ok(name));
                while !stop2.load(Ordering::Relaxed) {
                    std::thread::park_timeout(std::time::Duration::from_millis(100));
                }
                drop(stream);
            })
            .map_err(|e| e.to_string())?;
        match ready_rx.recv() {
            Ok(Ok(device)) => Ok(Capture { stop, thread: Some(thread), frames: rx, device, channels: if stereo { 2 } else { 1 } }),
            Ok(Err(e)) => Err(e),
            Err(_) => Err("capture thread ended".into()),
        }
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            t.thread().unpark();
            let _ = t.join();
        }
    }
}

fn open_input(dev: &cpal::Device, rate: u32, frame_size: usize, stereo: bool, tx: Sender<Vec<i16>>) -> Result<cpal::Stream, String> {
    let config = dev.default_input_config().map_err(|e| e.to_string())?;
    let format = config.sample_format();
    let cfg: cpal::StreamConfig = config.into();
    match format {
        cpal::SampleFormat::F32 => input_stream::<f32>(dev, &cfg, rate, frame_size, stereo, tx),
        cpal::SampleFormat::I16 => input_stream::<i16>(dev, &cfg, rate, frame_size, stereo, tx),
        cpal::SampleFormat::U16 => input_stream::<u16>(dev, &cfg, rate, frame_size, stereo, tx),
        cpal::SampleFormat::I32 => input_stream::<i32>(dev, &cfg, rate, frame_size, stereo, tx),
        f => Err(format!("unsupported microphone format {f:?}")),
    }
}

fn input_stream<T>(dev: &cpal::Device, cfg: &cpal::StreamConfig, rate: u32, frame_size: usize, stereo: bool, tx: Sender<Vec<i16>>) -> Result<cpal::Stream, String>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let channels = usize::from(cfg.channels.max(1));
    let out_channels = if stereo { 2 } else { 1 };
    let mut resamplers: Vec<Resampler> = (0..out_channels).map(|_| Resampler::new(cfg.sample_rate.0, rate)).collect();
    let mut split: Vec<Vec<f32>> = vec![Vec::new(); out_channels];
    let mut pending: Vec<Vec<f32>> = vec![Vec::new(); out_channels];
    dev.build_input_stream(
        cfg,
        move |data: &[T], _| {
            for c in &mut split {
                c.clear();
            }
            for chunk in data.chunks(channels) {
                if stereo {
                    let l = chunk[0].to_sample::<f32>();
                    split[0].push(l);
                    split[1].push(chunk.get(1).map_or(l, |r| r.to_sample::<f32>()));
                } else {
                    let sum: f32 = chunk.iter().map(|&s| s.to_sample::<f32>()).sum();
                    split[0].push(sum / channels as f32);
                }
            }
            for ((r, s), p) in resamplers.iter_mut().zip(&split).zip(&mut pending) {
                r.process(s, p);
            }
            while pending.iter().all(|p| p.len() >= frame_size) {
                let parts: Vec<Vec<f32>> = pending.iter_mut().map(|p| p.drain(..frame_size).collect()).collect();
                let frame: Vec<i16> = (0..frame_size)
                    .flat_map(|i| parts.iter().map(move |p| p[i]))
                    .map(|s| (s * 32767.0).clamp(-32768.0, 32767.0) as i16)
                    .collect();
                if tx.send(frame).is_err() {
                    return;
                }
            }
        },
        |e| eprintln!("plasmovoice: microphone error: {e}"),
        None,
    )
    .map_err(|e| e.to_string())
}

// ---- the speakers ---------------------------------------------------------------------------

/// What the output mixes: interleaved stereo at the voice sample rate, one queue per source.
#[derive(Default)]
pub struct Mix {
    pub sources: Vec<(u128, VecDeque<f32>)>,
}

impl Mix {
    /// Adds stereo samples for a source.
    pub fn push(&mut self, source: u128, stereo: &[f32]) {
        match self.sources.iter_mut().find(|(id, _)| *id == source) {
            Some((_, q)) => q.extend(stereo),
            None => self.sources.push((source, stereo.iter().copied().collect())),
        }
    }
    /// Removes a source's queue.
    pub fn remove(&mut self, source: u128) {
        self.sources.retain(|(id, _)| *id != source);
    }
    fn next_frame(&mut self) -> (f32, f32) {
        let (mut l, mut r) = (0.0, 0.0);
        for (_, q) in &mut self.sources {
            if q.len() >= 2 {
                l += q.pop_front().unwrap_or(0.0);
                r += q.pop_front().unwrap_or(0.0);
            }
        }
        // A source whose audio has all played is forgotten.
        self.sources.retain(|(_, q)| !q.is_empty());
        (l.clamp(-1.0, 1.0), r.clamp(-1.0, 1.0))
    }
}

/// The speakers, on a thread of their own, playing `mix`.
pub struct Output {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    pub mix: Arc<Mutex<Mix>>,
    pub device: String,
}

impl Output {
    pub fn open(device: Option<&str>, sample_rate: u32) -> Result<Output, String> {
        let mix = Arc::new(Mutex::new(Mix::default()));
        let mix2 = mix.clone();
        let (ready_tx, ready_rx) = channel::<Result<String, String>>();
        let stop = Arc::new(AtomicBool::new(false));
        let stop2 = stop.clone();
        let wanted = device.map(str::to_owned);
        let thread = std::thread::Builder::new()
            .name("plasmovoice-output".into())
            .spawn(move || {
                let host = cpal::default_host();
                let dev = wanted
                    .as_deref()
                    .and_then(|w| host.output_devices().ok()?.find(|d| d.name().ok().as_deref() == Some(w)))
                    .or_else(|| host.default_output_device());
                let Some(dev) = dev else {
                    let _ = ready_tx.send(Err("no speakers".into()));
                    return;
                };
                let name = dev.name().unwrap_or_default();
                let stream = match open_output(&dev, sample_rate, mix2) {
                    Ok(s) => s,
                    Err(e) => {
                        let _ = ready_tx.send(Err(e));
                        return;
                    }
                };
                if let Err(e) = stream.play() {
                    let _ = ready_tx.send(Err(e.to_string()));
                    return;
                }
                let _ = ready_tx.send(Ok(name));
                while !stop2.load(Ordering::Relaxed) {
                    std::thread::park_timeout(std::time::Duration::from_millis(100));
                }
                drop(stream);
            })
            .map_err(|e| e.to_string())?;
        match ready_rx.recv() {
            Ok(Ok(device)) => Ok(Output { stop, thread: Some(thread), mix, device }),
            Ok(Err(e)) => Err(e),
            Err(_) => Err("output thread ended".into()),
        }
    }
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

fn open_output(dev: &cpal::Device, rate: u32, mix: Arc<Mutex<Mix>>) -> Result<cpal::Stream, String> {
    let config = dev.default_output_config().map_err(|e| e.to_string())?;
    let format = config.sample_format();
    let cfg: cpal::StreamConfig = config.into();
    match format {
        cpal::SampleFormat::F32 => output_stream::<f32>(dev, &cfg, rate, mix),
        cpal::SampleFormat::I16 => output_stream::<i16>(dev, &cfg, rate, mix),
        cpal::SampleFormat::U16 => output_stream::<u16>(dev, &cfg, rate, mix),
        cpal::SampleFormat::I32 => output_stream::<i32>(dev, &cfg, rate, mix),
        f => Err(format!("unsupported speaker format {f:?}")),
    }
}

fn output_stream<T>(dev: &cpal::Device, cfg: &cpal::StreamConfig, rate: u32, mix: Arc<Mutex<Mix>>) -> Result<cpal::Stream, String>
where
    T: SizedSample + FromSample<f32>,
{
    let channels = usize::from(cfg.channels.max(1));
    let device_rate = f64::from(cfg.sample_rate.0);
    // Source frames per device frame.
    let step = f64::from(rate) / device_rate;
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
                    next = m.next_frame();
                    pos -= 1.0;
                }
                let t = pos as f32;
                let l = cur.0 + (next.0 - cur.0) * t;
                let r = cur.1 + (next.1 - cur.1) * t;
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
        |e| eprintln!("plasmovoice: speaker error: {e}"),
        None,
    )
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opus_round_trip() {
        let mut enc = Encoder::new(48000, false, "VOIP", -1000, 1024).unwrap();
        let mut dec = Decoder::new(48000, false).unwrap();
        let tone: Vec<i16> = (0..960).map(|i| ((i as f32 * 440.0 * std::f32::consts::TAU / 48000.0).sin() * 10000.0) as i16).collect();
        let mut out = Vec::new();
        for _ in 0..10 {
            let packet = enc.encode(&tone).unwrap();
            assert!(!packet.is_empty() && packet.len() <= 1024);
            out = dec.decode(Some(&packet));
        }
        assert_eq!(out.len(), 960);
        let energy: f32 = out.iter().map(|s| s * s).sum::<f32>() / out.len() as f32;
        assert!(energy > 0.01, "{energy}");
        // A lost packet is concealed with a full frame.
        assert_eq!(dec.decode(None).len(), 960);
    }

    #[test]
    fn auto_bitrate_matches_libopus() {
        assert_eq!(bitrate(-1000, 48000, 1, 960), 51000);
        assert_eq!(bitrate(24000, 48000, 1, 960), 24000);
    }

    #[test]
    fn resampler_keeps_the_length() {
        let mut r = Resampler::new(44100, 48000);
        let mut out = Vec::new();
        for _ in 0..100 {
            r.process(&[0.5; 441], &mut out);
        }
        assert!((out.len() as i64 - 48000).abs() <= 2, "{}", out.len());
        // Past the ramp up from silence at the very start.
        assert!(out.iter().skip(2).all(|&s| (s - 0.5).abs() < 1e-6));
    }
}
