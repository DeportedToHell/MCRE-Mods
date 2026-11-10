//! Emotes' songs: Ogg Opus files (`OggOpusReader`), the packets Emotecraft keeps and sends
//! (`OpusSound`, `OpusPackets`), their decoding to 48 kHz mono PCM with the Opus output gain,
//! and the EBU R128 loudness the "Normalize the volume" option evens out (`Loudness`).
//!
//! Decoded with libopus through rusty-opus (BSD-3-Clause), as Plasmo Voice's port does.

use std::sync::{Arc, Mutex};

pub const SAMPLE_RATE: i32 = 48000;
pub const MAX_PACKET_DURATION_MS: i32 = 120;
pub const NO_LOOP: i32 = -1;
const MAX_DURATION_MS: i64 = 10 * 60 * 1000;
const MAX_SAMPLES: i64 = MAX_DURATION_MS * (SAMPLE_RATE as i64 / 1000);
const MAX_BITRATE: i64 = 96000;
const TARGET_LUFS: f64 = -14.0;
const R128_REFERENCE_LUFS: f64 = -23.0;
const Q7_8: f64 = 256.0;
pub const MAX_PACKET_SIZE: usize = 1_048_576;

/// `OpusPackets.samplesPerFrame`.
pub fn samples_per_frame(toc: u8, rate: i32) -> i32 {
    if toc & 0x80 != 0 {
        return (rate << ((toc >> 3) & 0x3)) / 400;
    }
    if toc & 0x60 == 0x60 {
        return if toc & 0x08 != 0 { rate / 50 } else { rate / 100 };
    }
    let size = (toc >> 3) & 0x3;
    if size == 3 { rate * 60 / 1000 } else { (rate << size) / 100 }
}

fn frame_count(p: &[u8]) -> i32 {
    if p.is_empty() {
        return -1;
    }
    match p[0] & 0x3 {
        0 => 1,
        3 => {
            if p.len() < 2 {
                -1
            } else {
                i32::from(p[1] & 0x3F)
            }
        }
        _ => 2,
    }
}

/// `OpusPackets.sampleCount`: -1 for a malformed packet or one past 120 ms.
pub fn sample_count(p: &[u8]) -> i32 {
    let frames = frame_count(p);
    if frames < 1 {
        return -1;
    }
    let samples = frames * samples_per_frame(p[0], SAMPLE_RATE);
    if i64::from(samples) * 1000 > i64::from(SAMPLE_RATE) * i64::from(MAX_PACKET_DURATION_MS) {
        return -1;
    }
    samples
}

/// What a decoded song is: its samples and the volume it is normalized to.
pub struct Decoded {
    pub samples: Vec<i16>,
    pub normalization: f32,
}

/// `OpusSound`: a mono Opus stream as packets, its OpusHead values and loop point.
pub struct OpusSound {
    pub pre_skip: i32,
    pub end_trim: i32,
    pub output_gain: i32,
    pub track_gain: Option<i32>,
    pub loop_start: i32,
    pub data: Vec<u8>,
    /// One past the last packet too.
    pub offsets: Vec<usize>,
    pub sample_count: i32,
    pub comments: Vec<String>,
    decoded: Mutex<DecodeState>,
}

impl std::fmt::Debug for OpusSound {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "OpusSound({} packets, {} ms)", self.packet_count(), self.duration_ms())
    }
}

#[derive(Default)]
enum DecodeState {
    #[default]
    Idle,
    Running,
    Done(Arc<Decoded>),
    Failed,
}

