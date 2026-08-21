# EBUR128Stream

A Ruby binding for [ebur128-stream][rust-impl], a streaming, zero-allocation EBU R128 loudness measurement in pure Rust.

## Installation

Install the gem and add to the application's Gemfile by executing:

```bash
bundle add ebur128_stream
```

If bundler is not being used to manage dependencies, install the gem by executing:

```bash
gem install ebur128_stream
```

## Usage

```ruby
require "ebur128_stream"

include EBUR128Stream

analyzer = Analyzer.new(
  channels: [:left, :right],
  sample_rate: 48_000,
  modes: [:integrated, :true_peak]
)
analyzer.push_interleaved samples

report = analyzer.finalize
report.integrated_lufs # => -8.030723453035735
report.true_peak_dbtp  # => 20.01377283514713
```

EBUR128Stream provides two main classes:

* [Analyzer](#analyzer) analyzes audio data and reports loudness.
* [Normalizer](#normalizer) analyzes audio data and applies the calibrated gain.

<a name="analyzer"></a>
### Analyzer

EBUR128Stream::Analyzer is a streaming EBU R128 loudness analyzer. It reads audio data and reports statistics such as integrated LUFS, dBTP.

It provides a streaming API, which means you can push chunks of audio data incrementally and get the current status (EBUR128Stream::Snapshot) and the final report (EBUR128Stream::Report).

### Initialization

First, initialize EBUR128Stream::Analyzer:

```ruby
analyzer = EBUR128Stream::Analyzer.new(
  # Required. Supports :left, :right, :center, :left_surround, :right_surround, :lfe, :other.
  channels: [:left, :right],

  # Optional. Must be one of 22_050, 32_000, 44_100, 48_000, 88_200, 96_000, 192_000. Defaults to 48_000.
  sample_rate: 48_000,

  # Optional. Supports :integrated, :momentary, :short_term, :true_peak, :lra, :all. Defaults to [:all].
  modes: [:all],

  # Optional. Hint at audio length in seconds so that buffer is allocated first.
  expected_duration: 60
)
```

See [Rust documentation](https://docs.rs/ebur128-stream/latest/ebur128_stream/struct.AnalyzerBuilder.html) for details on arguments.

### Pushing samples

Then, push audio samples to EBUR128Stream::Analyzer#push_interleaved or EBUR128Stream::Analyzer#push_planar as you get them:

```ruby
while samples = get_samples
  analyzer.push_interleaved(samples)
  # Or, analyzer.push_planar samples
end
```

Interleaved samples looks like this:

```ruby
[L1, R1, L2, R2, L3, R3, ...]
```

(*Note* that it's not a 2-D array (`[[L1, R1], [L2, R2], ...]`) but a flat array.)

Planar samples looks like this:

```ruby
[[L1, L2, L3, ...], [R1, R2, R3, ...]]
```

In addition to 1-D or 2-D `Array`s, EBUR128Stream::Analyzer#push_interleaved and EBUR128Stream::Analyzer#push_planar accept MemoryView producers such as:

* `Gst::Sample` from [GStreamer][] gem
* `Torch::Tensor` from [TorchAudio][] or [TorchCodec][] gem (w/ [NDAV::TorchTensor][])
* `Numo::NArray` from [Numo::NArray][] or [Numo::NArray Alternative][] gem when generating and processing audio data with it (w/ [NDAV::Numo::NArray][])

TorchAudio example here:

```ruby
samples, sample_rate = TorchAudio.load("path/to/audio")
analyzer.push_planar(samples)
```

### Snapshots

At any point during streaming, you can get the current statistics (EBUR128Stream::Snapshot) by calling EBUR128Stream::Analyzer#snapshot:

```ruby
snapshot = analyzer.snapshot

snapshot.programme_duration_seconds #=> Current duration
snapshot.momentary_lufs             #=> Loudness sliding 400ms window in LUFS
snapshot.short_term_lufs            #=> Loudness sliding 3s window in LUFS
snapshot.integrated_lufs            #=> Loudness so far in LUFS
snapshot.true_peak_dbtp             #=> True peak so far in dBTP
snapshot.loudness_range_lu          #=> Loudness range in LU
```

The attributes may be `nil` when there are not enough samples or when the corresponding mode was not specified at initialization.

EBUR128Stream::Snapshot implements `#to_h`:

```ruby
snapshot.to_h #=> {momentary_lufs: ..., short_term_lufs: ..., ...}
```

Also, `#deconstruct_keys` is implemented:

```ruby
snapshot.deconstruct_keys(nil) #=> {momentary_lufs: ..., short_term_lufs: ..., ...}

case analyzer.snapshot
in EBUR128Stream::Snapshot[true_peak_dbtp: 1.0..] => snapshot
  $stderr.puts "High true peak found at #{snapshot.programme_duration_seconds}s: #{snapshot.true_peak_dbtp} dBTP"
else
  # noop
end
```

### Finalization

Finally, call EBUR128Stream::Analyzer#finalize to complete the analysis and get the report:

```ruby
report = analyzer.finalize

report.programme_duration_seconds #=> Total duration in seconds
report.integrated_lufs
report.loudness_range_lu
report.true_peak_dbtp
report.momentary_max_lufs  #=> Maximum momentary loudness in LUFS
report.short_term_max_lufs #=> Maximum short term loudness in LUFS
```

EBUR128Stream::Report also implements `#to_h` and `#deconstruct_keys` like EBUR128Stream::Snapshot.

<a name="normalizer"></a>
### Normalizer

EBUR128Stream::Normalizer analyzes audio data and normalizes it to the target loudness in place.

#### Initialization

Initialize EBUR128Stream::Normalizer with target loudness.

```ruby
normalizer = EBUR128Stream::Normalizer.new(
  # Required.
  sample_rate: 48_000,

  # Required.
  channels: [:left, :right],

  # Optional. Target loudness in LUFS. Defaults to -23.0.
  target_lufs: -14.0,

  # Optional. Cap the post-normalisation true peak at this dBTP value.
  true_peak_ceiling_dbtp: -1.0,
)
```

#### Normalization

Pass interleaved samples to EBUR128Stream::Normalizer#normalize_in_place:

```ruby
report = normalizer.normalize_in_place(interleaved_samples)
```

*Note* that it modifies the passed `Array` in place as the method name implies.

This returns EBUR128Stream::NormalizeReport:

```ruby
report.measured_integrated_lufs #=> Measured integrated loudness before normalization.
report.measured_true_peak_dbtp  # Measured true peak before normalization, in dBTP.
report.target_lufs              #=> Target loudness, in LUFS.
report.true_peak_ceiling_dbtp   #=> True peak ceiling in dBTP.
report.applied_gain_db          #=> Gain that was actually applied in dB.
report.limited_by_true_peak     #=> true if the gain was attenuated to honour the true peak ceiling.
```

EBUR128Stream::NormalizeReport also implements `#to_h` and `deconstruct_keys`.

Additionally, it accepts MemoryView as a `samples` argument. The MemoryView must be writable.

## Examples

Complete examples are available in the sample directory.

* sample/analyze-wavefile.rb - A basic example of analyzing audio from wave file using pure-Ruby [WaveFile][] gem.
* sample/analyze-microphone.rb - An example that displays microphone loudness in real time.
* sample/analyze-planar-data.rb - An example of analyzing planar audio data instead of interleaved data.
* sample/normalize.rb - An example of normalizing audio data to desired loudness and saving it to a file.

## Development

After checking out the repo, run `bundle install` to install dependencies. Then, run `bundle exec rake test` to run the tests. You can also run `bundle exec rake console` for an interactive prompt that will allow you to experiment.

To release a new version, update the version number in `version.rb`, and then run `bundle exec rake release`, which will create a git tag for the version, push git commits and the created tag, and push the `.gem` file to [rubygems.org](https://rubygems.org).

## Contributing

Bug reports and pull requests are welcome on GitHub at https://github.com/vanjamodrinjak21/ebur128_stream. Mention @KitaitiMakoto for issues and pull requests related to the Ruby binding.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## See also

* [Rust crate][rust-impl]
* [BS.1770][]

[rust-impl]: https://github.com/vanjamodrinjak21/ebur128-stream
[Analyzer]: rdoc-ref:EBUR128Stream::Analyzer
[BS.1770]: https://www.itu.int/rec/R-REC-BS.1770
[GStreamer]: https://github.com/ruby-gnome/ruby-gnome/tree/main/gstreamer
[TorchAudio]: https://github.com/ankane/torchaudio-ruby
[TorchCodec]: https://github.com/ankane/torchcodec-ruby
[Numo::NArray]: https://ruby-numo.github.io/numo-narray/
[Numo::NArray Alternative]: https://github.com/yoshoku/numo-narray-alt
[WaveFile]: https://wavefilegem.com/
[NDAV::TorchTensor]: https://gitlab.com/KitaitiMakoto/ndav-torch-tensor
[NDAV::Numo::NArray]: https://gitlab.com/KitaitiMakoto/ndav-numo-narray
