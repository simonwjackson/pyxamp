//! FFTNullsoft port: Webamp FFTNullsoft.ts, itself from WACUP/vis_classic.
//! Copyright 2005-2013 Nullsoft, Inc. BSD-3-Clause.
//! Webamp changes Copyright (c) 2015 Jordan Eldredge. MIT.
//! Full notices and pinned provenance: daemon/licenses/ and daemon/licenses/PROVENANCE.md.
const N: usize = 1024;
pub struct NullsoftFft {
    bit_reverse: Vec<usize>,
    envelope: Vec<f32>,
    equalize: Vec<f32>,
    twiddles: Vec<(f32, f32)>,
    real: Vec<f32>,
    imaginary: Vec<f32>,
}
impl Default for NullsoftFft {
    // Preserve the upstream envelope constants rather than changing FFT rounding.
    #[allow(clippy::approx_constant)]
    fn default() -> Self {
        let bit_reverse = (0..N)
            .map(|v| v.reverse_bits() >> (usize::BITS - 10))
            .collect();
        let envelope = (0..N)
            .map(|i| (0.5 + 0.5 * (i as f64 * 6.2831853 / N as f64 - 1.5707963268).sin()) as f32)
            .collect();
        let mut bias = 0.04;
        let equalize = (0..N / 2)
            .map(|i| {
                let v = (1.0_f64 + bias + (i + 1) as f64 * (9.0 - bias) / (N / 2) as f64).log10()
                    as f32;
                bias /= 1.0025;
                v
            })
            .collect();
        let twiddles = (1..=10)
            .map(|i| {
                let theta = -2.0 * std::f64::consts::PI / (1 << i) as f64;
                (theta.cos() as f32, theta.sin() as f32)
            })
            .collect();
        Self {
            bit_reverse,
            envelope,
            equalize,
            twiddles,
            real: vec![0.; N],
            imaginary: vec![0.; N],
        }
    }
}
impl NullsoftFft {
    pub fn spectrum(&mut self, samples: &[f32; N]) -> [f32; N / 2] {
        for i in 0..N {
            let index = self.bit_reverse[i];
            self.real[i] = samples[index] * self.envelope[index];
        }
        self.imaginary.fill(0.0);
        let mut size = 2;
        for &(cos, sin) in &self.twiddles {
            let (mut wr, mut wi) = (1.0_f32, 0.0_f32);
            let half = size / 2;
            for m in 0..half {
                for i in (m..N).step_by(size) {
                    let j = i + half;
                    let tr = wr * self.real[j] - wi * self.imaginary[j];
                    let ti = wr * self.imaginary[j] + wi * self.real[j];
                    self.real[j] = self.real[i] - tr;
                    self.imaginary[j] = self.imaginary[i] - ti;
                    self.real[i] += tr;
                    self.imaginary[i] += ti;
                }
                let old = wr;
                wr = wr * cos - wi * sin;
                wi = wi * cos + old * sin;
            }
            size <<= 1;
        }
        std::array::from_fn(|i| self.real[i].hypot(self.imaginary[i]) * self.equalize[i])
    }
    pub fn frame(&mut self, samples: &[f32; N]) -> (Vec<u8>, Vec<u8>) {
        let spectrum = self.spectrum(samples);
        let columns = (0..75)
            .map(|i| {
                let start = i * 512 / 75;
                let end = (i + 1) * 512 / 75;
                let magnitude = spectrum[start..end].iter().copied().fold(0.0_f32, f32::max);
                // Webamp feeds byte-centered PCM /24; normalize a 15-unit analyzer to 0..255.
                (magnitude * (128.0 / 24.0) * (255.0 / 15.0))
                    .round()
                    .clamp(0.0, 255.0) as u8
            })
            .collect();
        let waveform = (0..75)
            .map(|i| (128.0 + samples[i * N / 75].clamp(-1.0, 1.0) * 127.0).round() as u8)
            .collect();
        (columns, waveform)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn silence_dc_and_bin_centered_pcm_match_nullsoft_window() {
        let mut fft = NullsoftFft::default();
        let silence = [0.; N];
        let (columns, waveform) = fft.frame(&silence);
        assert_eq!(columns, vec![0; 75]);
        assert_eq!(waveform, vec![128; 75]);
        let dc = fft.spectrum(&[1.; N]);
        let eq0 = (1.0_f64 + 0.04 + (9.0 - 0.04) / 512.0).log10() as f32;
        assert!((dc[0] - 512.0 * eq0).abs() < 0.001);
        let tone = std::array::from_fn(|i| {
            (std::f32::consts::TAU * 32.0 * i as f32 / N as f32).sin() * 0.1
        });
        let result = fft.spectrum(&tone);
        let peak = result
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .unwrap()
            .0;
        assert_eq!(peak, 32);
        assert!(result[100] < 0.0001);
        let (columns, waveform) = fft.frame(&tone);
        assert_eq!(columns.len(), 75);
        assert_eq!(waveform.len(), 75);
        assert!(waveform.iter().any(|v| *v > 128));
        assert!(waveform.iter().any(|v| *v < 128));
    }
}