impl OpusSound {
    /// The `OpusSound` constructor's checks and trims.
    pub fn new(
        pre_skip: i32,
        end_trim: i32,
        output_gain: i32,
        track_gain: Option<i32>,
        loop_start: Option<i32>,
        mut data: Vec<u8>,
        mut offsets: Vec<usize>,
        comments: Vec<String>,
    ) -> Result<OpusSound, String> {
        if !(0..=0xffff).contains(&pre_skip) {
            return Err("Opus pre-skip does not fit in 16 bits".into());
        }
        if !(i32::from(i16::MIN)..=i32::from(i16::MAX)).contains(&output_gain) {
            return Err("Opus output gain does not fit in 16 bits".into());
        }
        if offsets.len() < 2 || offsets[offsets.len() - 1] > data.len() {
            return Err("Opus stream has no packets".into());
        }
        let mut samples_total: i64 = 0;
        let mut size = (crate::buf::varint_size(pre_skip | (output_gain << 16)) + crate::buf::varint_size(end_trim) + crate::buf::varint_size(offsets.len() as i32 - 1) + 4) as i64;
        for i in 0..offsets.len() - 1 {
            let p = &data[offsets[i]..offsets[i + 1]];
            let n = sample_count(p);
            if n <= 0 {
                return Err("Malformed Opus packet".into());
            }
            if p[0] & 0x04 != 0 {
                return Err("Opus stream must be mono".into());
            }
            samples_total += i64::from(n);
            if samples_total > MAX_SAMPLES {
                return Err(format!("Opus stream is longer than {MAX_DURATION_MS} ms"));
            }
            size += (crate::buf::varint_size(p.len() as i32) + p.len()) as i64;
            if size > MAX_PACKET_SIZE as i64 {
                return Err(format!("Opus stream is bigger than {MAX_PACKET_SIZE} bytes"));
            }
        }
        if samples_total <= i64::from(pre_skip) {
            return Err("Opus stream has no audio".into());
        }
        let bitrate = size * 8 * i64::from(SAMPLE_RATE) / samples_total;
        let limit = MAX_BITRATE.min(MAX_PACKET_SIZE as i64 * 8 * i64::from(SAMPLE_RATE) / samples_total);
        if bitrate > limit {
            return Err(format!("Opus stream is {bitrate} bps, over the {limit} bps limit"));
        }
        let mut end_trim = end_trim;
        if end_trim < 0 || i64::from(end_trim) >= samples_total - i64::from(pre_skip) {
            end_trim = 0;
        }
        let mut packets = offsets.len() - 1;
        while packets > 1 {
            let n = sample_count(&data[offsets[packets - 1]..offsets[packets]]);
            if n > end_trim {
                break;
            }
            end_trim -= n;
            samples_total -= i64::from(n);
            packets -= 1;
        }
        if packets < offsets.len() - 1 {
            offsets.truncate(packets + 1);
            data.truncate(offsets[packets]);
        }
        let mut s = OpusSound {
            pre_skip,
            end_trim,
            output_gain,
            track_gain,
            loop_start: NO_LOOP,
            data,
            offsets,
            sample_count: samples_total as i32,
            comments,
            decoded: Mutex::new(DecodeState::Idle),
        };
        s.loop_start = match loop_start {
            Some(l) if l >= 0 && l < s.playable() => l,
            _ => NO_LOOP,
        };
        Ok(s)
    }

    pub fn packet_count(&self) -> usize {
        self.offsets.len() - 1
    }
    pub fn packet(&self, i: usize) -> &[u8] {
        &self.data[self.offsets[i]..self.offsets[i + 1]]
    }
    /// Samples that play, after the pre-skip and end trim.
    pub fn playable(&self) -> i32 {
        self.sample_count - self.pre_skip - self.end_trim
    }
    pub fn duration_ms(&self) -> i32 {
        self.playable() / (SAMPLE_RATE / 1000)
    }

    /// `OpusSound.read`: an Ogg Opus file.
    pub fn read(bytes: &[u8]) -> Result<OpusSound, String> {
        if bytes.len() > MAX_PACKET_SIZE {
            return Err("Opus file is too big to send".into());
        }
        let r = ogg::read(bytes)?;
        if r.channels != 1 {
            return Err("Opus stream must be mono".into());
        }
        let mut data = Vec::new();
        let mut offsets = Vec::new();
        for p in &r.packets {
            offsets.push(data.len());
            data.extend_from_slice(p);
            if data.len() > MAX_PACKET_SIZE {
                return Err(format!("Opus stream is bigger than {MAX_PACKET_SIZE} bytes"));
            }
        }
        offsets.push(data.len());
        OpusSound::new(r.pre_skip, r.end_trim, r.output_gain, r.track_gain, r.loop_start, data, offsets, r.comments)
    }

