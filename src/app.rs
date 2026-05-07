//! Top-level application. Owns the winit event loop and holds references to
//! the output window (on the projector) and the egui control window (on the
//! primary display).
//!
//! T-M1-04 wires the bare M1 path: `EventLoop` → `ApplicationHandler::resumed`
//! brings up `GpuContext`, opens the borderless fullscreen [`OutputWindow`]
//! on monitor index 0, then constructs the [`Renderer`]. `window_event`
//! handles `CloseRequested`, Esc, `Resized` (re-configure surface), and
//! `RedrawRequested` (call into the renderer). `about_to_wait` requests
//! continuous redraws so we render at the display's vsync rate.
//!
//! Out of scope for M1: scene-recall hotkeys, blackout/freeze, the egui
//! control window, surface-error recovery beyond simple resize. T-M1-05 owns
//! surface recovery; T-M2-09 owns B/F/T keys; T-M4-14 opens the control window.
//! M6: `*.rmap.json` load/save, `--autostart`, and monitor index from project.

use std::path::PathBuf;

use crossbeam_channel::{Receiver, Sender};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::monitor::MonitorHandle;
use winit::window::WindowId;

use crate::clock::Clock;
use crate::controls::keyboard::KeyboardSource;
use crate::controls::ControlEvent;
use crate::controls::Source;
use crate::effects::blur::BlurPipeline;
use crate::effects::color::ColorPipeline;
use crate::effects::registry::ExternalRegistry;
use crate::effects::transform::TransformPipeline;
use crate::effects::RenderCtx;
use crate::error::{Result, RmapError};
use crate::project::schema::{self, Project};
use crate::project::{interpolate, restore, snapshot, snapshots_share_layer_topology, ProjectError};
use crate::render::compositor::Compositor;
use crate::render::gamma::GammaPipeline;
use crate::render::pipeline::EffectPipeline;
use crate::render::warp::WarpRenderer;
use crate::render::{GpuContext, RenderError, Renderer};
use crate::show_day::sleep_assertion::SleepAssertion;
use crate::svg_layer::SvgLayer;
use crate::svg_layer::render::SvgLayerPipeline;
use crate::svg_layer::watcher::{WatchEvent, Watcher};
use crate::svg_layer::worker::{LayerId, RasterDone, RasterJob, Worker};
use crate::test_patterns::{TestPattern, TestPatternRenderer};
use crate::windows::control::ControlWindow;
use crate::windows::control_panel::{show as control_panel_show, ControlPanelAction, ControlPanelState};
use crate::windows::output::OutputWindow;

/// Application root. Holds the persistent state across event-loop iterations.
///
/// `state` is `None` until the first `resumed` callback. macOS may fire
/// `resumed` more than once over the lifecycle (e.g. after suspend); the
/// handler guards against re-init.
pub struct App {
    /// CLI path: `*.rmap.json` (full project) or `*.svg` (single-layer bootstrap).
    project: Option<PathBuf>,
    /// With `.rmap.json`, logging + monitor selection semantics (see `resumed`).
    autostart: bool,
    /// Operator-supplied `--monitor INDEX` override.
    ///
    /// Overrides `Project.output_monitor_index` when set. When omitted, the
    /// loaded project’s index is used (default `0` for empty / SVG bootstrap).
    monitor_override: Option<usize>,
    /// `--windowed`: force decorated output window (1280×720 on chosen monitor).
    cli_windowed: bool,
    /// `--fullscreen`: force borderless fullscreen; wins over `--windowed` and project.
    cli_fullscreen: bool,
    /// Lazily-initialised GPU + window state.
    state: Option<RunningApp>,
}

/// Bundle of resources that exist only after `resumed`: the output window,
/// the renderer (which owns the [`GpuContext`]), the test-pattern renderer,
/// the optional SVG layer state, and the IOPMAssertion preventing display sleep.
struct RunningApp {
    output: OutputWindow,
    control: Option<ControlWindow>,
    renderer: Renderer,
    test_patterns: TestPatternRenderer,
    /// Live project (layers, warps, scenes, gamma). T-M5.
    project: Project,
    /// GPU runtime per `project.layers` row (order matches).
    layers: Vec<LayerState>,
    /// Shared textured-quad pipeline for SVG upload → effect source.
    svg_pipeline: SvgLayerPipeline,
    compositor: Compositor,
    /// One [`WarpRenderer`] per `project.warps` entry. Each holds its own
    /// vertex/SDF buffers; renders are chained in order with `LoadOp::Clear`
    /// for the first and `LoadOp::Load` for subsequent so non-overlapping
    /// `source_rect` regions co-exist on the same `warp_rt_view` (T-M7-02).
    /// Roadmap defers true multi-output until single-surface UX is mature;
    /// this lets multiple warps share one projector without that scope.
    warps: Vec<WarpRenderer>,
    gamma: GammaPipeline,
    warp_rt: wgpu::Texture,
    warp_rt_view: wgpu::TextureView,
    control_panel: ControlPanelState,
    clock: Clock,
    keyboard: KeyboardSource,
    color_pipeline: ColorPipeline,
    blur_pipeline: BlurPipeline,
    transform_pipeline: TransformPipeline,
    /// Extension-pass lookup for [`Effect::External`] (T-M7-07). Empty in
    /// stock v1; populated by future plugins or in-tree extensions.
    external_registry: ExternalRegistry,
    /// RAII guard for the optional cpal input stream (T-M7-03). Held on
    /// the main thread so dropping `RunningApp` stops capture; the
    /// underlying `cpal::Stream` is `!Send`, so it cannot live inside
    /// `Arc<dyn AudioProvider>` shared via the audio module's static.
    #[cfg(feature = "audio")]
    _audio_capture: Option<crate::modulators::audio::AudioCaptureGuard>,
    /// Optional MIDI input source (T-M7-05). Polled per frame alongside
    /// keyboard. Drop stops all `midir` subscriptions.
    #[cfg(feature = "midi")]
    midi: Option<crate::controls::midi::MidiSource>,
    /// Optional OSC UDP listener (T-M7-06). Polled per frame; drop joins
    /// the receive thread.
    #[cfg(feature = "osc")]
    osc: Option<crate::controls::osc::OscSource>,
    _sleep_assertion: SleepAssertion,
    /// Set when the session was started from a `*.rmap.json` CLI argument.
    #[allow(dead_code)]
    project_file_path: Option<PathBuf>,
    /// In-flight scene crossfade. `None` when no fade is active. Driven from
    /// `RedrawRequested` per frame; cleared at `t = 1`.
    crossfade: Option<ActiveCrossfade>,
}

