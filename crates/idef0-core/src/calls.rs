//! Call arrows: display labels and the Used At index (`docs/spec/metamodel.md` §7;
//! FR-CALL-03, -04, -07, -12).
//!
//! Targets are stored by key (FR-CALL-12). Their display labels and the Used At lists are
//! computed here, from the models a caller can see: its own, and in a workspace the others
//! the index knows. The validation rules over calls (`call_unresolved`, `call_recursive`,
//! `call_alternatives_without_note`, the interface checks) are Mangle rules (validation §5.9).

use std::collections::{BTreeMap, BTreeSet};

use crate::id::{ModelAbbrev, NodeId, NodeRef};
use crate::model::{CallArrow, KeyRef, Model};
use crate::text::BoxKey;

/// How a call label shows node numbers (layout theme option `call_label`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LabelStyle {
    /// FIPS reference style: the containing diagram's node and the box number, `A5.2`.
    #[default]
    Fips,
    /// The callee's node number, `A52`.
    Node,
}

/// The models a call can reach: `own`, and any others by abbrev.
#[derive(Debug, Clone, Copy)]
pub struct Scope<'a> {
    /// The calling model.
    pub own: &'a Model,
    /// Other models, as the workspace index provides them.
    pub others: &'a [&'a Model],
}

impl<'a> Scope<'a> {
    /// Just the calling model (single-file mode).
    pub fn alone(own: &'a Model) -> Self {
        Self { own, others: &[] }
    }

    /// The model a target names: its own model when unqualified or qualified with the own
    /// abbrev.
    pub fn model(&self, abbrev: Option<&ModelAbbrev>) -> Option<&'a Model> {
        match abbrev {
            None => Some(self.own),
            Some(a) if *a == self.own.meta.abbrev => Some(self.own),
            Some(a) => self.others.iter().copied().find(|m| m.meta.abbrev == *a),
        }
    }

    /// The callee's node, if the target resolves to a callable box (FR-CALL-07, -13): a box
    /// on a decomposition diagram, or another model's top box.
    pub fn resolve(&self, target: &KeyRef<BoxKey>) -> Option<(&'a Model, NodeId)> {
        let m = self.model(target.model.as_ref())?;
        let node = m.box_nodes().remove(&target.key)?;
        let own = std::ptr::eq(m, self.own);
        // the calling model's own top box is not callable: A0 must be decomposed
        (!(own && node == NodeId::a0())).then_some((m, node))
    }
}

/// The display label of one target (FR-CALL-03): `A5.2` for box A52 of the calling model,
/// `SEC/A2.1` in model SEC, or the stored key (`@pack`, `SEC/@pack`) when it does not
/// resolve. Renumbering the callee changes the label, never the stored target.
pub fn target_label(scope: &Scope<'_>, target: &KeyRef<BoxKey>, style: LabelStyle) -> String {
    let Some((m, node)) = scope.resolve(target) else {
        return target.to_string();
    };
    let prefix = if std::ptr::eq(m, scope.own) {
        String::new()
    } else {
        format!("{}/", m.meta.abbrev)
    };
    match style {
        LabelStyle::Fips => format!("{prefix}{}", node.dotted()),
        LabelStyle::Node => format!("{prefix}{node}"),
    }
}

/// The label of a call arrow: its targets' labels, alternatives separated by ` | `
/// (FR-CALL-05). The note glyph is drawn by the renderer.
pub fn call_label(scope: &Scope<'_>, call: &CallArrow, style: LabelStyle) -> String {
    call.targets
        .iter()
        .map(|t| target_label(scope, t, style))
        .collect::<Vec<_>>()
        .join(" | ")
}

/// The Used At index (FR-CALL-04, Annex C.4.1.6): for each diagram that contains a called
/// box, the node references of its callers. A caller in another model is qualified with that
/// model's abbrev. Keys: the called box's model abbrev and the diagram that contains it.
pub fn used_at(models: &[&Model]) -> BTreeMap<(ModelAbbrev, NodeId), BTreeSet<NodeRef>> {
    let mut out: BTreeMap<(ModelAbbrev, NodeId), BTreeSet<NodeRef>> = BTreeMap::new();
    for (k, &m) in models.iter().enumerate() {
        let others: Vec<&Model> = models
            .iter()
            .enumerate()
            .filter(|(j, _)| *j != k)
            .map(|(_, &o)| o)
            .collect();
        let scope = Scope {
            own: m,
            others: &others,
        };
        let box_node = m.box_nodes();
        for d in m.diagrams.values() {
            for b in &d.boxes {
                let (Some(call), Some(caller)) = (&b.call, box_node.get(&b.key)) else {
                    continue;
                };
                for t in &call.targets {
                    let Some((callee_model, callee_node)) = scope.resolve(t) else {
                        continue;
                    };
                    let Some(diagram) = callee_node
                        .parent()
                        .or_else(|| (callee_node == NodeId::a0()).then(|| NodeId::context(0)))
                    else {
                        continue;
                    };
                    let same = std::ptr::eq(callee_model, m);
                    let who =
                        NodeRef::new((!same).then(|| m.meta.abbrev.clone()), caller.clone(), None);
                    out.entry((callee_model.meta.abbrev.clone(), diagram))
                        .or_default()
                        .insert(who);
                }
            }
        }
    }
    out
}