    /// The decoded song, once a decode finished (`decoded` starts one on a thread of its own).
    pub fn decoded(self: &Arc<Self>) -> Option<Arc<Decoded>> {
        let mut g = self.decoded.lock().unwrap_or_else(|e| e.into_inner());
        match &*g {
            DecodeState::Done(d) => return Some(d.clone()),
            DecodeState::Running | DecodeState::Failed => return None,
            DecodeState::Idle => {}
        }
        *g = DecodeState::Running;
        drop(g);
        let me = self.clone();
        let spawned = std::thread::Builder::new().name("Emotecraft Opus decoder".into()).spawn(move || {
            let result = me.decode();
            let mut g = me.decoded.lock().unwrap_or_else(|e| e.into_inner());
            *g = match result {
                Ok(d) => DecodeState::Done(Arc::new(d)),
                Err(e) => {
                    log::error!("[Emotecraft] Failed to decode an emote sound: {e}");
                    DecodeState::Failed
                }
            };
        });
        if spawned.is_err() {
            *self.decoded.lock().unwrap_or_else(|e| e.into_inner()) = DecodeState::Idle;
        }
        None
    }

    pub fn failed(&self) -> bool {
        matches!(*self.decoded.lock().unwrap_or_else(|e| e.into_inner()), DecodeState::Failed)
    }

    /// `OpusSound.decode`.
    pub fn decode(&self) -> Result<Decoded, String> {
        let playable = self.playable().max(0) as usize;
        let mut samples = vec![0i16; playable];
        let mut loudness = self.track_gain.is_none().then(|| Loudness::new(playable));
        let gain = 10f64.powf(f64::from(self.output_gain) / Q7_8 / 20.0) as f32;
        let mut decoder = rusty_opus::OpusDecoder::new(SAMPLE_RATE, 1).map_err(|e| format!("{e:?}"))?;
        let frame = (SAMPLE_RATE / 1000 * MAX_PACKET_DURATION_MS) as usize;
        let mut scratch = vec![0f32; frame];
        let mut skip = self.pre_skip as usize;
        let mut offset = 0usize;
        for i in 0..self.packet_count() {
            let n = decoder.decode(self.packet(i), frame, &mut scratch).map_err(|e| format!("{e:?}"))?;
            let dropped = n.min(skip);
            let count = (n - dropped).min(playable - offset);
            for k in 0..count {
                let v = (scratch[dropped + k] * 32768.0).round().clamp(-32768.0, 32767.0) as i16;
                samples[offset + k] = if gain == 1.0 { v } else { (f32::from(v) * gain).round().clamp(-32768.0, 32767.0) as i16 };
            }
            skip -= dropped;
            if let Some(l) = loudness.as_mut() {
                l.feed(&samples[offset..offset + count]);
            }
            offset += count;
        }
        let lufs = match (self.track_gain, &loudness) {
            (Some(t), _) => R128_REFERENCE_LUFS - f64::from(t) / Q7_8,
            (None, Some(l)) => l.integrated(),
            _ => f64::NAN,
        };
        let normalization = if lufs.is_finite() { 10f64.powf((TARGET_LUFS - lufs) / 20.0).min(1.0) as f32 } else { 1.0 };
        Ok(Decoded { samples, normalization })
    }
}

/// `Loudness`: ITU-R BS.1770 integrated loudness (400 ms blocks every 100 ms, gated).
struct Loudness {
    step: usize,
    filter: KWeighting,
    steps: Vec<f64>,
    sum: f64,
    filled: usize,
}

impl Loudness {
    fn new(expected: usize) -> Loudness {
        let step = SAMPLE_RATE as usize * 100 / 1000;
        Loudness { step, filter: KWeighting::default(), steps: Vec::with_capacity(expected / step + 1), sum: 0.0, filled: 0 }
    }
    fn feed(&mut self, samples: &[i16]) {
        for &s in samples {
            let w = self.filter.apply(f64::from(s) / 32768.0);
            self.sum += w * w;
            self.filled += 1;
            if self.filled == self.step {
                self.steps.push(self.sum);
                self.sum = 0.0;
                self.filled = 0;
            }
        }
    }
    fn integrated(&self) -> f64 {
        if self.steps.len() < 4 {
            return f64::NEG_INFINITY;
        }
        let block = (self.step * 4) as f64;
        let blocks: Vec<f64> = (0..=self.steps.len() - 4).map(|i| self.steps[i..i + 4].iter().sum::<f64>() / block).collect();
        let loud = gated_mean(&blocks, -70.0);
        if loud.is_nan() {
            return f64::NEG_INFINITY;
        }
        let mean = gated_mean(&blocks, loudness(loud) - 10.0);
        if mean.is_nan() { f64::NEG_INFINITY } else { loudness(mean) }
    }
}

