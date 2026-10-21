//! Layout metrics (`docs/spec/layout.md` §3.1, §3.2; SR-42; FR-LAYOUT-24).
//!
//! Grid units (gu) carry every layout decision; 1 gu = 4 sub-units (su) = 4 px at zoom 1.
//! Fillet, branch and bump radii and arrowheads are in su. `Metrics` may be overridden per
//! workspace, but every override must pass [`Metrics::validate`], which reports `E-METRICS`.

use core::fmt;

use idef0_core::diag::{Diagnostic, E_METRICS, SubjectRef};

/// Sub-units per grid unit (§3.1).
pub const SU_PER_GU: i32 = 4;

/// The lattice step along x and y, in gu (§3.1): every vertex lies on `x ∈ 2ℤ`, `y ∈ 4ℤ`.
pub const LATTICE_X: i32 = 2;
/// See [`LATTICE_X`].
pub const LATTICE_Y: i32 = 4;

/// The fillet radius floor and ceiling, in su (FR-LAYOUT-24). Not overridable.
pub const R_MIN: i32 = 6;
/// See [`R_MIN`].
pub const R_MAX: i32 = 10;

/// The layout metrics of §3.2. Field names follow the spec; lengths are gu unless marked su.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Metrics {
    /// Spacing of vertical tracks.
    pub pitch_x: i32,
    /// Spacing of horizontal tracks.
    pub pitch_y: i32,
    /// Port spacing along top and bottom sides.
    pub port_pitch_x: i32,
    /// Port spacing along left and right sides.
    pub port_pitch_y: i32,
    /// Keep-out distance to tracks, corners and labels.
    pub clear: i32,
    /// Fillet radius, su.
    pub r_fillet: i32,
    /// Branch radius, su.
    pub r_branch: i32,
    /// Bridge bump radius, su.
    pub r_bump: i32,
    /// Arrowhead length, su.
    pub head_len: i32,
    /// Arrowhead half-width, su.
    pub head_halfw: i32,
    /// Monospace advance: one ASCII column.
    pub glyph_w: i32,
    /// Line height: one ASCII row.
    pub line_h: i32,
    /// Minimum box width.
    pub box_min_w: i32,
    /// Minimum box height.
    pub box_min_h: i32,
    /// Title wrap target width (soft; a box may grow past it, §4.2).
    pub w_max: i32,
    /// Text padding inside boxes and the frame.
    pub pad: i32,
    /// Minimum frame margin.
    pub margin: i32,
    /// Drop-shadow band to the right.
    pub shadow_x: i32,
    /// Drop-shadow band below.
    pub shadow_y: i32,
}

impl Default for Metrics {
    /// The §3.2 values.
    fn default() -> Self {
        Self {
            pitch_x: 6,
            pitch_y: 8,
            port_pitch_x: 8,
            port_pitch_y: 8,
            clear: 4,
            r_fillet: 8,
            r_branch: 8,
            r_bump: 8,
            head_len: 8,
            head_halfw: 4,
            glyph_w: 2,
            line_h: 4,
            box_min_w: 40,
            box_min_h: 24,
            w_max: 80,
            pad: 2,
            margin: 24,
            shadow_x: 2,
            shadow_y: 4,
        }
    }
}

/// A `Metrics` override that breaks one or more §3.2 constraints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetricsError {
    /// One sentence per broken constraint, in field order.
    pub violations: Vec<String>,
}

impl fmt::Display for MetricsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid layout metrics: {}", self.violations.join("; "))
    }
}

impl std::error::Error for MetricsError {}

impl MetricsError {
    /// The `E-METRICS` finding (validation §7.3; subject: model).
    pub fn diagnostic(&self) -> Diagnostic {
        Diagnostic::fatal(E_METRICS, SubjectRef::Model, self.to_string())
    }
}

impl Metrics {
    /// The height a title text block of `lines` lines needs inside a box. The corner label takes
    /// one more text row (`corner_label_h` in §4.1 is `line_h`).
    pub fn corner_label_h(&self) -> i32 {
        self.line_h
    }

