# frozen_string_literal: true

$LOAD_PATH.unshift File.expand_path("../lib", __dir__)
require "ebur128_stream"

require "test-unit"
require "numo/narray/alt"
require "ndav/numo/narray"

class Test::Unit::TestCase
  def generate_samples
    sample_rate = 48_000
    frame = 2 * Math::PI / (sample_rate / 4)
    sample_rate.times.flat_map do |n|
      Math.cos(frame * n) * 10
    end
  end
end
