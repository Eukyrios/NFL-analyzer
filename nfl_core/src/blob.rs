//! Packed per-play tracking: per frame, per slot, five little-endian `u16`s
//! (x, y in centi-yards; s in centi-yards/s; o, dir in deci-degrees), then
//! LZ4-compressed with the uncompressed size prepended. `0xFFFF` marks NULL; an
//! entity missing from a frame has x = NULL.

pub const NULL: u16 = 0xFFFF;
const FIELDS: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EntityPos {
    pub x: f32,
    pub y: f32,
    pub s: f32,
    pub o: Option<f32>,
    pub dir: Option<f32>,
}

/// One frame: one optional position per slot.
pub type Frame = Vec<Option<EntityPos>>;

fn q(v: f32, scale: f32) -> u16 {
    (v * scale).round().clamp(0.0, (NULL - 1) as f32) as u16
}
fn q_angle(v: Option<f32>) -> u16 {
    v.map_or(NULL, |a| q(a.rem_euclid(360.0), 10.0) % 3600)
}
fn dq(v: u16, scale: f32) -> Option<f32> {
    (v != NULL).then(|| v as f32 / scale)
}

pub fn encode(frames: &[Frame], slots: usize) -> Vec<u8> {
    let mut raw = Vec::with_capacity(frames.len() * slots * FIELDS * 2);
    for f in frames {
        for slot in 0..slots {
            let vals = match f.get(slot).copied().flatten() {
                Some(p) => [q(p.x, 100.0), q(p.y, 100.0), q(p.s, 100.0), q_angle(p.o), q_angle(p.dir)],
                None => [NULL; FIELDS],
            };
            for v in vals {
                raw.extend_from_slice(&v.to_le_bytes());
            }
        }
    }
    lz4_flex::compress_prepend_size(&raw)
}

pub fn decode(blob: &[u8], slots: usize) -> crate::Result<Vec<Frame>> {
    let raw = lz4_flex::decompress_size_prepended(blob)?;
    let stride = slots * FIELDS * 2;
    if slots == 0 || raw.len() % stride != 0 {
        return Err(format!("frame blob of {} bytes doesn't fit {slots} slots", raw.len()).into());
    }
    Ok(raw
        .chunks_exact(stride)
        .map(|frame| {
            frame
                .chunks_exact(FIELDS * 2)
                .map(|e| {
                    let v = |i: usize| u16::from_le_bytes([e[2 * i], e[2 * i + 1]]);
                    let x = dq(v(0), 100.0)?;
                    Some(EntityPos {
                        x,
                        y: dq(v(1), 100.0).unwrap_or(0.0),
                        s: dq(v(2), 100.0).unwrap_or(0.0),
                        o: dq(v(3), 10.0),
                        dir: dq(v(4), 10.0),
                    })
                })
                .collect()
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn pos() -> impl Strategy<Value = Option<EntityPos>> {
        prop::option::weighted(
            0.9,
            (0.0f32..120.0, 0.0f32..53.3, 0.0f32..40.0, prop::option::of(0.0f32..360.0), prop::option::of(0.0f32..360.0))
                .prop_map(|(x, y, s, o, dir)| EntityPos { x, y, s, o, dir }),
        )
    }

    fn angle_close(a: f32, b: f32) -> bool {
        let d = (a - b).rem_euclid(360.0);
        d.min(360.0 - d) <= 0.051
    }

    // Feature: native-play-viewer, Property 1: Blob round-trip
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(128))]
        #[test]
        fn round_trip(slots in 1usize..24, frames in 1usize..20, seed in prop::collection::vec(pos(), 23 * 20)) {
            let input: Vec<Frame> = (0..frames).map(|f| (0..slots).map(|s| seed[(f * 23 + s) % seed.len()]).collect()).collect();
            let out = decode(&encode(&input, slots), slots).unwrap();
            prop_assert_eq!(out.len(), input.len());
            for (fa, fb) in input.iter().zip(&out) {
                for (a, b) in fa.iter().zip(fb) {
                    match (a, b) {
                        (None, None) => {}
                        (Some(a), Some(b)) => {
                            // Half a centi-yard, plus f32 slack.
                            let tol = 0.0051;
                            prop_assert!((a.x - b.x).abs() <= tol && (a.y - b.y).abs() <= tol && (a.s - b.s).abs() <= tol);
                            prop_assert_eq!(a.o.is_some(), b.o.is_some());
                            prop_assert_eq!(a.dir.is_some(), b.dir.is_some());
                            if let (Some(x), Some(y)) = (a.o, b.o) { prop_assert!(angle_close(x, y)); }
                            if let (Some(x), Some(y)) = (a.dir, b.dir) { prop_assert!(angle_close(x, y)); }
                        }
                        _ => prop_assert!(false, "presence changed"),
                    }
                }
            }
        }
    }

    #[test]
    fn rejects_bad_length() {
        let blob = lz4_flex::compress_prepend_size(&[0u8; 7]);
        assert!(decode(&blob, 1).is_err());
    }
}
