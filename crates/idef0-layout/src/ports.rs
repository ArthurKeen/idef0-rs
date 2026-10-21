//! Port positions (`docs/spec/layout.md` §4.4; FR-LAYOUT-05, FR-LAYOUT-06, FR-LAYOUT-23).

use idef0_core::id::Side;

use crate::metrics::{LATTICE_X, LATTICE_Y};
use crate::size::PortCounts;

/// A box rectangle in gu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rect {
    /// Left.
    pub x: i32,
    /// Top.
    pub y: i32,
    /// Width.
    pub w: i32,
    /// Height.
    pub h: i32,
}

/// `offset_k = snap_g(L · k / (n + 1))` (§4.4, normative): the exact rational, rounded to the
/// nearest multiple of `g`, halves rounding down.
pub fn offset(len: i32, n: u8, k: u8, g: i32) -> i32 {
    let num = i64::from(len) * i64::from(k);
    let den = (i64::from(n) + 1) * i64::from(g);
    // round_half_down(num / den) = ceil((2·num − den) / 2·den)
    let units = (2 * num - den + 2 * den - 1).div_euclid(2 * den);
    i32::try_from(units * i64::from(g)).unwrap_or(i32::MAX)
}

/// The lattice step along a side (§4.4): 2 gu on top and bottom, 4 gu on left and right.
pub fn lattice_step(side: Side) -> i32 {
    match side {
        Side::C | Side::M => LATTICE_X,
        Side::I | Side::O => LATTICE_Y,
    }
}

/// The number of ports on `side`, the call port included on the bottom.
pub fn ports_on(counts: PortCounts, side: Side) -> u8 {
    match side {
        Side::I => counts.i,
        Side::C => counts.c,
        Side::O => counts.o,
        Side::M => counts.bottom(),
    }
}

/// The bottom rank of the call port: always the rightmost, after every mechanism
/// (FR-LAYOUT-23).
pub fn call_rank(counts: PortCounts) -> Option<u8> {
    counts.call.then(|| counts.bottom())
}

/// The point of the port of rank `k` on `side` of `rect` (§4.4). Rank order: I1 and O1
/// topmost, C1 and M1 leftmost (D5).
pub fn port_point(rect: Rect, counts: PortCounts, side: Side, k: u8) -> (i32, i32) {
    let n = ports_on(counts, side);
    let g = lattice_step(side);
    match side {
        Side::I => (rect.x, rect.y + offset(rect.h, n, k, g)),
        Side::O => (rect.x + rect.w, rect.y + offset(rect.h, n, k, g)),
        Side::C => (rect.x + offset(rect.w, n, k, g), rect.y),
        Side::M => (rect.x + offset(rect.w, n, k, g), rect.y + rect.h),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::Metrics;
    use crate::size::box_size;
    use crate::text::TextMetric;
    use proptest::prelude::*;

    // §4.4: exact rational, rounded half-down to the lattice step.
    #[test]
    fn offsets() {
        assert_eq!(offset(24, 1, 1, 4), 12, "the middle of a 24 gu side");
        assert_eq!(offset(24, 2, 1, 4), 8);
        assert_eq!(offset(24, 2, 2, 4), 16);
        assert_eq!(offset(40, 1, 1, 2), 20);
        assert_eq!(offset(40, 3, 1, 2), 10);
        // 44 · 1 / 4 = 11 lies halfway between 10 and 12: half rounds down
        assert_eq!(offset(44, 3, 1, 2), 10);
        // 44 · 3 / 4 = 33 lies halfway between 32 and 34: half rounds down
        assert_eq!(offset(44, 3, 3, 2), 32);
        // 30 · 1 / 4 = 7.5 is nearer 8 than 6
        assert_eq!(offset(30, 3, 1, 2), 8);
    }

    // FR-LAYOUT-23: the call port is the rightmost bottom port.
    #[test]
    fn call_port_is_rightmost() {
        let counts = PortCounts {
            m: 2,
            call: true,
            ..PortCounts::default()
        };
        assert_eq!(call_rank(counts), Some(3));
        assert_eq!(
            call_rank(PortCounts {
                m: 2,
                ..PortCounts::default()
            }),
            None
        );
        let rect = Rect {
            x: 0,
            y: 0,
            w: 40,
            h: 24,
        };
        let xs: Vec<i32> = (1..=3)
            .map(|k| port_point(rect, counts, Side::M, k).0)
            .collect();
        assert!(xs.windows(2).all(|w| w[0] < w[1]), "{xs:?}");
        assert_eq!(port_point(rect, counts, Side::M, 3).0, xs[2]);
    }

    // FR-LAYOUT-06: sides by role.
    #[test]
    fn sides_by_role() {
        let rect = Rect {
            x: 10,
            y: 20,
            w: 40,
            h: 24,
        };
        let c = PortCounts {
            i: 1,
            c: 1,
            o: 1,
            m: 1,
            call: false,
        };
        assert_eq!(port_point(rect, c, Side::I, 1), (10, 32));
        assert_eq!(port_point(rect, c, Side::O, 1), (50, 32));
        assert_eq!(port_point(rect, c, Side::C, 1), (30, 20));
        assert_eq!(port_point(rect, c, Side::M, 1), (30, 44));
    }

    proptest! {
        // §4.4, FR-LAYOUT-05: on a box sized by §4.1, ports are on the lattice, in rank order,
        // at least `port_pitch − g` apart, and at least `clear` from each corner.
        #[test]
        fn ports_on_a_sized_box(i in 0u8..10, c in 0u8..10, o in 0u8..10, mech in 0u8..10, call: bool) {
            let m = Metrics::default();
            let counts = PortCounts { i, c, o, m: mech, call };
            let s = box_size("Box", counts, TextMetric::Mono, &m);
            let rect = Rect { x: 0, y: 0, w: s.w, h: s.h };
            for side in [Side::I, Side::C, Side::O, Side::M] {
                let n = ports_on(counts, side);
                let g = lattice_step(side);
                let (len, pitch) = if matches!(side, Side::C | Side::M) { (s.w, m.port_pitch_x) } else { (s.h, m.port_pitch_y) };
                let offs: Vec<i32> = (1..=n)
                    .map(|k| {
                        let (x, y) = port_point(rect, counts, side, k);
                        // the point lies on its own side
                        match side {
                            Side::I => assert_eq!(x, rect.x),
                            Side::O => assert_eq!(x, rect.x + rect.w),
                            Side::C => assert_eq!(y, rect.y),
                            Side::M => assert_eq!(y, rect.y + rect.h),
                        }
                        if matches!(side, Side::C | Side::M) { x - rect.x } else { y - rect.y }
                    })
                    .collect();
                for (a, b) in offs.iter().zip(offs.iter().skip(1)) {
                    prop_assert!(b - a >= pitch - g, "{side:?} {offs:?}");
                }
                for o in &offs {
                    prop_assert_eq!(o % g, 0);
                    prop_assert!(*o >= m.clear && *o <= len - m.clear, "{side:?} {offs:?} len {len}");
                }
            }
        }
    }
}
