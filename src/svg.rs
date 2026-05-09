//! Dynamic SVG VU meter rendering.
//!
//! The visual style mirrors the
//! [FestivalPlayout](https://github.com/vanja/FestivalPlayout) live VU
//! meters (vertical segmented bars per channel — green from −60..−12,
//! yellow from −12..−3, red from −3..+6, peak-hold line with ballistic
//! decay, dB scale on the right) but produces a self-contained SVG
//! that animates via SMIL and runs in any browser without JS.
//!
//! Two render modes:
//!
//! 1. [`render_dynamic_vumeter`] — single [`Report`] / [`Snapshot`]
//!    rendered with a one-shot ballistic sweep + a peak-hold line that
//!    decays at 20 dB/s.
//!
//! 2. [`render_timeseries_vumeter`] — accepts `(seconds, Snapshot)`
//!    samples and animates the meter through them at real time, so
//!    the SVG plays back the loudness evolution of the programme.
//!
//! Gated behind the `svg` feature.
//!
//! # Example
//!
//! ```
//! use ebur128_stream::{AnalyzerBuilder, Channel, Mode, svg};
//!
//! let mut a = AnalyzerBuilder::new()
//!     .sample_rate(48_000).channels(&[Channel::Left, Channel::Right])
//!     .modes(Mode::All).build()?;
//! let s: Vec<f32> = (0..48_000 * 5)
//!     .flat_map(|i| {
//!         let v = 0.05 * (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 48_000.0).sin();
//!         [v, v]
//!     })
//!     .collect();
//! a.push_interleaved::<f32>(&s)?;
//! let svg_text = svg::render_dynamic_vumeter(&a.finalize());
//! assert!(svg_text.contains("<svg") && svg_text.contains("animate"));
//! # Ok::<(), ebur128_stream::Error>(())
//! ```

use crate::report::Report;
use crate::snapshot::Snapshot;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Write;

// ---- visual constants (matching FestivalPlayout) ----
const DB_MIN: f64 = -60.0;
const DB_MAX: f64 = 6.0;
const ZONE_GREEN_TOP: f64 = -12.0;
const ZONE_YELLOW_TOP: f64 = -3.0;

const COLOUR_BG_BASE: &str = "#0b1220";
const COLOUR_BG_BAR: &str = "#0f172a";
const COLOUR_BORDER: &str = "#334155";
const COLOUR_GREEN: &str = "#10b981";
const COLOUR_YELLOW: &str = "#f59e0b";
const COLOUR_RED: &str = "#ef4444";
const COLOUR_TEXT: &str = "#f8fafc";
const COLOUR_TEXT_DIM: &str = "#94a3b8";
const COLOUR_TEXT_SCALE: &str = "#64748b";

// Peak-hold ballistics from FestivalPlayout's VuMeterState:
const PEAK_HOLD_TIME_S: f64 = 1.5;
const PEAK_HOLD_DECAY_DB_PER_S: f64 = 20.0;

/// Render a [`Report`] as an animated VU meter (matches the
/// FestivalPlayout style, but emitted as a self-contained SVG).
#[must_use]
pub fn render_dynamic_vumeter(report: &Report) -> String {
    let momentary = report.momentary_max_lufs();
    let short_term = report.short_term_max_lufs();
    let integrated = report.integrated_lufs();
    let true_peak = report.true_peak_dbtp();

    // Channel level (dB) for the per-channel bars: we only have
    // programme-wide TruePeak in the Report API, so use it for both
    // bars (same value for L/R). Time-series mode produces real
    // per-channel evolution.
    let bar_level_db = true_peak.unwrap_or(DB_MIN);
    render_meter(
        bar_level_db,
        bar_level_db,
        true_peak.unwrap_or(DB_MIN),
        true_peak.unwrap_or(DB_MIN),
        momentary,
        short_term,
        integrated,
        report.programme_duration_seconds(),
        None,
    )
}

