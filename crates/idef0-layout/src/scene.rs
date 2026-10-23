//! The Scene: a laid-out diagram, ready to render (`docs/spec/layout.md` §14.1, §14.4;
//! FR-LAYOUT-20).
//!
//! [`layout`] runs sizing, placement, global routing and track assignment (§4–§7) and turns
//! the result into integer geometry in su (1 su = 1 px at zoom 1): the frame, the boxes with
//! their titles, corner labels, shadows and DREs, the arrows with their arrowheads, and the
//! diagram form around the frame. Vectors are in canonical order: boxes by box number, arrows
//! by id. Fillets, bridges, bundles, labels, squiggles, ICOM codes, tunnel glyphs and notes
//! arrive with M5.1–M5.4.

use idef0_core::diag::Diagnostic;
use idef0_core::id::NodeId;
use idef0_core::model::{Model, Status};
use idef0_core::profile::Profile;
use idef0_core::text::BoxKey;

use crate::metrics::{Metrics, SU_PER_GU};
use crate::ports::Rect;
use crate::route::EdgeKey;
use crate::text::TextMetric;

/// A theme (§13.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ThemeId {
    /// The default under the `extended` profile: shadow and DRE, node-number corner labels.
    Modern,
    /// Liberation Serif metrics and the classic conventions (M5.5).
    Classic,
    /// The default under the `fips183` profile: DRE only, box-number corner labels.
    Fips183,
    /// The ASCII theme; it shares the `modern` Scene (§13.1).
    Retro,
}

impl ThemeId {
    /// The profile's default theme (§13.1, D8).
    pub fn default_for(profile: Profile) -> Self {
        match profile {
            Profile::Extended => Self::Modern,
            Profile::Fips183 => Self::Fips183,
        }
    }

    /// The theme's name.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Modern => "modern",
            Self::Classic => "classic",
            Self::Fips183 => "fips183",
            Self::Retro => "retro",
        }
    }

    /// The text metric the theme measures with (§3.3).
    pub fn text_metric(self) -> TextMetric {
        TextMetric::Mono
    }

    /// Whether a decomposed box draws a drop shadow (FR-STYLE-06).
    pub fn shadows(self) -> bool {
        !matches!(self, Self::Fips183)
    }
}

impl core::str::FromStr for ThemeId {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "modern" => Ok(Self::Modern),
            "classic" => Ok(Self::Classic),
            "fips183" => Ok(Self::Fips183),
            "retro" => Ok(Self::Retro),
            other => Err(format!(
                "unknown theme `{other}` (modern, classic, fips183, retro)"
            )),
        }
    }
}

/// What [`layout`] needs besides the model.
#[derive(Debug, Clone, Copy)]
pub struct LayoutInput<'a> {
    /// The theme.
    pub theme: ThemeId,
    /// The metrics (§3.2).
    pub metrics: &'a Metrics,
}

/// A laid-out diagram and the layout findings (`L-LAYOUT-OVERFLOW`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutResult {
    /// The Scene.
    pub scene: Scene,
    /// Layout diagnostics.
    pub diagnostics: Vec<Diagnostic>,
}

/// Why a diagram could not be laid out.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LayoutError {
    /// The model has no such diagram.
    #[error("the model has no diagram {0}")]
    NoDiagram(NodeId),
    /// The theme needs something this version does not have yet.
    #[error("the {theme} theme is not available yet ({1})", theme = .0.as_str())]
    Theme(ThemeId, &'static str),
}

/// A line of text: its start and baseline in su, its measured width, and the text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextGeom {
    /// Left edge.
    pub x: i32,
    /// Baseline.
    pub y: i32,
    /// Width by the Scene's text metric.
    pub w: i32,
    /// The text.
    pub text: String,
}

/// A function box (§4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoxGeom {
    /// The box key, for `data-key`.
    pub key: BoxKey,
    /// The box number as shown (`0` on A-0 and for the A-1 lineage box).
    pub number: String,
    /// The box rectangle.
    pub rect: Rect,
    /// The drop shadow, when the box has a child diagram and the theme draws shadows.
    pub shadow: Option<Rect>,
    /// The wrapped title, centered.
    pub title: Vec<TextGeom>,
    /// The corner label inside the lower-right corner (FR-STYLE-07).
    pub corner: TextGeom,
    /// The DRE below the lower-right corner, when the box has a child diagram.
    pub dre: Option<TextGeom>,
}

