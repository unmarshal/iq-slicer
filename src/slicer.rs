use std::path::Path;
use chrono::Local;

use crate::input::wav::read_iq_wav;
use crate::input::stream::{IqStreamReader, StreamFormat};
use crate::input::IqSample;
use crate::detector::{auto_threshold, detect_segments, add_padding, calculate_peak_power_db};
use rustfft::FftPlanner;
use crate::output::{write_iq_wav, write_iq_wav_float32, generate_filename};

/// Process an IQ WAV file and output sliced IQ segments
#[allow(clippy::too_many_arguments)]
pub fn process_file(
    input_path: &Path,
    output_dir: &Path,
    min_duration_ms: u32,
    max_duration_ms: Option<u32>,
    gap_ms: u32,
    padding_ms: u32,
    verbose: bool,
    float32_output: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    // Read IQ file
    if verbose {
        println!("Reading IQ file...");
    }
    let (samples, metadata) = read_iq_wav(input_path)?;

    if verbose {
        println!(
            "Loaded {} samples at {} Hz ({:.2}s)",
            samples.len(),
            metadata.sample_rate,
            samples.len() as f32 / metadata.sample_rate as f32
        );
    }

    // Calculate detection parameters in samples
    let window_size = (metadata.sample_rate as usize / 1000).max(1); // 1ms windows for better burst detection
    let min_duration_samples = (min_duration_ms as f32 / 1000.0 * metadata.sample_rate as f32) as usize;
    let gap_samples = (gap_ms as f32 / 1000.0 * metadata.sample_rate as f32) as usize;
    let padding_samples = (padding_ms as f32 / 1000.0 * metadata.sample_rate as f32) as usize;

    // Auto-detect threshold
    let analysis = auto_threshold(&samples, window_size);
    if verbose {
        println!(
            "Auto-detected: noise_floor={:.1} dB, p95={:.1} dB, threshold={:.1} dB",
            analysis.noise_floor, analysis.p95, analysis.threshold
        );
    }
    let threshold = analysis.threshold;

    // Detect segments
    if verbose {
        println!("Detecting transmissions...");
    }
    let segments = detect_segments(
        &samples,
        window_size,
        threshold,
        min_duration_samples,
        gap_samples,
    );

    // Add padding
    let segments = add_padding(segments, padding_samples, samples.len());

    // Filter by max duration if specified
    let segments: Vec<_> = if let Some(max_ms) = max_duration_ms {
        let max_samples = (max_ms as f32 / 1000.0 * metadata.sample_rate as f32) as usize;
        segments
            .into_iter()
            .filter(|s| s.duration_samples() <= max_samples)
            .collect()
    } else {
        segments
    };

    if verbose {
        println!("Found {} transmission(s)", segments.len());
    }

    if segments.is_empty() {
        println!("No transmissions detected");
        return Ok(());
    }

    // Process each segment
    let base_time = Local::now();
    for (i, segment) in segments.iter().enumerate() {
        if verbose {
            println!(
                "  Slice {}: {:.2}s - {:.2}s ({:.2}s duration)",
                i + 1,
                segment.start_sample as f32 / metadata.sample_rate as f32,
                segment.end_sample as f32 / metadata.sample_rate as f32,
                segment.duration_ms(metadata.sample_rate) / 1000.0
            );
        }

        // Extract segment samples
        let segment_samples = &samples[segment.start_sample..segment.end_sample];

        // Generate output filename and write
        let filename = generate_filename(i + 1, segment.start_sample, metadata.sample_rate, base_time);
        let output_path = output_dir.join(&filename);

        if float32_output {
            write_iq_wav_float32(&output_path, segment_samples, metadata.sample_rate)?;
        } else {
            write_iq_wav(&output_path, segment_samples, metadata.sample_rate)?;
        }

        if verbose {
            println!("    Wrote: {}", filename);
        }
    }

    println!(
        "Saved {} slice(s) to {}",
        segments.len(),
        output_dir.display()
    );

    Ok(())
}