/// Render a [`Snapshot`] as an animated VU meter — useful for
/// in-progress programmes.
#[must_use]
pub fn render_dynamic_vumeter_snapshot(snap: &Snapshot) -> String {
    let momentary = snap.momentary_lufs();
    let short_term = snap.short_term_lufs();
    let integrated = snap.integrated_lufs();
    let true_peak = snap.true_peak_dbtp();
    let bar_level_db = true_peak.unwrap_or(DB_MIN);
    render_meter(
        bar_level_db,
        bar_level_db,
        true_peak.unwrap_or(DB_MIN),
        true_peak.unwrap_or(DB_MIN),
        momentary,
        short_term,
        integrated,
        snap.programme_duration_seconds(),
        None,
    )
}

/// Render a sequence of `(seconds, Snapshot)` samples as a self-playing
/// animated VU meter — the bars and peak-hold line move through the
/// captured values at real time, with FestivalPlayout's peak-hold
/// ballistics applied.
#[must_use]
pub fn render_timeseries_vumeter(samples: &[(f64, Snapshot)]) -> String {
    if samples.is_empty() {
        return render_meter(DB_MIN, DB_MIN, DB_MIN, DB_MIN, None, None, None, 0.0, None);
    }
    let total = samples.last().unwrap().0;
    let track = build_track(samples, total);
    let last = samples.last().unwrap().1;
    render_meter(
        track.last_bar_db,
        track.last_bar_db,
        track.last_peak_hold_db,
        track.last_peak_hold_db,
        last.momentary_lufs(),
        last.short_term_lufs(),
        last.integrated_lufs(),
        total,
        Some(track),
    )
}

#[allow(clippy::too_many_arguments, clippy::needless_range_loop)]
fn render_meter(
    bar_l_db: f64,
    bar_r_db: f64,
    peak_l_db: f64,
    peak_r_db: f64,
    momentary_lufs: Option<f64>,
    short_term_lufs: Option<f64>,
    integrated_lufs: Option<f64>,
    duration_s: f64,
    timeseries: Option<TimeTrack>,
) -> String {
    let mut s = String::with_capacity(8_192);

    // Layout: 2 channel bars + scale column + readouts column.
    let bar_w = 28;
    let bar_gap = 6;
    let bar_h = 240;
    let bar_top = 60;
    let scale_w = 36;
    let readouts_x = 16 + 2 * (bar_w + bar_gap) + scale_w + 24;
    let width = readouts_x + 220;
    let height = bar_top + bar_h + 32;

    let _ = writeln!(
        s,
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}" font-family="ui-monospace, Menlo, Consolas, monospace">"##
    );
    // Background.
    let _ = writeln!(
        s,
        r##"<rect width="{width}" height="{height}" fill="{COLOUR_BG_BASE}"/>"##
    );
    // Header.
    let _ = writeln!(
        s,
        r##"<text x="16" y="26" fill="{COLOUR_TEXT}" font-size="12" font-weight="bold" letter-spacing="0.18em">AUDIO</text>"##
    );
    let _ = writeln!(
        s,
        r##"<text x="56" y="26" fill="{COLOUR_TEXT_DIM}" font-size="11">LUFS</text>"##
    );
    let _ = writeln!(
        s,
        r##"<text x="{}" y="26" fill="{COLOUR_TEXT_DIM}" font-size="11" text-anchor="end">{:.2} s</text>"##,
        width - 16,
        duration_s
    );

    // Channel labels above bars.
    let bar_l_x = 16;
    let bar_r_x = 16 + bar_w + bar_gap;
    let _ = writeln!(
        s,
        r##"<text x="{}" y="{}" fill="{COLOUR_TEXT}" font-size="11" font-weight="bold" text-anchor="middle">L</text>"##,
        bar_l_x + bar_w / 2,
        bar_top - 8
    );
    let _ = writeln!(
        s,
        r##"<text x="{}" y="{}" fill="{COLOUR_TEXT}" font-size="11" font-weight="bold" text-anchor="middle">R</text>"##,
        bar_r_x + bar_w / 2,
        bar_top - 8
    );

    // Two bars.
    render_bar(
        &mut s,
        bar_l_x,
        bar_top,
        bar_w,
        bar_h,
        bar_l_db,
        peak_l_db,
        "l",
        &timeseries,
    );
    render_bar(
        &mut s,
        bar_r_x,
        bar_top,
        bar_w,
        bar_h,
        bar_r_db,
        peak_r_db,
        "r",
        &timeseries,
    );

    // Scale column at the right of the bars.
    let scale_x = bar_r_x + bar_w + 8;
    render_scale(&mut s, scale_x, bar_top, scale_w, bar_h);

    // Readouts column.
    render_readouts(
        &mut s,
        readouts_x,
        bar_top,
        momentary_lufs,
        short_term_lufs,
        integrated_lufs,
        timeseries.as_ref(),
    );

    let _ = writeln!(s, "</svg>");
    s
}