/// An arrowhead: a filled triangle whose tip touches the end of the polyline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Arrowhead {
    /// The tip.
    pub tip: (i32, i32),
    /// The back corner to the left of the travel direction.
    pub left: (i32, i32),
    /// The back corner to the right of the travel direction.
    pub right: (i32, i32),
}

/// An arrow: one routed edge (a plain arrow, a fork branch, a join leaf, or a call arrow).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArrowGeom {
    /// The edge id, for `data-key`.
    pub id: EdgeKey,
    /// The orthogonal polyline from source to destination.
    pub polyline: Vec<(i32, i32)>,
    /// The arrowhead at the destination.
    pub head: Arrowhead,
}

/// A small square in a form cell: a status checkbox, or a box of the context sketch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mark {
    /// Where.
    pub rect: Rect,
    /// Filled: the current status, or the box this diagram details.
    pub on: bool,
}

/// One field of the diagram form (§14.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    /// The field name, for `data-key` (`form:status`).
    pub field: &'static str,
    /// The cell rectangle.
    pub rect: Rect,
    /// Its text lines.
    pub lines: Vec<TextGeom>,
    /// Its marks.
    pub marks: Vec<Mark>,
}

/// A band of the form: the working band above the frame or the identification band below.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Band {
    /// The band rectangle.
    pub rect: Rect,
    /// Its fields, left to right.
    pub cells: Vec<Cell>,
}

/// The diagram form (FR-LAYOUT-20, Annex C.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormGeom {
    /// The working information band. Publication export drops it (M5.9).
    pub working: Option<Band>,
    /// The identification band: node, title, number.
    pub identification: Band,
}

/// A laid-out diagram.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scene {
    /// The diagram.
    pub node: NodeId,
    /// The theme.
    pub theme: ThemeId,
    /// The text metric every width was measured with (§3.3).
    pub text_metric: TextMetric,
    /// The page: the form and the frame.
    pub size: (i32, i32),
    /// The frame (the message field).
    pub frame: Rect,
    /// The form around the frame.
    pub form: FormGeom,
    /// Boxes by box number.
    pub boxes: Vec<BoxGeom>,
    /// Arrows by id.
    pub arrows: Vec<ArrowGeom>,
}

/// Band heights in text rows (§14.4: fixed multiples of `line_h`).
const WORKING_ROWS: i32 = 5;
const IDENT_ROWS: i32 = 2;

/// su for a length in gu.
fn su(gu: i32) -> i32 {
    gu * SU_PER_GU
}

fn su_rect(r: Rect, dy: i32) -> Rect {
    Rect {
        x: su(r.x),
        y: su(r.y) + dy,
        w: su(r.w),
        h: su(r.h),
    }
}

/// The text ascent within a line: three quarters of the line height.
fn ascent(m: &Metrics) -> i32 {
    su(m.line_h) * 3 / 4
}

/// `text` cut to fit `max` su, with an ellipsis when it had to be cut.
fn fit(text: &str, max: i32, metric: TextMetric, m: &Metrics) -> String {
    use unicode_segmentation::UnicodeSegmentation;
    if metric.width_su(text, m) <= max {
        return text.to_owned();
    }
    let mut out = String::new();
    for g in text.graphemes(true) {
        let next = format!("{out}{g}…");
        if metric.width_su(&next, m) > max {
            break;
        }
        out.push_str(g);
    }
    let mut out = out.trim_end().to_owned();
    out.push('…');
    out
}

fn line(text: &str, x: i32, y: i32, metric: TextMetric, m: &Metrics) -> TextGeom {
    TextGeom {
        x,
        y,
        w: metric.width_su(text, m),
        text: text.to_owned(),
    }
}

/// The number box `i` of diagram `node` shows: `0` on A-0, the lineage number for the A-n
/// lineage box, else `i + 1`.
fn box_number(node: &NodeId, lineage: Option<u8>, i: usize) -> String {
    match node {
        NodeId::Context { level: 0 } => "0".to_owned(),
        NodeId::Context { level } if lineage.map(usize::from) == Some(i) => {
            if *level == 1 {
                "0".to_owned()
            } else {
                format!("-{}", level - 1)
            }
        }
        _ => (i + 1).to_string(),
    }
}

