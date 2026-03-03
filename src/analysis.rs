use std::sync::Arc;
use rustfft::{num_complex::Complex, Fft, FftPlanner};

pub struct AudioAnalyzer {
    fft: Arc<dyn Fft<f32>>,
    fft_size: usize,
    window: Vec<f32>, // Hann window
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
}

impl AudioAnalyzer {
    pub fn new(fft_size: usize) -> Self {
        let window: Vec<f32> = (0..fft_size)
            .map(|i| {
                0.5 * (1.0
                    - (2.0 * std::f32::consts::PI * i as f32 / fft_size as f32).cos())
            })
            .collect();

        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(fft_size);

        let half = fft_size / 2;
        let mut band_ranges = [(0usize, 0usize); 16];
        for i in 0..16 {
            let start = (half as f32
                * (2.0f32.powf(i as f32 / 16.0 * 10.0) - 1.0)
                / 1023.0) as usize;
            let end = (half as f32
                * (2.0f32.powf((i + 1) as f32 / 16.0 * 10.0) - 1.0)
                / 1023.0) as usize;
            band_ranges[i] = (start.min(half - 1), end.min(half).max(start + 1));
        }

        Self {
            fft,
            fft_size,
            window,
            prev_energy: 0.0,
            buffer: vec![Complex::new(0.0, 0.0); fft_size],
            magnitudes: vec![0.0; half],
            band_ranges,
        }
    }

    pub fn analyze(&mut self, samples: &[f32]) -> AnalysisResult {
        // Fill pre-allocated buffer with windowed samples
        let sample_count = samples.len().min(self.fft_size);
        for i in 0..sample_count {
            self.buffer[i] = Complex::new(samples[i] * self.window[i], 0.0);
        }
        // Zero-pad remainder
        for i in sample_count..self.fft_size {
            self.buffer[i] = Complex::new(0.0, 0.0);
        }

        self.fft.process(&mut self.buffer);

        // Compute magnitudes in-place
        let inv = 1.0 / self.fft_size as f32;
        for i in 0..self.magnitudes.len() {
            self.magnitudes[i] = self.buffer[i].norm() * inv;
        }

        // Split into 16 bands using pre-computed ranges
        let mut bands = [0.0f32; 16];
        for (i, band) in bands.iter_mut().enumerate() {
            let (start, end) = self.band_ranges[i];
            *band = self.magnitudes[start..end].iter().sum::<f32>()
                / (end - start) as f32;
        }

        let bass = (bands[0] + bands[1] + bands[2]) / 3.0;
        let mids = (bands[4] + bands[5] + bands[6] + bands[7]) / 4.0;
        let highs = (bands[10] + bands[11] + bands[12] + bands[13]) / 4.0;
        let energy = bands.iter().sum::<f32>() / 16.0;

        let beat = if energy > self.prev_energy * 1.5 && energy > 0.01 {
            1.0
        } else {
            0.0
        };
        self.prev_energy = self.prev_energy * 0.9 + energy * 0.1;

        AnalysisResult {
            bands,
            bass,
            mids,
            highs,
            energy,
            beat,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analyzer_produces_16_bands() {
        let mut analyzer = AudioAnalyzer::new(2048);
        let samples = vec![0.0f32; 2048];
        let result = analyzer.analyze(&samples);
        assert_eq!(result.bands.len(), 16);
    }

    #[test]
    fn silence_produces_zero_energy() {
        let mut analyzer = AudioAnalyzer::new(2048);
        let samples = vec![0.0f32; 2048];
        let result = analyzer.analyze(&samples);
        assert_eq!(result.energy, 0.0);
        assert_eq!(result.bass, 0.0);
        assert_eq!(result.mids, 0.0);
        assert_eq!(result.highs, 0.0);
        assert_eq!(result.beat, 0.0);
    }

    #[test]
    fn loud_signal_produces_nonzero_energy() {
        let mut analyzer = AudioAnalyzer::new(2048);
        // Generate a 440Hz sine wave at sample rate 44100
        let samples: Vec<f32> = (0..2048)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 44100.0).sin())
            .collect();
        let result = analyzer.analyze(&samples);
        assert!(result.energy > 0.0, "energy should be > 0 for a sine wave");
    }

    #[test]
    fn bass_responds_to_low_frequency() {
        let mut analyzer = AudioAnalyzer::new(2048);
        // 60Hz bass tone
        let samples: Vec<f32> = (0..2048)
            .map(|i| (2.0 * std::f32::consts::PI * 60.0 * i as f32 / 44100.0).sin())
            .collect();
        let result = analyzer.analyze(&samples);
        assert!(result.bass > 0.0, "bass should respond to 60Hz tone");
    }

    #[test]
    fn beat_detection_triggers_on_energy_spike() {
        let mut analyzer = AudioAnalyzer::new(2048);
        // Prime with silence to establish low prev_energy
        let silence = vec![0.0f32; 2048];
        analyzer.analyze(&silence);
        analyzer.analyze(&silence);

        // Sudden loud broadband signal: sum of many frequencies at high amplitude
        // to ensure energy clearly exceeds the 0.01 threshold after FFT normalization
        let loud: Vec<f32> = (0..2048)
            .map(|i| {
                let t = i as f32 / 44100.0;
                5.0 * ((2.0 * std::f32::consts::PI * 100.0 * t).sin()
                    + (2.0 * std::f32::consts::PI * 500.0 * t).sin()
                    + (2.0 * std::f32::consts::PI * 2000.0 * t).sin())
            })
            .collect();
        let result = analyzer.analyze(&loud);
        assert!(result.energy > 0.01, "energy should exceed beat threshold, got {}", result.energy);
        assert_eq!(result.beat, 1.0, "beat should trigger on sudden energy spike");
    }

    #[test]
    fn beat_does_not_trigger_on_steady_signal() {
        let mut analyzer = AudioAnalyzer::new(2048);
        let signal: Vec<f32> = (0..2048)
            .map(|i| (2.0 * std::f32::consts::PI * 200.0 * i as f32 / 44100.0).sin() * 0.5)
            .collect();
        // Feed the same signal many times to stabilize prev_energy
        for _ in 0..20 {
            analyzer.analyze(&signal);
        }
        let result = analyzer.analyze(&signal);
        assert_eq!(result.beat, 0.0, "beat should not trigger on steady signal");
    }

    #[test]
    fn handles_short_input_gracefully() {
        let mut analyzer = AudioAnalyzer::new(2048);
        // Fewer samples than fft_size — should zero-pad, not crash
        let samples = vec![0.5f32; 100];
        let result = analyzer.analyze(&samples);
        assert_eq!(result.bands.len(), 16);
    }

    #[test]
    fn all_bands_are_finite() {
        let mut analyzer = AudioAnalyzer::new(2048);
        let samples: Vec<f32> = (0..2048)
            .map(|i| (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 44100.0).sin())
            .collect();
        let result = analyzer.analyze(&samples);
        for (i, band) in result.bands.iter().enumerate() {
            assert!(band.is_finite(), "band {i} should be finite, got {band}");
        }
        assert!(result.bass.is_finite());
        assert!(result.mids.is_finite());
        assert!(result.highs.is_finite());
        assert!(result.energy.is_finite());
    }
}