/// One scene-to-scene fade, scheduled by `ControlEvent::SceneRecall` when
/// `Project::crossfade_duration_s > 0` and both snapshots share layer
/// topology. Holds the original endpoints so each frame's interpolation
/// re-derives the live state from immutable inputs (no error accumulation).
struct ActiveCrossfade {
    from: serde_json::Value,
    to: serde_json::Value,
    started_at: std::time::Instant,
    duration_s: f32,
}

/// Outcome from scheduling a scene recall: did the live `Project` mutate
/// in a way that requires `rebuild_layers`? Crossfade ticks never need
/// a rebuild because they are gated on identical layer topology.
#[derive(Clone, Copy)]
enum RecallOutcome {
    /// Recall hit a non-existent slot; nothing changed.
    NoSlot,
    /// Crossfade scheduled; render loop ticks it. No immediate rebuild.
    Scheduled,
    /// Instant snap landed (either zero duration or topology mismatch).
    /// Caller should rebuild GPU layer state.
    Snapped,
    /// Snap path failed inside `restore`. Project unchanged.
    Failed,
}

/// Decide whether a recall snaps instantly or schedules a crossfade. Owns
/// the topology-compatibility check; mutates `crossfade` and `project`
/// per the chosen path. Used by both the keyboard and UI recall callers
/// so the policy lives in one place.
fn schedule_scene_recall(state: &mut RunningApp, slot: usize) -> RecallOutcome {
    let target = match state.project.scenes.get(slot).map(|s| s.snapshot.clone()) {
        Some(t) => t,
        None => return RecallOutcome::NoSlot,
    };
    let cur = snapshot(&state.project);
    let dur = state.project.crossfade_duration_s.max(0.0);
    let same_topology = snapshots_share_layer_topology(&cur, &target);
    if dur < 1e-3 || !same_topology {
        match restore(&mut state.project, &target) {
            Ok(()) => {
                state.crossfade = None;
                RecallOutcome::Snapped
            }
            Err(err) => {
                tracing::warn!(?err, slot, "scene restore failed");
                RecallOutcome::Failed
            }
        }
    } else {
        state.crossfade = Some(ActiveCrossfade {
            from: cur,
            to: target,
            started_at: std::time::Instant::now(),
            duration_s: dur,
        });
        tracing::info!(slot, duration_s = dur, "scene crossfade scheduled");
        RecallOutcome::Scheduled
    }
}

/// Apply one [`ControlEvent`] to `state`. Used by the keyboard, MIDI,
/// and OSC sources so all three drive the same behavior.
///
/// `Blackout` and `Freeze` toggle the corresponding `OutputState` flag —
/// matches the keyboard's physical-key handlers (B/F) so an external
/// surface and a hotkey have identical effect on the projector. The
/// keyboard's inline B/F handlers (in `window_event`) already do this
/// directly because they want layout-independent physical-key matching;
/// for the source-poll path we toggle through here.
fn dispatch_control_event(state: &mut RunningApp, event: ControlEvent) {
    match event {
        ControlEvent::TapTempo => {
            state.clock.tap();
            tracing::debug!(bpm = state.clock.bpm(), "tap tempo");
        }
        ControlEvent::SceneRecall(idx) => {
            if matches!(schedule_scene_recall(state, idx), RecallOutcome::Snapped) {
                rebuild_layers_for_state(state);
            }
        }
        ControlEvent::Blackout => {
            state.output.state.toggle_blackout();
            tracing::info!(blackout = state.output.state.blackout, "blackout via source");
        }
        ControlEvent::Freeze => {
            state.output.state.toggle_freeze();
            tracing::info!(freeze = state.output.state.freeze, "freeze via source");
        }
        ControlEvent::ParamSet { .. } => {
            // Reserved for Param::Bound resolution (v1.5+); v1 has no consumer.
        }
    }
}

/// Rebuild GPU layer state for the current `project.layers`. Common
/// post-snap hook so the keyboard and UI recall paths stay aligned.
fn rebuild_layers_for_state(state: &mut RunningApp) {
    let device = &state.renderer.gpu.device;
    let w = state.output.config.width.max(1);
    let h = state.output.config.height.max(1);
    let fmt = state.output.config.format;
    match rebuild_layers(device, &state.project, w, h, fmt) {
        Ok(layers) => {
            state.layers = layers;
            state.control_panel.selected_layer = state
                .control_panel
                .selected_layer
                .min(state.project.layers.len().saturating_sub(1));
        }
        Err(e) => tracing::error!(?e, "rebuild layers failed"),
    }
}