/// The node box `i` of diagram `node` details, if it has a node number.
fn box_node(node: &NodeId, i: usize) -> Option<NodeId> {
    match node {
        NodeId::Context { level: 0 } => Some(NodeId::a0()),
        NodeId::Context { .. } => None,
        NodeId::Node(_) => u8::try_from(i + 1).ok().and_then(|k| node.child(k)),
    }
}

/// The diagram's title: its own, or the name of the box it details (§14.4, C.4.4).
pub fn diagram_title(model: &Model, node: &NodeId) -> String {
    if let Some(t) = model.diagrams.get(node).and_then(|d| d.title.as_ref()) {
        return t.as_str().to_owned();
    }
    let (parent, index) = match node {
        NodeId::Node(path) if path.is_empty() => (NodeId::context(0), 0),
        NodeId::Node(path) => (
            node.parent().unwrap_or_else(NodeId::a0),
            usize::from(path[path.len() - 1]).saturating_sub(1),
        ),
        NodeId::Context { .. } => return String::new(),
    };
    model
        .diagrams
        .get(&parent)
        .and_then(|d| d.boxes.get(index))
        .map(|b| b.name.as_str().to_owned())
        .unwrap_or_default()
}

/// The diagram whose box this diagram details, and that box's index: the CONTEXT sketch
/// (FR-DIAG-09). A0's context is A-1 when there is one (its lineage box), else A-0.
fn context_of(model: &Model, node: &NodeId) -> Option<(NodeId, usize)> {
    let (parent, index) = match node {
        NodeId::Context { level } => {
            let up = NodeId::context(level + 1);
            let lineage = model.diagrams.get(&up)?.lineage?;
            (up, usize::from(lineage))
        }
        NodeId::Node(path) if path.is_empty() => {
            match model
                .diagrams
                .get(&NodeId::context(1))
                .and_then(|d| d.lineage)
            {
                Some(l) => (NodeId::context(1), usize::from(l)),
                None => (NodeId::context(0), 0),
            }
        }
        NodeId::Node(path) => (
            node.parent()?,
            usize::from(path[path.len() - 1]).saturating_sub(1),
        ),
    };
    model
        .diagrams
        .contains_key(&parent)
        .then_some((parent, index))
}

/// Lay out diagram `node` of `model` (§14.1).
///
/// # Errors
///
/// [`LayoutError::NoDiagram`] if the model has no such diagram; [`LayoutError::Theme`] for
/// `classic`, whose Liberation Serif metrics arrive with M5.5.
pub fn layout(
    model: &Model,
    node: &NodeId,
    input: &LayoutInput<'_>,
) -> Result<LayoutResult, LayoutError> {
    if input.theme == ThemeId::Classic {
        return Err(LayoutError::Theme(
            ThemeId::Classic,
            "Liberation Serif metrics arrive with M5.5",
        ));
    }
    let diagram = model
        .diagrams
        .get(node)
        .ok_or_else(|| LayoutError::NoDiagram(node.clone()))?;
    let m = input.metrics;
    let metric = input.theme.text_metric();
    let routed = crate::route::layout_routes(model, node, input.theme.shadows(), metric, m)
        .ok_or_else(|| LayoutError::NoDiagram(node.clone()))?;
    let (fw, fh) = routed.placed.placement.frame;
    let row = su(m.line_h);
    let top = WORKING_ROWS * row;
    let frame = Rect {
        x: 0,
        y: top,
        w: su(fw),
        h: su(fh),
    };
    let plans = crate::place::box_plans(model, node, input.theme.shadows(), metric, m);
    let counts = crate::place::port_counts(diagram);
    let boxes = diagram
        .boxes
        .iter()
        .enumerate()
        .map(|(i, _)| {
            let rect = su_rect(routed.placed.placement.boxes[i], top);
            box_geom(node, diagram, i, rect, &plans[i], counts[i], input)
        })
        .collect();
    let mut arrows: Vec<ArrowGeom> = routed
        .routing
        .routes
        .iter()
        .zip(&routed.wiring.paths)
        .map(|(r, p)| {
            let polyline: Vec<(i32, i32)> = p.iter().map(|&(x, y)| (su(x), su(y) + top)).collect();
            ArrowGeom {
                id: r.key.clone(),
                head: arrowhead(&polyline, m),
                polyline,
            }
        })
        .collect();
    arrows.sort_by(|a, b| a.id.cmp(&b.id));
    let form = form(model, node, frame, input, m);
    let size = (frame.w, top + frame.h + IDENT_ROWS * row);
    let diagnostics = routed
        .placed
        .overflow_diagnostic(node)
        .into_iter()
        .collect();
    Ok(LayoutResult {
        scene: Scene {
            node: node.clone(),
            theme: input.theme,
            text_metric: metric,
            size,
            frame,
            form,
            boxes,
            arrows,
        },
        diagnostics,
    })
}

