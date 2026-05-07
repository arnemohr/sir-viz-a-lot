//! Compile-time WGSL validation. Per the spec, every shader is parsed and
//! validated by naga during `cargo build`, so a broken shader fails the build
//! instead of crashing the renderer at startup.

use std::fs;
use std::path::Path;

fn main() {
    let shader_dir = Path::new("src/render/shaders");
    println!("cargo:rerun-if-changed=src/render/shaders");

    if !shader_dir.exists() {
        return;
    }

    for entry in fs::read_dir(shader_dir).expect("read shader dir") {
        let path = entry.expect("dir entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("wgsl") {
            continue;
        }
        println!("cargo:rerun-if-changed={}", path.display());

        let src = fs::read_to_string(&path).expect("read shader source");

        let module = match naga::front::wgsl::parse_str(&src) {
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
