use std::sync::Arc;
use rustfft::{num_complex::Complex, Fft, FftPlanner};

pub struct AudioAnalyzer {
    fft: Arc<dyn Fft<f32>>,
    fft_size: usize,
    window: Vec<f32>,
    prev_energy: f32,
    buffer: Vec<Complex<f32>>,
    magnitudes: Vec<f32>,
    band_ranges: [(usize, usize); 16],
}

pub struct AnalysisResult {
    pub bands: [f32; 16],
    pub bass: f32,
    pub mids: f32,
    pub highs: f32,
    pub energy: f32,
    pub beat: f32,
    pub spectral_profile: [f32; 5],
}

fn compute_hann_window(size: usize) -> Vec<f32> {
    (0..size)
        .map(|i| 0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / size as f32).cos()))
        .collect()
}

fn compute_band_ranges(half: usize) -> [(usize, usize); 16] {
    let mut ranges = [(0usize, 0usize); 16];
    for (i, range) in ranges.iter_mut().enumerate() {
        let lo = band_edge(half, i);
        let hi = band_edge(half, i + 1);
        *range = (lo.min(half - 1), hi.min(half).max(lo + 1));
    }
    ranges
}

fn band_edge(half: usize, i: usize) -> usize {
    (half as f32 * (2.0f32.powf(i as f32 / 16.0 * 10.0) - 1.0) / 1023.0) as usize
}

impl AudioAnalyzer {
    pub fn new(fft_size: usize) -> Self {
        let mut planner = FftPlanner::new();
        let half = fft_size / 2;
        Self {
            fft: planner.plan_fft_forward(fft_size),
            fft_size,
            window: compute_hann_window(fft_size),
            prev_energy: 0.0,
            buffer: vec![Complex::new(0.0, 0.0); fft_size],
            magnitudes: vec![0.0; half],
            band_ranges: compute_band_ranges(half),
        }
    }

    pub fn analyze(&mut self, samples: &[f32]) -> AnalysisResult {
        self.fill_buffer(samples);
        self.fft.process(&mut self.buffer);
        self.compute_magnitudes();
        let bands = self.compute_bands();
        let (bass, mids, highs) = compute_band_groups(&bands);
        let energy = bands.iter().sum::<f32>() / 16.0;
        let beat = self.detect_beat(energy);
        AnalysisResult {
            bands, bass, mids, highs, energy, beat,
            spectral_profile: [bass, mids, highs, energy, beat],
        }
    }

    fn fill_buffer(&mut self, samples: &[f32]) {
        let count = samples.len().min(self.fft_size);
        for (i, &s) in samples.iter().enumerate().take(count) {
            self.buffer[i] = Complex::new(s * self.window[i], 0.0);
        }
        for slot in &mut self.buffer[count..self.fft_size] {
            *slot = Complex::new(0.0, 0.0);
        }
    }

    fn compute_magnitudes(&mut self) {
        let inv = 1.0 / self.fft_size as f32;
        for i in 0..self.magnitudes.len() {
            self.magnitudes[i] = self.buffer[i].norm() * inv;
        }
    }

    fn compute_bands(&self) -> [f32; 16] {
        let mut bands = [0.0f32; 16];
        for (i, band) in bands.iter_mut().enumerate() {
            let (start, end) = self.band_ranges[i];
            *band = self.magnitudes[start..end].iter().sum::<f32>() / (end - start) as f32;
        }
        bands
    }

    fn detect_beat(&mut self, energy: f32) -> f32 {
        let beat = if energy > self.prev_energy * 1.5 && energy > 0.01 {
            1.0
        } else {
            0.0
        };
        self.prev_energy = self.prev_energy * 0.9 + energy * 0.1;
        beat
    }
}

