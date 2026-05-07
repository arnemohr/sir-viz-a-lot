//! Audio capture + FFT band extraction (T-M7-03), gated on `feature = "audio"`.
//!
//! Architecture: a `cpal` input stream pushes raw f32 samples into a bounded
//! `crossbeam-channel`; a worker thread consumes 1024-sample windows, applies
//! a Hann taper, runs `rustfft`, splits the spectrum into 8 logarithmic bands,
//! one-pole-smooths the magnitudes into a shared `RwLock<[f32; 8]>`, and the
//! `Modulator::Audio` arm reads that table via [`current_band`].
//!
//! Roadmap framing: Phase 5 — "Optional audio-reactive modulation only when
//! it supports scene design rather than adding chaos." Eight log-spaced
//! bands smoothed with a one-pole low-pass is intentionally simple, far from
//! a generic spectrum-analyser; that's the right side of "supports scene
//! design" for the wedding-scale target.

use std::sync::{Arc, OnceLock};

#[cfg(feature = "audio")]
use std::sync::RwLock;

/// Number of log-spaced bands the provider exposes. Each band averages
/// the magnitude across a half-octave-ish slice of the spectrum.
#[cfg_attr(not(feature = "audio"), allow(dead_code))]
pub const NUM_BANDS: usize = 8;

/// One named source of band magnitudes in `[0, 1]`. The trait is the
/// extension point: `CpalAudioProvider` is the in-tree default; tests
/// can install a stub that returns canned values without ever touching
/// the audio stack.
pub trait AudioProvider: Send + Sync {
    /// Magnitude in `[0, 1]` for the given band index. Out-of-range
    /// indices clamp to the last band rather than panic.
    fn band(&self, idx: u8) -> f32;
}

/// Process-wide audio provider, set once at app startup. `Modulator::Audio`
/// reads through here so the dispatch stays parameter-free.
static PROVIDER: OnceLock<Arc<dyn AudioProvider>> = OnceLock::new();

/// Install the active audio provider. Subsequent calls are silently ignored —
/// once the dispatch sees a provider it should not change for the lifetime
/// of the app, since FFT smoothing state would reset.
#[cfg_attr(not(feature = "audio"), allow(dead_code))]
pub fn install(provider: Arc<dyn AudioProvider>) {
    let _ = PROVIDER.set(provider);
}

/// Current value for `band` from the installed provider, or `0.0` if no
/// provider was installed (e.g. no input device, audio init failed).
pub fn current_band(idx: u8) -> f32 {
    PROVIDER.get().map(|p| p.band(idx)).unwrap_or(0.0)
}

#[cfg(feature = "audio")]
mod cpal_impl {
    use super::*;
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use rustfft::{num_complex::Complex, FftPlanner};
    use std::thread;

    /// `AudioProvider` implementation backed by an `Arc<RwLock<[f32]>>` of
    /// band magnitudes. The actual `cpal::Stream` is `!Send`, so it cannot
    /// live inside the `Arc<dyn AudioProvider: Send + Sync>` shared via the
    /// `PROVIDER` static — `start_default` returns the stream as a
    /// separate handle the caller stores on the main thread.
    pub struct CpalAudioProvider {
        bands: Arc<RwLock<[f32; NUM_BANDS]>>,
    }

    impl AudioProvider for CpalAudioProvider {
        fn band(&self, idx: u8) -> f32 {
            let i = (idx as usize).min(NUM_BANDS - 1);
            self.bands.read().map(|b| b[i]).unwrap_or(0.0)
        }
    }

    /// RAII guard for the live capture stream. Stored on the main thread
    /// in `RunningApp` so dropping it stops capture cleanly. Not `Send`,
    /// matching the underlying `cpal::Stream`.
    pub struct AudioCaptureGuard {
        // Field unused by name; presence in the struct ties the stream's
        // lifetime to the guard's.
        #[allow(dead_code)]
        stream: cpal::Stream,
    }

