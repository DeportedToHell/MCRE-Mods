// The real FFT RNNoise needs (960 points, forward and inverse, neither normalised), in place of
// the `easyfft` crate nnnoiseless uses: a mixed-radix Cooley-Tukey transform after Mark
// Borgerding's KISS FFT (its generic butterfly), which RNNoise itself is built on. BSD-3-Clause:
// see the notice in mod.rs.

use std::ops::{Add, AddAssign, Mul, MulAssign};

/// A complex number of `f32`s.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Complex {
    pub re: f32,
    pub im: f32,
}

impl Complex {
    pub const fn new(re: f32, im: f32) -> Complex {
        Complex { re, im }
    }
    fn conj(self) -> Complex {
        Complex::new(self.re, -self.im)
    }
}

impl Add for Complex {
    type Output = Complex;
    fn add(self, o: Complex) -> Complex {
        Complex::new(self.re + o.re, self.im + o.im)
    }
}

impl AddAssign for Complex {
    fn add_assign(&mut self, o: Complex) {
        self.re += o.re;
        self.im += o.im;
    }
}

impl Mul for Complex {
    type Output = Complex;
    fn mul(self, o: Complex) -> Complex {
        Complex::new(self.re * o.re - self.im * o.im, self.re * o.im + self.im * o.re)
    }
}

impl Mul<f32> for Complex {
    type Output = Complex;
    fn mul(self, k: f32) -> Complex {
        Complex::new(self.re * k, self.im * k)
    }
}

impl MulAssign<f32> for Complex {
    fn mul_assign(&mut self, k: f32) {
        self.re *= k;
        self.im *= k;
    }
}

/// A complex FFT of one size: its factors (radix, remaining length) and twiddles.
pub struct Fft {
    n: usize,
    factors: Vec<(usize, usize)>,
    twiddles: Vec<Complex>,
}

impl Fft {
    pub fn new(n: usize) -> Fft {
        // KISS FFT's factoring: fours first, then twos, threes, fives...
        let mut factors = Vec::new();
        let mut rest = n;
        let mut p = 4;
        while rest > 1 {
            while rest % p != 0 {
                p = match p {
                    4 => 2,
                    2 => 3,
                    _ => p + 2,
                };
                if p * p > rest {
                    p = rest;
                }
            }
            rest /= p;
            factors.push((p, rest));
        }
        let twiddles = (0..n)
            .map(|i| {
                let phase = -2.0 * std::f64::consts::PI * i as f64 / n as f64;
                Complex::new(phase.cos() as f32, phase.sin() as f32)
            })
            .collect();
        Fft { n, factors, twiddles }
    }

    /// `out[k] = sum input[j] e^(-2 pi i jk / n)`.
    pub fn forward(&self, input: &[Complex], out: &mut [Complex]) {
        assert!(input.len() == self.n && out.len() == self.n);
        self.work(out, input, 0, 1, &self.factors);
    }

    fn work(&self, out: &mut [Complex], input: &[Complex], start: usize, stride: usize, factors: &[(usize, usize)]) {
        let (p, m) = factors[0];
        if m == 1 {
            for (q, o) in out.iter_mut().enumerate().take(p) {
                *o = input[start + q * stride];
            }
        } else {
            for q in 0..p {
                self.work(&mut out[q * m..(q + 1) * m], input, start + q * stride, stride * p, &factors[1..]);
            }
        }
        // The generic butterfly (`kf_bfly_generic`): p DFTs of size p, twiddled.
        let mut scratch = [Complex::default(); 8];
        let mut big = Vec::new();
        let scratch: &mut [Complex] = if p <= scratch.len() {
            &mut scratch[..p]
        } else {
            big.resize(p, Complex::default());
            &mut big
        };
        for u in 0..m {
            for (q, s) in scratch.iter_mut().enumerate() {
                *s = out[u + q * m];
            }
            for q1 in 0..p {
                let k = u + q1 * m;
                let mut acc = scratch[0];
                let mut tw = 0;
                for s in scratch.iter().skip(1) {
                    tw += stride * k;
                    if tw >= self.n {
                        tw -= self.n;
                    }
                    acc += *s * self.twiddles[tw];
                }
                out[k] = acc;
            }
        }
    }
}

/// The transform of a real signal of even length `n`: its first `n / 2 + 1` bins, the DC term
/// first and the Nyquist one last (as `easyfft`'s `DynRealDft`, its offset in front).
#[derive(Clone, Debug)]
pub struct RealDft {
    pub bins: Vec<Complex>,
}

