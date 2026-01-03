use hound::{WavWriter, WavSpec, SampleFormat};
use std::path::Path;
use chrono::{DateTime, Local, Duration};
use crate::input::IqSample;

/// Write IQ samples to a WAV file (stereo int16 PCM, compatible with URH and most tools)
pub fn write_iq_wav<P: AsRef<Path>>(
    path: P,
    samples: &[IqSample],
    sample_rate: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    let spec = WavSpec {
        channels: 2,
        sample_rate,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };

    let mut writer = WavWriter::create(path, spec)?;

    for sample in samples {
        // Convert f32 [-1.0, 1.0] to i16, with some headroom
        let i = (sample.i * 32000.0).clamp(-32768.0, 32767.0) as i16;
        let q = (sample.q * 32000.0).clamp(-32768.0, 32767.0) as i16;
        writer.write_sample(i)?;
        writer.write_sample(q)?;
    }

    writer.finalize()?;
    Ok(())
}

/// Write raw IQ samples to a WAV file (stereo float32, compatible with inspectrum and SDR++)
pub fn write_iq_wav_float32<P: AsRef<Path>>(
    path: P,
    samples: &[IqSample],
    sample_rate: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    let spec = WavSpec {
        channels: 2,
        sample_rate,
        bits_per_sample: 32,
        sample_format: SampleFormat::Float,
    };

    let mut writer = WavWriter::create(path, spec)?;

    for sample in samples {
        writer.write_sample(sample.i)?;
        writer.write_sample(sample.q)?;
    }

    writer.finalize()?;
    Ok(())
}

/// Generate output filename for a slice
pub fn generate_filename(
    slice_index: usize,
    start_sample: usize,
    sample_rate: u32,
    base_time: DateTime<Local>,
) -> String {
    // Calculate timestamp offset from base time
    let offset_seconds = start_sample as f64 / sample_rate as f64;
    let offset_duration = Duration::milliseconds((offset_seconds * 1000.0) as i64);
    let slice_time = base_time + offset_duration;

    format!(
        "slice_{:03}_{}.wav",
        slice_index,
        slice_time.format("%Y-%m-%d_%H-%M-%S")
    )
}
