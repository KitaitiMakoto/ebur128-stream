require_relative "helper"

class TestReport < Test::Unit::TestCase
  def setup
    sample_rate = 48_000
    analyzer = EBUR128Stream::Analyzer.new(channels: [:left, :right], sample_rate:)
    frame = 2 * Math::PI / (sample_rate / 4)
    sample_rate.times do |n|
      value = Math.cos(frame * n) * 10
      analyzer.push_interleaved [value, value]
    end
    @report = analyzer.finalize
  end

  def test_attributes
    assert_instance_of Float, @report.integrated_lufs
    assert_nil @report.loudness_range_lu
    assert_instance_of Float, @report.true_peak_dbtp
    assert_instance_of Float, @report.momentary_max_lufs
    assert_nil @report.short_term_max_lufs
    assert_equal 1.0, @report.programme_duration_seconds
  end

  def test_deconstruct_keys
    deconstructed = @report.deconstruct_keys(nil)

    assert_instance_of Float, deconstructed[:integrated_lufs]
    assert_nil deconstructed[:loudness_range_lu]
    assert_instance_of Float, deconstructed[:true_peak_dbtp]
    assert_instance_of Float, deconstructed[:momentary_max_lufs]
    assert_nil deconstructed[:short_term_max_lufs]
    assert_equal 1.0, deconstructed[:programme_duration_seconds]
  end
end
