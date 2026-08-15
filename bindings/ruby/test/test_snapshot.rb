require_relative "helper"

class TestSnapshot < Test::Unit::TestCase
  include EBUR128Stream

  def setup
    sample_rate = 48_000
    analyzer = Analyzer.new(channels: [:left, :right], sample_rate:)
    frame = 2 * Math::PI / (sample_rate / 4)
    sample_rate.times do |n|
      value = Math.cos(frame * n) * 10
      analyzer.push_interleaved [value, value]
    end
    @snapshot = analyzer.snapshot
  end

  def test_attributes
    assert_instance_of Float, @snapshot.momentary_lufs
    assert_nil @snapshot.short_term_lufs
    assert_instance_of Float, @snapshot.integrated_lufs
    assert_nil @snapshot.loudness_range_lu
    assert_instance_of Float, @snapshot.true_peak_dbtp
    assert_equal 1.0, @snapshot.programme_duration_seconds
  end

  def test_deconstruct_keys
    deconstructed = @snapshot.deconstruct_keys(nil)

    assert_instance_of Float, deconstructed[:momentary_lufs]
    assert_nil deconstructed[:short_term_lufs]
    assert_instance_of Float, deconstructed[:integrated_lufs]
    assert_nil deconstructed[:loudness_range_lu]
    assert_instance_of Float, deconstructed[:true_peak_dbtp]
    assert_equal 1.0, deconstructed[:programme_duration_seconds]
  end
end