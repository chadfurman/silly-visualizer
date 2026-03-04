use std::fs;
use std::path::Path;

const MAGIC: &[u8; 4] = b"SVRX";
const FORMAT_VERSION: u32 = 1;
const HEADER_SIZE: usize = 20;

pub struct RecordingHeader {
    pub sample_rate: u32,
    pub channels: u32,
    pub frame_count: u32,
}

pub struct AudioRecorder {
    samples: Vec<f32>,
    sample_rate: u32,
    channels: u32,
    recording: bool,
}

impl AudioRecorder {
    pub fn new(sample_rate: u32, channels: u32) -> Self {
        Self { samples: Vec::new(), sample_rate, channels, recording: false }
    }

    pub fn is_recording(&self) -> bool {
        self.recording
    }

    pub fn start(&mut self) {
        self.samples.clear();
        self.recording = true;
        log::info!("recording started");
    }

    pub fn stop(&mut self) -> usize {
        self.recording = false;
        let count = self.samples.len();
        log::info!("recording stopped: {} samples", count);
        count
    }

    pub fn push_samples(&mut self, samples: &[f32]) {
        if self.recording {
            self.samples.extend_from_slice(samples);
        }
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let mut data = Vec::new();
        data.extend_from_slice(MAGIC);
        data.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
        data.extend_from_slice(&self.sample_rate.to_le_bytes());
        data.extend_from_slice(&self.channels.to_le_bytes());
        let frame_count = self.samples.len() as u32;
        data.extend_from_slice(&frame_count.to_le_bytes());
        for &s in &self.samples {
            data.extend_from_slice(&s.to_le_bytes());
        }
        fs::write(path, &data)
            .map_err(|e| format!("failed to write recording: {e}"))
    }
}

pub struct AudioPlayer {
    samples: Vec<f32>,
    #[allow(dead_code)]
    sample_rate: u32,
    #[allow(dead_code)]
    channels: u32,
    cursor: usize,
    chunk_size: usize,
    paused: bool,
    pub frame_number: u32,
}

fn read_u32_le(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap())
}

fn parse_header(data: &[u8]) -> Result<RecordingHeader, String> {
    if data.len() < HEADER_SIZE {
        return Err("file too short".into());
    }
    if &data[0..4] != MAGIC {
        return Err("invalid recording file (bad magic)".into());
    }
    let version = read_u32_le(data, 4);
    if version != FORMAT_VERSION {
        return Err(format!("unsupported format version: {version}"));
    }
    Ok(RecordingHeader {
        sample_rate: read_u32_le(data, 8),
        channels: read_u32_le(data, 12),
        frame_count: read_u32_le(data, 16),
    })
}

fn parse_samples(data: &[u8], header: &RecordingHeader) -> Result<Vec<f32>, String> {
    let expected_bytes = HEADER_SIZE + header.frame_count as usize * 4;
    if data.len() < expected_bytes {
        return Err("file truncated".into());
    }
    let samples: Vec<f32> = (0..header.frame_count as usize)
        .map(|i| {
            let offset = HEADER_SIZE + i * 4;
            f32::from_le_bytes(data[offset..offset + 4].try_into().unwrap())
        })
        .collect();
    Ok(samples)
}

impl AudioPlayer {
    pub fn load(path: &Path) -> Result<Self, String> {
        let data = fs::read(path)
            .map_err(|e| format!("failed to read recording: {e}"))?;
        let header = parse_header(&data)?;
        let samples = parse_samples(&data, &header)?;
        let chunk_size = (header.sample_rate as usize * header.channels as usize) / 60;
        Ok(Self {
            samples,
            sample_rate: header.sample_rate,
            channels: header.channels,
            cursor: 0,
            chunk_size,
            paused: false,
            frame_number: 0,
        })
    }

    #[allow(dead_code)]
    pub fn is_paused(&self) -> bool {
        self.paused
    }

    pub fn toggle_pause(&mut self) {
        self.paused = !self.paused;
        log::info!("playback {}", if self.paused { "paused" } else { "resumed" });
    }

    pub fn next_chunk(&mut self) -> Option<&[f32]> {
        if self.paused || self.cursor >= self.samples.len() {
            return None;
        }
        let end = (self.cursor + self.chunk_size).min(self.samples.len());
        let chunk = &self.samples[self.cursor..end];
        self.cursor = end;
        self.frame_number += 1;
        Some(chunk)
    }

