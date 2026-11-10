//! The microphone's processing as Plasmo Voice does it, on 16-bit mono frames: the gain
//! (`GainFilter`), the compressor and limiter (`CompressorFilter`, `LimiterFilter`) and the
//! levels voice activation compares (`AudioUtil`). Java's casts are kept (a float to `short`
//! goes through `int` and wraps).
//
// This Source Code Form is a port of Plasmo Voice (https://github.com/plasmoapp/plasmo-voice), LGPL-3.0.

/// `(short) f` in Java: truncate toward zero into an int (saturating), then keep the low 16 bits.
pub fn java_short(f: f32) -> i16 {
    (f as i32) as i16
}

/// `AudioUtil.dbToMul`.
pub fn db_to_mul(db: f32) -> f32 {
    if db.is_finite() { 10f32.powf(db / 20.0) } else { 0.0 }
}

/// `AudioUtil.mulToDB`.
pub fn mul_to_db(mul: f32) -> f32 {
    if mul == 0.0 { -f32::MAX } else { 20.0 * mul.log10() }
}

/// `AudioUtil.gainCoefficient`.
pub fn gain_coefficient(sample_rate: i32, time: f32) -> f32 {
    (-1.0 / (sample_rate as f32 * time)).exp()
}

/// `AudioUtil.calculateAudioLevel(samples, offset, length)`: dBFS of the RMS of
/// `samples[offset..end]`, divided by the whole frame's length as the original does.
pub fn audio_level(samples: &[i16], offset: usize, end: usize) -> f64 {
    let mut rms = 0.0f64;
    for &s in &samples[offset..end] {
        let v = f64::from(s) / f64::from(i16::MAX);
        rms += v * v;
    }
    level_from_rms(rms, samples.len())
}

/// `AudioUtil.calculateAudioLevelFromRMS`.
pub fn level_from_rms(rms: f64, count: usize) -> f64 {
    let rms = if count == 0 { 0.0 } else { (rms / count as f64).sqrt() };
    if rms > 0.0 { (20.0 * rms.log10()).clamp(-127.0, 0.0) } else { -127.0 }
}

/// `AudioUtil.containsMinAudioLevel`: any 50-sample window at least `min` dB.
pub fn contains_min_level(samples: &[i16], min: f64) -> bool {
    (0..samples.len()).step_by(50).any(|i| audio_level(samples, i, (i + 50).min(samples.len())) >= min)
}

/// `AudioUtil.calculateHighestAudioLevel`.
pub fn highest_level(samples: &[i16]) -> f64 {
    (0..samples.len())
        .step_by(50)
        .map(|i| audio_level(samples, i, (i + 50).min(samples.len())))
        .fold(-127.0, f64::max)
}

/// `AudioUtil.audioLevelToDoubleRange`: -60..0 dB to 0..1, for the activation slider.
pub fn level_to_range(level: f64) -> f64 {
    1.0 - (level.max(-60.0) / -60.0)
}

/// `AudioUtil.getHighestAbsoluteSample`.
pub fn highest_absolute(samples: &[i16]) -> i16 {
    samples.iter().map(|&s| if s == i16::MIN { i16::MAX } else { s.abs() }).max().unwrap_or(0)
}

/// `GainFilter`: the microphone volume (0..2), lowered so the loudest sample never clips, and
/// held at the lowest such factor of the last 48 frames.
#[derive(Clone, Debug, Default)]
pub struct Gain {
    highest: Vec<f32>,
}

impl Gain {
    pub fn process(&mut self, samples: &mut [i16], setting: f32) {
        let mut volume = setting;
        let highest = highest_absolute(samples);
        let possible = f32::from(i16::MAX - 1) / f32::from(highest);
        if volume > possible {
            volume = possible;
        }
        if self.highest.len() == 48 {
            self.highest.remove(0);
        }
        self.highest.push(volume);
        let min = self.highest.iter().copied().filter(|v| *v >= 0.0).fold(f32::INFINITY, f32::min);
        if min.is_finite() {
            volume = volume.min(min);
        }
        for s in samples.iter_mut() {
            *s = java_short(f32::from(*s) * volume);
        }
    }
}

/// The envelope follower shared by the compressor and the limiter.
#[derive(Clone, Debug, Default)]
struct Envelope {
    value: f32,
}

impl Envelope {
    fn analyze(&mut self, samples: &[f32], attack: f32, release: f32) -> Vec<f32> {
        let mut buf = vec![0f32; samples.len()];
        let mut env = self.value;
        for (i, &s) in samples.iter().enumerate() {
            let input = s.abs();
            env = if env < input { input + attack * (env - input) } else { input + release * (env - input) };
            buf[i] = buf[i].max(env);
        }
        if let Some(&last) = buf.last() {
            self.value = last;
        }
        buf
    }
}

