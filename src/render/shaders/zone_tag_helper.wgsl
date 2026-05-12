// P3.3.1 — Zone-tag helper: constant definitions and struct for zone-aware
// FX preset shaders.
//
// This file is prepended to zone-aware preset shaders at pipeline build time
// via ZONE_TAG_WGSL in `src/render/sdf.rs`, exactly as SDF_HELPER_WGSL is
// used for SDF-consuming shaders.
//
// Zone-aware shaders also declare:
//   @group(0) @binding(6) var<uniform> u_zone: ZoneTagUniform;
// directly in their own source (the binding is per-shader, not in this helper,
// so standalone naga validation of this file succeeds without an entry point).
//
// COUPLING: the u32 values below must match `From<ZoneRole> for u32` in
// `src/project/schema.rs` (P3.2.1). Changing one requires changing both.
//   None/absent → 0 (ZONE_NONE)
//   Window      → 1 (ZONE_WINDOW)
//   Portal      → 2 (ZONE_PORTAL)
//   Void        → 3 (ZONE_VOID)
//   Spill       → 4 (ZONE_SPILL)
//   Edge        → 5 (ZONE_EDGE)
//   Highlight   → 6 (ZONE_HIGHLIGHT)
//   LightSource → 7 (ZONE_LIGHT_SOURCE)

const ZONE_NONE: u32 = 0u;
const ZONE_WINDOW: u32 = 1u;
const ZONE_PORTAL: u32 = 2u;
const ZONE_VOID: u32 = 3u;
const ZONE_SPILL: u32 = 4u;
const ZONE_EDGE: u32 = 5u;
const ZONE_HIGHLIGHT: u32 = 6u;
const ZONE_LIGHT_SOURCE: u32 = 7u;

// 16-byte aligned ZoneTagUniform (wgpu min-binding-size safe).
// The three padding fields keep the struct at exactly 16 bytes, matching
// the Rust `[u32; 4]` layout used when writing the uniform buffer.
struct ZoneTagUniform {
    zone_tag: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
}
