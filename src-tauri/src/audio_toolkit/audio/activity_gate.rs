#[derive(Debug, Clone, Copy)]
pub struct AudioActivityStats {
    pub duration_ms: u64,
    pub peak_abs: f32,
    pub rms_dbfs: f32,
    pub active_frames: usize,
    pub total_frames: usize,
    pub active_ratio: f32,
}

const FRAME_MS: usize = 20;
const HOP_MS: usize = 10;
const FRAME_RMS_ACTIVE_DBFS: f32 = -52.0;
const FRAME_PEAK_ACTIVE: f32 = 0.012;

const SKIP_MAX_PEAK: f32 = 0.015;
const SKIP_MAX_RMS_DBFS: f32 = -50.0;
const SKIP_MAX_ACTIVE_RATIO: f32 = 0.10;
const SKIP_MAX_ACTIVE_FRAMES: usize = 8;

fn rms_dbfs(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return -120.0;
    }

    let sum_sq = samples.iter().map(|s| s * s).sum::<f32>();
    let rms = (sum_sq / samples.len() as f32).sqrt();
    if rms <= 1e-9 {
        -120.0
    } else {
        20.0 * rms.log10()
    }
}

pub fn analyze_activity(samples: &[f32], sample_rate: usize) -> AudioActivityStats {
    let duration_ms = if sample_rate == 0 {
        0
    } else {
        (samples.len() as u64).saturating_mul(1000) / sample_rate as u64
    };

    let peak_abs = samples
        .iter()
        .map(|s| s.abs())
        .fold(0.0_f32, |acc, s| acc.max(s));
    let global_rms_dbfs = rms_dbfs(samples);

    if samples.is_empty() || sample_rate == 0 {
        return AudioActivityStats {
            duration_ms,
            peak_abs,
            rms_dbfs: global_rms_dbfs,
            active_frames: 0,
            total_frames: 0,
            active_ratio: 0.0,
        };
    }

    let frame_len = (sample_rate * FRAME_MS / 1000).max(1);
    let hop_len = (sample_rate * HOP_MS / 1000).max(1);

    let mut total_frames = 0usize;
    let mut active_frames = 0usize;

    if samples.len() < frame_len {
        total_frames = 1;
        let frame_peak = peak_abs;
        let frame_rms_db = rms_dbfs(samples);
        if frame_rms_db >= FRAME_RMS_ACTIVE_DBFS || frame_peak >= FRAME_PEAK_ACTIVE {
            active_frames = 1;
        }
    } else {
        let mut start = 0usize;
        while start + frame_len <= samples.len() {
            let frame = &samples[start..start + frame_len];
            total_frames += 1;
            let frame_peak = frame
                .iter()
                .map(|s| s.abs())
                .fold(0.0_f32, |acc, s| acc.max(s));
            let frame_rms_db = rms_dbfs(frame);
            if frame_rms_db >= FRAME_RMS_ACTIVE_DBFS || frame_peak >= FRAME_PEAK_ACTIVE {
                active_frames += 1;
            }
            start += hop_len;
        }
    }

    let active_ratio = if total_frames == 0 {
        0.0
    } else {
        active_frames as f32 / total_frames as f32
    };

    AudioActivityStats {
        duration_ms,
        peak_abs,
        rms_dbfs: global_rms_dbfs,
        active_frames,
        total_frames,
        active_ratio,
    }
}

pub fn should_skip_transcription(stats: &AudioActivityStats) -> bool {
    if stats.total_frames == 0 {
        return true;
    }

    stats.peak_abs < SKIP_MAX_PEAK
        && stats.rms_dbfs < SKIP_MAX_RMS_DBFS
        && stats.active_ratio < SKIP_MAX_ACTIVE_RATIO
        && stats.active_frames < SKIP_MAX_ACTIVE_FRAMES
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: usize = 16_000;

    #[test]
    fn silence_is_skipped() {
        let samples = vec![0.0_f32; SR];
        let stats = analyze_activity(&samples, SR);
        assert!(should_skip_transcription(&stats));
    }

    #[test]
    fn very_low_noise_is_skipped() {
        let samples = vec![0.001_f32; SR];
        let stats = analyze_activity(&samples, SR);
        assert!(should_skip_transcription(&stats));
    }

    #[test]
    fn quiet_speech_like_signal_not_skipped() {
        let freq = 220.0_f32;
        let amp = 0.03_f32;
        let samples = (0..SR)
            .map(|i| {
                let t = i as f32 / SR as f32;
                amp * (2.0 * std::f32::consts::PI * freq * t).sin()
            })
            .collect::<Vec<_>>();
        let stats = analyze_activity(&samples, SR);
        assert!(!should_skip_transcription(&stats));
    }

    #[test]
    fn short_click_not_treated_as_speech() {
        let mut samples = vec![0.0_f32; SR];
        samples[100] = 0.014;
        let stats = analyze_activity(&samples, SR);
        assert!(should_skip_transcription(&stats));
    }
}