/// Process live IQ stream and output sliced IQ segments
#[allow(clippy::too_many_arguments)]
pub fn process_stream(
    addr: &str,
    output_dir: &Path,
    min_duration_ms: u32,
    gap_ms: u32,
    padding_ms: u32,
    threshold_margin: f32,
    sample_rate: u32,
    format: StreamFormat,
    verbose: bool,
    float32_output: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut reader = IqStreamReader::connect(addr, format)?;

    if verbose {
        println!("Connected to stream at {}", addr);
        println!("Sample rate: {} Hz", sample_rate);
        println!("Using FFT peak detection for wideband monitoring");
        println!("Threshold margin: +{:.0} dB above noise floor", threshold_margin);
    }

    // Streaming state machine
    let chunk_size = sample_rate as usize / 100; // 10ms chunks
    let min_duration_samples = (min_duration_ms as f32 / 1000.0 * sample_rate as f32) as usize;
    let gap_samples = (gap_ms as f32 / 1000.0 * sample_rate as f32) as usize;
    let padding_samples = (padding_ms as f32 / 1000.0 * sample_rate as f32) as usize;

    // Ring buffer for padding (stores recent samples before transmission)
    let mut pre_buffer: Vec<IqSample> = Vec::with_capacity(padding_samples);

    // Buffer for current transmission
    let mut tx_buffer: Vec<IqSample> = Vec::new();

    // FFT planner for peak detection
    let mut fft_planner = FftPlanner::new();

    // Noise floor estimation (running average of FFT peak power)
    let mut noise_floor_db: f32 = -60.0;
    let noise_alpha = 0.005; // Slower adaptation for FFT peak

    // State machine
    let mut in_transmission = false;
    let mut silence_counter = 0;
    let mut slice_counter = 0;

    println!("Listening for transmissions... (Ctrl+C to stop)");

    let mut debug_counter = 0;

    loop {
        let chunk = match reader.read_chunk(chunk_size)? {
            Some(c) => c,
            None => {
                println!("Stream closed");
                break;
            }
        };

        // Use FFT peak power detection for wideband monitoring
        let power_db = calculate_peak_power_db(&chunk, &mut fft_planner);

        // Debug: print power level every ~1 second
        debug_counter += 1;
        if verbose && debug_counter % 100 == 0 {
            let threshold = noise_floor_db + threshold_margin;
            println!("[debug] peak_power: {:.1} dB, noise_floor: {:.1} dB, threshold: {:.1} dB",
                     power_db, noise_floor_db, threshold);
        }

        // Update noise floor estimate when not in transmission
        if !in_transmission {
            noise_floor_db = noise_floor_db * (1.0 - noise_alpha) + power_db * noise_alpha;
        }

        let threshold = noise_floor_db + threshold_margin;
        let threshold_off = threshold - 3.0;

        if !in_transmission {
            // Update pre-buffer (ring buffer behavior)
            pre_buffer.extend(chunk.iter().cloned());
            if pre_buffer.len() > padding_samples {
                pre_buffer.drain(0..(pre_buffer.len() - padding_samples));
            }

            if power_db > threshold {
                // Start of transmission
                in_transmission = true;
                silence_counter = 0;
                tx_buffer.clear();

                // Add pre-buffer (padding before transmission)
                tx_buffer.extend(pre_buffer.iter().cloned());
                tx_buffer.extend(chunk);

                if verbose {
                    println!("Transmission detected (peak: {:.1} dB, threshold: {:.1} dB)", power_db, threshold);
                }
            }
        } else {
            // Currently recording
            tx_buffer.extend(chunk);

            if power_db < threshold_off {
                silence_counter += chunk_size;

                if silence_counter >= gap_samples {
                    // End of transmission
                    in_transmission = false;

                    // Check minimum duration (excluding padding)
                    let actual_duration = tx_buffer.len().saturating_sub(padding_samples);

                    if actual_duration >= min_duration_samples {
                        slice_counter += 1;

                        let filename = generate_filename(slice_counter, 0, sample_rate, Local::now());
                        let output_path = output_dir.join(&filename);

                        if float32_output {
                            write_iq_wav_float32(&output_path, &tx_buffer, sample_rate)?;
                        } else {
                            write_iq_wav(&output_path, &tx_buffer, sample_rate)?;
                        }

                        let duration_ms = tx_buffer.len() as f32 / sample_rate as f32 * 1000.0;
                        println!("Saved: {} ({:.1}ms)", filename, duration_ms);
                    } else if verbose {
                        println!("Discarded short transmission ({:.1}ms)", actual_duration as f32 / sample_rate as f32 * 1000.0);
                    }

                    tx_buffer.clear();
                }
            } else {
                // Reset silence counter if signal comes back
                silence_counter = 0;
            }
        }
    }

    // Handle any remaining transmission
    if in_transmission && tx_buffer.len() >= min_duration_samples {
        slice_counter += 1;
        let filename = generate_filename(slice_counter, 0, sample_rate, Local::now());
        let output_path = output_dir.join(&filename);

        if float32_output {
            write_iq_wav_float32(&output_path, &tx_buffer, sample_rate)?;
        } else {
            write_iq_wav(&output_path, &tx_buffer, sample_rate)?;
        }
        println!("Saved final: {} ({:.1}ms)", filename, tx_buffer.len() as f32 / sample_rate as f32 * 1000.0);
    }

    println!("Total slices saved: {}", slice_counter);
    Ok(())
}