fn compute_band_groups(bands: &[f32; 16]) -> (f32, f32, f32) {
    let bass = (bands[0] + bands[1] + bands[2]) / 3.0;
    let mids = (bands[4] + bands[5] + bands[6] + bands[7]) / 4.0;
    let highs = (bands[10] + bands[11] + bands[12] + bands[13]) / 4.0;
    (bass, mids, highs)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine_wave(freq: f32, len: usize, amplitude: f32) -> Vec<f32> {
        (0..len)
            .map(|i| amplitude * (2.0 * std::f32::consts::PI * freq * i as f32 / 44100.0).sin())
            .collect()
    }

    #[test]
    fn analyzer_produces_16_bands() {
        let mut a = AudioAnalyzer::new(2048);
        let result = a.analyze(&vec![0.0; 2048]);
        assert_eq!(result.bands.len(), 16);
    }

    #[test]
    fn silence_produces_zero_energy() {
        let mut a = AudioAnalyzer::new(2048);
        let result = a.analyze(&vec![0.0; 2048]);
        assert_eq!(result.energy, 0.0);
        assert_eq!(result.bass, 0.0);
        assert_eq!(result.mids, 0.0);
        assert_eq!(result.highs, 0.0);
        assert_eq!(result.beat, 0.0);
    }

    #[test]
    fn loud_signal_produces_nonzero_energy() {
        let mut a = AudioAnalyzer::new(2048);
        let result = a.analyze(&sine_wave(440.0, 2048, 1.0));
        assert!(result.energy > 0.0);
    }

    #[test]
    fn bass_responds_to_low_frequency() {
        let mut a = AudioAnalyzer::new(2048);
        let result = a.analyze(&sine_wave(60.0, 2048, 1.0));
        assert!(result.bass > 0.0);
    }

    #[test]
    fn beat_detection_triggers_on_energy_spike() {
        let mut a = AudioAnalyzer::new(2048);
        let silence = vec![0.0f32; 2048];
        a.analyze(&silence);
        a.analyze(&silence);
        let loud: Vec<f32> = (0..2048)
            .map(|i| {
                let t = i as f32 / 44100.0;
                5.0 * ((2.0 * std::f32::consts::PI * 100.0 * t).sin()
                    + (2.0 * std::f32::consts::PI * 500.0 * t).sin()
                    + (2.0 * std::f32::consts::PI * 2000.0 * t).sin())
            })
            .collect();
        let result = a.analyze(&loud);
        assert!(result.energy > 0.01);
        assert_eq!(result.beat, 1.0);
    }

    #[test]
    fn beat_does_not_trigger_on_steady_signal() {
        let mut a = AudioAnalyzer::new(2048);
        let signal = sine_wave(200.0, 2048, 0.5);
        for _ in 0..20 {
            a.analyze(&signal);
        }
        let result = a.analyze(&signal);
        assert_eq!(result.beat, 0.0);
    }

    #[test]
    fn handles_short_input_gracefully() {
        let mut a = AudioAnalyzer::new(2048);
        let result = a.analyze(&vec![0.5f32; 100]);
        assert_eq!(result.bands.len(), 16);
    }

    #[test]
    fn all_bands_are_finite() {
        let mut a = AudioAnalyzer::new(2048);
        let result = a.analyze(&sine_wave(1000.0, 2048, 1.0));
        for (i, band) in result.bands.iter().enumerate() {
            assert!(band.is_finite(), "band {i} should be finite");
        }
        assert!(result.bass.is_finite());
        assert!(result.mids.is_finite());
        assert!(result.highs.is_finite());
        assert!(result.energy.is_finite());
    }

    #[test]
    fn spectral_profile_matches_fields() {
        let mut a = AudioAnalyzer::new(2048);
        let result = a.analyze(&sine_wave(440.0, 2048, 1.0));
        assert_eq!(result.spectral_profile[0], result.bass);
        assert_eq!(result.spectral_profile[1], result.mids);
        assert_eq!(result.spectral_profile[2], result.highs);
        assert_eq!(result.spectral_profile[3], result.energy);
        assert_eq!(result.spectral_profile[4], result.beat);
    }

    #[test]
    fn hann_window_endpoints_near_zero() {
        let w = compute_hann_window(1024);
        assert!(w[0].abs() < 1e-6);
        assert!(w[1023].abs() < 0.01);
    }

    #[test]
    fn hann_window_midpoint_near_one() {
        let w = compute_hann_window(1024);
        assert!((w[512] - 1.0).abs() < 0.01);
    }

    #[test]
    fn band_ranges_cover_spectrum() {
        let ranges = compute_band_ranges(1024);
        assert_eq!(ranges[0].0, 0);
        for i in 0..16 {
            assert!(ranges[i].1 > ranges[i].0, "band {i} should have width > 0");
        }
    }

    #[test]
    fn compute_band_groups_correct() {
        let mut bands = [0.0f32; 16];
        bands[0] = 0.3; bands[1] = 0.3; bands[2] = 0.3;
        bands[4] = 0.4; bands[5] = 0.4; bands[6] = 0.4; bands[7] = 0.4;
        bands[10] = 0.5; bands[11] = 0.5; bands[12] = 0.5; bands[13] = 0.5;
        let (bass, mids, highs) = compute_band_groups(&bands);
        assert!((bass - 0.3).abs() < 1e-6);
        assert!((mids - 0.4).abs() < 1e-6);
        assert!((highs - 0.5).abs() < 1e-6);
    }

    #[test]
    fn detect_beat_requires_energy_threshold() {
        let mut a = AudioAnalyzer::new(2048);
        a.prev_energy = 0.001;
        // Energy above 1.5x prev but below 0.01 threshold
        let beat = a.detect_beat(0.005);
        assert_eq!(beat, 0.0);
    }
}
