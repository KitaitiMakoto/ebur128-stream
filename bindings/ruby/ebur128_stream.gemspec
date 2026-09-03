# frozen_string_literal: true

require_relative "lib/ebur128_stream/version"

Gem::Specification.new do |spec|
  spec.name = "ebur128_stream"
  spec.version = EBUR128Stream::VERSION
  spec.authors = ["Kitaiti Makoto"]
  spec.email = ["KitaitiMakoto@gmail.com"]
  spec.licenses = ["Apache-2.0", "MIT"]

  spec.summary = "Streaming, zero-allocation EBU R128 loudness measurement."
  spec.description = "Ruby binding for Streaming, zero-allocation EBU R128 loudness measurement in pure Rust."
  spec.homepage = "https://github.com/KitaitiMakoto/ebur128-stream/tree/ruby/bindings/ruby"
  spec.required_ruby_version = ">= 3.2.0"
  spec.metadata["allowed_push_host"] = "https://rubygems.org"
  spec.metadata["homepage_uri"] = spec.homepage
  spec.metadata["source_code_uri"] = "https://github.com/KitaitiMakoto/ebur128-stream/tree/ruby/bindings/ruby"
  spec.metadata["changelog_uri"] = "https://github.com/KitaitiMakoto/ebur128-stream/tree/ruby/bindings/ruby/CHANGELOG.md"

  # Uncomment the line below to require MFA for gem pushes.
  # This helps protect your gem from supply chain attacks by ensuring
  # no one can publish a new version without multi-factor authentication.
  # See: https://guides.rubygems.org/mfa-requirement-opt-in/
  # spec.metadata["rubygems_mfa_required"] = "true"

  # Specify which files should be added to the gem when it is released.
  # The `git ls-files -z` loads the files in the RubyGem that have been added into git.
  gemspec = File.basename(__FILE__)
  spec.files = IO.popen(%w[git ls-files -z], chdir: __dir__, err: IO::NULL) do |ls|
    ls.readlines("\x0", chomp: true)
  end + ["LICENSE-APACHE", "LICENSE-MIT"]
  spec.executables = spec.files.grep(%r{\Abin/}) { |f| File.basename(f) }
  spec.require_paths = ["lib"]
  spec.extensions = ["ext/ebur128_stream/Cargo.toml"]

  # Uncomment to register a new dependency of your gem
  # spec.add_dependency "example-gem", "~> 1.0"

  spec.add_development_dependency "irb"
  spec.add_development_dependency "rake"
  spec.add_development_dependency "test-unit"
  spec.add_development_dependency "rubygems-tasks"
  spec.add_development_dependency "kar"
  spec.add_development_dependency "numo-narray-alt"
  spec.add_development_dependency "ndav-numo-narray"

  # For more information and examples about making a new gem, check out our
  # guide at: https://guides.rubygems.org/make-your-own-gem/
end
