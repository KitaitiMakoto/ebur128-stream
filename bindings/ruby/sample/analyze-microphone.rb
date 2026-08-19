require "ebur128_stream"
require "gstreamer"
require "numo/narray/alt"
require "ndav/numo/narray"
require "io/console"

CHANNELS = [:left, :right]
RATE = 48_000
WIDTH = $stderr.winsize[1]
LIMIT = -70

include NDAV::Converter

def main(argv)
  analyser = setup_ebur128_stream
  setup_gstreamer do |sample|
    # GStreamer's Gst::Sample is 2-D but EBUR128Stream requires 1-D.
    # Reshapes it using Numo::NArray
    samples = NumoNArray(sample)
    samples.reshape!(*samples.shape.reduce(:*))

    analyser.push_interleaved samples
    analyser.snapshot => {momentary_lufs:}
    next unless momentary_lufs

    len = (-LIMIT + momentary_lufs) * (-WIDTH / LIMIT)
    volume = "|" * len
    padding = " " * (WIDTH - len)
    print "\r#{volume}#{padding}"
  end
end

def setup_ebur128_stream
  EBUR128Stream::Analyzer.new(channels: CHANNELS, sample_rate: RATE, modes: [:momentary])
end

def setup_gstreamer
  pipeline = Gst::Pipeline.new("ebur128-stream")
  src = Gst::ElementFactory.make("autoaudiosrc", nil)
  convert = Gst::ElementFactory.make("audioconvert", nil)
  resample = Gst::ElementFactory.make("audioresample", nil)
  sink = Gst::ElementFactory.make("appsink", nil)

  caps = Gst::Caps.new("audio/x-raw")
  caps["format"] = "F32LE" # F32 doesn't work for macOS
  caps["rate", :int] = RATE
  caps["channels", :int] = CHANNELS.length
  caps["layout"] = "interleaved"
  sink.caps = caps

  sink.emit_signals = true
  sink.signal_connect :new_sample do |_|
    begin
      yield sink.pull_sample
    rescue => err
      $stderr.puts err
    end
    Gst::FlowReturn::OK
  end

  pipeline << src << convert << resample << sink
  src >> convert >> resample >> sink

  loop = GLib::MainLoop.new

  bus = pipeline.bus
  bus.add_watch do |bus, message|
    case message.type
    when Gst::MessageType::EOS
      loop.quit
    when Gst::MessageType::ERROR
      error, debug = message.parse_error
      $stderr.puts error
      $stderr.puts debug
      loop.quit
    end
    true
  end

  pipeline.play
  begin
    loop.run
  rescue Interrupt
    pp :Interrupt
  rescue err
    $stderr.puts err
  ensure
    pipeline.stop
    GC.start
  end

end

main ARGV
