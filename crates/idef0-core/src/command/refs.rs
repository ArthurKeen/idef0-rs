//! Free-text node references follow renumbering (FR-EDIT-20, metamodel §13.1).
//!
//! A token that matches the node part of a `BoxRef` or `PortRef` at a word boundary, with no
//! model prefix or with the model's own, is rewritten when its node was renamed. The token
//! keeps its written style: dotted (`A2.1`) stays dotted. Port suffixes (`/I1`, FIPS `I1`)
//! and other models' references are left alone.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::id::NodeId;
use crate::model::Model;

/// Rewrite `text`'s node references through `moved` (old → new). Returns `None` when
/// nothing changed.
pub(crate) fn rewrite(text: &str, own: &str, moved: &BTreeMap<NodeId, NodeId>) -> Option<String> {
    let b = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    let mut last = 0;
    let mut changed = false;
    while i < b.len() {
        let boundary = i == 0 || !(b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_');
        if b[i] != b'A' || !boundary {
            i += 1;
            continue;
        }
        // a model prefix must be this model's own
        if i > 0 && b[i - 1] == b'/' {
            let start = text[..i - 1]
                .rfind(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-'))
                .map_or(0, |p| p + 1);
            if &text[start..i - 1] != own {
                i += 1;
                continue;
            }
        }
        let mut j = i + 1;
        while j < b.len()
            && (b[j].is_ascii_digit()
                || (b[j] == b'.' && b.get(j + 1).is_some_and(u8::is_ascii_digit)))
        {
            j += 1;
        }
        let next_ok = j >= b.len()
            || !(b[j].is_ascii_alphanumeric() || b[j] == b'_')
            || (matches!(b[j], b'I' | b'C' | b'O' | b'M')
                && b.get(j + 1).is_some_and(u8::is_ascii_digit));
        if j > i + 1
            && next_ok
            && let Ok(node) = text[i..j].parse::<NodeId>()
            && let Some(new) = moved.get(&node)
        {
            out.push_str(&text[last..i]);
            if text[i..j].contains('.') {
                out.push_str(&new.dotted().to_string());
            } else {
                out.push_str(&new.to_string());
            }
            last = j;
            changed = true;
        }
        i = j.max(i + 1);
    }
    changed.then(|| {
        out.push_str(&text[last..]);
        out
    })
}

/// Rewrite every box, diagram and concept description and every model note of `model`.
pub(crate) fn rewrite_model(model: &mut Model, moved: &BTreeMap<NodeId, NodeId>) {
    let own = model.meta.abbrev.to_string();
    for d in model.diagrams.values_mut() {
        let mut next = None;
        let get = |next: &mut Option<crate::model::Diagram>, d: &crate::model::Diagram| {
            if next.is_none() {
                *next = Some(d.clone());
            }
        };
        if let Some(t) = rewrite(&d.description.text, &own, moved) {
            get(&mut next, d);
            if let Some(n) = next.as_mut() {
                n.description.text = t;
            }
        }
        for (i, b) in d.boxes.iter().enumerate() {
            if let Some(t) = rewrite(&b.description.text, &own, moved) {
                get(&mut next, d);
                if let Some(n) = next.as_mut() {
                    n.boxes[i].description.text = t;
                }
            }
        }
        for (i, note) in d.notes.iter().enumerate() {
            if let Some(t) = rewrite(&note.text, &own, moved) {
                get(&mut next, d);
                if let Some(n) = next.as_mut() {
                    n.notes[i].text = t;
                }
            }
        }
        if let Some(n) = next {
            *d = Arc::new(n);
        }
    }
    for c in model.concepts.values_mut() {
        if let Some(t) = rewrite(&c.description.text, &own, moved) {
            c.description.text = t;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n(s: &str) -> NodeId {
        s.parse().expect("node")
    }

    #[test]
    fn rewrites_keep_style_and_scope() {
        let moved: BTreeMap<NodeId, NodeId> = [(n("A3"), n("A2")), (n("A31"), n("A21"))].into();
        let r = |s: &str| rewrite(s, "ORD", &moved);
        assert_eq!(r("see A3 and A3.1.").as_deref(), Some("see A2 and A2.1."));
        assert_eq!(
            r("port A31/I1 and A31I1").as_deref(),
            Some("port A21/I1 and A21I1")
        );
        assert_eq!(r("ORD/A3 but FAC/A3").as_deref(), Some("ORD/A2 but FAC/A3"));
        assert_eq!(
            r("XA3 A30 A3x"),
            None,
            "not at a word boundary, or another node"
        );
        assert_eq!(r("A4 stays"), None);
    }
}
