use clap::{Parser, Subcommand, Args as ClapArgs, ValueEnum};
use std::path::PathBuf;

mod input;
mod detector;
mod output;
mod slicer;

/// Automatically detect and slice transmissions from IQ recordings
#[derive(Parser, Debug)]
#[command(name = "iq-slicer")]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Process a WAV file
    File(FileArgs),
    /// Connect to SDR++ Network Sink for live streaming
    Stream(StreamArgs),
}

/// Output WAV sample format
#[derive(ValueEnum, Clone, Debug)]
enum OutputFormat {
    /// 16-bit integer (for URH)
    Int16,
    /// 32-bit float (for inspectrum)
    Float32,
}

/// Input stream sample format
#[derive(ValueEnum, Clone, Debug)]
enum InputFormat {
    /// 8-bit signed integer
    Int8,
    /// 16-bit signed integer
    Int16,
    /// 32-bit signed integer
    Int32,
    /// 32-bit float
    Float32,
}

/// Common options for both file and stream modes
#[derive(ClapArgs, Debug)]
struct CommonArgs {
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

    /// Verbose output
    #[arg(short, long)]
    verbose: bool,

    /// Output WAV sample format
    #[arg(long, value_enum, default_value_t = OutputFormat::Int16)]
    output_format: OutputFormat,
}

#[derive(ClapArgs, Debug)]
struct FileArgs {
    /// Input WAV file to process
    #[arg(value_name = "INPUT")]
    input_file: PathBuf,

    #[command(flatten)]
    common: CommonArgs,
}

#[derive(ClapArgs, Debug)]
struct StreamArgs {
    /// Host and port to connect to (e.g., localhost:5555)
    #[arg(value_name = "HOST:PORT")]
    address: String,

    /// Sample rate (Hz)
    #[arg(short, long, default_value = "48000")]
    rate: u32,

    /// Threshold margin above noise floor in dB
    #[arg(long, default_value = "15")]
    margin: f32,

    /// Input stream sample format
    #[arg(long, value_enum, default_value_t = InputFormat::Float32)]
    input_format: InputFormat,

    #[command(flatten)]
    common: CommonArgs,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Command::File(args) => {
            std::fs::create_dir_all(&args.common.output_dir)?;
            let output_float32 = matches!(args.common.output_format, OutputFormat::Float32);
            if args.common.verbose {
                println!("Processing file: {}", args.input_file.display());
            }
            slicer::process_file(
                &args.input_file,
                &args.common.output_dir,
                args.common.min_duration,
                args.common.max_duration,
                args.common.gap,
                args.common.padding,
                args.common.verbose,
                output_float32,
            )?;
        }
        Command::Stream(args) => {
            std::fs::create_dir_all(&args.common.output_dir)?;
            let format = match args.input_format {
                InputFormat::Int8 => input::StreamFormat::Int8,
                InputFormat::Int16 => input::StreamFormat::Int16,
                InputFormat::Int32 => input::StreamFormat::Int32,
                InputFormat::Float32 => input::StreamFormat::Float32,
            };
            let output_float32 = matches!(args.common.output_format, OutputFormat::Float32);
            if args.common.verbose {
                println!("Connecting to stream: {} (input: {:?})", args.address, args.input_format);
            }
            slicer::process_stream(
                &args.address,
                &args.common.output_dir,
                args.common.min_duration,
                args.common.gap,
                args.common.padding,
                args.margin,
                args.rate,
                format,
                args.common.verbose,
                output_float32,
            )?;
        }
    }

    Ok(())
}
