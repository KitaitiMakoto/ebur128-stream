require "ebur128_stream"
require "torchaudio"
require "ndav/torch/tensor"

def main(argv)
  input = argv.shift
  output = argv.shift
  unless output
    abort "Usage: ruby #{$PROGRAM_NAME} INPUT OUTPUT"
  end
  
  waveform, sample_rate = TorchAudio.load(input)

  # TorchAudio returns a planar samples
  # but, EBUR128Stream requires an interleaved samples for normalization.
  # We need to reshape the waveform.
  samples = waveform.transpose(1, 0)
  shape = samples.shape

  # Currently 2-D array. Reshapes to 1-D.
  samples = samples.reshape(shape.reduce(&:*))

  normalizer = EBUR128Stream::Normalizer.new(
    sample_rate:,
    channels: [:left, :right],
    target_lufs: -14.0
  )
  normalize_report = normalizer.normalize_in_place(samples)

  # Restores the samples to planar layout
  out_samples = samples.reshape(shape).transpose(1, 0)
  TorchAudio.save(output, samples, sample_rate)
end

main ARGV