fn loudness(e: f64) -> f64 {
    if e <= 0.0 { f64::NEG_INFINITY } else { -0.691 + 10.0 * e.log10() }
}

fn gated_mean(blocks: &[f64], gate: f64) -> f64 {
    let kept: Vec<f64> = blocks.iter().copied().filter(|&e| loudness(e) > gate).collect();
    if kept.is_empty() { f64::NAN } else { kept.iter().sum::<f64>() / kept.len() as f64 }
}

#[derive(Default)]
struct KWeighting {
    sx1: f64,
    sx2: f64,
    sy1: f64,
    sy2: f64,
    px1: f64,
    px2: f64,
    py1: f64,
    py2: f64,
}

impl KWeighting {
    fn apply(&mut self, x: f64) -> f64 {
        let shelf = 1.53512485958697 * x + -2.69169618940638 * self.sx1 + 1.19839281085285 * self.sx2 - -1.69065929318241 * self.sy1 - 0.73248077421585 * self.sy2;
        self.sx2 = self.sx1;
        self.sx1 = x;
        self.sy2 = self.sy1;
        self.sy1 = shelf;
        let pass = shelf - 2.0 * self.px1 + self.px2 - -1.99004745483398 * self.py1 - 0.99007225036621 * self.py2;
        self.px2 = self.px1;
        self.px1 = shelf;
        self.py2 = self.py1;
        self.py1 = pass;
        pass
    }
}

/// The Ogg Opus container (`OggOpusReader`).
mod ogg {
    pub struct Read {
        pub channels: u8,
        pub pre_skip: i32,
        pub output_gain: i32,
        pub track_gain: Option<i32>,
        pub loop_start: Option<i32>,
        pub comments: Vec<String>,
        pub packets: Vec<Vec<u8>>,
        pub end_trim: i32,
    }

    fn le32(b: &[u8]) -> u32 {
        u32::from_le_bytes([b[0], b[1], b[2], b[3]])
    }