/// One box's geometry (§4.1–§4.3): the title centered above the corner label, the corner
/// label, the shadow and the DRE.
fn box_geom(
    node: &NodeId,
    diagram: &idef0_core::model::Diagram,
    i: usize,
    rect: Rect,
    plan: &crate::place::BoxPlan,
    ports: crate::size::PortCounts,
    input: &LayoutInput<'_>,
) -> BoxGeom {
    let m = input.metrics;
    let metric = input.theme.text_metric();
    let row = su(m.line_h);
    let size = crate::size::box_size(diagram.boxes[i].name.as_str(), ports, metric, m);
    let pad = su(m.pad);
    let area_top = rect.y + pad;
    let area_h = rect.h - 2 * pad - su(m.corner_label_h());
    let block = row * i32::try_from(size.lines.len()).unwrap_or(1);
    let first = area_top + (area_h - block).max(0) / 2;
    let title = size
        .lines
        .iter()
        .zip(0..)
        .map(|(t, k)| {
            let w = metric.width_su(t, m);
            line(
                t,
                rect.x + (rect.w - w) / 2,
                first + k * row + ascent(m),
                metric,
                m,
            )
        })
        .collect();
    let number = box_number(node, diagram.lineage, i);
    let corner_text = match (input.theme, box_node(node, i)) {
        (ThemeId::Fips183, _) | (_, None) => number.clone(),
        (_, Some(n)) => n.to_string(),
    };
    let cw = metric.width_su(&corner_text, m);
    let corner = line(
        &corner_text,
        rect.x + rect.w - pad - cw,
        rect.y + rect.h - pad,
        metric,
        m,
    );
    let shadow = plan.shadow.then(|| Rect {
        x: rect.x + su(m.shadow_x),
        y: rect.y + su(m.shadow_y),
        ..rect
    });
    let dre = plan.dre_w.and_then(|_| {
        let child = box_node(node, i)?;
        let text = child.to_string();
        let w = metric.width_su(&text, m);
        let (right, bottom) = shadow.map_or((rect.x + rect.w, rect.y + rect.h), |s| {
            (s.x + s.w, s.y + s.h)
        });
        Some(line(&text, right - w, bottom + ascent(m), metric, m))
    });
    BoxGeom {
        key: diagram.boxes[i].key.clone(),
        number,
        rect,
        shadow,
        title,
        corner,
        dre,
    }
}

/// The arrowhead at the end of `p`, pointing along its last segment.
fn arrowhead(p: &[(i32, i32)], m: &Metrics) -> Arrowhead {
    let tip = p.last().copied().unwrap_or((0, 0));
    let from = p.len().checked_sub(2).map_or(tip, |i| p[i]);
    let (dx, dy) = ((tip.0 - from.0).signum(), (tip.1 - from.1).signum());
    let base = (tip.0 - dx * m.head_len, tip.1 - dy * m.head_len);
    let (px, py) = (-dy * m.head_halfw, dx * m.head_halfw);
    Arrowhead {
        tip,
        left: (base.0 + px, base.1 + py),
        right: (base.0 - px, base.1 - py),
    }
}

