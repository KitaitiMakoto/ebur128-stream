# frozen_string_literal: true

require_relative "helper"

class EBUR128StreamTest < Test::Unit::TestCase
  test "VERSION" do
    assert do
      ::EBUR128Stream.const_defined?(:VERSION)
    end
  end

  test "new" do
    assert_raise ArgumentError do
      EBUR128Stream::Analyzer.new
    end
    assert_instance_of EBUR128Stream::Analyzer, EBUR128Stream::Analyzer.new(channels: [:left])
    assert_raise ArgumentError do
      EBUR128Stream::Analyzer.new(channels: [:left, :nothing])
    end
    assert_instance_of EBUR128Stream::Analyzer, EBUR128Stream::Analyzer.new(channels: [:center], sample_rate: 48_000)
    assert_raise RuntimeError do
      EBUR128Stream::Analyzer.new(channels: [:center], modes: [])
    end
    assert_instance_of EBUR128Stream::Analyzer, EBUR128Stream::Analyzer.new(channels: [:center], modes: [:integrated, :true_peak])
    assert_raise ArgumentError do
      EBUR128Stream::Analyzer.new(channels: [:center], modes: [:unknown])
    end
  end

  test "push_interleaved" do
    analyzer = EBUR128Stream::Analyzer.new(channels: [:left, :right], modes: [:all])
    assert_nothing_raised do
      analyzer.push_interleaved [1.0, 1.0, 2.0, 2.0, 3.0, 3.0]
    end
  end

  test "finalize" do
    analyzer = EBUR128Stream::Analyzer.new(channels: [:left, :right], modes: [:all])
    analyzer.push_interleaved [1.0, 1.0, 2.0, 2.0, 3.0, 3.0]
    report = nil
    assert_nothing_raised do
      report = analyzer.finalize
    end
    assert_instance_of EBUR128Stream::Report, report
    assert_raise_with_message RuntimeError, /finalized/ do
      analyzer.finalize
    end
  end
end