impl RealDft {
    pub fn new(n: usize) -> RealDft {
        RealDft { bins: vec![Complex::default(); n / 2 + 1] }
    }
}

impl std::ops::Deref for RealDft {
    type Target = [Complex];
    fn deref(&self) -> &[Complex] {
        &self.bins
    }
}

impl std::ops::DerefMut for RealDft {
    fn deref_mut(&mut self) -> &mut [Complex] {
        &mut self.bins
    }
}

impl MulAssign<f32> for RealDft {
    fn mul_assign(&mut self, k: f32) {
        for b in &mut self.bins {
            *b *= k;
        }
    }
}

impl MulAssign<&[f32]> for RealDft {
    fn mul_assign(&mut self, k: &[f32]) {
        for (b, &k) in self.bins.iter_mut().zip(k) {
            *b *= k;
        }
    }
}

/// Real forward and inverse transforms of one length, with their scratch buffers.
pub struct RealFft {
    fft: Fft,
    a: Vec<Complex>,
    b: Vec<Complex>,
}

impl RealFft {
    pub fn new(n: usize) -> RealFft {
        RealFft { fft: Fft::new(n), a: vec![Complex::default(); n], b: vec![Complex::default(); n] }
    }

    /// `out` = the unnormalised transform of `input`.
    pub fn forward(&mut self, input: &[f32], out: &mut RealDft) {
        for (a, &x) in self.a.iter_mut().zip(input) {
            *a = Complex::new(x, 0.0);
        }
        self.fft.forward(&self.a, &mut self.b);
        let half = out.bins.len();
        out.bins.copy_from_slice(&self.b[..half]);
        // A real signal's DC and Nyquist bins are real.
        out.bins[0].im = 0.0;
        out.bins[half - 1].im = 0.0;
    }

    /// `out` = the unnormalised inverse transform of the conjugate-symmetric spectrum `input`
    /// (the imaginary parts of its DC and Nyquist bins are ignored).
    pub fn inverse(&mut self, input: &RealDft, out: &mut [f32]) {
        let n = self.a.len();
        let half = input.bins.len();
        for k in 0..n {
            let x = if k < half { input.bins[k] } else { input.bins[n - k].conj() };
            self.a[k] = x;
        }
        self.a[0].im = 0.0;
        self.a[half - 1].im = 0.0;
        // ifft(x) = conj(fft(conj(x))).
        for a in &mut self.a {
            *a = a.conj();
        }
        self.fft.forward(&self.a, &mut self.b);
        for (o, b) in out.iter_mut().zip(&self.b) {
            *o = b.re;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dft(x: &[Complex]) -> Vec<Complex> {
        let n = x.len();
        (0..n)
            .map(|k| {
                let mut acc = (0.0f64, 0.0f64);
                for (j, v) in x.iter().enumerate() {
                    let ph = -2.0 * std::f64::consts::PI * ((j * k) % n) as f64 / n as f64;
                    acc.0 += f64::from(v.re) * ph.cos() - f64::from(v.im) * ph.sin();
                    acc.1 += f64::from(v.re) * ph.sin() + f64::from(v.im) * ph.cos();
                }
                Complex::new(acc.0 as f32, acc.1 as f32)
            })
            .collect()
    }

    #[test]
    fn matches_a_plain_dft() {
        for n in [960, 480, 12, 7] {
            let x: Vec<Complex> = (0..n).map(|i| Complex::new(((i * 7919) % 101) as f32 - 50.0, ((i * 31) % 17) as f32 - 8.0)).collect();
            let mut out = vec![Complex::default(); n];
            Fft::new(n).forward(&x, &mut out);
            for (a, b) in out.iter().zip(dft(&x)) {
                assert!((a.re - b.re).abs() < 0.05 && (a.im - b.im).abs() < 0.05, "{n}: {a:?} {b:?}");
            }
        }
    }

    #[test]
    fn real_round_trip() {
        let n = 960;
        let x: Vec<f32> = (0..n).map(|i| (i as f32 * 0.37).sin() * 1000.0 + ((i * 13) % 7) as f32).collect();
        let mut r = RealFft::new(n);
        let mut spec = RealDft::new(n);
        r.forward(&x, &mut spec);
        let mut back = vec![0.0; n];
        r.inverse(&spec, &mut back);
        for (a, b) in x.iter().zip(&back) {
            assert!((a * n as f32 - b).abs() < 2.0, "{a} {b}");
        }
    }
}