    /// Check the constraint column of §3.2 and the lattice rule of SR-42 (vertical metrics are
    /// multiples of 4 gu, horizontal metrics multiples of 2 gu).
    ///
    /// # Errors
    ///
    /// [`MetricsError`] listing every broken constraint.
    pub fn validate(&self) -> Result<(), MetricsError> {
        let mut v = Vec::new();
        let mut need = |ok: bool, msg: String| {
            if !ok {
                v.push(msg);
            }
        };
        let positive = [
            ("pitch_x", self.pitch_x),
            ("pitch_y", self.pitch_y),
            ("port_pitch_x", self.port_pitch_x),
            ("port_pitch_y", self.port_pitch_y),
            ("clear", self.clear),
            ("head_len", self.head_len),
            ("head_halfw", self.head_halfw),
            ("glyph_w", self.glyph_w),
            ("line_h", self.line_h),
            ("box_min_w", self.box_min_w),
            ("box_min_h", self.box_min_h),
            ("w_max", self.w_max),
            ("margin", self.margin),
        ];
        for (name, value) in positive {
            need(value > 0, format!("`{name}` must be positive, not {value}"));
        }
        let r_bump_gu =
            self.r_bump.div_euclid(SU_PER_GU) + i32::from(self.r_bump.rem_euclid(SU_PER_GU) != 0);
        need(
            self.pitch_x >= 2 * r_bump_gu + 2 && self.pitch_x % 2 == 0,
            format!(
                "`pitch_x` ({}) must be even and at least 2·r_bump + 2 gu ({})",
                self.pitch_x,
                2 * r_bump_gu + 2
            ),
        );
        need(
            self.pitch_y % LATTICE_Y == 0,
            format!("`pitch_y` ({}) must be a multiple of 4 gu", self.pitch_y),
        );
        need(
            self.port_pitch_x >= 2 * LATTICE_X,
            format!(
                "`port_pitch_x` ({}) must be at least two lattice steps (4 gu)",
                self.port_pitch_x
            ),
        );
        need(
            self.port_pitch_y >= 2 * LATTICE_Y,
            format!(
                "`port_pitch_y` ({}) must be at least two lattice steps (8 gu)",
                self.port_pitch_y
            ),
        );
        need(
            self.clear * SU_PER_GU >= self.r_fillet + self.head_len,
            format!(
                "`clear` ({} gu) must be at least r_fillet + head_len ({} su)",
                self.clear,
                self.r_fillet + self.head_len
            ),
        );
        for (name, r) in [
            ("r_fillet", self.r_fillet),
            ("r_branch", self.r_branch),
            ("r_bump", self.r_bump),
        ] {
            need(
                (R_MIN..=R_MAX).contains(&r),
                format!("`{name}` ({r} su) must be within [{R_MIN}, {R_MAX}] su (FR-LAYOUT-24)"),
            );
        }
        for (name, value) in [
            ("pitch_y", self.pitch_y),
            ("port_pitch_y", self.port_pitch_y),
            ("line_h", self.line_h),
            ("box_min_h", self.box_min_h),
            ("shadow_y", self.shadow_y),
        ] {
            need(
                value % LATTICE_Y == 0,
                format!("`{name}` ({value}) is vertical and must be a multiple of 4 gu (SR-42)"),
            );
        }
        for (name, value) in [
            ("port_pitch_x", self.port_pitch_x),
            ("glyph_w", self.glyph_w),
            ("box_min_w", self.box_min_w),
            ("w_max", self.w_max),
            ("shadow_x", self.shadow_x),
        ] {
            need(
                value % LATTICE_X == 0,
                format!("`{name}` ({value}) is horizontal and must be a multiple of 2 gu (SR-42)"),
            );
        }
        if v.is_empty() {
            Ok(())
        } else {
            v.dedup();
            Err(MetricsError { violations: v })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // §3.2: the defaults satisfy every constraint.
    #[test]
    fn defaults_are_valid() {
        assert_eq!(Metrics::default().validate(), Ok(()));
    }

    // §3.2 constraint column and SR-42; E-METRICS.
    /// A change to the defaults and a fragment of the violation it must produce.
    type Case = (fn(&mut Metrics), &'static str);

    #[test]
    fn broken_constraints_are_reported() {
        let cases: [Case; 9] = [
            (|m| m.pitch_x = 4, "`pitch_x` (4) must be even and at least"),
            (|m| m.pitch_x = 7, "`pitch_x` (7) must be even"),
            (
                |m| m.pitch_y = 6,
                "`pitch_y` (6) must be a multiple of 4 gu",
            ),
            (
                |m| m.port_pitch_x = 2,
                "`port_pitch_x` (2) must be at least two lattice steps",
            ),
            (
                |m| m.port_pitch_y = 4,
                "`port_pitch_y` (4) must be at least two lattice steps",
            ),
            (
                |m| m.clear = 3,
                "`clear` (3 gu) must be at least r_fillet + head_len",
            ),
            (
                |m| m.r_fillet = 12,
                "`r_fillet` (12 su) must be within [6, 10] su",
            ),
            (
                |m| m.box_min_h = 26,
                "`box_min_h` (26) is vertical and must be a multiple of 4 gu",
            ),
            (
                |m| m.w_max = 81,
                "`w_max` (81) is horizontal and must be a multiple of 2 gu",
            ),
        ];
        for (change, want) in cases {
            let mut m = Metrics::default();
            change(&mut m);
            let err = m.validate().unwrap_err();
            assert!(
                err.violations.iter().any(|v| v.contains(want)),
                "{want}: {err}"
            );
            let d = err.diagnostic();
            assert_eq!(d.code.as_str(), "E-METRICS");
        }
    }
}