/// `AudioUtil.shortsToFloatsRange`.
fn to_range(samples: &[i16]) -> Vec<f32> {
    samples.iter().map(|&s| f32::from(s) / 32768.0).collect()
}

fn from_range(samples: &mut [i16], floats: &[f32]) {
    for (s, f) in samples.iter_mut().zip(floats) {
        *s = java_short(f * 32768.0);
    }
}

/// `CompressorFilter`: ratio 10:1 above the threshold, 6 ms attack, 60 ms release.
#[derive(Clone, Debug, Default)]
pub struct Compressor {
    env: Envelope,
}

impl Compressor {
    pub fn process(&mut self, samples: &mut [i16], sample_rate: i32, threshold_db: f32) {
        let mut f = to_range(samples);
        let buf = self.env.analyze(&f, gain_coefficient(sample_rate, 6.0 / 1000.0), gain_coefficient(sample_rate, 60.0 / 1000.0));
        let slope = 1.0 - 1.0 / 10.0;
        for (s, e) in f.iter_mut().zip(&buf) {
            let gain = slope * (threshold_db - mul_to_db(*e));
            *s *= db_to_mul(gain.min(0.0)) * db_to_mul(0.0);
        }
        from_range(samples, &f);
    }
}

/// `LimiterFilter`: a hard ceiling at the threshold, instant attack, 60 ms release.
#[derive(Clone, Debug, Default)]
pub struct Limiter {
    env: Envelope,
}

impl Limiter {
    pub fn process(&mut self, samples: &mut [i16], sample_rate: i32, threshold_db: f32) {
        let mut f = to_range(samples);
        let buf = self.env.analyze(&f, gain_coefficient(sample_rate, 0.001 / 1000.0), gain_coefficient(sample_rate, 60.0 / 1000.0));
        for (s, e) in f.iter_mut().zip(&buf) {
            let gain = threshold_db - mul_to_db(*e);
            *s *= db_to_mul(gain.min(0.0)) * db_to_mul(0.0);
        }
        from_range(samples, &f);
    }
}

/// `AudioUtil.convertToMonoShorts` (`StereoToMonoFilter`): each pair's mean, Java's int division.
pub fn stereo_to_mono(samples: &[i16]) -> Vec<i16> {
    samples.chunks_exact(2).map(|p| ((i32::from(p[0]) + i32::from(p[1])) / 2) as i16).collect()
}

/// `NoiseSuppressionFilter`: RNNoise (`crate::rnnoise`, the nnnoiseless 0.5.2 Plasmo's
/// `rnnoise-jni-rust` wraps) on each channel, a state of its own for the left and the right
/// one. At 48 kHz, as Plasmo Voice: the frame goes through in 480-sample pieces, all of them
/// kept (the wrapper's `first` flag never skips one). At another sample rate RNNoise, made for
/// 48 kHz, gets the frame resampled to 48 kHz and back, 10 ms later (Plasmo Voice hands it the
/// frame as it is: wrong pitches, and at 8 and 16 kHz frames under 480 samples come out empty).
/// The filter's limiter (-6 dB) isn't here: Plasmo Voice runs it on a copy it throws away.
pub struct NoiseSuppression {
    channels: [DenoiseChannel; 2],
}

struct DenoiseChannel {
    state: Box<crate::rnnoise::DenoiseState>,
    /// Off 48 kHz: the resamplers there and back and the samples between them.
    up: crate::audio::Resampler,
    down: crate::audio::Resampler,
    pending: Vec<f32>,
    out: std::collections::VecDeque<f32>,
    rate: i32,
}

impl DenoiseChannel {
    fn new() -> DenoiseChannel {
        DenoiseChannel {
            state: crate::rnnoise::DenoiseState::new(),
            up: crate::audio::Resampler::new(48000, 48000),
            down: crate::audio::Resampler::new(48000, 48000),
            pending: Vec::new(),
            out: std::collections::VecDeque::new(),
            rate: 48000,
        }
    }

