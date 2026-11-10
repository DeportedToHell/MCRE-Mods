// The denoiser, from nnnoiseless 0.5.2 (src/denoise.rs), always on RNNoise's model.
// BSD-3-Clause: see the notice in mod.rs.

use super::rnn::{RnnModel, RnnState};
use super::{FRAME_SIZE, FREQ_SIZE, NB_BANDS};

/// RNNoise's state for one channel (`DenoiseState`). Large: keep it behind a pointer.
pub struct DenoiseState {
    /// Most recent gains that we applied.
    lastg: [f32; NB_BANDS],
    rnn: RnnState,
    feat: super::features::DenoiseFeatures,
}

impl DenoiseState {
    /// A `DenoiseState` processes this many samples at a time.
    pub const FRAME_SIZE: usize = FRAME_SIZE;

    /// Creates a new `DenoiseState`.
    pub fn new() -> Box<DenoiseState> {
        Box::new(DenoiseState {
            lastg: [0.0; NB_BANDS],
            rnn: RnnState::new(RnnModel::rnnoise()),
            feat: super::features::DenoiseFeatures::new(),
        })
    }

    /// Processes a chunk of samples.
    ///
    /// Both `output` and `input` should be slices of length `DenoiseState::FRAME_SIZE`, and they
    /// are assumed to be in 16-bit, 48kHz signed PCM format. Note that although the input and
    /// output are `f32`s, they are supposed to come from 16-bit integers. In particular, they
    /// should be in the range `[-32768.0, 32767.0]` instead of the range `[-1.0, 1.0]` which
    /// is more common for floating-point PCM.
    ///
    /// The current output of `process_frame` depends on the current input, but also on the
    /// preceding inputs. Because of this, you might prefer to discard the very first output; it
    /// will contain some fade-in artifacts.
    pub fn process_frame(&mut self, output: &mut [f32], input: &[f32]) -> f32 {
        let mut g = [0.0; NB_BANDS];
        let mut gf = [1.0; FREQ_SIZE];
        let mut vad_prob = [0.0];

        self.feat.shift_and_filter_input(input);
        let silence = self.feat.compute_frame_features();
        if !silence {
            self.rnn.compute(&mut g[..], &mut vad_prob[..], self.feat.features());
            self.feat.pitch_filter(&g);
            for i in 0..NB_BANDS {
                g[i] = g[i].max(0.6 * self.lastg[i]);
                self.lastg[i] = g[i];
            }
            super::interp_band_gain(&mut gf[..], &g[..]);
            self.feat.apply_gain(&gf);
        }

        self.feat.frame_synthesis(output);
        vad_prob[0]
    }
}
