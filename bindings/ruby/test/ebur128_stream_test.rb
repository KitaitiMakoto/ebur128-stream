# frozen_string_literal: true

require "test_helper"

class Ebur128StreamTest < Test::Unit::TestCase
  test "VERSION" do
    assert do
      ::Ebur128Stream.const_defined?(:VERSION)
    end
  end

  test "something useful" do
    assert_equal("expected", "actual")
  end
end
