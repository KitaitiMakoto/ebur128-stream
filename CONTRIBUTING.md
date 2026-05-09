# Contributing to `ebur128-stream`

Pull requests are welcome. To keep things small and sharp, please:

1. **Run the full check before opening a PR:**

   ```bash
   cargo test --all-features
   cargo test --no-default-features
   cargo test --no-default-features --features alloc
   cargo clippy --all-features --all-targets -- -D warnings
   cargo fmt --check
   cargo run --example 05_streaming_chunks    # determinism proof
   ```

2. **Don't break determinism.** `05_streaming_chunks` is the headline contract — it must continue to print `✓ All chunk sizes produce identical results within 1e-9.`

3. **Don't break compliance.** `tests/calibration.rs` is the internal-consistency oracle. The official EBU Tech 3341 suite lands in v0.1.1; that becomes the integrity gate from then on.

4. **Use [conventional commits](https://www.conventionalcommits.org/):** `feat:`, `fix:`, `docs:`, `test:`, `refactor:`, `chore:`.

5. **One concern per PR.** Bundling unrelated changes makes review hard. If you're tempted to "while I'm in here, also fix X," open a second PR.

## Scope discipline

This crate is deliberately small (see [SPEC.md](SPEC.md) §15 anti-scope). Things we won't accept:

- File I/O — caller's job
- Resampling — caller's job
- An async runtime layer beyond optional `tokio::Sink` — runtime integration is the caller's
- A CLI — separate crate (`lufs` would make sense)
- Loudness *correction* — different problem; `r128-normalize` would be a separate crate

If your PR is about adding any of those, please open an issue first to discuss whether it should live here or somewhere else.

## Asking questions

Open an issue. Discussions over chat get lost; an issue persists.
