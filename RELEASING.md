# Releasing `ebur128-stream`

The release checklist *is* a portfolio artifact. Reviewers reading this file see "this person ships with discipline."

## Checklist

- [ ] Working tree clean (`git status` shows nothing staged/unstaged)
- [ ] On `main` branch, up to date with origin
- [ ] All tests pass: `cargo test --all-features`
- [ ] All `--no-default-features` builds pass:
    - `cargo build --no-default-features`
    - `cargo build --no-default-features --features alloc`
- [ ] All doctests pass: `cargo test --doc --all-features`
- [ ] All examples build: `cargo build --examples --all-features`
- [ ] `examples/05_streaming_chunks` prints "✓" — streaming determinism gate
- [ ] EBU Tech 3341 compliance suite: pending in v0.1.1
- [ ] Benchmarks recorded in `bench-results.md`: `cargo bench --bench throughput`
- [ ] Clippy clean: `cargo clippy --all-features --all-targets -- -D warnings`
- [ ] Format check: `cargo fmt --check`
- [ ] MSRV check: `cargo +1.74 build`
- [ ] Doc warnings: `cargo doc --no-deps --all-features` produces no warnings
- [ ] CHANGELOG updated with the version and date
- [ ] `Cargo.toml` version bumped
- [ ] `cargo publish --dry-run --allow-dirty=false` clean
- [ ] Git tag pushed: `git tag v$VERSION && git push --tags`
- [ ] GitHub release created with notes referencing CHANGELOG entry
- [ ] `cargo publish` (this is the one-way action — verify everything else above first)
- [ ] Update dependent portfolio repos (broadcast-qc-api, Mediq, FestivalPlayout) to pull the new version

## After publishing

- [ ] Watch crates.io build — docs.rs sometimes fails on first build for niche feature flag combinations
- [ ] Verify the docs.rs page renders: https://docs.rs/ebur128-stream/$VERSION/ebur128_stream/
- [ ] Verify the README hero example compiles when copy-pasted (the README's first ```rust block should be runnable as-is)

## What we deliberately do NOT do

- Don't version-bump for documentation-only changes
- Don't ship without the EBU compliance test suite once v0.1.1 lands — it's the integrity gate
- Don't merge changes that break determinism (`05_streaming_chunks` ✓)
- Don't add SIMD or platform-specific code paths in V0.1
