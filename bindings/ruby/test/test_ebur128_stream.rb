# frozen_string_literal: true

require_relative "helper"

class EBUR128StreamTest < Test::Unit::TestCase
  test "VERSION" do
    assert do
      ::EBUR128Stream.const_defined?(:VERSION)
    end
  end

  test "something useful" do
    assert_equal("expected", "actual")
  end
end
