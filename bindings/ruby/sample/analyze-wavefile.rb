require "ebur128_stream"
require "wavefile"
require "pathname"
require "tempfile"

SAMPLE_RATE = 48_000
FORMAT = WaveFile::Format.new(:stereo, :float, SAMPLE_RATE)

def main(argv)
  audio_path = argv.shift || make_fixture_audio
  
  analyzer = EBUR128Stream::Analyzer.new(channels: [:left, :right], sample_rate: SAMPLE_RATE)
  WaveFile::Reader.new(audio_path).each_buffer do |buffer|
    # WaveFile::Buffer#samples returns 2-D array:
    # [[L1, R2], [L2, R2], [L3, R3], ...]
    # EBUR128Stream::Analyzer#push_interleaved requires flat array:
    # [L1, R1, L2, R2, L3, R3, ...]
    analyzer.push_interleaved buffer.samples.flatten
    snapshot = analyzer.snapshot

    puts format_snapshot(snapshot)
  end
  report = analyzer.finalize

  puts
  puts format_report(report)
end

def format_snapshot(snapshot)
  template = "[%<dur>f] momentary: %{mom}, short term: %{st}, integrated: %{int}, true peak: %{tp}"
  template % {
    dur: snapshot.programme_duration_seconds,
    mom: (snapshot.momentary_lufs&.to_s  || "N/A")[..3],
    st:  (snapshot.short_term_lufs&.to_s || "N/A")[..3],
    int: (snapshot.integrated_lufs&.to_s || "N/A")[..3],
    tp:  (snapshot.true_peak_dbtp&.to_s  || "N/A")[..3],
  }
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

def make_fixture_audio
  file = Tempfile.new(["", ".wav"])

  freq = 440
  length = 5 # seconds
  WaveFile::Writer.new file.to_path, FORMAT do |writer|
    (SAMPLE_RATE * length).times do |n|
      phase = 2 * Math::PI * n / (SAMPLE_RATE / freq)
      value = Math.sin(phase)
      buffer = WaveFile::Buffer.new([[value, value]], FORMAT) # left, right
      writer.write buffer
    end
  end

  file.to_path
end

main ARGV