/// Per-layer SVG raster + effect ping-pong + worker.
struct LayerState {
    layer: SvgLayer,
    layer_id: LayerId,
    generation: u64,
    job_tx: Sender<RasterJob>,
    result_rx: Receiver<RasterDone>,
    watch_rx: Receiver<WatchEvent>,
    _watcher: Watcher,
    effect_pipeline: EffectPipeline,
    _intermediate_texture: wgpu::Texture,
    intermediate_view: wgpu::TextureView,
    /// Separate from shared [`ColorPipeline`] so each layer’s write_buffer lands in its own memory.
    color_uniform: wgpu::Buffer,
    blur_uniform: wgpu::Buffer,
    transform_uniform: wgpu::Buffer,
    compositor_uniform: wgpu::Buffer,
}

fn create_layer_uniform_buffers(device: &wgpu::Device) -> (
    wgpu::Buffer,
    wgpu::Buffer,
    wgpu::Buffer,
    wgpu::Buffer,
) {
    let mk = |label: &'static str, size: u64| {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    };
    (
        mk("layer color uniform", 16),
        mk("layer blur uniform", 16),
        mk("layer transform uniform", 64),
        mk("layer compositor uniform", 16),
    )
}

impl App {
    pub fn run(
        project: Option<PathBuf>,
        autostart: bool,
        monitor_index: Option<usize>,
        cli_windowed: bool,
        cli_fullscreen: bool,
    ) -> Result<()> {
        let event_loop =
            EventLoop::new().map_err(|e| RmapError::Other(format!("event loop: {e}")))?;
        event_loop.set_control_flow(ControlFlow::Poll);

        let mut app = App {
            project,
            autostart,
            monitor_override: monitor_index,
            cli_windowed,
            cli_fullscreen,
            state: None,
        };

        event_loop
            .run_app(&mut app)
            .map_err(|e| RmapError::Other(format!("run_app: {e}")))?;

        Ok(())
    }

    /// Enumerate winit-reported monitors and print them to stdout, then exit.
    ///
    /// Operator-facing CLI output (driven by `--list-monitors`) — uses
    /// `println!` rather than `tracing::info!` because the user typed a
    /// command and expects its output on stdout, not in log telemetry.
    ///
    /// Implementation note: winit 0.30 only exposes `available_monitors()` on
    /// `ActiveEventLoop`, so we have to spin up a real `EventLoop` and
    /// drive it via `ApplicationHandler::resumed`. We exit immediately —
    /// no window or GPU device is created.
    pub fn print_monitors() -> Result<()> {
        let event_loop =
            EventLoop::new().map_err(|e| RmapError::Other(format!("event loop: {e}")))?;
        event_loop.set_control_flow(ControlFlow::Poll);

        let mut handler = ListMonitorsApp { printed: false };
        event_loop
            .run_app(&mut handler)
            .map_err(|e| RmapError::Other(format!("run_app: {e}")))?;
        Ok(())
    }
}

/// One-shot `ApplicationHandler` that prints the monitor list on the first
/// `resumed` callback and exits the loop. Does not open a window or
/// initialize wgpu.
struct ListMonitorsApp {
    printed: bool,
}

impl ApplicationHandler for ListMonitorsApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.printed {
            return;
        }
        self.printed = true;

        let monitors = crate::monitors::list(event_loop);
        println!("Monitors detected by winit (use --monitor INDEX to pick):");
        if monitors.is_empty() {
            println!("  (none reported)");
        } else {
            for m in &monitors {
                println!(
                    "  {}  {:?}   {}x{} @ ({},{})    scale {:.2}",
                    m.index, m.name, m.size.0, m.size.1, m.position.0, m.position.1, m.scale_factor,
                );
            }
        }

        event_loop.exit();
    }

    fn window_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        _event: WindowEvent,
    ) {
        // No window is created, so nothing to do here.
    }
}

/// Bring up the GPU and the output window, and optionally load the SVG layer.
/// Pulled into a free function so the error path in `resumed` can `?` cleanly.
fn is_rmap_project_file(path: &std::path::Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.ends_with(".rmap.json"))
}

/// Effective windowed vs fullscreen output for this session.
///
/// Precedence: `--fullscreen` → fullscreen; else `--windowed` → windowed; else if the
/// session started from a saved `.rmap.json`, honor `project.output_windowed`; else fullscreen.
fn resolve_output_windowed(
    cli_fullscreen: bool,
    cli_windowed: bool,
    project: &Project,
    loaded_from_project_file: bool,
) -> bool {
    if cli_fullscreen {
        return false;
    }
    if cli_windowed {
        return true;
    }
    loaded_from_project_file && project.output_windowed
}

fn load_project_for_startup(
    cli_path: Option<&PathBuf>,
) -> std::result::Result<(Project, Option<PathBuf>), ProjectError> {
    match cli_path {
        None => Ok((build_initial_project(None), None)),
        Some(path) => {
            if is_rmap_project_file(path) {
                let p = crate::project::Project::load(path)?;
                Ok((p, Some(path.clone())))
            } else if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("svg")) {
                Ok((build_initial_project(Some(path.clone())), None))
            } else {
                tracing::warn!(
                    path = %path.display(),
                    "expected *.rmap.json or *.svg; starting empty project",
                );
                Ok((build_initial_project(None), None))
            }
        }
    }
}

