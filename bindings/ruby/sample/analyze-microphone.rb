require "ebur128_stream"
require "gstreamer"
require "numo/narray/alt"
require "ndav/numo/narray"
require "io/console"

CHANNELS = [:left, :right]
RATE = 48_000
HEIGHT = $stdout.winsize[0]
WIDTH = $stdout.winsize[1]
PADDING_BLOCK_START = HEIGHT / 2 - 2
PADDING_INLINE = WIDTH / 5
AREA_WIDTH = WIDTH - PADDING_INLINE * 2
LIMIT = -70

include NDAV::Converter

def main(argv)
  analyser = setup_ebur128_stream

  print "\e[2J"

  setup_gstreamer do |sample|
    # GStreamer's Gst::Sample is 2-D but EBUR128Stream requires 1-D.
    # Reshapes it using Numo::NArray
    samples = NumoNArray(sample)
    samples.reshape!(*samples.shape.reduce(:*))

    analyser.push_interleaved samples
    analyser.snapshot => {momentary_lufs:}
    next unless momentary_lufs

    render_loudness momentary_lufs
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

def render_loudness(loudness)
  len = (-LIMIT + loudness) * (-AREA_WIDTH / LIMIT)
  volume = "|" * len
  ws = " " * (AREA_WIDTH - len)
  print "\e[#{PADDING_BLOCK_START};#{PADDING_INLINE}H"
  print "\e[#{PADDING_INLINE}G"
  puts "#{volume}#{ws}"
  puts
  digits = "%.3f" % loudness
  print "\e[#{WIDTH - PADDING_INLINE - digits.to_s.length}G"
  print digits
end

main ARGV
