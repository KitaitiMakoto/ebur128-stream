require_relative "helper"

class TestAnalyzer < Test::Unit::TestCase
  include EBUR128Stream

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

    test "push_interleaved MemoryView" do
    analyzer = Analyzer.new(channels: [:left, :right], modes: [:all])

    valid_data = Numo::SFloat[1.0, 1.0, 2.0, 2.0, 3.0, 3.0]
    assert_nothing_raised do
      analyzer.push_interleaved valid_data
    end

    assert_nothing_raised do
      analyzer.push_interleaved Numo::SFloat[1.0, 1.0, 2.0, 2.0, 3.0, 3.0]
    end
    assert_raise ArgumentError do
      analyzer.push_interleaved Numo::SFloat[1.0, 1.0, 2.0]
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

  test "push_planar MemoryView" do
    analyzer = Analyzer.new(channels: [:left, :right], modes: [:all])
    assert_nothing_raised do
      analyzer.push_planar Numo::SFloat[[1.0, 2.0, 3.0], [1.0, 2.0, 3.0]]
      analyzer.push_planar Numo::SFloat[[1.0, 2.0, 3.0], [1.0, 2.0, 3.0]]
    end
    assert_raise ArgumentError do
      analyzer.push_planar Numo::SFloat[[1.0, 2.0, 3.0], [1.0, 2.0, 3.0], [1.0, 2.0, 3.0]]
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

  test "reset" do
    analyzer = Analyzer.new(channels: [:left, :right], modes: [:all])
    analyzer.push_interleaved [1.0] * 48_000 * 2

    assert_equal 1.0, analyzer.snapshot.programme_duration_seconds
    analyzer.reset
    assert_equal 0.0, analyzer.snapshot.programme_duration_seconds
  end

  test "modes" do
    analyzer = Analyzer.new(channels: [:left, :right], modes: [:momentary, :integrated])

    assert_equal [:integrated, :momentary], analyzer.modes
  end

  test "push_interleaved Array and MemoryView" do
    samples = generate_samples

    analyzer_ary = Analyzer.new(channels: [:left, :right], modes: [:all])
    analyzer_ary.push_interleaved samples
    report_ary = analyzer_ary.finalize

    samples_mv = Numo::SFloat.cast(samples)
    analyzer_mv = Analyzer.new(channels: [:left, :right], modes: [:all])
    analyzer_mv.push_interleaved samples_mv
    report_mv = analyzer_mv.finalize

    assert_equal report_ary, report_mv
  end
end
