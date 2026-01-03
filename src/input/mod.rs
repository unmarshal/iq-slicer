pub mod wav;
pub mod stream;

pub use stream::StreamFormat;

/// IQ sample pair (In-phase, Quadrature)
#[derive(Debug, Clone, Copy)]
pub struct IqSample {
    pub i: f32,
    pub q: f32,
}

impl IqSample {
    pub fn new(i: f32, q: f32) -> Self {
        Self { i, q }
    }
}

/// Metadata about the IQ source
#[derive(Debug, Clone)]
pub struct IqMetadata {
    pub sample_rate: u32,
    #[allow(dead_code)]
    pub total_samples: Option<usize>, // None for streams
}
