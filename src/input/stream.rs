use std::io::{Read, BufReader};
use std::net::TcpStream;
use super::IqSample;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StreamFormat {
    Int8,
    Int16,
    Int32,
    Float32,
}

impl StreamFormat {
    pub fn bytes_per_sample(&self) -> usize {
        match self {
            StreamFormat::Int8 => 2,    // I + Q = 2 bytes
            StreamFormat::Int16 => 4,   // I + Q = 4 bytes
            StreamFormat::Int32 => 8,   // I + Q = 8 bytes
            StreamFormat::Float32 => 8, // I + Q = 8 bytes
        }
    }
}

/// Connect to SDR++ IQ Exporter via TCP
pub struct IqStreamReader {
    reader: BufReader<TcpStream>,
    format: StreamFormat,
}

impl IqStreamReader {
    pub fn connect(addr: &str, format: StreamFormat) -> Result<Self, Box<dyn std::error::Error>> {
        let stream = TcpStream::connect(addr)?;
        Ok(Self {
            reader: BufReader::new(stream),
            format,
        })
    }

    /// Read a chunk of IQ samples from the stream
    /// Returns None on connection close
    pub fn read_chunk(&mut self, num_samples: usize) -> Result<Option<Vec<IqSample>>, Box<dyn std::error::Error>> {
        let bytes_per_sample = self.format.bytes_per_sample();
        let bytes_needed = num_samples * bytes_per_sample;
        let mut buffer = vec![0u8; bytes_needed];

        match self.reader.read_exact(&mut buffer) {
            Ok(_) => {},
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
            Err(e) => return Err(e.into()),
        }

        let samples = match self.format {
            StreamFormat::Int8 => {
                buffer.chunks_exact(2).map(|chunk| {
                    let i = (chunk[0] as i8) as f32 / 128.0;
                    let q = (chunk[1] as i8) as f32 / 128.0;
                    IqSample::new(i, q)
                }).collect()
            }
            StreamFormat::Int16 => {
                buffer.chunks_exact(4).map(|chunk| {
                    let i = i16::from_le_bytes([chunk[0], chunk[1]]) as f32 / 32768.0;
                    let q = i16::from_le_bytes([chunk[2], chunk[3]]) as f32 / 32768.0;
                    IqSample::new(i, q)
                }).collect()
            }
            StreamFormat::Int32 => {
                buffer.chunks_exact(8).map(|chunk| {
                    let i = i32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]) as f32 / 2147483648.0;
                    let q = i32::from_le_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]) as f32 / 2147483648.0;
                    IqSample::new(i, q)
                }).collect()
            }
            StreamFormat::Float32 => {
                buffer.chunks_exact(8).map(|chunk| {
                    let i = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                    let q = f32::from_le_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]);
                    IqSample::new(i, q)
                }).collect()
            }
        };

        Ok(Some(samples))
    }
}