/// Column widths as sixteenths of `w`, the last taking the remainder, on the 8 su lattice.
fn columns(x: i32, w: i32, sixteenths: &[i32]) -> Vec<(i32, i32)> {
    let mut out = Vec::with_capacity(sixteenths.len());
    let mut at = x;
    for (k, &s) in sixteenths.iter().enumerate() {
        let cw = if k + 1 == sixteenths.len() {
            x + w - at
        } else {
            (w * s / 16).div_euclid(8) * 8
        };
        out.push((at, cw));
        at += cw;
    }
    out
}

#[allow(
    clippy::many_single_char_names,
    reason = "cell geometry: x, y, w, h, k, m, t"
)]
fn cell(
    field: &'static str,
    (x, w): (i32, i32),
    y: i32,
    h: i32,
    texts: &[String],
    metric: TextMetric,
    m: &Metrics,
) -> Cell {
    let pad = su(m.pad);
    let row = su(m.line_h);
    Cell {
        field,
        rect: Rect { x, y, w, h },
        lines: texts
            .iter()
            .zip(0..)
            .map(|(t, k)| {
                let t = fit(t, w - 2 * pad, metric, m);
                line(&t, x + pad, y + k * row + ascent(m), metric, m)
            })
            .collect(),
        marks: Vec::new(),
    }
}

/// The diagram form around `frame` (§14.4): the working band above, identification below.
fn form(
    model: &Model,
    node: &NodeId,
    frame: Rect,
    input: &LayoutInput<'_>,
    m: &Metrics,
) -> FormGeom {
    let metric = input.theme.text_metric();
    let row = su(m.line_h);
    let diagram = &model.diagrams[node];
    let opt = |s: Option<&str>| s.unwrap_or("").to_owned();
    let wh = WORKING_ROWS * row;
    let cols = columns(frame.x, frame.w, &[3, 5, 3, 2, 3]);
    // FR-CALL-04: the callers of this diagram's boxes, within this model
    let callers: Vec<String> = idef0_core::calls::used_at_diagram(&[model], model, node)
        .iter()
        .map(ToString::to_string)
        .collect();
    let mut used = vec!["USED AT:".to_owned()];
    if !callers.is_empty() {
        used.push(callers.join(", "));
    }
    let used_at = cell("form:used-at", cols[0], 0, wh, &used, metric, m);
    let author = cell(
        "form:author",
        cols[1],
        0,
        wh,
        &[
            format!(
                "AUTHOR: {}",
                opt(diagram.author.as_deref().or(model.meta.author.as_deref()))
            ),
            format!("PROJECT: {}", opt(model.meta.project.as_deref())),
            format!("DATE: {}", opt(diagram.date.as_deref())),
        ],
        metric,
        m,
    );
    let statuses = [
        (Status::Working, "WORKING"),
        (Status::Draft, "DRAFT"),
        (Status::Recommended, "RECOMMENDED"),
        (Status::Publication, "PUBLICATION"),
    ];
    let box_side = row / 2;
    let mut status = cell(
        "form:status",
        (
            cols[2].0 + box_side + su(m.pad),
            cols[2].1 - box_side - su(m.pad),
        ),
        0,
        wh,
        &statuses
            .iter()
            .map(|(_, s)| (*s).to_owned())
            .collect::<Vec<_>>(),
        metric,
        m,
    );
    status.rect = Rect {
        x: cols[2].0,
        y: 0,
        w: cols[2].1,
        h: wh,
    };
    status.marks = statuses
        .iter()
        .zip(0..)
        .map(|((s, _), k)| Mark {
            rect: Rect {
                x: cols[2].0 + su(m.pad),
                y: k * row + (row - box_side) / 2,
                w: box_side,
                h: box_side,
            },
            on: *s == diagram.status,
        })
        .collect();
    let reader = cell(
        "form:reader",
        cols[3],
        0,
        wh,
        &["READER:".to_owned(), "DATE:".to_owned()],
        metric,
        m,
    );
    let ctx = context(model, node, cols[4], wh, input, m);
    let identification = identification(model, node, frame, metric, m);
    FormGeom {
        working: Some(Band {
            rect: Rect {
                x: frame.x,
                y: 0,
                w: frame.w,
                h: wh,
            },
            cells: vec![used_at, author, status, reader, ctx],
        }),
        identification,
    }
}