fn init_running_app(
    event_loop: &ActiveEventLoop,
    monitor: Option<MonitorHandle>,
    project: Project,
    project_file_path: Option<PathBuf>,
    output_windowed: bool,
) -> Result<RunningApp> {
    let gpu = GpuContext::new()?;
    let output = OutputWindow::new(
        event_loop,
        monitor,
        &gpu.instance,
        &gpu.adapter,
        &gpu.device,
        output_windowed,
    )?;
    let control = match ControlWindow::new(event_loop, &gpu.instance, &gpu.adapter, &gpu.device) {
        Ok(c) => Some(c),
        Err(e) => {
            tracing::warn!(
                ?e,
                "control window init failed; continuing without it (D-01 fallback)"
            );
            None
        }
    };
    let surface_format = output.config.format;
    let test_patterns = TestPatternRenderer::new(&gpu.device, surface_format);
    let color_pipeline = ColorPipeline::new(&gpu.device, surface_format);
    let blur_pipeline = BlurPipeline::new(&gpu.device, surface_format);
    let transform_pipeline = TransformPipeline::new(&gpu.device, surface_format);
    let renderer = Renderer::new(gpu, surface_format)?;
    let sleep_assertion = SleepAssertion::acquire("rmap output window");

    // T-M7-03: bring up the audio capture provider when the `audio` feature
    // is on. Failure to open the input device is non-fatal — a wedding
    // venue without a mic still wants the projector running.
    #[cfg(feature = "audio")]
    let audio_capture = match crate::modulators::audio::start_default() {
        Ok((provider, guard)) => {
            crate::modulators::audio::install(std::sync::Arc::new(provider));
            tracing::info!("audio capture started; Modulator::Audio bands live");
            Some(guard)
        }
        Err(err) => {
            tracing::warn!(?err, "audio init failed; Modulator::Audio will read 0.0");
            None
        }
    };

    // T-M7-05: subscribe to every MIDI input port. Empty port list is fine
    // (Source produces no events); only init failure of midir itself is logged.
    #[cfg(feature = "midi")]
    let midi = match crate::controls::midi::MidiSource::start_all() {
        Ok(src) => Some(src),
        Err(err) => {
            tracing::warn!(?err, "midi init failed; midi events disabled");
            None
        }
    };

    // T-M7-06: bind UDP for OSC. Default port from the controls::osc module;
    // future work can plumb the port through Project / CLI.
    #[cfg(feature = "osc")]
    let osc = match crate::controls::osc::OscSource::start(0) {
        Ok(src) => Some(src),
        Err(err) => {
            tracing::warn!(?err, "osc bind failed; osc events disabled");
            None
        }
    };

    let mut control_panel = ControlPanelState::default();
    if let Some(ref p) = project_file_path {
        control_panel.project_save_path = p.display().to_string();
    }

    let w = output.config.width.max(1);
    let h = output.config.height.max(1);
    let svg_pipeline = SvgLayerPipeline::new(&renderer.gpu.device, surface_format);
    let compositor = Compositor::new(&renderer.gpu.device, w, h, surface_format);
    // T-M7-02: one WarpRenderer per project.warps entry. Project::load
    // guarantees a default warp when the file omits `warps`, so this Vec
    // is always non-empty after init.
    let warp_count = project.warps.len().max(1);
    let warps: Vec<WarpRenderer> = (0..warp_count)
        .map(|_| WarpRenderer::new(&renderer.gpu.device, surface_format))
        .collect();
    let gamma = GammaPipeline::new(&renderer.gpu.device, surface_format);
    let (warp_rt, warp_rt_view) = make_warp_render_target(&renderer.gpu.device, w, h, surface_format);
    let layers = rebuild_layers(&renderer.gpu.device, &project, w, h, surface_format)?;

    Ok(RunningApp {
        output,
        control,
        renderer,
        test_patterns,
        project,
        layers,
        svg_pipeline,
        compositor,
        warps,
        gamma,
        warp_rt,
        warp_rt_view,
        control_panel,
        clock: Clock::new(),
        keyboard: KeyboardSource::new(),
        color_pipeline,
        blur_pipeline,
        transform_pipeline,
        external_registry: ExternalRegistry::new(),
        #[cfg(feature = "audio")]
        _audio_capture: audio_capture,
        #[cfg(feature = "midi")]
        midi,
        #[cfg(feature = "osc")]
        osc,
        _sleep_assertion: sleep_assertion,
        project_file_path,
        crossfade: None,
    })
}

fn build_initial_project(svg_path: Option<PathBuf>) -> Project {
    let mut project = Project::default();
    if let Some(path) = svg_path.filter(|p| p.extension().is_some_and(|e| e == "svg")) {
        project.layers.push(schema::layer_from_svg_path("layer0", path));
    }
    if project.warps.is_empty() {
        project.warps.push(schema::default_warp_mesh());
    }
    project
}

