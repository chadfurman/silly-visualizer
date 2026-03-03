use rustfft::{num_complex::Complex, FftPlanner};

pub struct AudioAnalyzer {
    planner: FftPlanner<f32>,
    fft_size: usize,
    window: Vec<f32>, // Hann window
    prev_energy: f32,
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

        Self {
            planner: FftPlanner::new(),
            fft_size,
            window,
            prev_energy: 0.0,
        }
    }

    pub fn analyze(&mut self, samples: &[f32]) -> AnalysisResult {
        let fft = self.planner.plan_fft_forward(self.fft_size);
        let mut buffer: Vec<Complex<f32>> = samples
            .iter()
            .take(self.fft_size)
            .zip(self.window.iter())
            .map(|(s, w)| Complex::new(s * w, 0.0))
            .collect();

        buffer.resize(self.fft_size, Complex::new(0.0, 0.0));
        fft.process(&mut buffer);

        let magnitudes: Vec<f32> = buffer[..self.fft_size / 2]
            .iter()
            .map(|c| c.norm() / self.fft_size as f32)
            .collect();

        // Split into 16 bands (logarithmic distribution)
        let mut bands = [0.0f32; 16];
        let half = magnitudes.len();
        for (i, band) in bands.iter_mut().enumerate() {
            let start = (half as f32
                * (2.0f32.powf(i as f32 / 16.0 * 10.0) - 1.0)
                / 1023.0) as usize;
            let end = (half as f32
                * (2.0f32.powf((i + 1) as f32 / 16.0 * 10.0) - 1.0)
                / 1023.0) as usize;
            let start = start.min(half - 1);
            let end = end.min(half).max(start + 1);
            *band = magnitudes[start..end].iter().sum::<f32>()
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
