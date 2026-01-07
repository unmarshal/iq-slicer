use crate::input::IqSample;
use rustfft::{FftPlanner, num_complex::Complex};
use std::f32::consts::PI;

/// Generate Blackman window coefficients
/// Better sidelobe suppression (-58 dB) than Hann (-31 dB) at cost of wider main lobe
pub fn blackman_window(size: usize) -> Vec<f32> {
    let a0 = 0.42;
    let a1 = 0.5;
    let a2 = 0.08;
    (0..size)
        .map(|n| {
            let x = n as f32 / (size - 1) as f32;
            a0 - a1 * (2.0 * PI * x).cos() + a2 * (4.0 * PI * x).cos()
        })
        .collect()
}

/// A detected transmission segment
#[derive(Debug, Clone)]
pub struct Segment {
    pub start_sample: usize,
    pub end_sample: usize,
}

impl Segment {
    pub fn duration_samples(&self) -> usize {
        self.end_sample - self.start_sample
    }

    pub fn duration_ms(&self, sample_rate: u32) -> f32 {
        self.duration_samples() as f32 / sample_rate as f32 * 1000.0
    }
}

/// Calculate peak FFT bin power in dB for a window of samples
/// This finds the strongest signal in any frequency bin, much better for narrowband bursts
/// Applies window function to reduce spectral leakage
pub fn calculate_peak_power_db(samples: &[IqSample], window: &[f32], planner: &mut FftPlanner<f32>) -> f32 {
    if samples.is_empty() {
        return f32::NEG_INFINITY;
    }

    let fft_size = samples.len();
    let fft = planner.plan_fft_forward(fft_size);

    // Apply window and convert to complex
    let mut buffer: Vec<Complex<f32>> = samples
        .iter()
        .zip(window.iter())
        .map(|(s, w)| Complex::new(s.i * w, s.q * w))
        .collect();

    // Compute FFT
    fft.process(&mut buffer);

    // Find peak magnitude (skip DC bin)
    let peak_power = buffer
        .iter()
        .skip(1)
        .map(|c| c.norm_sqr())
        .fold(0.0f32, f32::max);

    // Normalize by FFT size and convert to dB
    let normalized_power = peak_power / (fft_size * fft_size) as f32;
    10.0 * normalized_power.log10()
}

/// Calculate peak power profile over time using FFT with 50% overlap
/// Uses Blackman window for reduced spectral leakage
fn calculate_peak_power_profile(samples: &[IqSample], window_size: usize) -> Vec<f32> {
    if samples.len() < window_size {
        let window = blackman_window(samples.len());
        let mut planner = FftPlanner::new();
        return vec![calculate_peak_power_db(samples, &window, &mut planner)];
    }

    let window = blackman_window(window_size);
    let mut planner = FftPlanner::new();
    let hop_size = window_size / 2; // 50% overlap
    let num_frames = (samples.len().saturating_sub(window_size)) / hop_size + 1;
    let mut profile = Vec::with_capacity(num_frames);

    for i in 0..num_frames {
        let start = i * hop_size;
        let end = (start + window_size).min(samples.len());
        if end - start == window_size {
            profile.push(calculate_peak_power_db(&samples[start..end], &window, &mut planner));
        }
    }
    profile
}

/// Result of auto-threshold analysis
pub struct ThresholdAnalysis {
    pub threshold: f32,
    pub noise_floor: f32,
    pub p95: f32,
}

/// Auto-detect threshold based on noise floor analysis
/// Uses FFT peak power and percentile approach for narrowband burst detection
pub fn auto_threshold(samples: &[IqSample], window_size: usize) -> ThresholdAnalysis {
    let mut power_profile = calculate_peak_power_profile(samples, window_size);

    if power_profile.is_empty() {
        return ThresholdAnalysis {
            threshold: -60.0,
            noise_floor: -70.0,
            p95: -50.0,
        };
    }

    // Sort to find percentiles
    power_profile.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    // Find 10th percentile as noise floor estimate (lowest power periods)
    let p10_idx = (power_profile.len() as f32 * 0.10) as usize;
    let noise_floor = power_profile[p10_idx];

    // Find 99th percentile to catch burst peaks
    let p99_idx = (power_profile.len() as f32 * 0.99) as usize;
    let p95 = power_profile[p99_idx.min(power_profile.len() - 1)];

    // Threshold is 70% of the way from noise floor to peak
    // This catches bursts while rejecting noise
    let threshold = noise_floor + (p95 - noise_floor) * 0.7;

    ThresholdAnalysis {
        threshold,
        noise_floor,
        p95,
    }
}

/// Detect transmission segments based on power threshold
/// Uses FFT peak power with 50% overlap and hysteresis: triggers ON at threshold, OFF at threshold - 3dB
pub fn detect_segments(
    samples: &[IqSample],
    window_size: usize,
    threshold_db: f32,
    min_duration_samples: usize,
    max_gap_samples: usize,
) -> Vec<Segment> {
    let power_profile = calculate_peak_power_profile(samples, window_size);

    if power_profile.is_empty() {
        return vec![];
    }

    let hop_size = window_size / 2; // Must match calculate_peak_power_profile
    let threshold_on = threshold_db;
    let threshold_off = threshold_db - 3.0; // Hysteresis

    let mut segments = Vec::new();
    let mut in_transmission = false;
    let mut start_idx = 0;

    for (idx, &power) in power_profile.iter().enumerate() {
        if !in_transmission && power > threshold_on {
            // Start of transmission
            in_transmission = true;
            start_idx = idx;
        } else if in_transmission && power < threshold_off {
            // End of transmission
            in_transmission = false;
            segments.push(Segment {
                start_sample: start_idx * hop_size,
                end_sample: idx * hop_size + window_size,
            });
        }
    }

    // Handle transmission that extends to end of file
    if in_transmission {
        segments.push(Segment {
            start_sample: start_idx * hop_size,
            end_sample: samples.len(),
        });
    }

    // Merge segments that are close together
    let merged = merge_segments(segments, max_gap_samples);

    // Filter out short segments
    merged
        .into_iter()
        .filter(|s| s.duration_samples() >= min_duration_samples)
        .collect()
}

/// Merge segments that are separated by less than max_gap samples
fn merge_segments(segments: Vec<Segment>, max_gap: usize) -> Vec<Segment> {
    if segments.is_empty() {
        return segments;
    }

    let mut merged = Vec::new();
    let mut current = segments[0].clone();

    for segment in segments.into_iter().skip(1) {
        if segment.start_sample <= current.end_sample + max_gap {
            // Merge
            current.end_sample = segment.end_sample;
        } else {
            merged.push(current);
            current = segment;
        }
    }
    merged.push(current);

    merged
}

/// Add padding to segments, clamping to valid bounds
pub fn add_padding(segments: Vec<Segment>, padding_samples: usize, total_samples: usize) -> Vec<Segment> {
    segments
        .into_iter()
        .map(|mut s| {
            s.start_sample = s.start_sample.saturating_sub(padding_samples);
            s.end_sample = (s.end_sample + padding_samples).min(total_samples);
            s
        })
        .collect()
}
