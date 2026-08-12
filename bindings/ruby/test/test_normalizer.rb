require_relative "helper"

class TestNormalizer < Test::Unit::TestCase
  include EBUR128Stream

  def test_new
    assert_instance_of Normalizer, Normalizer.new(sample_rate: 48_000, channels: [:left, :right])
  end

  def test_normalize_in_place
    sample_rate = 48_000
    normalizer = Normalizer.new(channels: [:left, :right], sample_rate:)
    frame = 2 * Math::PI / (sample_rate / 4)
    samples = sample_rate.times.flat_map do |n|
      value = Math.cos(frame * n) * 10
    end
    before = samples.dup
    normalizer.normalize_in_place(samples)

    assert do
      samples != before
    end
  end
end
