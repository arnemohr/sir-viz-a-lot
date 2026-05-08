//! 003-T2.8 — generate the placeholder photo for the bundled
//! `window-glow` demo project.
//!
//! Run with `cargo run --example gen_demo_photo`. Writes
//! `assets/demos/window-glow/photo.jpg` if it doesn't already
//! exist. The placeholder is a 64 × 96 portrait-orientation solid
//! warm-dark colour — enough for `image::open` to succeed and the
//! render pipeline to produce a non-black pixel, which is what
//! `T-003-T2.21`'s `demo_loads_clean` property test checks.
//!
//! Replace this file with the license-cleared CC0 photo from T0.2
//! before declaring M2; until then, this keeps the demo project
//! loadable so the launcher button (T-003-T2.9) can be exercised.

use std::path::Path;

use image::{ImageBuffer, Rgb};

fn main() {
    let path = Path::new("assets/demos/window-glow/photo.jpg");
    if path.exists() {
        println!("{} already exists; not overwriting", path.display());
        return;
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create photo dir");
    }
    // 64 × 96 portrait, warm-dark hue. The pixel value is arbitrary;
    // the important property is that `image::open` decodes it and the
    // composited frame's centre pixel is non-zero (T2.21 golden test).
    let img: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::from_pixel(64, 96, Rgb([22, 18, 30]));
    img.save(path).expect("save jpg");
    println!("wrote placeholder demo photo at {}", path.display());
}
