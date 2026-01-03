use clap::Parser;
use std::path::PathBuf;

mod input;
mod detector;
mod output;
mod slicer;

/// Automatically detect and slice transmissions from IQ recordings
#[derive(Parser, Debug)]
#[command(name = "iq-slicer")]
#[command(version, about, long_about = None)]
struct Args {
    /// Input WAV file to process
    #[arg(value_name = "INPUT")]
    input_file: Option<PathBuf>,

    /// Connect to SDR++ Network Sink (TCP) for live streaming
    #[arg(short, long, value_name = "HOST:PORT")]
    stream: Option<String>,

    /// Output directory for sliced audio files
    #[arg(short, long, default_value = "./slices")]
    output_dir: PathBuf,

    /// Minimum transmission duration in milliseconds
    #[arg(short, long, default_value = "500")]
    min_duration: u32,

    /// Maximum transmission duration in milliseconds (filter out long noise)
    #[arg(short = 'M', long)]
    max_duration: Option<u32>,

    /// Maximum gap to merge transmissions in milliseconds
    #[arg(short, long, default_value = "200")]
    gap: u32,

    /// Padding before/after each slice in milliseconds
    #[arg(short, long, default_value = "100")]
    padding: u32,

    /// Sample rate for stream mode (Hz)
    #[arg(short, long, default_value = "48000")]
    rate: u32,

    /// Threshold margin above noise floor in dB (stream mode auto-threshold)
    #[arg(long, default_value = "15")]
    margin: f32,

    /// Stream format: int8, int16, int32, float32
    #[arg(long, default_value = "float32")]
    format: String,

    /// Verbose output
    #[arg(short, long)]
    verbose: bool,

    /// Output float32 WAV (for inspectrum) instead of int16 (for URH)
    #[arg(long)]
    float32: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // Validate input mode
    if args.input_file.is_none() && args.stream.is_none() {
        eprintln!("Error: Must specify either an input file or --stream <host:port>");
        std::process::exit(1);
    }
    if args.input_file.is_some() && args.stream.is_some() {
        eprintln!("Error: Cannot specify both input file and --stream");
        std::process::exit(1);
    }

    // Create output directory if it doesn't exist
    std::fs::create_dir_all(&args.output_dir)?;

    if let Some(input_path) = &args.input_file {
        // File mode
        if args.verbose {
            println!("Processing file: {}", input_path.display());
        }
        slicer::process_file(
            input_path,
            &args.output_dir,
            args.min_duration,
            args.max_duration,
            args.gap,
            args.padding,
            args.verbose,
            args.float32,
        )?;
    } else if let Some(stream_addr) = &args.stream {
        // Stream mode
        let format = match args.format.to_lowercase().as_str() {
            "int8" => input::StreamFormat::Int8,
            "int16" => input::StreamFormat::Int16,
            "int32" => input::StreamFormat::Int32,
            "float32" => input::StreamFormat::Float32,
            _ => {
                eprintln!("Error: Invalid format '{}'. Use: int8, int16, int32, float32", args.format);
                std::process::exit(1);
            }
        };
        if args.verbose {
            println!("Connecting to stream: {} (format: {:?})", stream_addr, format);
        }
        slicer::process_stream(
            stream_addr,
            &args.output_dir,
            args.min_duration,
            args.gap,
            args.padding,
            args.margin,
            args.rate,
            format,
            args.verbose,
            args.float32,
        )?;
    }

    Ok(())
}
