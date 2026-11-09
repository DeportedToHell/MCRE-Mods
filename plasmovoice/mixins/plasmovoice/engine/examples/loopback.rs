//! Microphone -> filters -> Opus -> speakers, for a few seconds: `cargo run --example loopback [seconds]`.
//! Prints each second's frame count, encoded size and level, so the chain can be checked without
//! the game.
use plasmovoice_engine::{audio, dsp};

fn main() {
    let secs: u64 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(5);
    println!("inputs: {:?}", audio::input_devices());
    println!("outputs: {:?}", audio::output_devices());
    let cap = audio::Capture::open(None, 48000, 960).expect("microphone");
    let out = audio::Output::open(None, 48000).expect("speakers");
    println!("microphone: {} / speakers: {}", cap.device, out.device);
    let mut enc = audio::Encoder::new(48000, false, "VOIP", -1000, 1024).unwrap();
    let mut dec = audio::Decoder::new(48000, false).unwrap();
    let (mut gain, mut comp, mut lim) = (dsp::Gain::default(), dsp::Compressor::default(), dsp::Limiter::default());
    let (mut frames, mut bytes, mut loudest) = (0, 0, -127.0f64);
    let start = std::time::Instant::now();
    let mut next = 1;
    while start.elapsed().as_secs() < secs {
        let Ok(mut frame) = cap.frames.recv_timeout(std::time::Duration::from_millis(500)) else { continue };
        gain.process(&mut frame, 1.0);
        comp.process(&mut frame, 48000, -10.0);
        lim.process(&mut frame, 48000, -6.0);
        loudest = loudest.max(dsp::highest_level(&frame));
        let packet = enc.encode(&frame).unwrap();
        bytes += packet.len();
        frames += 1;
        let mono = dec.decode(Some(&packet));
        let stereo: Vec<f32> = mono.iter().flat_map(|&s| [s, s]).collect();
        out.mix.lock().unwrap().push(1, &stereo);
        if start.elapsed().as_secs() >= next {
            println!("{next}s: {frames} frames, {bytes} bytes, loudest {loudest:.1} dB");
            (frames, bytes, loudest) = (0, 0, -127.0);
            next += 1;
        }
    }
}
