// Particle Rain's weather sounds (`particlerain:weather.snow`, `weather.snow.above`,
// `weather.sandstorm`, `weather.sandstorm.above`: its `sounds.json`) and their 18 files, which
// ship in the bundle (`native_data!`) instead of a resource pack: the events go into the sound
// registry and the decoded files into the sound cache under the keys the mixer looks them up by
// (`particlerain:sounds/weather/<name>.ogg`), and they play on the Weather volume
// (`SoundSource.WEATHER`). Spliced into horizon-client-render's audio.rs as `particlerain_sound`.
//
// Particle Rain by PigCart (https://github.com/PigCart/particle-rain), MIT.

use super::Audio;
use std::sync::{Arc, OnceLock};

/// The original's `assets/particlerain/sounds.json`.
const SOUNDS_JSON: &[u8] = br#"{
  "weather.snow": {"sounds": ["particlerain:weather/snow-01", "particlerain:weather/snow-02", "particlerain:weather/snow-03", "particlerain:weather/snow-04", "particlerain:weather/snow-05", "particlerain:weather/snow-06"]},
  "weather.snow.above": {"sounds": ["particlerain:weather/snow-01", "particlerain:weather/snow-02", "particlerain:weather/snow-03", "particlerain:weather/snow-04", "particlerain:weather/snow-05", "particlerain:weather/snow-06"]},
  "weather.sandstorm": {"sounds": ["particlerain:weather/sandstorm-01", "particlerain:weather/sandstorm-02", "particlerain:weather/sandstorm-03", "particlerain:weather/sandstorm-04", "particlerain:weather/sandstorm-05", "particlerain:weather/sandstorm-06"]},
  "weather.sandstorm.above": {"sounds": ["particlerain:weather/sandstormabove-01", "particlerain:weather/sandstormabove-02", "particlerain:weather/sandstormabove-03", "particlerain:weather/sandstormabove-04", "particlerain:weather/sandstormabove-05", "particlerain:weather/sandstormabove-06"]}
}"#;

type Samples = Option<(Arc<Vec<f32>>, u32, u16)>;

static DECODED: OnceLock<Vec<(String, Samples)>> = OnceLock::new();

/// An ogg file as the mixer's samples: interleaved floats, rate, channels (mono or stereo).
fn decode(ogg: &[u8]) -> Samples {
    use symphonia::core::{
        audio::SampleBuffer, codecs::DecoderOptions, formats::FormatOptions, io::MediaSourceStream, meta::MetadataOptions,
        probe::Hint,
    };
    let stream = MediaSourceStream::new(Box::new(std::io::Cursor::new(ogg.to_vec())), Default::default());
    let mut hint = Hint::new();
    hint.with_extension("ogg");
    let probed = symphonia::default::get_probe().format(&hint, stream, &FormatOptions::default(), &MetadataOptions::default()).ok()?;
    let mut format = probed.format;
    let track = format.default_track()?;
    let id = track.id;
    let rate = track.codec_params.sample_rate?;
    let mut decoder = symphonia::default::get_codecs().make(&track.codec_params, &DecoderOptions::default()).ok()?;
    let mut samples = Vec::new();
    let mut channels = 0u16;
    while let Ok(packet) = format.next_packet() {
        if packet.track_id() != id {
            continue;
        }
        let Ok(buffer) = decoder.decode(&packet) else { continue };
        let spec = *buffer.spec();
        let n = spec.channels.count().max(1);
        if n > 2 {
            return None;
        }
        let mut decoded = SampleBuffer::<f32>::new(buffer.capacity() as u64, spec);
        decoded.copy_interleaved_ref(buffer);
        if channels == 0 {
            channels = n as u16;
        }
        samples.extend_from_slice(decoded.samples());
    }
    (channels != 0).then(|| (Arc::new(samples), rate, channels))
}

/// Makes the events playable: registered (again after a resource reload, which replaces the
/// registry) and every file in the cache (again when the cache let one go). `files` are the
/// bundle's files by name (`snow-01`, ...).
fn prepare(audio: &mut Audio, files: &[(&str, &'static [u8])]) {
    let decoded = DECODED.get_or_init(|| files.iter().map(|(name, ogg)| ((*name).to_owned(), decode(ogg))).collect());
    if !audio.engine.registry.contains("particlerain:weather.snow") {
        audio.engine.registry.add_file("particlerain", SOUNDS_JSON, &|_, _| true);
    }
    if let Ok(mut cache) = audio.backend.cache.lock() {
        for (name, samples) in decoded {
            let key = format!("particlerain:sounds/weather/{name}.ogg");
            if samples.is_some() && cache.get(&key).is_none() {
                cache.insert(key, samples.clone());
            }
        }
    }
}

/// `ClientLevel.playLocalSound(pos, event, SoundSource.WEATHER, volume, pitch, false)` of one of
/// the mod's events (`snow`, `snow.above`, `sandstorm`, `sandstorm.above`).
pub fn play(audio: &mut Audio, files: &[(&str, &'static [u8])], event: &str, at: [f64; 3], volume: f32, pitch: f32) {
    prepare(audio, files);
    let weather = crate::settings::sound_source("minecraft:weather.rain");
    let previous = audio.source_override.replace(weather);
    audio.play_unshared(&format!("particlerain:weather.{event}"), Some(at), volume, pitch);
    audio.source_override = previous;
}
