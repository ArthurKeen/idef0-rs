//! Text metrics (`docs/spec/layout.md` §3.3; SR-49). Widths are in sub-units (su), so a
//! proportional metric can round once per string.

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::metrics::{Metrics, SU_PER_GU};

/// How a theme measures text (§3.3). The Scene records which metric produced it.
///
/// `Serif` (Liberation Serif advances, for the `classic` theme) arrives with roadmap M5.5,
/// together with the bundled font; until then no theme selects it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TextMetric {
    /// Σ over grapheme clusters of `unicode-width(cluster)` × `glyph_w`: East Asian wide
    /// characters and emoji count 2 columns, zero-width clusters 0.
    Mono,
}

impl TextMetric {
    /// The width of `s` in su.
    pub fn width_su(self, s: &str, m: &Metrics) -> i32 {
        match self {
            Self::Mono => {
                let cols: usize = s.graphemes(true).map(UnicodeWidthStr::width).sum();
                i32::try_from(cols).unwrap_or(i32::MAX / (m.glyph_w * SU_PER_GU))
                    * m.glyph_w
                    * SU_PER_GU
            }
        }
    }

    /// The width of `s` in gu, rounded up.
    pub fn width_gu(self, s: &str, m: &Metrics) -> i32 {
        let su = self.width_su(s, m);
        su.div_euclid(SU_PER_GU) + i32::from(su.rem_euclid(SU_PER_GU) != 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // §3.3 Mono: grapheme clusters × unicode-width × glyph_w.
    #[test]
    fn mono_widths() {
        let m = Metrics::default();
        let w = |s: &str| TextMetric::Mono.width_gu(s, &m);
        assert_eq!(w(""), 0);
        assert_eq!(w("Plan"), 8);
        assert_eq!(w("Café"), 8, "é is one cluster");
        assert_eq!(
            w("Cafe\u{301}"),
            8,
            "e + combining acute is one cluster of width 1"
        );
        assert_eq!(w("注文"), 8, "East Asian wide: 2 columns each");
        assert_eq!(
            w("👍🏽"),
            4,
            "an emoji with a skin-tone modifier is one wide cluster"
        );
        assert_eq!(w("a\u{200b}b"), 4, "zero-width space counts 0");
        assert_eq!(TextMetric::Mono.width_su("ab", &m), 16);
    }
}
