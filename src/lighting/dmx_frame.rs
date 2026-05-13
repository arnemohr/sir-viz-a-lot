//! P5.3.6 — DMX-frame builder.
//!
//! `build_universe_frame` converts a list of fixture groups and their
//! sampled colours into a map of `DmxUniverse` values, one per Art-Net
//! universe. Groups on the same universe accumulate into the same buffer.
//!
//! The DMX writer iterates each group's `personality.channels` with
//! `enumerate`, writing the sampled colour byte to the correct offset.
//! Unrecognised `ChannelRole` variants (added in Phase 7) are left at
//! zero; the `_ => {}` fallback matches the `#[non_exhaustive]` contract.

use std::collections::HashMap;

use crate::lighting::fixture::{ChannelRole, FixtureGroup, FixtureGroupId};
use crate::lighting::universe::{DmxUniverse, UniverseId};

/// The RGB colour sampled for a fixture group from the canvas.
///
/// Output of `sample_and_convert` (P5.4.3); input to `build_universe_frame`.
#[derive(Debug, Clone, Copy, Default)]
pub struct SampledColor {
    /// Red channel byte (0–255).
    pub r: u8,
    /// Green channel byte (0–255).
    pub g: u8,
    /// Blue channel byte (0–255).
    pub b: u8,
}