    pub fn step_frames(&mut self, count: i32) {
        let delta = count as i64 * self.chunk_size as i64;
        let new_cursor = (self.cursor as i64 + delta).max(0) as usize;
        self.cursor = new_cursor.min(self.samples.len());
        let new_frame = (self.cursor / self.chunk_size.max(1)) as u32;
        self.frame_number = new_frame;
        log::info!("frame {} (cursor {})", self.frame_number, self.cursor);
    }

    #[allow(dead_code)]
    pub fn is_done(&self) -> bool {
        self.cursor >= self.samples.len()
    }

    pub fn total_frames(&self) -> u32 {
        (self.samples.len() / self.chunk_size.max(1)) as u32
    }

    #[allow(dead_code)]
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    #[allow(dead_code)]
    pub fn channels(&self) -> u32 {
        self.channels
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "silly-viz-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn recorder_starts_empty() {
        let r = AudioRecorder::new(44100, 1);
        assert!(!r.is_recording());
    }

    #[test]
    fn recorder_captures_while_recording() {
        let mut r = AudioRecorder::new(44100, 1);
        r.push_samples(&[1.0, 2.0]);
        assert_eq!(r.samples.len(), 0);
        r.start();
        r.push_samples(&[1.0, 2.0, 3.0]);
        assert_eq!(r.samples.len(), 3);
        let count = r.stop();
        assert_eq!(count, 3);
        r.push_samples(&[4.0]);
        assert_eq!(r.samples.len(), 3);
    }

    #[test]
    fn save_and_load_round_trip() {
        let mut r = AudioRecorder::new(44100, 2);
        r.start();
        r.push_samples(&[0.5, -0.5, 1.0, -1.0]);
        r.stop();
        let dir = temp_dir();
        let path = dir.join("test.svrx");
        r.save(&path).unwrap();

        let player = AudioPlayer::load(&path).unwrap();
        assert_eq!(player.sample_rate(), 44100);
        assert_eq!(player.channels(), 2);
        assert_eq!(player.samples.len(), 4);
        assert_eq!(player.samples[0], 0.5);
        assert_eq!(player.samples[3], -1.0);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn player_chunks_through_samples() {
        let mut r = AudioRecorder::new(44100, 1);
        r.start();
        let data: Vec<f32> = (0..4410).map(|i| i as f32).collect();
        r.push_samples(&data);
        r.stop();
        let dir = temp_dir();
        let path = dir.join("test2.svrx");
        r.save(&path).unwrap();

        let mut player = AudioPlayer::load(&path).unwrap();
        let mut chunks = 0;
        while player.next_chunk().is_some() {
            chunks += 1;
        }
        assert!(chunks > 0);
        assert!(player.is_done());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn player_step_frames_clamps() {
        let mut r = AudioRecorder::new(44100, 1);
        r.start();
        r.push_samples(&vec![0.0; 44100]);
        r.stop();
        let dir = temp_dir();
        let path = dir.join("test3.svrx");
        r.save(&path).unwrap();

        let mut player = AudioPlayer::load(&path).unwrap();
        player.step_frames(-100);
        assert_eq!(player.cursor, 0);
        player.step_frames(100000);
        assert!(player.cursor <= player.samples.len());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_rejects_bad_magic() {
        let dir = temp_dir();
        let path = dir.join("bad.svrx");
        fs::write(&path, b"NOT_A_RECORDING_FILE_AT_ALL").unwrap();
        assert!(AudioPlayer::load(&path).is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn pause_toggle_works() {
        let mut r = AudioRecorder::new(44100, 1);
        r.start();
        r.push_samples(&vec![0.0; 44100]);
        r.stop();
        let dir = temp_dir();
        let path = dir.join("test4.svrx");
        r.save(&path).unwrap();

        let mut player = AudioPlayer::load(&path).unwrap();
        assert!(!player.is_paused());
        player.toggle_pause();
        assert!(player.is_paused());
        assert!(player.next_chunk().is_none());
        player.toggle_pause();
        assert!(!player.is_paused());
        assert!(player.next_chunk().is_some());
        let _ = fs::remove_dir_all(&dir);
    }
}
