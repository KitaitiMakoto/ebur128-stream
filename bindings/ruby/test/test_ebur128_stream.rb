# frozen_string_literal: true

require_relative "helper"

class EBUR128StreamTest < Test::Unit::TestCase
  include EBUR128Stream

  test "VERSION" do
    assert do
      ::EBUR128Stream.const_defined?(:VERSION)
    end
  end
end
