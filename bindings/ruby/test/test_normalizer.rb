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
      Math.cos(frame * n) * 10
    end
    before = samples.dup
    normalizer.normalize_in_place(samples)

    assert do
      samples != before
    end
  end

  def test_normalize_in_place_memory_view
    sample_rate = 48_000
    normalizer = Normalizer.new(channels: [:left, :right], sample_rate:)
    frame = 2 * Math::PI / (sample_rate / 4)
    samples = sample_rate.times.flat_map do |n|
      Math.cos(frame * n) * 10
    end
    samples_mv = Numo::SFloat.cast(samples)
    normalizer.normalize_in_place(samples_mv)

    assert do
      samples_mv.to_a != samples
    end

    samples2 = sample_rate.times.flat_map do |n|
      Math.cos(frame * n) * 10
    end
    normalizer2 = Normalizer.new(channels: [:left, :right], sample_rate:)
    normalizer2.normalize_in_place(samples2)
    assert_equal samples2.length, samples_mv.length
    samples_mv.each_with_index do |sample, i|
      assert_in_delta sample, samples2[i], 0.001, "at sample #{i}"
    end
  end
end
