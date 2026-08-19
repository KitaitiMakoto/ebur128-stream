require "ebur128_stream"
require "torchaudio"
require "ndav/torch/tensor"

def main(argv)
  waveform, sample_rate = TorchAudio.load(argv.shift)

  # TorchAudio.load returns planar waveform:
  # [[L1, L2, L3, ...], [R1, R2, R3, ...]]
  # which is suitable for push_panar
  analyzer = EBUR128Stream::Analyzer.new(sample_rate:, channels: [:left, :right])
  analyzer.push_planar waveform
  report = analyzer.finalize

  puts format_report(report)
end

def format_report(report)
  template = <<~EOS
    === Report ===
    duration:        %<dur>.2f seconds
    integrated:     %<int>.2f LUFS
    LRA:             %<lra>.2f LU
    true peak:       %<tp>.2f dBTP
    momentary max:  %<mom>.2f LUFS
    short term max: %<st>.2f LUFS
  EOS
  template % {
    dur: report.programme_duration_seconds,
    int: report.integrated_lufs,
    lra: report.loudness_range_lu,
    tp:  report.true_peak_dbtp,
    mom: report.momentary_max_lufs,
    st:  report.short_term_max_lufs,
  }
end

main ARGV
