//! Compile-time WGSL validation. Per the spec, every shader is parsed and
//! validated by naga during `cargo build`, so a broken shader fails the build
//! instead of crashing the renderer at startup.
//!
//! P0.5.2: Some shaders are "consumers" of sdf_helper.wgsl — they call
//! `sample_sdf_bilinear` / `sample_sdf_gradient` / `sample_sdf` which are
//! defined in the helper and prepended at runtime (see warp.rs). For build-time
//! validation to exercise the same merged source, we replicate that
//! concatenation here.
//!
//! `SDF_CONSUMERS` lists basename prefixes that need the helper prepended.
//! P0.5.3 will add "fx_" for fx_ripple_wash.wgsl — a one-line extension.
//!
//! P3.3.1: zone-aware presets (`fx_zone_` prefix) need BOTH `sdf_helper.wgsl`
//! AND `zone_tag_helper.wgsl` prepended, in that order. `ZONE_CONSUMERS` lists
//! these prefixes; they are also in `SDF_CONSUMERS` so both helpers are added.
//!
//! PCleanup.2.4: Treatment-particle compute shaders need BOTH `sdf_helper.wgsl`
//! AND `treatment_particles_helper.wgsl` prepended, in that order (SDF first so
//! `sample_sdf_bilinear` is defined before the particle helper references it;
//! the particle helper itself is function-only and references SDF helpers
//! indirectly through compute shaders). `TREATMENT_PARTICLE_CONSUMERS` lists
//! shaders that need all three layers: SDF + particle helper + shader source.
//! The fragment shader (`treat_spotlights.wgsl`) only needs the particle
//! helper (not the SDF helper) — it is listed in `PARTICLE_ONLY_CONSUMERS`.

use std::fs;
use std::path::Path;

/// Basename prefixes whose source must be prefixed with sdf_helper.wgsl
/// for validation. Mirrors the runtime concatenation in warp.rs (and future
/// fx_presets.rs). Extend this list in P0.5.3 when fx_ripple_wash.wgsl ships.
const SDF_CONSUMERS: &[&str] = &[
    "warp",
    "fx_",
    "treat_blur",
    "treat_displacement",
    "treat_refraction",
    // PCleanup.2.1 — `treat_ripple_lens.wgsl` consumes the SDF helper
    // (sample_sdf_bilinear + sample_sdf_normal) to drive concentric-
    // ring UV displacement keyed to mask distance.
    "treat_ripple_lens",
    // PCleanup.2.2 — `treat_edge_lens.wgsl` consumes sample_sdf_normal
    // to drive N traveling refraction bumps around the mask boundary.
    "treat_edge_lens",
    // PCleanup.2.7 — `treat_field_advect.wgsl` consumes sample_sdf_gradient
    // to advect the source image along the mask's normal field.
    "treat_field_advect",
    // PCleanup.2.9 — `treat_zone_brighten.wgsl` consumes sample_sdf_bilinear
    // to gate the brightness boost by distance-to-edge, and ZoneTagUniform
    // to restrict the effect to ZONE_WINDOW layers.
    "treat_zone_",
];

/// P3.3.1 — Basename prefixes that additionally need zone_tag_helper.wgsl
/// prepended after sdf_helper.wgsl. Zone-aware preset shaders declare
/// `@group(0) @binding(6) var<uniform> u_zone: ZoneTagUniform;` in their
/// own source; the helper provides the constants and struct.
/// PCleanup.2.9 adds `"treat_zone_"` alongside `"fx_zone_"` so that
/// `treat_zone_brighten.wgsl` (and future zone-aware treatment siblings)
/// get both helpers prepended during build-time validation.
const ZONE_CONSUMERS: &[&str] = &["fx_zone_", "treat_zone_"];

/// PCleanup.8.3a — Basename prefixes that need ONLY zone_tag_helper.wgsl
/// prepended (not the SDF helper). Used for treatments that consume the
/// ZoneTagUniform but do not call any `sample_sdf_*` functions.
const ZONE_ONLY_CONSUMERS: &[&str] = &["treat_palette_extract"];

/// PCleanup.2.4 — Treatment compute shaders that need SDF helper +
/// treatment_particles_helper.wgsl prepended (in that order, SDF first).
/// These are compute shaders that both call `sample_sdf_bilinear` AND use
/// the `Particle` struct from the particle helper.
const TREATMENT_PARTICLE_COMPUTE_CONSUMERS: &[&str] = &[
    "treat_spotlights_compute",
    "treat_edge_sparks_compute",
    "treat_collision_ripples_compute",
];

/// W2 — Treatment fragment shaders that need ONLY the particle helper
/// prepended (no SDF calls in the fragment pass).
const TREATMENT_PARTICLE_FRAG_CONSUMERS: &[&str] = &[
    "treat_spotlights",
    "treat_drift_pinholes",
    "treat_drift_brushstrokes",
    "treat_edge_sparks",
    "treat_collision_ripples",
    "treat_portal_warp",
];