/// Build one `DmxUniverse` per universe, writing each fixture group's
/// sampled colour into the correct DMX channel offsets.
///
/// # Arguments
///
/// - `groups` — all fixture groups to write. Each group contributes
///   `fixture_count × personality.channel_count()` channels.
/// - `colors` — one `(FixtureGroupId, SampledColor)` per group. Groups
///   without a matching entry are written as all-zero (blackout).
///
/// # Returns
///
/// A `HashMap<UniverseId, DmxUniverse>` containing one entry per universe
/// that received at least one write. Groups on the same universe accumulate
/// without overwriting each other.
pub fn build_universe_frame(
    groups: &[FixtureGroup],
    colors: &[(FixtureGroupId, SampledColor)],
) -> HashMap<UniverseId, DmxUniverse> {
    let mut universes: HashMap<UniverseId, DmxUniverse> = HashMap::new();

    for group in groups {
        // Resolve the sampled colour for this group (default: black).
        let color = colors
            .iter()
            .find(|(id, _)| *id == group.id)
            .map(|(_, c)| *c)
            .unwrap_or_default();

        let channel_count = group.personality.channel_count();
        if channel_count == 0 {
            continue;
        }

        // P7.9.2 — Apply RGBW white-point subtraction when enabled.
        // When disabled, r/g/b pass through unchanged and w = 0.
        let (r_out, g_out, b_out, w_out) =
            crate::lighting::rgbw::apply_rgbw(color.r, color.g, color.b, &group.rgbw_config);

        let universe = universes.entry(group.universe_id).or_default();

        for fixture_idx in 0..usize::from(group.fixture_count) {
            let base = usize::from(group.base_channel) + fixture_idx * channel_count;

            for (ch_offset, role) in group.personality.channels.iter().enumerate() {
                let dmx_addr = base + ch_offset;
                if dmx_addr >= 512 {
                    // Out of universe range — skip (tracing in the caller).
                    break;
                }
                // Note: `#[non_exhaustive]` on `ChannelRole` means external crates
                // must use `_ => {}`, but within the same crate all variants must be
                // explicitly handled to avoid unused-patterns warnings.
                let byte = match role {
                    ChannelRole::Red => r_out,
                    ChannelRole::Green => g_out,
                    ChannelRole::Blue => b_out,
                    // P7.9.2 — White channel: receives the CCT-aware extracted W byte.
                    ChannelRole::White => w_out,
                };
                *universe.channel_mut(dmx_addr) = byte;
            }
        }
    }

    universes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lighting::fixture::{
        FixtureGroupId, FixturePersonality, FixtureSource, OutputStrategy,
    };
    use crate::lighting::universe::UniverseId;

    fn rgb_group(id: u64, universe: u16, base_channel: u8, fixture_count: u8) -> FixtureGroup {
        FixtureGroup {
            id: FixtureGroupId(id),
            label: format!("group-{id}"),
            personality: FixturePersonality::default_rgb(),
            universe_id: UniverseId(universe),
            base_channel,
            fixture_count,
            output_strategy: OutputStrategy::RgbDirect,
            source: FixtureSource::default(),
            rgbw_config: crate::lighting::rgbw::RgbwConfig::default(),
        }
    }

    /// P5.3.6 — single RGB fixture at address 0, verify bytes at correct offsets.
    #[test]
    fn single_rgb_fixture_writes_correct_bytes() {
        let group = rgb_group(1, 1, 0, 1);
        let color = SampledColor {
            r: 255,
            g: 128,
            b: 64,
        };
        let result = build_universe_frame(&[group], &[(FixtureGroupId(1), color)]);

        let universe = result.get(&UniverseId(1)).expect("universe 1 should exist");
        assert_eq!(universe.channel(0), 255, "ch0 should be red");
        assert_eq!(universe.channel(1), 128, "ch1 should be green");
        assert_eq!(universe.channel(2), 64, "ch2 should be blue");
        // All other channels should be zero.
        assert_eq!(universe.channel(3), 0, "ch3 should be zero");
    }

    /// P5.3.6 — two fixtures on the same universe: no clobbering.
    #[test]
    fn two_fixtures_same_universe_no_clobbering() {
        // Fixture 1: base 0, 3-channel RGB (occupies ch 0-2).
        // Fixture 2: base 3, 3-channel RGB (occupies ch 3-5).
        let group = rgb_group(1, 1, 0, 2);
        let color = SampledColor {
            r: 100,
            g: 150,
            b: 200,
        };
        let result = build_universe_frame(&[group], &[(FixtureGroupId(1), color)]);

        let universe = result.get(&UniverseId(1)).expect("universe 1 should exist");
        // Fixture 0: channels 0, 1, 2.
        assert_eq!(universe.channel(0), 100);
        assert_eq!(universe.channel(1), 150);
        assert_eq!(universe.channel(2), 200);
        // Fixture 1: channels 3, 4, 5.
        assert_eq!(universe.channel(3), 100);
        assert_eq!(universe.channel(4), 150);
        assert_eq!(universe.channel(5), 200);
        // Unwritten channels remain zero.
        assert_eq!(universe.channel(6), 0);
    }

    /// P5.3.6 — two groups on different universes don't share state.
    #[test]
    fn two_groups_different_universes() {
        let group1 = rgb_group(1, 1, 0, 1);
        let group2 = rgb_group(2, 2, 0, 1);
        let colors = [
            (
                FixtureGroupId(1),
                SampledColor {
                    r: 10,
                    g: 20,
                    b: 30,
                },
            ),
            (
                FixtureGroupId(2),
                SampledColor {
                    r: 40,
                    g: 50,
                    b: 60,
                },
            ),
        ];
        let result = build_universe_frame(&[group1, group2], &colors);

        let u1 = result.get(&UniverseId(1)).expect("universe 1");
        let u2 = result.get(&UniverseId(2)).expect("universe 2");
        assert_eq!(u1.channel(0), 10);
        assert_eq!(u2.channel(0), 40);
    }

    /// P5.3.6 — group with no matching colour gets zeroed output.
    #[test]
    fn group_without_color_is_blackout() {
        let group = rgb_group(1, 1, 0, 1);
        // Pass empty colors list.
        let result = build_universe_frame(&[group], &[]);

        let universe = result.get(&UniverseId(1)).expect("universe 1");
        assert_eq!(universe.channel(0), 0);
        assert_eq!(universe.channel(1), 0);
        assert_eq!(universe.channel(2), 0);
    }

    /// P7.9.2 — RGBW enabled: neutral grey at 6500K produces significant W,
    /// near-zero residual RGB channels.
    #[test]
    fn rgbw_enabled_neutral_grey_high_w() {
        use crate::lighting::fixture::ChannelRole;
        use crate::lighting::rgbw::RgbwConfig;

        let mut group = FixtureGroup {
            id: FixtureGroupId(1),
            label: "rgbw-group".to_string(),
            personality: FixturePersonality::default_rgbw(),
            universe_id: UniverseId(1),
            base_channel: 0,
            fixture_count: 1,
            output_strategy: OutputStrategy::RgbDirect,
            source: FixtureSource::default(),
            rgbw_config: RgbwConfig {
                enabled: true,
                w_channel_cct_k: 6500,
                w_scale: 1.0,
            },
        };
        // Verify the personality has W as the 4th channel.
        assert_eq!(group.personality.channels[3], ChannelRole::White);

        let color = SampledColor {
            r: 128,
            g: 128,
            b: 128,
        };
        let result = build_universe_frame(&[group.clone()], &[(FixtureGroupId(1), color)]);
        let universe = result.get(&UniverseId(1)).expect("universe 1");
        let r = universe.channel(0);
        let g = universe.channel(1);
        let b = universe.channel(2);
        let w = universe.channel(3);
        assert!(
            r < 20,
            "R should be low for neutral grey RGBW at 6500K, got {r}"
        );
        assert!(
            g < 20,
            "G should be low for neutral grey RGBW at 6500K, got {g}"
        );
        assert!(
            b < 20,
            "B should be low for neutral grey RGBW at 6500K, got {b}"
        );
        assert!(
            w > 100,
            "W should be high for neutral grey RGBW at 6500K, got {w}"
        );

        // P7.9.2 — enabled: false → existing RGB output unchanged, W = 0.
        group.rgbw_config.enabled = false;
        let result_rgb = build_universe_frame(&[group], &[(FixtureGroupId(1), color)]);
        let universe_rgb = result_rgb.get(&UniverseId(1)).expect("universe 1 (rgb)");
        assert_eq!(
            universe_rgb.channel(0),
            128,
            "R should pass through when RGBW disabled"
        );
        assert_eq!(
            universe_rgb.channel(1),
            128,
            "G should pass through when RGBW disabled"
        );
        assert_eq!(
            universe_rgb.channel(2),
            128,
            "B should pass through when RGBW disabled"
        );
        assert_eq!(
            universe_rgb.channel(3),
            0,
            "W should be 0 when RGBW disabled"
        );
    }
}
