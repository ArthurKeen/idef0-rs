//! Box sizing and title wrap (`docs/spec/layout.md` §4.1, §4.2; FR-LAYOUT-03).

use crate::metrics::{LATTICE_X, LATTICE_Y, Metrics, SU_PER_GU};
use crate::text::TextMetric;

/// The number of ports on each side of a box. A call arrow takes the rightmost bottom port,
/// after the mechanisms (FR-LAYOUT-23).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PortCounts {
    /// Inputs (left).
    pub i: u8,
    /// Controls (top).
    pub c: u8,
    /// Outputs (right).
    pub o: u8,
    /// Mechanisms (bottom).
    pub m: u8,
    /// Whether the box has a call arrow (bottom, rightmost).
    pub call: bool,
}

impl PortCounts {
    /// `nB` in §4.1: mechanisms plus the call port.
    pub fn bottom(&self) -> u8 {
        self.m.saturating_add(u8::from(self.call))
    }
}

/// A sized box: its width and height in gu and its wrapped title.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoxSize {
    /// Width, a multiple of 2 gu.
    pub w: i32,
    /// Height, a multiple of 4 gu.
    pub h: i32,
    /// The title, one string per line. Never truncated.
    pub lines: Vec<String>,
}

fn ceil_to(v: i32, k: i32) -> i32 {
    (v + k - 1).div_euclid(k) * k
}

/// A piece of a title and whether a space separated it from the previous piece.
struct Piece<'a> {
    text: &'a str,
    space_before: bool,
}

/// Break opportunities (§4.2): spaces, and after `/` and `-`.
fn pieces(title: &str) -> Vec<Piece<'_>> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut space_before = false;
    for (i, c) in title.char_indices() {
        let end = i + c.len_utf8();
        if c == ' ' {
            if start < i {
                out.push(Piece {
                    text: &title[start..i],
                    space_before,
                });
            }
            start = end;
            space_before = !out.is_empty();
        } else if c == '/' || c == '-' {
            out.push(Piece {
                text: &title[start..end],
                space_before,
            });
            start = end;
            space_before = false;
        }
    }
    if start < title.len() {
        out.push(Piece {
            text: &title[start..],
            space_before,
        });
    }
    out
}

/// Greedy wrap of `title` into lines no wider than `limit_su` where possible. A piece wider
/// than the limit gets a line of its own.
pub fn wrap(title: &str, limit_su: i32, metric: TextMetric, m: &Metrics) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for p in pieces(title) {
        if let Some(line) = lines.last_mut() {
            let joined = if p.space_before {
                format!("{line} {}", p.text)
            } else {
                format!("{line}{}", p.text)
            };
            if metric.width_su(&joined, m) <= limit_su {
                *line = joined;
                continue;
            }
        }
        lines.push(p.text.to_owned());
    }
    lines
}

fn widest_su(lines: &[String], metric: TextMetric, m: &Metrics) -> i32 {
    lines
        .iter()
        .map(|l| metric.width_su(l, m))
        .max()
        .unwrap_or(0)
}

fn count(n: usize) -> i32 {
    i32::try_from(n).unwrap_or(i32::MAX)
}