/// The identification band below `frame` (§14.4): NODE, TITLE, NUMBER.
fn identification(
    model: &Model,
    node: &NodeId,
    frame: Rect,
    metric: TextMetric,
    m: &Metrics,
) -> Band {
    let diagram = &model.diagrams[node];
    let row = su(m.line_h);
    let ih = IDENT_ROWS * row;
    let iy = frame.y + frame.h;
    let icols = columns(frame.x, frame.w, &[4, 9, 3]);
    let ident = vec![
        cell(
            "form:node",
            icols[0],
            iy,
            ih,
            &["NODE:".to_owned(), format!("{}/{node}", model.meta.abbrev)],
            metric,
            m,
        ),
        cell(
            "form:title",
            icols[1],
            iy,
            ih,
            &["TITLE:".to_owned(), diagram_title(model, node)],
            metric,
            m,
        ),
        cell(
            "form:number",
            icols[2],
            iy,
            ih,
            &[
                "NUMBER:".to_owned(),
                diagram
                    .cnumber
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_default(),
            ],
            metric,
            m,
        ),
    ];
    Band {
        rect: Rect {
            x: frame.x,
            y: iy,
            w: frame.w,
            h: ih,
        },
        cells: ident,
    }
}

/// The CONTEXT field (C.4.1.5): `TOP` on A-0, `NONE` where there is no higher diagram, else
/// a sketch of the higher diagram's boxes with the detailed box filled, scaled by 1/8 or
/// smaller to fit.
fn context(
    model: &Model,
    node: &NodeId,
    col: (i32, i32),
    h: i32,
    input: &LayoutInput<'_>,
    m: &Metrics,
) -> Cell {
    let metric = input.theme.text_metric();
    let row = su(m.line_h);
    let pad = su(m.pad);
    if *node == NodeId::context(0) {
        return cell(
            "form:context",
            col,
            0,
            h,
            &["CONTEXT:".to_owned(), "TOP".to_owned()],
            metric,
            m,
        );
    }
    let Some((parent, index)) = context_of(model, node) else {
        return cell(
            "form:context",
            col,
            0,
            h,
            &["CONTEXT:".to_owned(), "NONE".to_owned()],
            metric,
            m,
        );
    };
    let mut c = cell(
        "form:context",
        col,
        0,
        h,
        &["CONTEXT:".to_owned()],
        metric,
        m,
    );
    let Some(up) = crate::route::layout_routes(model, &parent, input.theme.shadows(), metric, m)
    else {
        return c;
    };
    let (pw, ph) = up.placed.placement.frame;
    let (room_w, room_h) = (col.1 - 2 * pad, h - row - 2 * pad);
    let mut k = 8;
    while (su(pw) > room_w * k || su(ph) > room_h * k) && k < 1 << 12 {
        k *= 2;
    }
    let (ox, oy) = (col.0 + pad, row + pad);
    c.marks = up
        .placed
        .placement
        .boxes
        .iter()
        .enumerate()
        .map(|(i, b)| Mark {
            rect: Rect {
                x: ox + su(b.x) / k,
                y: oy + su(b.y) / k,
                w: (su(b.w) / k).max(1),
                h: (su(b.h) / k).max(1),
            },
            on: i == index,
        })
        .collect();
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model(src: &str) -> Model {
        let p = idef0_core::dsl::parse(src);
        assert!(p.diagnostics.is_empty(), "{:?}", p.diagnostics);
        p.model.expect("model")
    }

    fn fixture(name: &str) -> Model {
        let path = format!("{}/../../fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
        model(&std::fs::read_to_string(path).expect("fixture"))
    }

    fn scene(m: &Model, node: &NodeId, theme: ThemeId) -> Scene {
        layout(
            m,
            node,
            &LayoutInput {
                theme,
                metrics: &Metrics::default(),
            },
        )
        .expect("layout")
        .scene
    }

    // §14.1: Scene coordinates are su, 4 per gu; the frame sits below the working band and
    // arrows run in id order.
    #[test]
    fn su_coordinates_and_order() {
        let m = fixture("valid/three-boxes.idef0");
        let s = scene(&m, &NodeId::a0(), ThemeId::Modern);
        let metrics = Metrics::default();
        let x = crate::route::layout_routes(&m, &NodeId::a0(), true, TextMetric::Mono, &metrics)
            .expect("A0");
        assert_eq!(s.frame.y, WORKING_ROWS * 16);
        assert_eq!(
            (s.frame.w, s.frame.h),
            (
                x.placed.placement.frame.0 * 4,
                x.placed.placement.frame.1 * 4
            )
        );
        assert_eq!(s.boxes[0].rect.x, x.placed.placement.boxes[0].x * 4);
        assert_eq!(
            s.boxes[0].rect.y,
            x.placed.placement.boxes[0].y * 4 + s.frame.y
        );
        assert!(s.arrows.windows(2).all(|w| w[0].id < w[1].id));
        assert_eq!(s.size.1, s.frame.y + s.frame.h + IDENT_ROWS * 16);
        assert_eq!(s.size.0, s.frame.w);
    }

    // Arrowheads point along the last segment: a control enters its box going down.
    #[test]
    fn arrowheads() {
        let m = Metrics::default();
        let h = arrowhead(&[(40, 0), (40, 100)], &m);
        assert_eq!(h.tip, (40, 100));
        assert_eq!((h.left, h.right), ((36, 92), (44, 92)));
        let h = arrowhead(&[(0, 20), (60, 20)], &m);
        assert_eq!((h.left, h.right), ((52, 24), (52, 16)));
    }

    // FR-STYLE-06/-07 by theme: modern shows shadows and node numbers, fips183 box numbers
    // and no shadow; both show the DRE of a decomposed box.
    #[test]
    fn theme_conventions() {
        let m = fixture("dogfood/idef0-rs.idef0");
        let top = NodeId::context(0);
        let modern = scene(&m, &top, ThemeId::Modern);
        let fips = scene(&m, &top, ThemeId::Fips183);
        let (a, b) = (&modern.boxes[0], &fips.boxes[0]);
        assert!(a.shadow.is_some(), "A-0's box is detailed by A0");
        assert!(b.shadow.is_none());
        assert_eq!(a.dre.as_ref().map(|d| d.text.as_str()), Some("A0"));
        assert_eq!(b.dre.as_ref().map(|d| d.text.as_str()), Some("A0"));
        assert_eq!(
            (a.corner.text.as_str(), b.corner.text.as_str()),
            ("A0", "0")
        );
        assert_eq!(a.number, "0");
        let a0 = scene(&m, &NodeId::a0(), ThemeId::Modern);
        assert!(
            a0.boxes
                .iter()
                .all(|b| b.shadow.is_none() && b.dre.is_none()),
            "leaves"
        );
        assert_eq!(a0.boxes[0].corner.text, "A1");
        assert_eq!(
            scene(&m, &NodeId::a0(), ThemeId::Fips183).boxes[0]
                .corner
                .text,
            "1"
        );
    }

    // §14.4: the form's fields come from the model, the status mark follows the diagram, and
    // CONTEXT is TOP on A-0 and a sketch with the detailed box filled below it.
    #[test]
    fn diagram_form() {
        let m = fixture("valid/constructs.idef0");
        let s = scene(&m, &NodeId::a0(), ThemeId::Modern);
        let w = s.form.working.as_ref().expect("working band");
        let text = |band: &Band, field: &str| -> Vec<String> {
            band.cells
                .iter()
                .find(|c| c.field == field)
                .expect("field")
                .lines
                .iter()
                .map(|l| l.text.clone())
                .collect()
        };
        assert!(
            text(w, "form:author")[0].starts_with("AUTHOR: A."),
            "{:?}",
            text(w, "form:author")
        );
        let status = w
            .cells
            .iter()
            .find(|c| c.field == "form:status")
            .expect("status");
        let on: Vec<bool> = status.marks.iter().map(|k| k.on).collect();
        assert_eq!(on, [false, true, false, false], "draft");
        let ctx = w
            .cells
            .iter()
            .find(|c| c.field == "form:context")
            .expect("context");
        assert_eq!(ctx.marks.iter().filter(|k| k.on).count(), 1);
        let id = &s.form.identification;
        assert_eq!(
            text(id, "form:node"),
            ["NODE:", "ORD/A0"].map(String::from),
            "{:?}",
            text(id, "form:node")
        );
        assert_eq!(
            text(id, "form:title")[1],
            "Fulfill Orders",
            "the A-0 box name"
        );
        assert!(text(id, "form:number")[1].starts_with("AK15"));
        let top = scene(&m, &NodeId::context(0), ThemeId::Modern);
        let tw = top.form.working.as_ref().expect("working band");
        assert_eq!(text(tw, "form:context"), ["CONTEXT:", "TOP"]);
        let child = scene(&m, &NodeId::from_boxes(&[3]).expect("A3"), ThemeId::Modern);
        let cw = child.form.working.as_ref().expect("working band");
        let sketch = cw
            .cells
            .iter()
            .find(|c| c.field == "form:context")
            .expect("context");
        let lit: Vec<usize> = (0..sketch.marks.len())
            .filter(|&i| sketch.marks[i].on)
            .collect();
        assert_eq!(lit, [2], "box 3 of A0 is filled");
    }

    // FR-CALL-04: USED AT lists the callers of the diagram's boxes; FR-DIAG-09: A0's
    // CONTEXT is A-1 when there is one.
    #[test]
    fn used_at_and_a0_context() {
        let src = "idef0 0.2\nmodel \"U\" {\n  abbrev \"USE\"\n}\ndiagram A-0 \"Run\" {\n  box 0 \"Run\"\n}\ndiagram A0 {\n  box 1 \"Take\"\n  box 2 \"Pack\"\n  box 3 \"Ship\"\n  call 3 -> @wrap\n}\ndiagram A2 {\n  box 1 \"Pick\"\n  box 2 \"Wrap\"\n  box 3 \"Label\"\n}\n";
        let m = model(src);
        let s = scene(&m, &"A2".parse().expect("A2"), ThemeId::Modern);
        let used = s.form.working.as_ref().expect("working").cells[0]
            .lines
            .iter()
            .map(|l| l.text.clone())
            .collect::<Vec<_>>();
        assert_eq!(used, ["USED AT:", "A3"]);
        let c = fixture("valid/constructs.idef0");
        let a0 = scene(&c, &NodeId::a0(), ThemeId::Modern);
        let ctx = a0
            .form
            .working
            .as_ref()
            .expect("working")
            .cells
            .iter()
            .find(|x| x.field == "form:context")
            .expect("context");
        assert_eq!(ctx.marks.len(), 3, "the A-1 sketch has three boxes");
        assert_eq!(
            ctx.marks.iter().position(|k| k.on),
            Some(1),
            "its lineage box"
        );
    }

    // Box numbers as shown: 0 on A-0, the lineage number on A-n (0 on A-1, −(n−1) above),
    // else the position.
    #[test]
    fn box_numbers() {
        assert_eq!(box_number(&NodeId::context(0), None, 0), "0");
        assert_eq!(box_number(&NodeId::context(1), Some(1), 1), "0");
        assert_eq!(box_number(&NodeId::context(3), Some(1), 1), "-2");
        assert_eq!(box_number(&NodeId::context(3), Some(1), 2), "3");
        assert_eq!(box_number(&NodeId::a0(), None, 4), "5");
    }

    // Long field text is cut with an ellipsis, never past its cell.
    #[test]
    fn fit_cuts_with_ellipsis() {
        let m = Metrics::default();
        assert_eq!(fit("short", 100, TextMetric::Mono, &m), "short");
        let cut = fit("a long project name", 64, TextMetric::Mono, &m);
        assert_eq!(cut, "a long…", "no space before the ellipsis");
        assert!(TextMetric::Mono.width_su(&cut, &m) <= 64);
    }

    #[test]
    fn errors() {
        let m = fixture("valid/three-boxes.idef0");
        let input = LayoutInput {
            theme: ThemeId::Modern,
            metrics: &Metrics::default(),
        };
        let missing = NodeId::from_boxes(&[7]).expect("A7");
        assert_eq!(
            layout(&m, &missing, &input),
            Err(LayoutError::NoDiagram(missing))
        );
        let classic = LayoutInput {
            theme: ThemeId::Classic,
            ..input
        };
        assert!(matches!(
            layout(&m, &NodeId::a0(), &classic),
            Err(LayoutError::Theme(ThemeId::Classic, _))
        ));
    }
}
