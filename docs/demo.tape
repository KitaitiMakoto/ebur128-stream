# vhs tape file — produces docs/demo.gif
# Run with: `vhs docs/demo.tape`
# vhs (charmbracelet) homepage: https://github.com/charmbracelet/vhs
#
# This recording demonstrates the streaming-determinism property:
# the same audio fed in different chunk sizes produces bitwise-identical
# results. The output GIF is what's embedded in the README's first screen.

Output docs/demo.gif

Set FontFamily "JetBrainsMono Nerd Font Mono"
Set FontSize 14
Set Width 1100
Set Height 540
Set Theme "Dracula"
Set TypingSpeed 35ms
Set PlaybackSpeed 1.0

# ---- Banner ----
Sleep 600ms
Type "# ebur128-stream — streaming determinism demo"
Sleep 800ms
Enter
Sleep 400ms
Type "# the same 10 s of audio, fed at 4 different chunk sizes,"
Enter
Type "# yields bit-identical Integrated / M-max / S-max / TruePeak / LRA"
Enter
Sleep 600ms
Enter

# ---- Run the example ----
Type "cargo run --release --example 05_streaming_chunks"
Sleep 600ms
Enter

# Let the build finish + output appear.
Sleep 12s

# ---- Finalise ----
Sleep 1s
Enter
Type "# ✓ All chunk sizes produce identical results within 1e-9."
Sleep 1s
