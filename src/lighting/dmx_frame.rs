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

        let universe = universes.entry(group.universe_id).or_default();

        for fixture_idx in 0..usize::from(group.fixture_count) {
            let base = usize::from(group.base_channel) + fixture_idx * channel_count;

            for (ch_offset, role) in group.personality.channels.iter().enumerate() {
                let dmx_addr = base + ch_offset;
                if dmx_addr >= 512 {
                    // Out of universe range — skip (tracing in the caller).
                    break;
                }
                // `#[allow(unreachable_patterns)]` is intentional: Phase 7 adds
                // new `ChannelRole` variants (White, ColorTemp, etc.) to this crate.
                // The wildcard arm exists so this match stays correct after Phase 7
                // without any modification; it's currently unreachable because all
                // Phase 5 variants are explicitly handled above.
                #[allow(unreachable_patterns)]
                let byte = match role {
                    ChannelRole::Red => color.r,
                    ChannelRole::Green => color.g,
                    ChannelRole::Blue => color.b,
                    _ => 0, // Phase 7: White, ColorTemp, Intensity, etc. map here.
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
}