fn rebuild_layers(
    device: &wgpu::Device,
    project: &Project,
    width: u32,
    height: u32,
    surface_format: wgpu::TextureFormat,
) -> Result<Vec<LayerState>> {
    let mut out = Vec::with_capacity(project.layers.len());
    for lc in project.layers.iter() {
        let asset_path = lc.kind.asset_path().to_path_buf();
        let layer = SvgLayer::pending(asset_path.clone());
        let (job_tx, result_rx) = Worker::spawn();
        let (watcher, watch_rx) = Watcher::new(std::slice::from_ref(&asset_path))?;
        let effect_pipeline =
            EffectPipeline::new(device, width.max(1), height.max(1), surface_format);
        let (intermediate_texture, intermediate_view) =
            make_intermediate_texture(device, width.max(1), height.max(1), surface_format);
        let layer_id = LayerId::next();
        let generation = 1u64;
        let _ = job_tx.send(RasterJob {
            layer_id,
            path: asset_path.clone(),
            size: (width, height),
            generation,
        });
        let (color_uniform, blur_uniform, transform_uniform, compositor_uniform) =
            create_layer_uniform_buffers(device);
        out.push(LayerState {
            layer,
            layer_id,
            generation,
            job_tx,
            result_rx,
            watch_rx,
            _watcher: watcher,
            effect_pipeline,
            _intermediate_texture: intermediate_texture,
            intermediate_view,
            color_uniform,
            blur_uniform,
            transform_uniform,
            compositor_uniform,
        });
    }
    Ok(out)
}

fn make_warp_render_target(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("warp rt"),
        size: wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}

fn resize_m5_gpu(state: &mut RunningApp) {
    let w = state.output.config.width.max(1);
    let h = state.output.config.height.max(1);
    let device = &state.renderer.gpu.device;
    let fmt = state.output.config.format;
    state.compositor.resize(device, w, h);
    let (tex, view) = make_warp_render_target(device, w, h, fmt);
    state.warp_rt = tex;
    state.warp_rt_view = view;
    for (i, layer) in state.layers.iter_mut().enumerate() {
        if i >= state.project.layers.len() {
            break;
        }
        layer.effect_pipeline.resize(device, w, h);
        let (itex, iview) = make_intermediate_texture(device, w, h, fmt);
        layer._intermediate_texture = itex;
        layer.intermediate_view = iview;
        layer.generation = layer.generation.wrapping_add(1);
        let path = state.project.layers[i].kind.asset_path().to_path_buf();
        let _ = layer.job_tx.send(RasterJob {
            layer_id: layer.layer_id,
            path,
            size: (w, h),
            generation: layer.generation,
        });
    }
}

/// Allocate the scratch intermediate texture used by multi-pass effects
/// (currently only blur). Same format / dimensions as the per-layer
/// ping-pong textures.
fn make_intermediate_texture(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("effect intermediate"),
        size: wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}

/// Acquire the next surface texture and translate wgpu 29's
/// [`wgpu::CurrentSurfaceTexture`] discriminants into the same
/// `Result<Option<SurfaceTexture>, RenderError>` shape used by
/// [`Renderer::render_frame`]. `Ok(None)` means "drop this frame
/// silently" (Timeout / Occluded); `Ok(Some(_))` means draw.
///
/// Mirrors `Renderer::render_frame` arm-for-arm so the App's recovery
/// arms behave identically regardless of which render path was active
/// when the surface fell over. (A future M3+ refactor can hoist this
/// onto `OutputWindow` as `acquire_frame`; M2 keeps the duplication
/// localized rather than touching `render::mod`.)
fn acquire_frame(
    output: &OutputWindow,
) -> std::result::Result<Option<wgpu::SurfaceTexture>, RenderError> {
    match output.surface.get_current_texture() {
        wgpu::CurrentSurfaceTexture::Success(f) => Ok(Some(f)),
        wgpu::CurrentSurfaceTexture::Suboptimal(_) => Err(RenderError::SurfaceSuboptimal),
        wgpu::CurrentSurfaceTexture::Timeout => {
            tracing::warn!("surface acquire timed out; dropping frame");
            Ok(None)
        }
        wgpu::CurrentSurfaceTexture::Occluded => {
            tracing::trace!("surface occluded; skipping frame");
            Ok(None)
        }
        wgpu::CurrentSurfaceTexture::Outdated => Err(RenderError::SurfaceOutdated),
        wgpu::CurrentSurfaceTexture::Lost => Err(RenderError::SurfaceLost),
        wgpu::CurrentSurfaceTexture::Validation => Err(RenderError::Surface(
            "surface acquire validation error".into(),
        )),
    }
}

/// Render a single black frame to the output window. Used when the
/// operator hits Blackout (`B`). Wrapped in
/// `panic_restore::run_frame_assert_unwind_safe` so a panic mid-frame
/// converts to `RenderError::RenderPanic` and reaches the App's
/// recovery arm — same contract as `Renderer::render_frame`.
fn render_blackout(
    renderer: &Renderer,
    output: &OutputWindow,
) -> std::result::Result<(), RenderError> {
    crate::show_day::panic_restore::run_frame_assert_unwind_safe(|| {
        let frame = match acquire_frame(output)? {
            Some(f) => f,
            None => return Ok(()),
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder =
            renderer
                .gpu
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("blackout encoder"),
                });
        // Single empty render pass with Clear → BLACK. No draw call: the
        // clear *is* the frame.
        let _ = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("blackout pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        renderer.gpu.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
        Ok(())
    })
}

/// Render the chosen test pattern via [`TestPatternRenderer::render`].
/// Same wgpu boilerplate as `render_blackout`; differs only in the
/// inner pass body, which is delegated.
fn render_test_pattern(
    renderer: &Renderer,
    output: &OutputWindow,
    test_patterns: &TestPatternRenderer,
    pattern: TestPattern,
) -> std::result::Result<(), RenderError> {
    crate::show_day::panic_restore::run_frame_assert_unwind_safe(|| {
        let frame = match acquire_frame(output)? {
            Some(f) => f,
            None => return Ok(()),
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder =
            renderer
                .gpu
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("test_pattern encoder"),
                });
        test_patterns.render(pattern, &mut encoder, &view);
        renderer.gpu.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
        Ok(())
    })
}

