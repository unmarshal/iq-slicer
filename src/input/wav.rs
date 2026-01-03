use hound::{WavReader, SampleFormat};
use std::path::Path;
use super::{IqSample, IqMetadata};

/// Read IQ samples from an SDR++ WAV file
/// SDR++ saves IQ as stereo float32: I=left channel, Q=right channel
pub fn read_iq_wav<P: AsRef<Path>>(path: P) -> Result<(Vec<IqSample>, IqMetadata), Box<dyn std::error::Error>> {
    let reader = WavReader::open(path)?;
    let spec = reader.spec();

    // Validate format
    if spec.channels != 2 {
        return Err(format!("Expected stereo WAV (I/Q), got {} channels", spec.channels).into());
    }

    let metadata = IqMetadata {
        sample_rate: spec.sample_rate,
        total_samples: Some(reader.len() as usize / 2), // stereo samples
    };

    let samples = match spec.sample_format {
        SampleFormat::Float => read_float_samples(reader)?,
        SampleFormat::Int => read_int_samples(reader, spec.bits_per_sample)?,
    };

    Ok((samples, metadata))
}

fn read_float_samples(mut reader: WavReader<std::io::BufReader<std::fs::File>>) -> Result<Vec<IqSample>, Box<dyn std::error::Error>> {
    let mut samples = Vec::new();
    let mut iter = reader.samples::<f32>();

    while let (Some(i_result), Some(q_result)) = (iter.next(), iter.next()) {
        let i = i_result?;
        let q = q_result?;
        samples.push(IqSample::new(i, q));
    }

    Ok(samples)
}

fn read_int_samples(mut reader: WavReader<std::io::BufReader<std::fs::File>>, bits: u16) -> Result<Vec<IqSample>, Box<dyn std::error::Error>> {
    let mut samples = Vec::new();
    let max_val = (1i32 << (bits - 1)) as f32;

    let mut iter = reader.samples::<i32>();

    while let (Some(i_result), Some(q_result)) = (iter.next(), iter.next()) {
        let i = i_result? as f32 / max_val;
        let q = q_result? as f32 / max_val;
        samples.push(IqSample::new(i, q));
    }

    Ok(samples)
}