/// The Used At list of diagram `node` of `model`, within `models` (which include it).
pub fn used_at_diagram(models: &[&Model], model: &Model, node: &NodeId) -> Vec<NodeRef> {
    used_at(models)
        .remove(&(model.meta.abbrev.clone(), node.clone()))
        .map(|s| s.into_iter().collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model(src: &str) -> Model {
        let p = crate::dsl::parse(src);
        assert!(p.diagnostics.is_empty(), "{:?}", p.diagnostics);
        p.model.expect("model")
    }

    const ORD: &str = "idef0 0.2\nmodel \"Orders\" {\n  abbrev \"ORD\"\n}\ndiagram A-0 \"Run\" {\n  box 0 \"Run\"\n}\ndiagram A0 {\n  box 1 \"Take\"\n  box 2 \"Pack\"\n  box 3 \"Ship\"\n  call 3 -> @pick | SEC/@seal\n}\ndiagram A2 {\n  box 1 \"Pick\"\n  box 2 \"Wrap\"\n  box 3 \"Label\"\n}\n";
    const SEC: &str = "idef0 0.2\nmodel \"Security\" {\n  abbrev \"SEC\"\n}\ndiagram A-0 \"Guard\" {\n  box 0 \"Guard\"\n}\ndiagram A0 {\n  box 1 \"Check\"\n  box 2 \"Seal\"\n  box 3 \"Log\"\n  call 1 -> ORD/@wrap\n}\n";

    fn kr(s: &str) -> KeyRef<BoxKey> {
        s.parse().expect("target")
    }

    // FR-CALL-03, -05, -07: labels name the diagram and box (`A2.1`), qualified in another
    // model; alternatives join with `|`; an unresolved target shows its key.
    #[test]
    fn labels() {
        let (ord, sec) = (model(ORD), model(SEC));
        let others = [&sec];
        let scope = Scope {
            own: &ord,
            others: &others,
        };
        assert_eq!(target_label(&scope, &kr("@pick"), LabelStyle::Fips), "A2.1");
        assert_eq!(target_label(&scope, &kr("@pick"), LabelStyle::Node), "A21");
        assert_eq!(target_label(&scope, &kr("@pack"), LabelStyle::Fips), "A2");
        assert_eq!(
            target_label(&scope, &kr("SEC/@seal"), LabelStyle::Fips),
            "SEC/A2"
        );
        assert_eq!(
            target_label(&scope, &kr("ORD/@wrap"), LabelStyle::Fips),
            "A2.2",
            "own prefix"
        );
        assert_eq!(
            target_label(&Scope::alone(&ord), &kr("SEC/@seal"), LabelStyle::Fips),
            "SEC/@seal"
        );
        assert_eq!(
            target_label(&scope, &kr("@run"), LabelStyle::Fips),
            "@run",
            "the own top box is not callable"
        );
        let call = ord.diagrams[&NodeId::a0()].boxes[2]
            .call
            .as_ref()
            .expect("call");
        assert_eq!(call_label(&scope, call, LabelStyle::Fips), "A2.1 | SEC/A2");
        // a callee renumbered: the label follows, the stored key stays
        let mut moved = ord.clone();
        let a0 = std::sync::Arc::make_mut(moved.diagrams.get_mut(&NodeId::a0()).expect("A0"));
        a0.boxes.swap(0, 1);
        let a2 = moved
            .diagrams
            .remove(&"A2".parse().expect("A2"))
            .expect("A2");
        moved.diagrams.insert("A1".parse().expect("A1"), a2);
        assert_eq!(
            target_label(&Scope::alone(&moved), &kr("@pick"), LabelStyle::Fips),
            "A1.1"
        );
    }

    // FR-CALL-04: Used At lists every caller on the diagram that contains the called box,
    // qualified when the caller is in another model.
    #[test]
    fn used_at_index() {
        let (ord, sec) = (model(ORD), model(SEC));
        let both = [&ord, &sec];
        let refs = |m: &Model, n: &str| -> Vec<String> {
            used_at_diagram(&both, m, &n.parse().expect("node"))
                .iter()
                .map(ToString::to_string)
                .collect()
        };
        assert_eq!(refs(&ord, "A2"), ["A3", "SEC/A1"]);
        assert_eq!(refs(&sec, "A0"), ["ORD/A3"]);
        assert!(refs(&ord, "A0").is_empty());
        let alone = [&ord];
        assert_eq!(
            used_at_diagram(&alone, &ord, &"A2".parse().expect("A2")).len(),
            1,
            "without SEC its callers are unknown"
        );
    }
}
