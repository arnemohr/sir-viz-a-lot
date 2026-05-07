//! Warp mesh + per-warp polygon mask. v1 default is a 1×1 mesh (corner-pin);
//! mesh subdivision in M7 only changes vertex count.

#[derive(Default)]
pub struct Warp {
    // TODO(M5): vertex buffer for the (rows+1)×(cols+1) grid,
    //           index buffer for the triangle strip,
    //           mask polygon as a triangle fan (or signed-distance texture),
    //           feather radius uniform.
}

impl Warp {
    pub fn new() -> Self {
        Self::default()
    }
}

#[cfg(test)]
mod tests {
    /// 4-point homography solve via glam::Mat3. The renderer uses wgpu's
    /// projective rasterization for the actual warp; this test pins the
    /// math path that any pre-computed transform falls back to.
    #[test]
    fn homography_round_trip_smoke() {
        // TODO(M5): once the homography solver lands, project the four
        // corners of the unit square through to a known quad and assert
        // near-zero residual.
    }
}
