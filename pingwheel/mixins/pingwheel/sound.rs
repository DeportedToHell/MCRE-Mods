// Ping Wheel's sound event `pingwheel:ping` (its `sounds.json`) and its `sounds/ping.ogg`, which
// ship in the bundle (`native_data!`) instead of a resource pack: the event goes into the sound
// registry and the decoded file into the sound cache under the key the mixer looks it up by
// (`pingwheel:sounds/ping.ogg`), so `Audio::play*` plays it as any sound of the resources, on
// the master source as Java's `SoundSource.MASTER`. Spliced into horizon-client-render's
// `audio.rs` as `pingwheel_sound`, beside the private cache.
//
// Ping Wheel by Luken (https://github.com/LukenSkyne/Minecraft-Ping-Wheel), MIT.

use super::Audio;
use std::sync::{Arc, OnceLock};

/// The event.
pub const EVENT: &str = "pingwheel:ping";
/// The original's `assets/pingwheel/sounds.json`.
const SOUNDS_JSON: &[u8] = br#"{"ping": {"subtitle": "subtitles.pingwheel.ping", "sounds": ["pingwheel:ping"]}}"#;
/// What the mixer caches the file as (`Sound.getPath` with its namespace).
const KEY: &str = "pingwheel:sounds/ping.ogg";

static DECODED: OnceLock<Option<(Arc<Vec<f32>>, u32, u16)>> = OnceLock::new();

/// `ping.ogg` as the mixer's samples: interleaved floats, rate, channels (mono or stereo).
fn decode(ogg: &[u8]) -> Option<(Arc<Vec<f32>>, u32, u16)> {
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

/// Makes `pingwheel:ping` playable: the event registered (again after a resource reload, which
/// replaces the registry) and the samples in the cache (again when the cache let them go).
/// False when the file can't be decoded.
pub fn prepare(audio: &mut Audio, ogg: &'static [u8]) -> bool {
    let samples = DECODED.get_or_init(|| decode(ogg)).clone();
    if samples.is_none() {
        return false;
    }
    if !audio.engine.registry.contains(EVENT) {
        audio.engine.registry.add_file("pingwheel", SOUNDS_JSON, &|_, _| true);
    }
    if let Ok(mut cache) = audio.backend.cache.lock()
        && cache.get(KEY).is_none()
    {
        cache.insert(KEY.to_owned(), samples);
    }
    true
}
