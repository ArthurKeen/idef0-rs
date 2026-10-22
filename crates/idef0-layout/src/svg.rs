//! SVG output (`docs/spec/layout.md` §14.1).
//!
//! The output is a pure function of the Scene: integer px coordinates (1 su = 1 px), a fixed
//! element and attribute order, `data-key` on every element, and no timestamps, versions or
//! generator comments, so identical Scenes give identical bytes on every target (D6).

use core::fmt::Write as _;

use crate::ports::Rect;
use crate::scene::{Scene, TextGeom};

/// How text is emitted (§3.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextMode {
    /// A font-family reference only. Until the bundled fonts arrive (M5.5), this is the only
    /// mode and the default.
    #[default]
    Ref,
}

/// SVG options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SvgOptions {
    /// The text mode.
    pub text: TextMode,
}

/// The fixed stylesheet. Text is unkerned and ligature-free, as layout measured it (§3.3).
const STYLE: &str = "\
.page{fill:#fff}\
.band,.cell,.frame{fill:none;stroke:#000;stroke-width:1}\
.box{fill:#fff;stroke:#000;stroke-width:2}\
.shadow{fill:#000}\
.arrow{fill:none;stroke:#000;stroke-width:1}\
.head{fill:#000}\
.mark{fill:#fff;stroke:#000;stroke-width:1}\
.mark.on{fill:#000}\
text{font-family:monospace;font-size:13px;fill:#000;\
font-kerning:none;font-variant-ligatures:none;white-space:pre}";

/// XML text and attribute escaping.
fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c => out.push(c),
        }
    }
    out
}

fn rect(out: &mut String, key: &str, class: &str, r: Rect) {
    let _ = writeln!(
        out,
        r#"<rect data-key="{}" class="{class}" x="{}" y="{}" width="{}" height="{}"/>"#,
        esc(key),
        r.x,
        r.y,
        r.w,
        r.h
    );
}

fn text(out: &mut String, key: &str, class: &str, t: &TextGeom) {
    let _ = writeln!(
        out,
        r#"<text data-key="{}" class="{class}" x="{}" y="{}">{}</text>"#,
        esc(key),
        t.x,
        t.y,
        esc(&t.text)
    );
}

/// Render `scene` as an SVG document.
pub fn render(scene: &Scene, opts: &SvgOptions) -> String {
    let TextMode::Ref = opts.text;
    let (w, h) = scene.size;
    let node = scene.node.to_string();
    let mut out = String::new();
    let _ = writeln!(
        out,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}" data-key="{}" data-theme="{}">"#,
        esc(&node),
        scene.theme.as_str()
    );
    let _ = writeln!(out, r#"<style data-key="style">{STYLE}</style>"#);
    rect(&mut out, "page", "page", Rect { x: 0, y: 0, w, h });
    let bands = scene
        .form
        .working
        .iter()
        .chain(core::iter::once(&scene.form.identification));
    for band in bands {
        let key = if band.rect.y == 0 {
            "form:working"
        } else {
            "form:identification"
        };
        rect(&mut out, key, "band", band.rect);
        for c in &band.cells {
            rect(&mut out, c.field, "cell", c.rect);
            for m in &c.marks {
                rect(
                    &mut out,
                    c.field,
                    if m.on { "mark on" } else { "mark" },
                    m.rect,
                );
            }
            for l in &c.lines {
                text(&mut out, c.field, "field", l);
            }
        }
    }
    rect(&mut out, "frame", "frame", scene.frame);
    for b in &scene.boxes {
        let key = b.key.as_str();
        if let Some(s) = b.shadow {
            rect(&mut out, key, "shadow", s);
        }
        rect(&mut out, key, "box", b.rect);
        for l in &b.title {
            text(&mut out, key, "title", l);
        }
        text(&mut out, key, "corner", &b.corner);
        if let Some(d) = &b.dre {
            text(&mut out, key, "dre", d);
        }
    }
    for a in &scene.arrows {
        let key = esc(&a.id.to_string());
        let mut d = String::new();
        for (i, (x, y)) in a.polyline.iter().enumerate() {
            let _ = write!(d, "{}{x} {y}", if i == 0 { "M" } else { " L" });
        }
        let _ = writeln!(out, r#"<path data-key="{key}" class="arrow" d="{d}"/>"#);
        let hd = a.head;
        let _ = writeln!(
            out,
            r#"<path data-key="{key}" class="head" d="M{} {} L{} {} L{} {} Z"/>"#,
            hd.tip.0, hd.tip.1, hd.left.0, hd.left.1, hd.right.0, hd.right.1
        );
    }
    out.push_str("</svg>\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::Metrics;
    use crate::scene::{LayoutInput, ThemeId, layout};
    use idef0_core::id::NodeId;

    fn svg(name: &str, node: &NodeId) -> String {
        let path = format!("{}/../../fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
        let model = idef0_core::dsl::parse(&std::fs::read_to_string(path).expect("fixture"))
            .model
            .expect("model");
        let input = LayoutInput {
            theme: ThemeId::Modern,
            metrics: &Metrics::default(),
        };
        render(
            &layout(&model, node, &input).expect("layout").scene,
            &SvgOptions::default(),
        )
    }

    #[test]
    fn escaping() {
        assert_eq!(
            esc(r#"a<b & "c" 'd'>"#),
            "a&lt;b &amp; &quot;c&quot; &apos;d&apos;&gt;"
        );
    }

    // §14.1: every element carries `data-key`, coordinates are integers, and nothing
    // time- or version-dependent is emitted.
    #[test]
    fn keyed_integer_deterministic() {
        let out = svg("dogfood/idef0-rs.idef0", &NodeId::a0());
        let mut elements = 0;
        for tag in out.split('<').skip(1) {
            if tag.starts_with('/') {
                continue;
            }
            elements += 1;
            assert!(tag.contains("data-key=\""), "unkeyed element <{tag}");
        }
        assert!(elements > 50, "{elements}");
        for attr in [" x=\"", " y=\"", " width=\"", " height=\""] {
            for v in out.split(attr).skip(1) {
                let v = &v[..v.find('"').expect("closed")];
                assert!(v.parse::<i32>().is_ok(), "{attr}{v}");
            }
        }
        assert!(!out.contains(env!("CARGO_PKG_VERSION")));
        assert_eq!(out, svg("dogfood/idef0-rs.idef0", &NodeId::a0()));
    }
}
