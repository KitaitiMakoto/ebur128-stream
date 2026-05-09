//! `04_multichannel_5_1` — channel-weighted loudness for 5.1 surround.
//!
//! Demonstrates:
//! - `LeftSurround` / `RightSurround` get +1.5 dB power weight
//! - `Lfe` is excluded from the loudness sum entirely

use ebur128_stream::{AnalyzerBuilder, Channel, Mode};

fn main() {
    const FS: u32 = 48_000;
    const SECONDS: f32 = 5.0;
    const AMP: f32 = 0.10;

    let n = (FS as f32 * SECONDS) as usize;
    let two_pi_f = 2.0 * std::f32::consts::PI * 1000.0 / FS as f32;
    let sine: Vec<f32> = (0..n).map(|i| AMP * (two_pi_f * i as f32).sin()).collect();
    let zero = vec![0.0f32; n];

    let layout = [
        Channel::Left,
        Channel::Right,
        Channel::Center,
        Channel::Lfe,
        Channel::LeftSurround,
        Channel::RightSurround,
    ];

    println!("5.1 channel-weighting demo (1 kHz sine, amp = {AMP})\n");
    println!("{:<14} {:>8}", "Active channel", "Integrated");
    println!("{}", "─".repeat(28));

    let cases: [(&str, &[&[f32]]); 4] = [
        ("Center only ", &[&zero, &zero, &sine, &zero, &zero, &zero]),
        ("L+R         ", &[&sine, &sine, &zero, &zero, &zero, &zero]),
        ("Surround    ", &[&zero, &zero, &zero, &zero, &sine, &sine]),
        ("LFE only    ", &[&zero, &zero, &zero, &sine, &zero, &zero]),
    ];

    for (name, planar) in cases {
        let mut a = AnalyzerBuilder::new()
            .sample_rate(FS)
            .channels(&layout)
            .modes(Mode::Integrated)
            .build()
            .unwrap();
        a.push_planar::<f32>(planar).unwrap();
        let r = a.finalize();
        match r.integrated_lufs() {
            Some(l) => println!("{name} {:>8.2} LUFS", l),
            None => println!("{name} {:>8} LUFS", "  --  "),
        }
    }
    println!("\nLFE-only programme integrates to None — channel is excluded per BS.1770-4.");
}
