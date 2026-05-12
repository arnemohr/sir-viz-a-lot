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

use std::fs;
use std::path::Path;

/// Basename prefixes whose source must be prefixed with sdf_helper.wgsl
/// for validation. Mirrors the runtime concatenation in warp.rs (and future
/// fx_presets.rs). Extend this list in P0.5.3 when fx_ripple_wash.wgsl ships.
const SDF_CONSUMERS: &[&str] = &["warp", "fx_", "treat_blur", "treat_displacement"];

fn main() {
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

        // sdf_helper.wgsl is a function-only module (no entry points). Validate
        // it standalone so naga confirms the helper itself is well-formed.
        // Consumer shaders get the helper prepended (see SDF_CONSUMERS).
        let src = fs::read_to_string(&path).expect("read shader source");

        let is_consumer = SDF_CONSUMERS
            .iter()
            .any(|prefix| basename.starts_with(prefix));

        let validated_src = if is_consumer && !sdf_helper_src.is_empty() {
            format!("{}\n{}", sdf_helper_src, src)
        } else {
            src
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
