require_relative "helper"

class TestPackage < Test::Unit::TestCase
  ENGINE_VERSION = JSON.parse(`cargo metadata --format-version=1 --manifest-path=ext/ebur128_stream/Cargo.toml`)["packages"].find {|package|
    package["name"] == "ebur128-stream"
  }["version"]

  def setup
    @gemspec_file = "ebur128_stream.gemspec"
    @gemspec = Gem::Specification.load(@gemspec_file)
    @pkg_file = File.join("pkg", @gemspec.full_name + ".gem")
    FileUtils.rmtree @pkg_file
  end

  def test_build
    assert system("rake", "build")
    assert File.size(@pkg_file) > 0
  end

  def test_install
    system "rake", "build", exception: true
    gem_file = File.join("pkg", @gemspec.full_name + ".gem")
    Dir.mktmpdir do |dir|
      dir = File.realpath(dir)
      install_dir = File.join(dir, "install")
      FileUtils.mkdir install_dir
      system "gem", "install", "--install-dir", install_dir, "--no-document", gem_file, exception: true

      assert_path_exist File.join(install_dir, "extensions", Gem::Platform.local.to_s, RbConfig::CONFIG["ruby_version"], @gemspec.full_name, "ebur128_stream/ebur128_stream_ruby.#{RbConfig::CONFIG['DLEXT']}")

      lib_dir = File.join(install_dir, "gems", @gemspec.full_name)
      manifest = Tomlrb.load_file(File.join(lib_dir, "ext/ebur128_stream/Cargo.toml"))

      assert_equal ENGINE_VERSION, manifest["dependencies"]["ebur128-stream"]["version"]
      assert_nil manifest["dependencies"]["ebur128-stream"]["path"]

      lock = Tomlrb.load_file(File.join(lib_dir, "ext/ebur128_stream/Cargo.lock"))
      assert_equal ENGINE_VERSION, lock["package"].find {|package| package["name"] == "ebur128-stream"}["version"]
    end
  end
end
