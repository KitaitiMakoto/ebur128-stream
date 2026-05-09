# `ebur128-stream` — Python bindings

Python bindings for [`ebur128-stream`](../..) via [`pyo3`].

## Building

Requires:

- Rust 1.85+
- Python 3.9+
- [`maturin`](https://github.com/PyO3/maturin) (`pip install maturin`)

```bash
cd bindings/python
maturin develop --release
```

This compiles the Rust extension and installs it into the active
Python environment as `ebur128_stream`.

To build a redistributable wheel:

```bash
maturin build --release --strip
# wheel in target/wheels/*.whl
```

## Quick start

```python
import numpy as np
import ebur128_stream as ebs

# 5 s of stereo silence
samples = np.zeros(48_000 * 5 * 2, dtype=np.float32)

a = ebs.Analyzer(sample_rate=48_000, channels=2, mode="all")
a.push_interleaved(samples.tolist())
report = a.finalize()
print(repr(report))
print("Integrated:", report.integrated_lufs)  # None for silence
print("True peak: ", report.true_peak_dbtp)
```

## API

```python
class Analyzer:
    def __init__(self, sample_rate: int, channels: int, mode: str = "all"): ...
    def push_interleaved(self, samples: list[float]) -> None: ...
    def snapshot(self) -> Snapshot: ...
    def finalize(self) -> Report: ...
    def reset(self) -> None: ...

class Snapshot:
    momentary_lufs:   float | None
    short_term_lufs:  float | None
    integrated_lufs:  float | None
    true_peak_dbtp:   float | None
    loudness_range_lu: float | None
    programme_duration_seconds: float

class Report:
    integrated_lufs:    float | None
    loudness_range_lu:  float | None
    true_peak_dbtp:     float | None
    momentary_max_lufs: float | None
    short_term_max_lufs: float | None
    programme_duration_seconds: float
```

`mode` accepts a comma-separated string of any of: `i`, `m`, `s`, `tp`,
`lra`, `all`. Default is `"all"`.

## Status

The Rust source compiles and follows the pyo3 0.22 conventions.
Verifying the build into a usable `.so` / `.pyd` requires a Python
development environment and `maturin`, which is not part of this
repo's CI. PRs welcome to add a `bindings/python` CI job once a
maintainer has tested it locally.