/// The size of a box titled `title` with `ports` (§4.1), its title wrapped per §4.2:
///
/// - the width is the smallest lattice width, from the port-driven minimum up to `w_max`, at
///   which the title wraps into at most 3 lines that each fit;
/// - if none fits, the width stays at `w_max` and the box grows taller;
/// - a single word wider than a line widens the box to fit it. A name is never truncated.
pub fn box_size(title: &str, ports: PortCounts, metric: TextMetric, m: &Metrics) -> BoxSize {
    let n_top = i32::from(ports.c.max(ports.bottom()));
    let n_side = i32::from(ports.i.max(ports.o));
    let floor_w = ceil_to(m.box_min_w.max((n_top + 1) * m.port_pitch_x), LATTICE_X);
    let inner = |w: i32| (w - 2 * m.pad) * SU_PER_GU;
    let fits = |w: i32| {
        let lines = wrap(title, inner(w), metric, m);
        (lines.len() <= 3 && widest_su(&lines, metric, m) <= inner(w)).then_some(lines)
    };
    let (mut w, lines) = (floor_w..=m.w_max.max(floor_w))
        .step_by(usize::try_from(LATTICE_X).unwrap_or(2))
        .find_map(|w| fits(w).map(|lines| (w, lines)))
        .unwrap_or_else(|| {
            let w = m.w_max.max(floor_w);
            (w, wrap(title, inner(w), metric, m))
        });
    let text_w = widest_su(&lines, metric, m);
    let text_gu = text_w.div_euclid(SU_PER_GU) + i32::from(text_w.rem_euclid(SU_PER_GU) != 0);
    w = ceil_to(w.max(text_gu + 2 * m.pad), LATTICE_X);
    let text_h = count(lines.len()) * m.line_h + m.corner_label_h() + 2 * m.pad;
    let h = ceil_to(
        m.box_min_h.max((n_side + 1) * m.port_pitch_y).max(text_h),
        LATTICE_Y,
    );
    BoxSize { w, h, lines }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn size(title: &str, ports: PortCounts) -> BoxSize {
        box_size(title, ports, TextMetric::Mono, &Metrics::default())
    }

    // §4.2: break after `/` and `-` as well as at spaces.
    #[test]
    fn break_opportunities() {
        let m = Metrics::default();
        let w = |t: &str, cols: i32| wrap(t, cols * 8, TextMetric::Mono, &m);
        assert_eq!(
            w("Develop/Update a Factory", 9),
            ["Develop/", "Update a", "Factory"]
        );
        assert_eq!(
            w("Develop/Update a Factory", 40),
            ["Develop/Update a Factory"]
        );
        assert_eq!(w("Make-or-Buy Decision", 12), ["Make-or-Buy", "Decision"]);
        assert_eq!(
            w("Make-or-Buy Decision", 6),
            ["Make-", "or-Buy", "Decision"]
        );
        assert_eq!(
            w("Plan", 1),
            ["Plan"],
            "a piece wider than the limit keeps a line of its own"
        );
    }

    // §4.1, §4.2: the box_min floor for a short name; widths and heights on the lattice.
    #[test]
    fn small_box_is_box_min() {
        let s = size("Plan", PortCounts::default());
        assert_eq!((s.w, s.h), (40, 24));
        assert_eq!(s.lines, ["Plan"]);
    }

    // §4.2: the smallest width that wraps into at most 3 lines.
    #[test]
    fn smallest_width_with_three_lines() {
        let title = "Validate Customer Credit Against Current Order Policy";
        let s = size(title, PortCounts::default());
        assert!(s.lines.len() <= 3, "{:?}", s.lines);
        assert!(s.w <= 80);
        let narrower = box_size(
            title,
            PortCounts::default(),
            TextMetric::Mono,
            &Metrics {
                box_min_w: 4,
                ..Metrics::default()
            },
        );
        assert!(narrower.w <= s.w);
        // one lattice step narrower would need a fourth line
        let m = Metrics::default();
        if s.w > 40 {
            let tighter = wrap(
                title,
                (s.w - 2 - 2 * m.pad) * SU_PER_GU,
                TextMetric::Mono,
                &m,
            );
            assert!(
                tighter.len() > 3
                    || TextMetric::Mono.width_su(&tighter[0], &m)
                        > (s.w - 2 - 2 * m.pad) * SU_PER_GU
            );
        }
    }

    // §4.2: past w_max the box keeps w_max and grows taller; nothing is truncated.
    #[test]
    fn long_names_grow_taller_and_are_never_truncated() {
        let title = "Collect Reconcile Normalize Validate Enrich Deduplicate Archive Summarize Forecast And Report Every Incoming Supplier Invoice Batch For The Quarter";
        let s = size(title, PortCounts::default());
        assert_eq!(s.w, 80);
        assert!(s.lines.len() > 3, "{:?}", s.lines);
        assert_eq!(s.lines.join(" "), title, "every word kept, in order");
        assert!(s.h >= i32::try_from(s.lines.len()).unwrap() * 4 + 4 + 4);
    }

    // §4.1: when the title sets the height, it is lines · line_h + corner_label_h + 2·pad,
    // rounded up to 4 gu; the corner label takes its own row.
    #[test]
    fn title_driven_height_reserves_the_corner_label() {
        let title = [
            "Collect",
            "and",
            "reconcile",
            "every",
            "incoming",
            "supplier",
            "invoice",
            "batch",
            "for",
            "the",
            "quarter",
        ]
        .repeat(3)
        .join(" ");
        let s = size(&title, PortCounts::default());
        let n = i32::try_from(s.lines.len()).unwrap();
        assert!(n >= 6, "{:?}", s.lines);
        let m = Metrics::default();
        assert_eq!(
            s.h,
            ceil_to(n * m.line_h + m.corner_label_h() + 2 * m.pad, 4)
        );
    }

    // §4.2: a single unbreakable word wider than w_max − 2·pad widens the box.
    #[test]
    fn unbreakable_word_widens_the_box() {
        let word = "Pneumonoultramicroscopicsilicovolcanoconiosis"; // 45 columns = 90 gu
        let s = size(word, PortCounts::default());
        assert_eq!(s.lines, [word]);
        assert_eq!(s.w, 90 + 4);
    }

    // §4.1: port counts drive the size: (n + 1) · port_pitch on each axis.
    #[test]
    fn ports_drive_the_size() {
        let s = size(
            "Plan",
            PortCounts {
                i: 5,
                c: 2,
                o: 1,
                m: 4,
                call: true,
            },
        );
        assert_eq!(s.w, (5 + 1) * 8, "nB = 4 mechanisms + 1 call");
        assert_eq!(s.h, (5 + 1) * 8);
        let tall = size(
            "Plan",
            PortCounts {
                o: 9,
                ..PortCounts::default()
            },
        );
        assert_eq!(tall.h, 80);
    }
}