    pub fn read(bytes: &[u8]) -> Result<Read, String> {
        // Every packet, with the granule of the page it ends on.
        let mut packets: Vec<(Vec<u8>, i64, usize)> = Vec::new();
        let mut at = 0usize;
        let mut partial: Option<Vec<u8>> = None;
        let mut serial: Option<u32> = None;
        let mut page_index = 0usize;
        let mut ended = false;
        while at < bytes.len() && !ended {
            if bytes.len() < at + 27 || &bytes[at..at + 4] != b"OggS" {
                return Err("Not an Ogg page".into());
            }
            if bytes[at + 4] != 0 {
                return Err("Unsupported Ogg version".into());
            }
            let kind = bytes[at + 5];
            let granule = i64::from_le_bytes(bytes[at + 6..at + 14].try_into().map_err(|_| "bad page")?);
            let this_serial = le32(&bytes[at + 14..]);
            let segments = bytes[at + 26] as usize;
            let table_at = at + 27;
            if bytes.len() < table_at + segments {
                return Err("Ogg page ends early".into());
            }
            let table = &bytes[table_at..table_at + segments];
            let mut body = table_at + segments;
            let total: usize = table.iter().map(|&b| b as usize).sum();
            if bytes.len() < body + total {
                return Err("Ogg page ends early".into());
            }
            match serial {
                None => {
                    if kind & 0x02 == 0 {
                        return Err("Ogg stream does not begin with a header page".into());
                    }
                    serial = Some(this_serial);
                }
                Some(s) if s != this_serial => {
                    at = body + total;
                    continue;
                }
                _ => {}
            }
            if (partial.is_some()) != (kind & 0x01 != 0) {
                return Err("Lost packet continuation".into());
            }
            let mut i = 0;
            while i < segments {
                let mut len = 0usize;
                let mut done = false;
                while i < segments {
                    let l = table[i] as usize;
                    i += 1;
                    len += l;
                    if l != 255 {
                        done = true;
                        break;
                    }
                }
                let chunk = &bytes[body..body + len];
                body += len;
                let mut p = partial.take().unwrap_or_default();
                p.extend_from_slice(chunk);
                if done {
                    packets.push((p, granule, page_index));
                } else {
                    partial = Some(p);
                }
            }
            ended = kind & 0x04 != 0;
            at = body;
            page_index += 1;
        }
        if partial.is_some() {
            return Err("Truncated Opus packet".into());
        }
        if packets.len() < 2 {
            return Err("Opus headers end early".into());
        }
        let head = &packets[0].0;
        if head.len() < 19 || &head[..8] != b"OpusHead" {
            return Err("Opus stream has no OpusHead header".into());
        }
        if head[8] >> 4 != 0 {
            return Err("Unsupported OpusHead version".into());
        }
        let channels = head[9];
        let pre_skip = i32::from(u16::from_le_bytes([head[10], head[11]]));
        let output_gain = i32::from(i16::from_le_bytes([head[16], head[17]]));
        if head[18] != 0 {
            return Err("Unsupported channel mapping family".into());
        }
        let tags = &packets[1].0;
        if tags.len() < 8 || &tags[..8] != b"OpusTags" {
            return Err("Opus stream has no OpusTags header".into());
        }
        let mut comments = Vec::new();
        let (mut track_gain, mut loop_start) = (None, None);
        let mut p = 8usize;
        let take_len = |p: &mut usize| -> Option<usize> {
            let l = le32(tags.get(*p..*p + 4)?) as usize;
            *p += 4;
            Some(l)
        };
        if let Some(vendor) = take_len(&mut p) {
            p += vendor;
            if let Some(count) = take_len(&mut p) {
                for _ in 0..count {
                    let Some(l) = take_len(&mut p) else { break };
                    let Some(c) = tags.get(p..p + l) else { break };
                    p += l;
                    if l > 128 {
                        continue;
                    }
                    let c = String::from_utf8_lossy(c).into_owned();
                    let upper = c.to_ascii_uppercase();
                    let number = |v: &str, min: i64, max: i64| v.trim().parse::<i64>().ok().filter(|n| (min..=max).contains(n)).map(|n| n as i32);
                    if track_gain.is_none() && upper.starts_with("R128_TRACK_GAIN=") {
                        track_gain = number(&c[16..], i64::from(i16::MIN), i64::from(i16::MAX));
                    } else if loop_start.is_none() && upper.starts_with("LOOPSTART=") {
                        loop_start = number(&c[10..], 0, i64::from(i32::MAX));
                    }
                    comments.push(c);
                }
            }
        }
        // The end trim, as the reader counts the pages: the samples of the last page with a
        // granule position against what that granule says it keeps.
        let audio = &packets[2..];
        let (mut samples, mut prev_samples, mut granule, mut prev_granule) = (0i64, 0i64, 0i64, 0i64);
        let mut counted_page: Option<usize> = None;
        for (pkt, page_granule, page) in audio {
            let n = super::sample_count(pkt);
            if n > 0 {
                if counted_page != Some(*page) && *page_granule >= 0 {
                    counted_page = Some(*page);
                    prev_granule = granule;
                    prev_samples = samples;
                    granule = *page_granule;
                }
                samples += i64::from(n);
            }
        }
        let last = samples - prev_samples;
        let kept = granule - prev_granule;
        let end_trim = (last - kept).clamp(0, last.max(0)) as i32;
        Ok(Read {
            channels,
            pre_skip,
            output_gain,
            track_gain,
            loop_start,
            comments,
            packets: audio.iter().map(|p| p.0.clone()).collect(),
            end_trim,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_songs_read_and_decode() {
        for (name, bytes) in crate::tests::BUILT_IN_SONGS {
            let s = Arc::new(OpusSound::read(bytes).unwrap_or_else(|e| panic!("{name}: {e}")));
            assert!(s.duration_ms() > 1000, "{name}: {} ms", s.duration_ms());
            let d = s.decode().unwrap();
            assert_eq!(d.samples.len(), s.playable() as usize);
            let peak = d.samples.iter().map(|v| v.unsigned_abs()).max().unwrap();
            assert!(peak > 1000, "{name} is silent");
            assert!(d.normalization > 0.0 && d.normalization <= 1.0);
        }
    }
}