/// Present the swapchain texture on drop so a panic after acquire cannot
/// strand wgpu in "Surface image is already acquired".
struct SurfacePresentGuard(Option<wgpu::SurfaceTexture>);

impl SurfacePresentGuard {
    fn new(frame: wgpu::SurfaceTexture) -> Self {
        Self(Some(frame))
    }

    fn texture_view(&self) -> wgpu::TextureView {
        self.0
            .as_ref()
            .expect("surface present guard: missing frame")
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default())
    }

    fn present(mut self) {
        if let Some(f) = self.0.take() {
            f.present();
        }
    }
}

impl Drop for SurfacePresentGuard {
    fn drop(&mut self) {
        if let Some(f) = self.0.take() {
            f.present();
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn render_m5_pipeline(
    renderer: &Renderer,
    output: &OutputWindow,
    project: &Project,
    layers: &mut [LayerState],
    svg_pipeline: &SvgLayerPipeline,
    compositor: &Compositor,
    warps: &mut Vec<WarpRenderer>,
    gamma: &GammaPipeline,
    warp_rt_view: &wgpu::TextureView,
    color: &ColorPipeline,
    blur: &BlurPipeline,
    transform: &TransformPipeline,
    external_registry: &ExternalRegistry,
    surface_format: wgpu::TextureFormat,
    clock: &Clock,
) -> std::result::Result<(), RenderError> {
    crate::show_day::panic_restore::run_frame_assert_unwind_safe(|| {
        // T-M7-02: ensure WarpRenderer count matches project.warps. Most
        // frames this is a no-op compare; only scene recall / project load
        // can change the warp count.
        let want = project.warps.len().max(1);
        if warps.len() != want {
            warps.resize_with(want, || WarpRenderer::new(&renderer.gpu.device, surface_format));
        }

        let mut encoder = renderer.gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("m5 offscreen encoder"),
        });

        let mut composite_inputs: Vec<(&wgpu::TextureView, schema::BlendMode, f32, &wgpu::Buffer)> =
            Vec::with_capacity(project.layers.len());

        if project.layers.len() != layers.len() {
            tracing::warn!(
                project_layers = project.layers.len(),
                gpu_layers = layers.len(),
                "project.layers and GPU LayerState count differ; rebuild layers (render uses zipped pairs only)",
            );
        }

        for (cfg, ls) in project.layers.iter().zip(layers.iter_mut()) {
            if !cfg.enabled {
                continue;
            }
            let Some(tex_view) = ls.layer.texture_view() else {
                continue;
            };
            ls.effect_pipeline.reset_for_layer_pass();
            {
                let (src_view, _dst_view) = ls.effect_pipeline.current_pair();
                svg_pipeline.render(&renderer.gpu.device, &mut encoder, src_view, tex_view);
            }
            for effect in &cfg.effects {
                {
                    let (src, dst) = ls.effect_pipeline.current_pair();
                    let mut ctx = RenderCtx {
                        device: &renderer.gpu.device,
                        queue: &renderer.gpu.queue,
                        encoder: &mut encoder,
                        source_view: src,
                        dst_view: dst,
                        intermediate_view: &ls.intermediate_view,
                        color,
                        blur,
                        transform,
                        color_uniform: &ls.color_uniform,
                        blur_uniform: &ls.blur_uniform,
                        transform_uniform: &ls.transform_uniform,
                        external_registry,
                    };
                    if effect.render(&mut ctx, clock) {
                        ls.effect_pipeline.flip();
                    }
                }
            }
            composite_inputs.push((
                ls.effect_pipeline.final_view(),
                cfg.blend_mode,
                cfg.opacity,
                &ls.compositor_uniform,
            ));
        }

        let c = project.background_color;
        let bg = wgpu::Color {
            r: c[0] as f64,
            g: c[1] as f64,
            b: c[2] as f64,
            a: c[3] as f64,
        };
        let composed = compositor.composite(
            &renderer.gpu.device,
            &renderer.gpu.queue,
            &mut encoder,
            bg,
            &composite_inputs,
        );

        // Iterate every warp in `project.warps`. First pass clears the
        // shared `warp_rt_view`; subsequent passes use `LoadOp::Load` so
        // disjoint `source_rect` regions co-exist on one output. Where
        // dst quads overlap the second warp's REPLACE write wins
        // (matches roadmap-deferred multi-output simplification: this is
        // multi-region, not multi-projector).
        let default_mesh = schema::default_warp_mesh();
        for (i, warp_renderer) in warps.iter_mut().enumerate() {
            let mesh_ref: &schema::WarpMesh = project.warps.get(i).unwrap_or(&default_mesh);
            warp_renderer.sync_mesh_and_mask(&renderer.gpu.device, &renderer.gpu.queue, mesh_ref);
            let load = if i == 0 {
                wgpu::LoadOp::Clear(wgpu::Color::BLACK)
            } else {
                wgpu::LoadOp::Load
            };
            warp_renderer.render(
                &renderer.gpu.device,
                &renderer.gpu.queue,
                &mut encoder,
                warp_rt_view,
                composed,
                mesh_ref,
                load,
            );
        }

        renderer.gpu.queue.submit(std::iter::once(encoder.finish()));

        let frame = match acquire_frame(output)? {
            Some(f) => f,
            None => return Ok(()),
        };
        let guard = SurfacePresentGuard::new(frame);
        {
            let surface_view = guard.texture_view();
            let mut enc_gamma =
                renderer
                    .gpu
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("m5 gamma encoder"),
                    });
            gamma.render(
                &renderer.gpu.device,
                &renderer.gpu.queue,
                &mut enc_gamma,
                &surface_view,
                warp_rt_view,
                project.gamma,
                project.brightness,
                project.contrast,
                wgpu::LoadOp::Clear(wgpu::Color::BLACK),
            );
            renderer
                .gpu
                .queue
                .submit(std::iter::once(enc_gamma.finish()));
        }
        guard.present();
        Ok(())
    })
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            // macOS can fire `resumed` more than once on lifecycle changes;
            // the first call already brought everything up.
            return;
        }

        let (project, project_file_path) = match load_project_for_startup(self.project.as_ref()) {
            Ok(v) => v,
            Err(e) => {
                tracing::error!(?e, "failed to load project file");
                event_loop.exit();
                return;
            }
        };

        // `--monitor` overrides [`Project::output_monitor_index`] from the file.
        let monitor_index = self
            .monitor_override
            .unwrap_or(project.output_monitor_index);
        let monitor = event_loop.available_monitors().nth(monitor_index);
        if monitor.is_none() {
            tracing::warn!(
                requested = monitor_index,
                available = event_loop.available_monitors().count(),
                "requested monitor index out of range; falling back to platform default",
            );
        }

        if self.autostart && self.project.as_ref().is_some_and(|p| is_rmap_project_file(p)) {
            tracing::info!(
                monitor_index,
                project_path = ?self.project,
                "autostart: loaded .rmap.json; output targets monitor index (unless --monitor)",
            );
        }

        let output_windowed = resolve_output_windowed(
            self.cli_fullscreen,
            self.cli_windowed,
            &project,
            project_file_path.is_some(),
        );
        tracing::info!(
            output_windowed,
            monitor_index,
            "output presentation (windowed = decorated window on monitor)",
        );

        match init_running_app(
            event_loop,
            monitor,
            project,
            project_file_path,
            output_windowed,
        ) {
            Ok(running) => {
                self.state = Some(running);
            }
            Err(e) => {
                tracing::error!(?e, "init failed; exiting");
                event_loop.exit();
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(state) = self.state.as_mut() else {
            return;
        };

        // T-M4-14: handle events for the egui control window first. If the
        // event belongs to the control window, handle it and return — do NOT
        // fall through to the output-window arms below.
        if state.control.as_ref().is_some_and(|c| c.id() == window_id) {
            let ctrl = state.control.as_mut().unwrap();
            let _ = ctrl.on_window_event(&event);
            match event {
                WindowEvent::Resized(new_size) => {
                    ctrl.resize(&state.renderer.gpu.device, new_size);
                }
                WindowEvent::CloseRequested => {
                    // Drop the control window without exiting the app.
                    state.control = None;
                }
                WindowEvent::RedrawRequested => {
                    let device = &state.renderer.gpu.device;
                    let queue = &state.renderer.gpu.queue;
                    let mut panel_action = ControlPanelAction::None;
                    if let Some(ctrl) = state.control.as_mut() {
                        let result = ctrl.render(device, queue, |ui| {
                            panel_action =
                                control_panel_show(ui, &mut state.project, &mut state.control_panel);
                        });
                        if let Err(e) = result {
                            tracing::warn!(?e, "control window render error");
                        }
                    }
                    match panel_action {
                        ControlPanelAction::None => {}
                        ControlPanelAction::RebuildLayers => {
                            rebuild_layers_for_state(state);
                        }
                        ControlPanelAction::SceneRecall(slot) => {
                            if matches!(
                                schedule_scene_recall(state, slot),
                                RecallOutcome::Snapped
                            ) {
                                rebuild_layers_for_state(state);
                            }
                        }
                    }
                }
                _ => {}
            }
            return;
        }

        // Guard: only act on events for the output window from here down.
        if window_id != state.output.window.id() {
            return;
        }

        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::KeyboardInput {
                event: key_event, ..
            } if key_event.state == ElementState::Pressed => {
                // Use `physical_key` so the bindings are layout-
                // independent: a French-AZERTY operator still hits the
                // same physical keys for Blackout / Freeze / cycle Test
                // Pattern. Letter keys arrive as `Key::Character` (not
                // `Named`), so logical-key matching is not reliable for
                // single letters across layouts.
                match key_event.physical_key {
                    PhysicalKey::Code(KeyCode::Escape) => event_loop.exit(),
                    PhysicalKey::Code(KeyCode::KeyB) => {
                        state.output.state.toggle_blackout();
                        tracing::info!(blackout = state.output.state.blackout, "blackout toggled");
                    }
                    PhysicalKey::Code(KeyCode::KeyF) => {
                        state.output.state.toggle_freeze();
                        tracing::info!(freeze = state.output.state.freeze, "freeze toggled");
                    }
                    PhysicalKey::Code(KeyCode::KeyT) => {
                        state.output.state.cycle_test_pattern();
                        tracing::info!(
                            pattern = state.output.state.test_pattern.label(),
                            "test pattern"
                        );
                    }
                    _ => {}
                }
                // T-M4-10: buffer every pressed event into KeyboardSource for
                // the source-based control path. Unmapped keys (Escape, T, …)
                // are silently dropped inside push_winit_key.
                state.keyboard.push_winit_key(key_event.physical_key);
            }
            WindowEvent::Resized(new_size) => {
                state.output.config.width = new_size.width.max(1);
                state.output.config.height = new_size.height.max(1);
                state.output.recreate_surface(&state.renderer.gpu.device);
                resize_m5_gpu(state);
            }
            WindowEvent::RedrawRequested => {
                // Drain every registered source through one common dispatcher.
                // Order doesn't matter for v1 — each event is independent.
                #[cfg_attr(
                    not(any(feature = "midi", feature = "osc")),
                    allow(unused_mut)
                )]
                let mut events: Vec<ControlEvent> = state.keyboard.poll();
                #[cfg(feature = "midi")]
                if let Some(midi) = state.midi.as_mut() {
                    events.extend(crate::controls::Source::poll(midi));
                }
                #[cfg(feature = "osc")]
                if let Some(osc) = state.osc.as_mut() {
                    events.extend(crate::controls::Source::poll(osc));
                }
                for e in events {
                    dispatch_control_event(state, e);
                }

                for (i, ls) in state.layers.iter_mut().enumerate() {
                    while let Ok(_event) = ls.watch_rx.try_recv() {
                        ls.generation = ls.generation.wrapping_add(1);
                        let size = (state.output.config.width, state.output.config.height);
                        let path = state.project.layers[i].kind.asset_path().to_path_buf();
                        let layer_id = ls.layer_id;
                        let generation = ls.generation;
                        let _ = ls.job_tx.send(RasterJob {
                            layer_id,
                            path,
                            size,
                            generation,
                        });
                        tracing::debug!(
                            generation = ls.generation,
                            layer = i,
                            "svg watcher fired; enqueued raster job"
                        );
                    }
                    while let Ok(done) = ls.result_rx.try_recv() {
                        if done.layer_id != ls.layer_id || done.generation != ls.generation {
                            tracing::debug!(
                                done_gen = done.generation,
                                current_gen = ls.generation,
                                "stale raster result dropped",
                            );
                            continue;
                        }
                        ls.layer.generation = done.generation;
                        if let Err(e) = ls.layer.upload(
                            &state.renderer.gpu.device,
                            &state.renderer.gpu.queue,
                            &done.pixmap,
                        ) {
                            tracing::warn!(?e, "svg gpu upload failed");
                        } else {
                            tracing::debug!(generation = done.generation, "svg uploaded to gpu");
                        }
                    }
                }

                // T-M7-04: tick the in-flight scene crossfade. Topology was
                // already verified at scheduling time so neither endpoint
                // changes layer count or paths — no `rebuild_layers` needed
                // mid-fade. Numeric fields blend via `interpolate`; categorical
                // fields snap at `t = 0.5`.
                if let Some(cf) = state.crossfade.as_ref() {
                    let elapsed = cf.started_at.elapsed().as_secs_f32();
                    let t = (elapsed / cf.duration_s.max(1e-3)).clamp(0.0, 1.0);
                    let interp = interpolate(&cf.from, &cf.to, t);
                    if let Err(err) = restore(&mut state.project, &interp) {
                        tracing::warn!(?err, "crossfade tick restore failed; aborting fade");
                        state.crossfade = None;
                    } else if t >= 1.0 {
                        state.crossfade = None;
                    }
                }

                // Render-order priority per spec §3.6 / T-M2-09:
                // blackout > freeze > test_pattern > svg > normal.
                // Blackout wins over freeze: if the operator hits B
                // while frozen, the projector goes black immediately
                // rather than continuing to show the frozen frame.
                let result = if state.output.state.blackout {
                    render_blackout(&state.renderer, &state.output)
                } else if state.output.state.freeze {
                    // Freeze: skip rendering entirely. The window keeps
                    // showing its last presented frame because we
                    // never call `frame.present()` again. Pragmatic M2
                    // implementation; a perfect "freeze" would copy
                    // and re-present the last framebuffer every frame.
                    Ok(())
                } else if state.output.state.test_pattern != TestPattern::None {
                    render_test_pattern(
                        &state.renderer,
                        &state.output,
                        &state.test_patterns,
                        state.output.state.test_pattern,
                    )
                } else if !state.project.layers.is_empty() {
                    let surface_format = state.output.config.format;
                    render_m5_pipeline(
                        &state.renderer,
                        &state.output,
                        &state.project,
                        &mut state.layers,
                        &state.svg_pipeline,
                        &state.compositor,
                        &mut state.warps,
                        &state.gamma,
                        &state.warp_rt_view,
                        &state.color_pipeline,
                        &state.blur_pipeline,
                        &state.transform_pipeline,
                        &state.external_registry,
                        surface_format,
                        &state.clock,
                    )
                } else {
                    state.renderer.render_frame(&state.output)
                };
                match result {
                    Ok(()) => {}
                    Err(RenderError::SurfaceLost) => {
                        tracing::warn!("surface lost; recreating");
                        state.output.recreate_surface(&state.renderer.gpu.device);
                    }
                    Err(RenderError::SurfaceOutdated) => {
                        tracing::warn!("surface outdated; recreating");
                        state.output.recreate_surface(&state.renderer.gpu.device);
                    }
                    Err(RenderError::SurfaceSuboptimal) => {
                        tracing::warn!("surface suboptimal; recreating");
                        state.output.recreate_surface(&state.renderer.gpu.device);
                    }
                    Err(RenderError::RenderPanic { message }) => {
                        tracing::error!(%message, "renderer panicked; recovered");
                        crate::windows::control::error_overlay(&message);
                    }
                    Err(e) => {
                        tracing::error!(?e, "render error");
                    }
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(state) = self.state.as_ref() {
            state.output.window.request_redraw();
            if let Some(ctrl) = state.control.as_ref() {
                ctrl.window.request_redraw();
            }
        }
    }
}