    /// `Denoise.process(AudioUtil.shortsToFloats(samples))` then `AudioUtil.floatsToShorts`.
    fn process(&mut self, samples: &[i16], rate: i32) -> Vec<i16> {
        const FRAME: usize = crate::rnnoise::FRAME_SIZE;
        let input: Vec<f32> = samples.iter().map(|&s| f32::from(s)).collect();
        let mut buf = [0f32; FRAME];
        let output = if rate == 48000 {
            let mut output = Vec::with_capacity(input.len());
            for chunk in input.chunks_exact(FRAME) {
                self.state.process_frame(&mut buf, chunk);
                output.extend_from_slice(&buf);
            }
            output
        } else {
            if self.rate != rate {
                self.rate = rate;
                self.up = crate::audio::Resampler::new(rate as u32, 48000);
                self.down = crate::audio::Resampler::new(48000, rate as u32);
                self.pending.clear();
                // RNNoise's 480 samples, at this rate, wait in front of the first frame.
                self.out = std::iter::repeat_n(0.0, (FRAME * rate as usize).div_ceil(48000) + 2).collect();
            }
            self.up.process(&input, &mut self.pending);
            let mut down = Vec::new();
            while self.pending.len() >= FRAME {
                let chunk: Vec<f32> = self.pending.drain(..FRAME).collect();
                self.state.process_frame(&mut buf, &chunk);
                self.down.process(&buf, &mut down);
            }
            self.out.extend(down);
            (0..input.len()).map(|_| self.out.pop_front().unwrap_or(0.0)).collect()
        };
        // `Float.shortValue()` of the value clamped to a short.
        output.iter().map(|&f| f.clamp(-32768.0, 32767.0) as i16).collect()
    }
}

impl Default for NoiseSuppression {
    fn default() -> Self {
        NoiseSuppression { channels: [DenoiseChannel::new(), DenoiseChannel::new()] }
    }
}

