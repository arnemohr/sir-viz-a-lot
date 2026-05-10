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
//! design" for the event-scale target.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};

#[cfg(feature = "audio")]
use std::sync::RwLock;

/// Process-wide counter of audio chunks dropped on the cpal callback's
/// `try_send` overflow path (P0.3.2). Read by the diagnostics surface
/// alongside the texture-upload queue's dropped counter for the
/// aggregated "dropped: N" badge.
static DROPPED: AtomicU64 = AtomicU64::new(0);

/// Total audio chunks dropped since process start. The cpal worker's
/// bounded channel (`tx.try_send` at the cpal callback site) prefers
/// dropping over blocking the audio thread; every drop bumps this
/// counter so the operator can see when audio capture is starving.
pub fn dropped_count() -> u64 {
    DROPPED.load(Ordering::Relaxed)
}

/// Increment the dropped counter. Called by the cpal callback (gated
/// on `feature = "audio"`); exposed for tests in non-audio builds so
/// the diagnostics surface can be exercised without a live audio
/// stack.
#[cfg_attr(not(feature = "audio"), allow(dead_code))]
pub fn record_drop() {
    DROPPED.fetch_add(1, Ordering::Relaxed);
}

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

    /// Bulk read: fill `out` with all 8 bands in a single operation.
    /// Default impl falls back to per-band reads; `CpalAudioProvider`
    /// overrides for a single read-lock take (32 bytes, one atomic CAS).
    ///
    /// V31.9.1: called by `current_bands_snapshot` so the UI thread
    /// acquires the lock only once per frame.
    fn bands(&self, out: &mut [f32; NUM_BANDS]) {
        for (i, slot) in out.iter_mut().enumerate() {
            *slot = self.band(i as u8);
        }
    }
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

/// Snapshot of all 8 bands as a `Copy` array, in one operation.
/// Returns `[0.0; 8]` when no provider is installed (audio feature off,
/// init failed, or no input device).
///
/// V31.9.1: the per-frame UI read path. The UI thread polls this once per
/// frame; the strip (V31.9.2) renders 8 vertical bars from the result.
/// Allocation-free; uses the bulk [`AudioProvider::bands`] method so
/// `CpalAudioProvider` satisfies the read with a single lock acquisition.
pub fn current_bands_snapshot() -> [f32; NUM_BANDS] {
    PROVIDER
        .get()
        .map(|p| {
            let mut buf = [0.0f32; NUM_BANDS];
            p.bands(&mut buf);
            buf
        })
        .unwrap_or([0.0; NUM_BANDS])
}

/// Returns `true` when an [`AudioProvider`] is installed.
///
/// V31.9.1: used by the UI strip (V31.9.2) to decide whether to render
/// the bands meter at all — when no audio source is active the strip is
/// hidden entirely. A provider is set once at app startup if and only if
/// audio capture started successfully, so this is a reliable proxy for
/// "audio is running".
pub fn is_audio_active() -> bool {
    PROVIDER.get().is_some()
}

#[cfg(feature = "audio")]
mod cpal_impl {
    use super::*;
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use rustfft::{FftPlanner, num_complex::Complex};
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

        /// Override: acquire the read-lock once, copy all 8 bands in a
        /// single 32-byte `*out = *guard` assignment.
        /// V31.9.1: this is the "no lock contention" path for UI reads.
        fn bands(&self, out: &mut [f32; NUM_BANDS]) {
            if let Ok(guard) = self.bands.read() {
                *out = *guard;
            }
            // If poisoned, `out` keeps its caller-provided default ([0.0; 8]).
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
                        // callback. P0.3.2: count drops so the diagnostics
                        // surface can warn when audio capture is starving.
                        if tx.try_send(data.to_vec()).is_err() {
                            super::record_drop();
                        }
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
    fn run_worker(rx: crossbeam_channel::Receiver<Vec<f32>>, bands: Arc<RwLock<[f32; NUM_BANDS]>>) {
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
                    // event-PA peaks are usually < 1 after this scaling;
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

    #[cfg(test)]
    mod tests {
        use super::*;

        /// V31.9.1: bulk `bands` and per-band `band(i)` agree for
        /// `CpalAudioProvider` when the backing RwLock holds known values.
        /// Constructs the provider without a live cpal stream; tests the
        /// trait implementation directly.
        #[test]
        fn bulk_bands_matches_per_band_reads() {
            let known: [f32; NUM_BANDS] = [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8];
            let provider = CpalAudioProvider {
                bands: Arc::new(RwLock::new(known)),
            };

            // Per-band reads.
            let per_band: [f32; NUM_BANDS] = std::array::from_fn(|i| provider.band(i as u8));

            // Bulk read.
            let mut bulk = [0.0f32; NUM_BANDS];
            provider.bands(&mut bulk);

            assert_eq!(bulk, per_band, "bulk and per-band reads must agree");
            assert_eq!(bulk, known, "values must match what was written");
        }
    }
}

#[cfg(feature = "audio")]
pub use cpal_impl::{AudioCaptureGuard, start_default};
// `CpalAudioProvider` is constructed inside `start_default` and erased into
// the `Arc<dyn AudioProvider>` registered with `install`; no external caller
// names the type, so there is no re-export.

#[cfg(test)]
mod tests {
    use super::*;

    // ---------------------------------------------------------------------------
    // V31.9.1 tests
    // ---------------------------------------------------------------------------

    /// `current_bands_snapshot` returns zeros when no provider is installed.
    ///
    /// Safe to run unconditionally: `audio::install` is only called at app
    /// startup (`src/app.rs`) and never in any test, so `PROVIDER` is
    /// guaranteed to be unset in the test process.
    #[test]
    fn snapshot_zeros_when_no_provider() {
        // PROVIDER is a OnceLock that has not been set in any test.
        let snap = current_bands_snapshot();
        assert_eq!(
            snap, [0.0f32; NUM_BANDS],
            "snapshot must be all zeros when no provider is installed"
        );
    }

    /// `is_audio_active` returns `false` when no provider is installed.
    #[test]
    fn is_audio_active_false_when_no_provider() {
        assert!(
            !is_audio_active(),
            "is_audio_active must be false when no provider is installed"
        );
    }

    /// Stub provider that returns canned values. Tests the `AudioProvider`
    /// trait — both the single-band and bulk paths — without touching the
    /// process-global `PROVIDER`.
    struct StubProvider([f32; NUM_BANDS]);

    impl AudioProvider for StubProvider {
        fn band(&self, idx: u8) -> f32 {
            let i = (idx as usize).min(NUM_BANDS - 1);
            self.0[i]
        }
        // Uses the default `bands` impl (per-band loop) — exercises that path.
    }

    /// Calling `bands` on a stub provider via the default impl returns the
    /// same values as per-band `band(i)` calls.
    #[test]
    fn snapshot_returns_provider_values() {
        let canned: [f32; NUM_BANDS] = [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8];
        let provider = StubProvider(canned);

        // Test the trait directly — no global indirection.
        let mut out = [0.0f32; NUM_BANDS];
        provider.bands(&mut out);
        assert_eq!(
            out, canned,
            "bulk bands via default impl must match canned values"
        );

        // Also verify per-band path.
        for (i, &expected) in canned.iter().enumerate() {
            assert_eq!(
                provider.band(i as u8),
                expected,
                "band({i}) must match canned[{i}]"
            );
        }
    }
}