    /// Open the platform default input device, start capture, return both a
    /// provider (Send+Sync, sharable via `Arc`) and a non-Send stream guard
    /// the caller must keep alive on its own thread.
    ///
    /// Errors propagate via `anyhow` so app startup can warn-and-continue
    /// without audio support if no device is available.
    pub fn start_default() -> anyhow::Result<(CpalAudioProvider, AudioCaptureGuard)> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| anyhow::anyhow!("no default input device"))?;
        let supported = device.default_input_config()?;
        let bands: Arc<RwLock<[f32; NUM_BANDS]>> = Arc::new(RwLock::new([0.0; NUM_BANDS]));

        // Bounded channel: cpal callback drops if worker can't keep up.
        // Audio-quality is best-effort here; the goal is scene-driving
        // not preservation.
        let (tx, rx) = crossbeam_channel::bounded::<Vec<f32>>(8);

        let bands_for_worker = bands.clone();
        thread::Builder::new()
            .name("rmap-audio-fft".into())
            .spawn(move || run_worker(rx, bands_for_worker))?;

        let stream = match supported.sample_format() {
            cpal::SampleFormat::F32 => {
                let cfg: cpal::StreamConfig = supported.config();
                device.build_input_stream(
                    &cfg,
                    move |data: &[f32], _: &_| {
                        // try_send: drop on overflow rather than block the audio
                        // callback.
                        let _ = tx.try_send(data.to_vec());
                    },
                    |err| tracing::warn!(?err, "cpal input stream error"),
                    None,
                )?
            }
            other => {
                anyhow::bail!("unsupported audio sample format: {other:?}")
            }
        };
        stream.play()?;
        Ok((CpalAudioProvider { bands }, AudioCaptureGuard { stream }))
    }

    /// FFT worker: drains the channel, accumulates samples up to a 1024
    /// window, runs `rustfft`, computes log-spaced band magnitudes,
    /// one-pole-smooths into the shared bands table.
    fn run_worker(
        rx: crossbeam_channel::Receiver<Vec<f32>>,
        bands: Arc<RwLock<[f32; NUM_BANDS]>>,
    ) {
        let fft_size = 1024usize;
        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(fft_size);
        let mut acc: Vec<f32> = Vec::with_capacity(fft_size * 2);
        let mut buf = vec![Complex::<f32> { re: 0.0, im: 0.0 }; fft_size];
        // Bands sub-divide the half-spectrum [0, fft_size/2). Edges chosen
        // log-ish; half-octave below 4 kHz, octave above. Adjust per taste.
        let half = fft_size / 2;
        let edges: [usize; NUM_BANDS + 1] = [0, 4, 8, 16, 32, 64, 128, 256, half];

        // Hann window, precomputed (no allocation in the hot loop).
        let mut hann = vec![0f32; fft_size];
        for (i, w) in hann.iter_mut().enumerate() {
            let phase = 2.0 * std::f32::consts::PI * i as f32 / (fft_size - 1) as f32;
            *w = 0.5 - 0.5 * phase.cos();
        }

        while let Ok(samples) = rx.recv() {
            acc.extend(samples);
            while acc.len() >= fft_size {
                for (i, x) in acc.iter().take(fft_size).enumerate() {
                    buf[i] = Complex {
                        re: x * hann[i],
                        im: 0.0,
                    };
                }
                fft.process(&mut buf);
                let mut new_bands = [0f32; NUM_BANDS];
                for b in 0..NUM_BANDS {
                    let lo = edges[b].min(half);
                    let hi = edges[b + 1].min(half);
                    if hi <= lo {
                        continue;
                    }
                    let mut sum = 0.0;
                    for k in lo..hi {
                        let m = (buf[k].re * buf[k].re + buf[k].im * buf[k].im).sqrt();
                        sum += m;
                    }
                    let avg = sum / (hi - lo) as f32;
                    // Normalize: divide by sqrt(fft_size). Squash to [0, 1] —
                    // wedding-PA peaks are usually < 1 after this scaling;
                    // clamp covers the rest.
                    new_bands[b] = (avg / (fft_size as f32).sqrt()).clamp(0.0, 1.0);
                }
                if let Ok(mut g) = bands.write() {
                    // One-pole low-pass per band; constant chosen for
                    // ~80 ms response at 44.1 kHz audio.
                    for b in 0..NUM_BANDS {
                        g[b] = g[b] * 0.7 + new_bands[b] * 0.3;
                    }
                }
                acc.drain(0..fft_size);
            }
        }
    }
}

#[cfg(feature = "audio")]
pub use cpal_impl::{start_default, AudioCaptureGuard};
// `CpalAudioProvider` is constructed inside `start_default` and erased into
// the `Arc<dyn AudioProvider>` registered with `install`; no external caller
// names the type, so there is no re-export.
