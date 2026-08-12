# frozen_string_literal: true

require_relative "ebur128_stream/version"
require "ebur128_stream/ebur128_stream"

module EBUR128Stream
  class Error < StandardError; end

  class Report
    ATTRS = [
      :integrated_lufs,
      :loudness_range_lu,
      :true_peak_dbtp,
      :momentary_max_lufs,
      :short_term_max_lufs,
      :programme_duration_seconds
    ]

    def inspect
      "#<%{class} %{attrs}>" % {
        class: self.class,
        attrs: ATTRS.collect {|attr| "#{attr}=#{send(attr) || nil.inspect}"}.join(" ")
      }
    end
  end
end
