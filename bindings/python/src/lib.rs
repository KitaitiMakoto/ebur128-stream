//! Python bindings for `ebur128-stream` via [`pyo3`].
//!
//! Build with [maturin](https://github.com/PyO3/maturin):
//!
//! ```bash
//! cd bindings/python
//! maturin develop --release
//! ```
//!
//! Then in Python:
//!
//! ```python
//! import ebur128_stream as ebs
//! a = ebs.Analyzer(48_000, 2, mode="all")
//! a.push_interleaved(samples)   # numpy.float32 1-D array
//! r = a.finalize()
//! print(r.integrated_lufs)      # float | None
//! ```

use ebur128_stream::{AnalyzerBuilder, Channel, Mode};
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;

fn map_err(e: ebur128_stream::Error) -> PyErr {
    PyRuntimeError::new_err(format!("{e}"))
}

fn parse_modes(spec: &str) -> Result<Mode, PyErr> {
    let mut m = Mode::empty();
    for part in spec.split(',').map(str::trim) {
        match part.to_ascii_lowercase().as_str() {
            "i" | "integrated" => m |= Mode::Integrated,
            "m" | "momentary" => m |= Mode::Momentary,
            "s" | "shortterm" | "short_term" => m |= Mode::ShortTerm,
            "tp" | "true_peak" | "truepeak" => m |= Mode::TruePeak,
            "lra" => m |= Mode::Lra,
            "all" => m |= Mode::All,
            other => return Err(PyValueError::new_err(format!("unknown mode: {other}"))),
        }
    }
    if m.is_empty() {
        return Err(PyValueError::new_err("at least one mode must be selected"));
    }
    Ok(m)
}

fn default_layout(n: usize) -> Vec<Channel> {
    match n {
        1 => vec![Channel::Center],
        2 => vec![Channel::Left, Channel::Right],
        6 => vec![
            Channel::Left,
            Channel::Right,
            Channel::Center,
            Channel::Lfe,
            Channel::LeftSurround,
            Channel::RightSurround,
        ],
        n => (0..n).map(|_| Channel::Other).collect(),
    }
}

#[pyclass(name = "Analyzer")]
struct PyAnalyzer {
    inner: Option<ebur128_stream::Analyzer>,
}

#[pymethods]
impl PyAnalyzer {
    #[new]
    #[pyo3(signature = (sample_rate, channels, mode="all"))]
    fn new(sample_rate: u32, channels: usize, mode: &str) -> PyResult<Self> {
        let layout = default_layout(channels);
        let modes = parse_modes(mode)?;
        let inner = AnalyzerBuilder::new()
            .sample_rate(sample_rate)
            .channels(&layout)
            .modes(modes)
            .build()
            .map_err(map_err)?;
        Ok(Self { inner: Some(inner) })
    }

    /// Push a 1-D numpy.float32 array of interleaved samples.
    fn push_interleaved(&mut self, samples: Vec<f32>) -> PyResult<()> {
        let inner = self
            .inner
            .as_mut()
            .ok_or_else(|| PyRuntimeError::new_err("analyzer already finalized"))?;
        inner.push_interleaved::<f32>(&samples).map_err(map_err)
    }

    /// Take a current measurement snapshot as a `Snapshot`.
    fn snapshot(&mut self) -> PyResult<PySnapshot> {
        let inner = self
            .inner
            .as_mut()
            .ok_or_else(|| PyRuntimeError::new_err("analyzer already finalized"))?;
        let s = inner.snapshot();
        Ok(PySnapshot {
            momentary_lufs: s.momentary_lufs(),
            short_term_lufs: s.short_term_lufs(),
            integrated_lufs: s.integrated_lufs(),
            true_peak_dbtp: s.true_peak_dbtp(),
            loudness_range_lu: s.loudness_range_lu(),
            programme_duration_seconds: s.programme_duration_seconds(),
        })
    }

    /// Consume the analyzer and return a final `Report`.
    fn finalize(&mut self) -> PyResult<PyReport> {
        let inner = self
            .inner
            .take()
            .ok_or_else(|| PyRuntimeError::new_err("analyzer already finalized"))?;
        let r = inner.finalize();
        Ok(PyReport {
            integrated_lufs: r.integrated_lufs(),
            loudness_range_lu: r.loudness_range_lu(),
            true_peak_dbtp: r.true_peak_dbtp(),
            momentary_max_lufs: r.momentary_max_lufs(),
            short_term_max_lufs: r.short_term_max_lufs(),
            programme_duration_seconds: r.programme_duration_seconds(),
        })
    }

    fn reset(&mut self) -> PyResult<()> {
        let inner = self
            .inner
            .as_mut()
            .ok_or_else(|| PyRuntimeError::new_err("analyzer already finalized"))?;
        inner.reset();
        Ok(())
    }
}

#[pyclass(name = "Snapshot", get_all)]
struct PySnapshot {
    momentary_lufs: Option<f64>,
    short_term_lufs: Option<f64>,
    integrated_lufs: Option<f64>,
    true_peak_dbtp: Option<f64>,
    loudness_range_lu: Option<f64>,
    programme_duration_seconds: f64,
}

#[pymethods]
impl PySnapshot {
    fn __repr__(&self) -> String {
        format!(
            "Snapshot(M={:?}, S={:?}, I={:?}, TP={:?}, LRA={:?}, dur={:.2})",
            self.momentary_lufs,
            self.short_term_lufs,
            self.integrated_lufs,
            self.true_peak_dbtp,
            self.loudness_range_lu,
            self.programme_duration_seconds,
        )
    }
}

#[pyclass(name = "Report", get_all)]
struct PyReport {
    integrated_lufs: Option<f64>,
    loudness_range_lu: Option<f64>,
    true_peak_dbtp: Option<f64>,
    momentary_max_lufs: Option<f64>,
    short_term_max_lufs: Option<f64>,
    programme_duration_seconds: f64,
}

#[pymethods]
impl PyReport {
    fn __repr__(&self) -> String {
        format!(
            "Report(I={:?}, LRA={:?}, TP={:?}, M_max={:?}, S_max={:?}, dur={:.2})",
            self.integrated_lufs,
            self.loudness_range_lu,
            self.true_peak_dbtp,
            self.momentary_max_lufs,
            self.short_term_max_lufs,
            self.programme_duration_seconds,
        )
    }
}

#[pymodule]
fn ebur128_stream(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyAnalyzer>()?;
    m.add_class::<PySnapshot>()?;
    m.add_class::<PyReport>()?;
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}
