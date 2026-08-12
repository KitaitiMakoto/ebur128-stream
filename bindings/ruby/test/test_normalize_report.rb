require_relative "helper"

class TestNormalizeReport < Test::Unit::TestCase
  include EBUR128Stream

  def setup
    sample_rate = 48_000
    normalizer = Normalizer.new(channels: [:left, :right], sample_rate:)
    frame = 2 * Math::PI / (sample_rate / 4)
    samples = sample_rate.times.flat_map do |n|
      value = Math.cos(frame * n) * 10
    end
    @report = normalizer.normalize_in_place(samples)
  end

  def test_attributes
    assert_instance_of Float, @report.measured_integrated_lufs
    assert_instance_of Float, @report.measured_true_peak_dbtp
    assert_instance_of Float, @report.target_lufs
    assert_nil @report.true_peak_ceiling_dbtp
    assert_instance_of Float, @report.applied_gain_db
    assert_false @report.limited_by_true_peak
  end

  def test_deconstruct_keys
    deconstructed = @report.deconstruct_keys(nil)

    assert_instance_of Float, deconstructed[:measured_integrated_lufs]
    assert_instance_of Float, deconstructed[:measured_true_peak_dbtp]
    assert_instance_of Float, deconstructed[:target_lufs]
    assert_nil deconstructed[:true_peak_ceiling_dbtp]
    assert_instance_of Float, deconstructed[:applied_gain_db]
    assert_false deconstructed[:limited_by_true_peak]
  end
end
