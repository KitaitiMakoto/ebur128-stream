# frozen_string_literal: true

require_relative "ebur128_stream/version"
require "ebur128_stream/ebur128_stream"

module EBUR128Stream
  class Error < StandardError; end

  module Reportable
    def deconstruct_keys(keys = nil)
      keys = self.class::ATTRS if keys.nil?
      (keys & self.class::ATTRS).inject({}) {|deconstructed, attr|
        deconstructed[attr] = send(attr)
        deconstructed
      }
    end

    def inspect
      "#<%{class} %{attrs}>" % {
        class: self.class,
        attrs: self.class::ATTRS.collect {|attr| "#{attr}=#{send(attr).inspect}"}.join(" ")
      }
    end
  end

  class Snapshot
    include Reportable

    ATTRS = [
      :momentary_lufs,
      :short_term_lufs,
      :integrated_lufs,
      :loudness_range_lu,
      :true_peak_dbtp,
      :programme_duration_seconds
    ]
  end

  class Report
    include Reportable

    ATTRS = [
      :integrated_lufs,
      :loudness_range_lu,
      :true_peak_dbtp,
      :momentary_max_lufs,
      :short_term_max_lufs,
      :programme_duration_seconds
    ]
  end

  class NormalizeReport
    include Reportable

    ATTRS = [
      :measured_integrated_lufs,
      :measured_true_peak_dbtp,
      :target_lufs,
      :true_peak_ceiling_dbtp,
      :applied_gain_db,
      :limited_by_true_peak,
    ]
  end
end
