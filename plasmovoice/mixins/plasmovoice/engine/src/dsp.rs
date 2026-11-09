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

#[cfg(test)]
mod tests {
    use super::*;

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