fn main() {
    // P7.2.1 — Syphon.framework linkage scaffold.
    //
    // When `--features syphon-out` is passed, emit the linker search path and
    // framework name so the ObjC wrapper (future W2.2) can call Syphon symbols.
    //
    // The framework binary is NOT checked in (it is ~800 KB; it ships from
    // https://github.com/Syphon/Syphon-Framework/releases).  Place the unpacked
    // framework at:
    //
    //     vendor/frameworks/Syphon.framework/
    //
    // Then rebuild.  The directory is gitignored by extension (vendor/frameworks/
    // contains only the .gitkeep placeholder in the repo).
    //
    // `cargo build --no-default-features` must remain successful; this block is
    // gated on the feature flag, which is off by default.
    #[cfg(feature = "syphon-out")]
    {
        println!("cargo:rerun-if-changed=vendor/frameworks/Syphon.framework");
        println!("cargo:rustc-link-search=framework=vendor/frameworks");
        println!("cargo:rustc-link-lib=framework=Syphon");
    }

    let shader_dir = Path::new("src/render/shaders");
    println!("cargo:rerun-if-changed=src/render/shaders");

    if !shader_dir.exists() {
        return;
    }

    let helper_path = shader_dir.join("sdf_helper.wgsl");
    let sdf_helper_src = if helper_path.exists() {
        fs::read_to_string(&helper_path).expect("read sdf_helper.wgsl")
    } else {
        String::new()
    };

    // P3.3.1 — zone-tag helper for zone-aware preset shaders.
    let zone_helper_path = shader_dir.join("zone_tag_helper.wgsl");
    let zone_helper_src = if zone_helper_path.exists() {
        fs::read_to_string(&zone_helper_path).expect("read zone_tag_helper.wgsl")
    } else {
        String::new()
    };

    // PCleanup.2.4 — treatment-particle helper (Particle struct + hash fns).
    let tp_helper_path = shader_dir.join("treatment_particles_helper.wgsl");
    let tp_helper_src = if tp_helper_path.exists() {
        fs::read_to_string(&tp_helper_path).expect("read treatment_particles_helper.wgsl")
    } else {
        String::new()
    };

    for entry in fs::read_dir(shader_dir).expect("read shader dir") {
        let path = entry.expect("dir entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("wgsl") {
            continue;
        }
        println!("cargo:rerun-if-changed={}", path.display());

        let basename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();

        // sdf_helper.wgsl, zone_tag_helper.wgsl, and
        // treatment_particles_helper.wgsl are function-only modules (no entry
        // points). Validate them standalone; consumer shaders get them
        // prepended.
        let src = fs::read_to_string(&path).expect("read shader source");

        let is_sdf_consumer = SDF_CONSUMERS
            .iter()
            .any(|prefix| basename.starts_with(prefix));

        let is_zone_consumer = ZONE_CONSUMERS
            .iter()
            .any(|prefix| basename.starts_with(prefix));

        // PCleanup.8.3a — shaders that need ONLY the zone-tag helper (no SDF).
        let is_zone_only_consumer = ZONE_ONLY_CONSUMERS
            .iter()
            .any(|prefix| basename.starts_with(prefix));

        // PCleanup.2.4 — treatment-particle consumers.
        let is_tp_compute_consumer = TREATMENT_PARTICLE_COMPUTE_CONSUMERS
            .iter()
            .any(|prefix| basename.starts_with(prefix));
        let is_tp_frag_consumer = TREATMENT_PARTICLE_FRAG_CONSUMERS
            .iter()
            .any(|&name| basename == format!("{name}.wgsl"));

        // Build the validated source. Order: SDF helper → zone helper (if
        // needed) → particle helper (if needed) → shader source.
        // Matches the runtime concat order.
        let validated_src = if is_tp_compute_consumer {
            // SDF + particle helper + compute shader source.
            format!("{}\n{}\n{}", sdf_helper_src, tp_helper_src, src)
        } else if is_tp_frag_consumer {
            // Particle helper only + fragment shader source.
            format!("{}\n{}", tp_helper_src, src)
        } else if is_zone_only_consumer {
            // PCleanup.8.3a — zone-tag helper only (no SDF helper needed).
            // Used for treatments that consume ZoneTagUniform but do not
            // call any `sample_sdf_*` functions.
            format!("{}\n{}", zone_helper_src, src)
        } else {
            match (is_sdf_consumer, is_zone_consumer) {
                (true, true) => format!("{}\n{}\n{}", sdf_helper_src, zone_helper_src, src),
                (true, false) if !sdf_helper_src.is_empty() => {
                    format!("{}\n{}", sdf_helper_src, src)
                }
                _ => src,
            }
        };

        let module = match naga::front::wgsl::parse_str(&validated_src) {
            Ok(m) => m,
            Err(e) => panic!("WGSL parse error in {}:\n{}", path.display(), e),
        };

        let mut validator = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        );
        if let Err(e) = validator.validate(&module) {
            panic!("WGSL validation error in {}: {:?}", path.display(), e);
        }
    }
}
