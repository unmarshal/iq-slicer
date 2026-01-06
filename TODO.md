# TODO

## Frequency metadata

Currently iq-slicer captures bursts but doesn't record frequency information:

- **Center frequency**: What the SDR was tuned to
- **Peak bin offset**: Where in the bandwidth the signal appeared

For wideband captures this matters. If you're monitoring 10 MHz and catch a burst, you want to know it was at +2.3 MHz offset from center.

### Proposed implementation

1. Add `--center-freq <Hz>` flag to specify tuned frequency
2. During detection, record which FFT bin had the peak
3. Compute actual frequency: `center_freq + (peak_bin - fft_size/2) * bin_width`
4. Output options:
   - Embed in filename: `slice_001_390.1MHz_2026-01-03_14-23-01.wav`
   - SigMF sidecar file (`.sigmf-meta`) with full metadata
   - Both

## Multi-signal detection with bandpass filtering

Currently, if multiple signals transmit simultaneously at different frequencies, they all end up in the same output file.

Better behavior: detect multiple peaks, bandpass filter each, output separate files.

### Proposed implementation

1. Find all peaks above threshold (not just the max)
2. Cluster nearby bins to identify distinct signals
3. For each detected signal:
   - Apply bandpass filter centered on peak frequency
   - Configurable filter width (e.g., `--bandpass-width 25000` for 25 kHz)
   - Output to separate file with frequency in filename
4. Handle overlapping time slices (same timestamp, different frequencies)
5. Optional bandwidth filtering: `--signal-width 20000` to only capture signals ~20 kHz wide, ignoring wider/narrower transmissions
