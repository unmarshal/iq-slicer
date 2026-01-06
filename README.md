# iq-slicer

Automatic transmission detection and slicing for IQ recordings. Monitors wideband spectrum and captures individual signal bursts to separate files.

## Features

- **Live streaming** from SDR++ IQ Exporter via TCP
- **File processing** for WAV IQ recordings
- **FFT peak detection** for wideband monitoring - catches narrowband bursts in wide spectrum
- **Auto-threshold** adapts to noise floor automatically

## Installation

```bash
cargo build --release
```

## Usage

### Stream Mode (Live)

Connect to SDR++ IQ Exporter for real-time burst capture:

```bash
# Basic streaming - captures any bursts detected
iq-slicer stream 127.0.0.1:4532 -r 4000000 --output-format float32

# With verbose output to see detection in action
iq-slicer stream 127.0.0.1:4532 -r 4000000 --output-format float32 -v

# Tuned for short bursts (garage doors, key fobs)
iq-slicer stream 127.0.0.1:4532 -r 4000000 -m 50 -g 200 -p 20 --output-format float32

# Tuned for smart meters (short FSK bursts)
iq-slicer stream 127.0.0.1:4532 -r 4000000 -m 5 -g 20 -p 10 --output-format float32
```

### File Mode

Process existing IQ WAV recordings:

```bash
# Slice an IQ recording into separate files
iq-slicer file recording.wav -o ./slices

# Float32 output for inspectrum
iq-slicer file recording.wav -o ./slices --output-format float32
```

### SDR++ Setup

1. In SDR++, open **Module Manager**
2. Add **IQ Exporter** module
3. Set: Mode: **Baseband**, Protocol: **TCP (Server)**, Port: **4532**
4. Set sample type to match `--input-format` (float32/int16/int8/int32)
5. Click **Start**
6. Run iq-slicer with `iq-slicer stream 127.0.0.1:4532`

## Options

### Common Options (both modes)

```
  -o, --output-dir <DIR>             Output directory [default: ./slices]
  -m, --min-duration <MS>            Minimum burst duration [default: 500]
  -M, --max-duration <MS>            Maximum burst duration (filter noise)
  -g, --gap <MS>                     Max gap to merge bursts [default: 200]
  -p, --padding <MS>                 Padding before/after slice [default: 100]
  -v, --verbose                      Show detection details
      --output-format <FORMAT>       Output WAV format: int16 (URH) or float32 (inspectrum) [default: int16]
```

### Stream-only Options

```
  -r, --rate <HZ>                    Sample rate [default: 48000]
      --margin <DB>                  Threshold margin above noise floor [default: 15]
      --input-format <FORMAT>        Input format: int8/int16/int32/float32 [default: float32]
```

## Output Formats

- **int16** (default): Int16 stereo WAV (I=left, Q=right) - compatible with URH
- **float32**: Float32 stereo WAV - compatible with inspectrum, SDR++

## How It Works

1. **FFT Peak Detection**: Computes FFT of each chunk with Hann window and 50% overlap, finding the strongest frequency bin. This catches narrowband signals anywhere in the monitored bandwidth with reduced spectral leakage.

2. **Adaptive Threshold**: Tracks noise floor with exponential moving average. Triggers when peak power exceeds `noise_floor + margin`.

3. **Hysteresis**: Uses 3dB hysteresis to avoid chattering at threshold boundary.

4. **Segment Merging**: Bursts separated by less than `--gap` are merged into single files.

## Example Workflows

### Smart Meter Monitoring (900 MHz ISM)
```bash
# Tune SDR++ to 912 MHz, 4 MHz bandwidth
iq-slicer stream 127.0.0.1:4532 -r 4000000 -m 5 -g 20 -p 10 --margin 15 --output-format float32 -v
# Analyze in inspectrum - look for FSK bursts
```

### Garage Door / Key Fob (315 MHz)
```bash
# Tune SDR++ to 315 MHz, 2-4 MHz bandwidth
iq-slicer stream 127.0.0.1:4532 -r 4000000 -m 50 -g 200 -p 20 --margin 15 --output-format float32
# Captures each button press as separate file
```

### Post-Processing Recording
```bash
# Slice an existing IQ recording
iq-slicer file recording.wav -o ./analysis --output-format float32
# Opens slices in inspectrum for symbol analysis
```

## License

MIT