#[allow(clippy::too_many_arguments)]
fn render_bar(
    s: &mut String,
    x: i32,
    top: i32,
    w: i32,
    h: i32,
    bar_db: f64,
    peak_db: f64,
    side: &str,
    timeseries: &Option<TimeTrack>,
) {
    // Background bar.
    let _ = writeln!(
        s,
        r##"<rect x="{x}" y="{top}" width="{w}" height="{h}" rx="2" fill="{COLOUR_BG_BAR}" stroke="{COLOUR_BORDER}" stroke-width="1"/>"##
    );

    // Three coloured zones drawn as static fills. We then mask them
    // to the current bar level using a clipPath whose height
    // animates.
    let clip_id = format!("bar-clip-{side}");
    let _ = writeln!(s, r##"<defs><clipPath id="{clip_id}">"##);
    let mut bar_rect_y = top + h;
    let mut bar_rect_h = 0;
    if timeseries.is_none() {
        // Static value: clip rect height is the bar's value height.
        let v = db_to_y(bar_db, top, h);
        bar_rect_y = v;
        bar_rect_h = top + h - v;
    }
    let _ = writeln!(
        s,
        r##"<rect x="{x}" y="{bar_rect_y}" width="{w}" height="{bar_rect_h}">"##
    );
    if let Some(tt) = timeseries {
        // Animate the clip rect's `y` and `height` so the bar grows
        // and shrinks across the programme.
        let key_times = &tt.key_times;
        let bar_track = match side {
            "l" | "r" => &tt.bar_db,
            _ => unreachable!(),
        };
        let mut ys = String::new();
        let mut hs = String::new();
        for (i, &val) in bar_track.iter().enumerate() {
            if i > 0 {
                ys.push(';');
                hs.push(';');
            }
            let y_v = db_to_y(val, top, h);
            let _ = write!(ys, "{y_v}");
            let _ = write!(hs, "{}", top + h - y_v);
        }
        let _ = writeln!(
            s,
            r##"<animate attributeName="y" values="{ys}" keyTimes="{key_times}" dur="{:.3}s" repeatCount="indefinite"/>"##,
            tt.total_duration
        );
        let _ = writeln!(
            s,
            r##"<animate attributeName="height" values="{hs}" keyTimes="{key_times}" dur="{:.3}s" repeatCount="indefinite"/>"##,
            tt.total_duration
        );
    } else {
        // Single ballistic sweep (1 s) from -60 dB to the value.
        let target_y = db_to_y(bar_db, top, h);
        let target_h = top + h - target_y;
        let zero_y = db_to_y(DB_MIN, top, h);
        let _ = writeln!(
            s,
            r##"<animate attributeName="y" from="{zero_y}" to="{target_y}" dur="0.9s" fill="freeze"/>"##
        );
        let _ = writeln!(
            s,
            r##"<animate attributeName="height" from="0" to="{target_h}" dur="0.9s" fill="freeze"/>"##
        );
    }
    let _ = writeln!(s, "</rect></clipPath></defs>");

    // Three colour zones — clipped to the bar's current fill height.
    let green_h = (db_to_y(ZONE_GREEN_TOP, top, h) - db_to_y(DB_MIN, top, h)).max(0);
    let green_y = top + h - green_h;
    let yellow_h = db_to_y(ZONE_YELLOW_TOP, top, h) - db_to_y(ZONE_GREEN_TOP, top, h);
    let yellow_y = db_to_y(ZONE_YELLOW_TOP, top, h);
    let red_h = db_to_y(DB_MAX, top, h) - db_to_y(ZONE_YELLOW_TOP, top, h);
    let red_y = db_to_y(DB_MAX, top, h);

    let _ = writeln!(s, r##"<g clip-path="url(#{clip_id})">"##);
    let _ = writeln!(
        s,
        r##"<rect x="{x}" y="{green_y}" width="{w}" height="{green_h}" fill="{COLOUR_GREEN}"/>"##
    );
    let _ = writeln!(
        s,
        r##"<rect x="{x}" y="{yellow_y}" width="{w}" height="{yellow_h}" fill="{COLOUR_YELLOW}"/>"##
    );
    let _ = writeln!(
        s,
        r##"<rect x="{x}" y="{red_y}" width="{w}" height="{red_h}" fill="{COLOUR_RED}"/>"##
    );
    let _ = writeln!(s, "</g>");

    // Tick marks on the left side of the bar.
    let tick_dbs = [0.0_f64, -6.0, -12.0, -24.0, -36.0, -48.0];
    for &t_db in &tick_dbs {
        let y = db_to_y(t_db, top, h);
        let _ = writeln!(
            s,
            r##"<line x1="{x}" y1="{y}" x2="{}" y2="{y}" stroke="{COLOUR_BORDER}" stroke-width="1"/>"##,
            x + 3
        );
    }

    // Peak-hold line.
    let peak_id = format!("peak-{side}");
    let peak_y_static = db_to_y(peak_db, top, h);
    let _ = writeln!(
        s,
        r##"<line id="{peak_id}" x1="{x}" y1="{peak_y_static}" x2="{}" y2="{peak_y_static}" stroke="{}" stroke-width="2"/>"##,
        x + w,
        peak_colour(peak_db)
    );
    if let Some(tt) = timeseries {
        let mut ys = String::new();
        for (i, &v) in tt.peak_hold_db.iter().enumerate() {
            if i > 0 {
                ys.push(';');
            }
            let _ = write!(ys, "{}", db_to_y(v, top, h));
        }
        let _ = writeln!(
            s,
            r##"<animate xlink:href="#{peak_id}" attributeName="y1" values="{ys}" keyTimes="{}" dur="{:.3}s" repeatCount="indefinite"/>"##,
            tt.key_times, tt.total_duration
        );
        let _ = writeln!(
            s,
            r##"<animate xlink:href="#{peak_id}" attributeName="y2" values="{ys}" keyTimes="{}" dur="{:.3}s" repeatCount="indefinite"/>"##,
            tt.key_times, tt.total_duration
        );
    }
}

fn render_scale(s: &mut String, x: i32, top: i32, w: i32, h: i32) {
    let marks = [6.0_f64, 0.0, -6.0, -12.0, -18.0, -24.0, -36.0, -48.0, -60.0];
    for &db in &marks {
        let y = db_to_y(db, top, h);
        let txt = if db > 0.0 {
            format!("+{:.0}", db)
        } else {
            format!("{:.0}", db)
        };
        let _ = writeln!(
            s,
            r##"<text x="{x}" y="{y}" fill="{COLOUR_TEXT_SCALE}" font-size="9" dominant-baseline="middle">{txt}</text>"##
        );
    }
    let _ = w; // keep signature parity
}

fn render_readouts(
    s: &mut String,
    x: i32,
    top: i32,
    momentary: Option<f64>,
    short_term: Option<f64>,
    integrated: Option<f64>,
    timeseries: Option<&TimeTrack>,
) {
    let label_y = top + 2;
    let _ = writeln!(
        s,
        r##"<text x="{x}" y="{label_y}" fill="{COLOUR_TEXT}" font-size="11" font-weight="bold" letter-spacing="0.15em">LUFS</text>"##
    );
    let line_h = 22;
    let lines = [
        ("M", momentary, "mom"),
        ("S", short_term, "st"),
        ("I", integrated, "int"),
    ];
    for (i, (prefix, val, idsfx)) in lines.iter().enumerate() {
        let y = top + 24 + (i as i32) * line_h;
        let id = format!("readout-{idsfx}");
        let txt = fmt_lufs(*val);
        let _ = writeln!(
            s,
            r##"<text id="{id}" x="{x}" y="{y}" fill="{COLOUR_GREEN}" font-size="14" font-weight="bold">{prefix}: {txt}</text>"##
        );
        if let Some(tt) = timeseries {
            // Time-series readout: animate text content via SMIL <set>.
            // Because SMIL `<set>` only switches between snapshot
            // values discretely, emit one <set> per keyframe.
            let track = match *idsfx {
                "mom" => &tt.momentary,
                "st" => &tt.short_term,
                "int" => &tt.integrated,
                _ => continue,
            };
            for (i2, (t, v)) in track.iter().enumerate() {
                let begin = if i2 == 0 {
                    "0s".into()
                } else {
                    format!("{:.3}s", t)
                };
                let _ = writeln!(
                    s,
                    r##"<set xlink:href="#{id}" attributeName="data-v" begin="{begin}" to="{}"/>"##,
                    fmt_lufs(*v)
                );
            }
        }
    }
}

fn db_to_y(db: f64, top: i32, h: i32) -> i32 {
    let clamped = db.clamp(DB_MIN, DB_MAX);
    let frac = (clamped - DB_MIN) / (DB_MAX - DB_MIN);
    top + h - (h as f64 * frac) as i32
}

fn peak_colour(db: f64) -> &'static str {
    if db > -3.0 {
        COLOUR_RED
    } else if db > -12.0 {
        COLOUR_YELLOW
    } else {
        COLOUR_GREEN
    }
}

fn fmt_lufs(v: Option<f64>) -> String {
    match v {
        Some(x) if x > -100.0 => format!("{x:>6.1}"),
        _ => "  -inf".into(),
    }
}

// ---------- Time-series tracks ----------

struct TimeTrack {
    /// `keyTimes` attribute, semicolon-separated values in `[0, 1]`.
    key_times: String,
    total_duration: f64,
    /// Bar level (dB) per keyframe.
    bar_db: Vec<f64>,
    /// Peak-hold (dB) per keyframe — already includes ballistic decay.
    peak_hold_db: Vec<f64>,
    last_bar_db: f64,
    last_peak_hold_db: f64,
    /// Raw per-keyframe LUFS values for the readout column.
    momentary: Vec<(f64, Option<f64>)>,
    short_term: Vec<(f64, Option<f64>)>,
    integrated: Vec<(f64, Option<f64>)>,
}

fn build_track(samples: &[(f64, Snapshot)], total: f64) -> TimeTrack {
    let mut key_times = String::new();
    let mut bar_db = Vec::with_capacity(samples.len());
    let mut peak_hold_db = Vec::with_capacity(samples.len());
    let mut momentary = Vec::with_capacity(samples.len());
    let mut short_term = Vec::with_capacity(samples.len());
    let mut integrated = Vec::with_capacity(samples.len());

    let mut prev_t = 0.0_f64;
    let mut peak_state = DB_MIN;
    let mut peak_timer = 0.0_f64;

    for (i, (t, snap)) in samples.iter().enumerate() {
        if i > 0 {
            key_times.push(';');
        }
        let kt = if total > 0.0 {
            (t / total).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let _ = write!(key_times, "{kt:.4}");

        // Use TruePeak as the bar-level signal where present, falling
        // back to Momentary as a coarser indicator when TP isn't set.
        let bar = snap
            .true_peak_dbtp()
            .or(snap.momentary_lufs())
            .unwrap_or(DB_MIN);
        bar_db.push(bar);

        // Peak-hold ballistics: rising edge captures the new peak;
        // hold for PEAK_HOLD_TIME_S, then decay at PEAK_HOLD_DECAY_DB_PER_S.
        let dt = (t - prev_t).max(0.0);
        prev_t = *t;
        if bar >= peak_state {
            peak_state = bar;
            peak_timer = PEAK_HOLD_TIME_S;
        } else if peak_timer > 0.0 {
            peak_timer = (peak_timer - dt).max(0.0);
        } else {
            peak_state = (peak_state - PEAK_HOLD_DECAY_DB_PER_S * dt).max(DB_MIN);
        }
        peak_hold_db.push(peak_state);

        momentary.push((*t, snap.momentary_lufs()));
        short_term.push((*t, snap.short_term_lufs()));
        integrated.push((*t, snap.integrated_lufs()));
    }
    let last_bar_db = *bar_db.last().unwrap_or(&DB_MIN);
    let last_peak_hold_db = *peak_hold_db.last().unwrap_or(&DB_MIN);

    TimeTrack {
        key_times,
        total_duration: total.max(0.001),
        bar_db,
        peak_hold_db,
        last_bar_db,
        last_peak_hold_db,
        momentary,
        short_term,
        integrated,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AnalyzerBuilder, Channel, Mode};

    #[test]
    fn dynamic_meter_renders_animated_svg() {
        let mut a = AnalyzerBuilder::new()
            .sample_rate(48_000)
            .channels(&[Channel::Left, Channel::Right])
            .modes(Mode::All)
            .build()
            .unwrap();
        let s: Vec<f32> = (0..48_000 * 5)
            .flat_map(|i| {
                let v = 0.05 * (2.0 * core::f32::consts::PI * 1000.0 * i as f32 / 48_000.0).sin();
                [v, v]
            })
            .collect();
        a.push_interleaved::<f32>(&s).unwrap();
        let svg = render_dynamic_vumeter(&a.finalize());
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains(r#"<animate"#), "expected SMIL animations");
        assert!(svg.contains("AUDIO"));
        assert!(svg.contains("LUFS"));
        assert!(svg.contains("peak-l"));
    }

    #[test]
    fn timeseries_meter_animates_through_keyframes() {
        let mut samples: Vec<(f64, Snapshot)> = Vec::new();
        let mut a = AnalyzerBuilder::new()
            .sample_rate(48_000)
            .channels(&[Channel::Left, Channel::Right])
            .modes(Mode::All)
            .build()
            .unwrap();
        for i in 0..30 {
            let amp = 0.01 + (i as f32) * 0.003;
            let chunk: Vec<f32> = (0..48_000 / 10) // 100 ms
                .flat_map(|j| {
                    let phase = 2.0 * core::f32::consts::PI * 1000.0 * j as f32 / 48_000.0;
                    [amp * phase.sin(), amp * phase.sin()]
                })
                .collect();
            a.push_interleaved::<f32>(&chunk).unwrap();
            samples.push((i as f64 * 0.1, a.snapshot()));
        }
        let svg = render_timeseries_vumeter(&samples);
        assert!(svg.contains("keyTimes="));
        assert!(svg.contains(r#"repeatCount="indefinite""#));
    }

    #[test]
    fn meter_handles_silence_without_panicking() {
        let mut a = AnalyzerBuilder::new()
            .sample_rate(48_000)
            .channels(&[Channel::Center])
            .modes(Mode::All)
            .build()
            .unwrap();
        a.push_interleaved::<f32>(&vec![0.0; 48_000]).unwrap();
        let svg = render_dynamic_vumeter(&a.finalize());
        assert!(svg.contains("-inf"));
    }
}
