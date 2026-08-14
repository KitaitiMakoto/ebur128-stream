# frozen_string_literal: true

require_relative "helper"

class EBUR128StreamTest < Test::Unit::TestCase
  include EBUR128Stream

  test "VERSION" do
    assert do
      ::EBUR128Stream.const_defined?(:VERSION)
    end
  end

  test "new" do
    assert_raise ArgumentError do
      Analyzer.new
    end
    assert_instance_of Analyzer, Analyzer.new(channels: [:left])
    assert_raise ArgumentError do
      Analyzer.new(channels: [:left, :nothing])
    end
    assert_instance_of Analyzer, Analyzer.new(channels: [:center], sample_rate: 48_000)
    assert_raise RuntimeError do
      Analyzer.new(channels: [:center], modes: [])
    end
    assert_instance_of Analyzer, Analyzer.new(channels: [:center], modes: [:integrated, :true_peak])
    assert_raise ArgumentError do
      Analyzer.new(channels: [:center], modes: [:unknown])
    end
  end

  test "push_interleaved" do
    analyzer = Analyzer.new(channels: [:left, :right], modes: [:all])
    assert_nothing_raised do
      analyzer.push_interleaved [1.0, 1.0, 2.0, 2.0, 3.0, 3.0]
    end
    assert_raise ArgumentError do
      analyzer.push_interleaved [1.0, 1.0, 2.0]
    end
  end

  test "push_planar" do
    analyzer = Analyzer.new(channels: [:left, :right], modes: [:all])
    assert_nothing_raised do
      analyzer.push_planar [[1.0, 2.0, 3.0], [1.0, 2.0, 3.0]]
      analyzer.push_planar [[1.0, 2.0, 3.0], [1.0, 2.0, 3.0]]
    end
    assert_raise ArgumentError do
      analyzer.push_planar [[1.0, 2.0, 3.0], [1.0, 2.0, 3.0], [1.0, 2.0, 3.0]]
    end
  end

  test "finalize" do
    analyzer = Analyzer.new(channels: [:left, :right], modes: [:all])
    analyzer.push_interleaved [1.0, 1.0, 2.0, 2.0, 3.0, 3.0]
    report = nil
    assert_nothing_raised do
      report = analyzer.finalize
    end
    assert_instance_of Report, report
    assert_raise_with_message RuntimeError, /finalized/ do
      analyzer.finalize
    end
  end
end