impl NoiseSuppression {
    /// One frame of `channels` interleaved channels (1 or 2; others pass through).
    pub fn process(&mut self, samples: &[i16], channels: usize, sample_rate: i32) -> Vec<i16> {
        match channels {
            1 => self.channels[0].process(samples, sample_rate),
            2 => {
                let left: Vec<i16> = samples.iter().step_by(2).copied().collect();
                let right: Vec<i16> = samples.iter().skip(1).step_by(2).copied().collect();
                let left = self.channels[0].process(&left, sample_rate);
                let right = self.channels[1].process(&right, sample_rate);
                left.iter().zip(&right).flat_map(|(&l, &r)| [l, r]).collect()
            }
            _ => samples.to_vec(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pink noise (Paul Kellet's filter on xorshift white noise) at `db` dBFS RMS: a room's
    /// hiss and hum, the noise RNNoise was trained to take out.
    fn pink(n: usize, db: f32) -> Vec<f32> {
        let mut seed = 0x1234_5678u32;
        let mut b = [0f32; 7];
        let raw: Vec<f32> = (0..n)
            .map(|_| {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                let w = (seed as f32 / u32::MAX as f32) * 2.0 - 1.0;
                b[0] = 0.99886 * b[0] + w * 0.0555179;
                b[1] = 0.99332 * b[1] + w * 0.0750759;
                b[2] = 0.96900 * b[2] + w * 0.1538520;
                b[3] = 0.86650 * b[3] + w * 0.3104856;
                b[4] = 0.55000 * b[4] + w * 0.5329522;
                b[5] = -0.7616 * b[5] - w * 0.0168980;
                let p = b[0] + b[1] + b[2] + b[3] + b[4] + b[5] + b[6] + w * 0.5362;
                b[6] = w * 0.115926;
                p
            })
            .collect();
        let rms = (raw.iter().map(|v| v * v).sum::<f32>() / n as f32).sqrt();
        let k = db_to_mul(db) * 32768.0 / rms;
        raw.iter().map(|v| v * k).collect()
    }

    /// A voiced sound: 150 Hz and its harmonics up to 3 kHz, falling 6 dB an octave, about -21 dBFS.
    fn voice(i: usize, rate: i32) -> f32 {
        let t = i as f32 / rate as f32;
        (1..=20).map(|h| (t * 150.0 * h as f32 * std::f32::consts::TAU).sin() / h as f32).sum::<f32>() * 0.1 * 32768.0
    }

    fn rms_db(s: &[i16]) -> f64 {
        let e: f64 = s.iter().map(|&v| f64::from(v) * f64::from(v)).sum::<f64>() / s.len() as f64;
        10.0 * (e / (32768.0 * 32768.0)).log10()
    }

    fn denoise(input: &[i16], rate: i32) -> Vec<i16> {
        let frame = (rate / 1000 * 20) as usize;
        let mut ns = NoiseSuppression::default();
        input
            .chunks_exact(frame)
            .flat_map(|f| {
                let o = ns.process(f, 1, rate);
                assert_eq!(o.len(), frame);
                o
            })
            .collect()
    }

    #[test]
    fn noise_suppression_removes_room_noise() {
        // Pink noise at -40 dBFS, three seconds; the last one is measured.
        for rate in [48000, 24000, 16000] {
            let n = rate as usize * 3;
            let noise: Vec<i16> = pink(n, -40.0).iter().map(|&f| f as i16).collect();
            let out = denoise(&noise, rate);
            let last = rate as usize;
            let (before, after) = (rms_db(&noise[n - last..]), rms_db(&out[n - last..]));
            eprintln!("{rate} Hz: pink noise {before:.1} dBFS -> {after:.1} dBFS");
            assert!(after < before - 30.0, "{rate} Hz: pink noise {before:.1} dBFS -> {after:.1} dBFS");
        }
    }

    #[test]
    fn noise_suppression_keeps_the_voice_and_takes_the_noise_between_words() {
        // A voiced sound 400 ms on, 400 ms off, in pink noise at -40 dBFS, four seconds.
        let rate = 48000;
        let n = rate as usize * 4;
        let span = rate as usize * 2 / 5;
        let noise = pink(n, -40.0);
        let talking = |i: usize| (i / span) % 2 == 0;
        let input: Vec<i16> = (0..n).map(|i| ((if talking(i) { voice(i, rate) } else { 0.0 }) + noise[i]) as i16).collect();
        let out = denoise(&input, rate);
        // From the second second on: the words (past their first 50 ms) and the pauses (past
        // their first 100 ms, RNNoise's release).
        let pick = |s: &[i16], words: bool, skip: usize| -> Vec<i16> {
            (rate as usize..n).filter(|&i| talking(i) == words && i % span >= skip).map(|i| s[i]).collect()
        };
        let (words_in, words_out) = (rms_db(&pick(&input, true, 2400)), rms_db(&pick(&out, true, 2400)));
        assert!((words_in - words_out).abs() < 3.0, "words {words_in:.1} dBFS -> {words_out:.1} dBFS");
        let (pause_in, pause_out) = (rms_db(&pick(&input, false, 4800)), rms_db(&pick(&out, false, 4800)));
        eprintln!("words {words_in:.1} -> {words_out:.1} dBFS, pauses {pause_in:.1} -> {pause_out:.1} dBFS");
        assert!(pause_out < pause_in - 20.0, "pauses {pause_in:.1} dBFS -> {pause_out:.1} dBFS");
        // Stereo: each channel through a state of its own, the frame's length kept.
        let mut ns = NoiseSuppression::default();
        let stereo: Vec<i16> = input[..960].iter().flat_map(|&v| [v, v / 2]).collect();
        assert_eq!(ns.process(&stereo, 2, rate).len(), 2 * 960);
    }

    #[test]
    fn stereo_to_mono_is_javas() {
        assert_eq!(stereo_to_mono(&[3, 4, -3, -4, 32767, 32767]), vec![3, -3, 32767]);
    }

    #[test]
    fn java_casts() {
        assert_eq!(java_short(1.9), 1);
        assert_eq!(java_short(-1.9), -1);
        assert_eq!(java_short(32768.0), -32768);
        assert_eq!(java_short(40000.0), -25536);
    }

    #[test]
    fn levels() {
        assert_eq!(audio_level(&[0; 960], 0, 960), -127.0);
        let loud = vec![i16::MAX; 960];
        assert!((audio_level(&loud, 0, 960)).abs() < 1e-9);
        // A 50-sample window of a 960-sample frame is divided by 960 (the original's quirk).
        let w = audio_level(&loud, 0, 50);
        assert!((w - 20.0 * (50.0f64 / 960.0).sqrt().log10()).abs() < 1e-9);
        assert!(contains_min_level(&loud, -30.0));
        assert!(!contains_min_level(&[1; 960], -30.0));
    }

    #[test]
    fn limiter_keeps_peaks_under_the_threshold() {
        let mut l = Limiter::default();
        let mut frame: Vec<i16> = (0..960).map(|i| if i % 2 == 0 { 30000 } else { -30000 }).collect();
        for _ in 0..5 {
            l.process(&mut frame, 48000, -6.0);
        }
        let ceiling = (db_to_mul(-6.0) * 32768.0) as i16 + 50;
        assert!(highest_absolute(&frame) <= ceiling, "{}", highest_absolute(&frame));
    }

    #[test]
    fn gain_never_clips() {
        let mut g = Gain::default();
        let mut frame = vec![20000i16; 960];
        g.process(&mut frame, 2.0);
        assert!(frame.iter().all(|&s| s > 0 && s < i16::MAX));
    }
}
