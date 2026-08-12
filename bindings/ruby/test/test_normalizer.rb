require_relative "helper"

class TestNormalizer < Test::Unit::TestCase
  include EBUR128Stream

  def test_new
    assert_instance_of Normalizer, Normalizer.new(sample_rate: 48_000, channels: [:left, :right])
  end
end
