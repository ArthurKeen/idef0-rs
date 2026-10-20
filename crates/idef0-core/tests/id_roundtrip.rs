//! Property tests for node and port references (roadmap M1.1; FR-ID-07, FR-ID-08;
//! `docs/spec/metamodel.md` §3.6). Case counts follow `PROPTEST_CASES` (default 256, the
//! PR depth in `docs/testing.md`); CI fixes the seed with `PROPTEST_RNG_SEED`.

use idef0_core::id::{
    ModelAbbrev, NodeId, NodeRef, PortCode, PortRef, SheetKind, SheetSuffix, Side,
};
use proptest::prelude::*;

fn node_id() -> impl Strategy<Value = NodeId> {
    prop_oneof![
        any::<u8>().prop_map(NodeId::context),
        proptest::collection::vec(1u8..=7, 0..8)
            .prop_map(|boxes| NodeId::from_boxes(&boxes).expect("digits are 1-7")),
    ]
}

fn box_node() -> impl Strategy<Value = NodeId> {
    proptest::collection::vec(1u8..=7, 0..8)
        .prop_map(|boxes| NodeId::from_boxes(&boxes).expect("digits are 1-7"))
}

fn model() -> impl Strategy<Value = Option<ModelAbbrev>> {
    proptest::option::of("[A-Z][A-Z0-9]{0,7}".prop_map(|s| s.parse().expect("matches FR-ID-01")))
}

fn sheet() -> impl Strategy<Value = Option<SheetSuffix>> {
    let kind = prop_oneof![
        Just(SheetKind::Feo),
        Just(SheetKind::Text),
        Just(SheetKind::Glossary)
    ];
    proptest::option::of(
        (kind, proptest::option::of(1u32..=u32::MAX))
            .prop_map(|(k, page)| SheetSuffix::new(k, page).expect("page is not 0")),
    )
}

fn side() -> impl Strategy<Value = Side> {
    prop_oneof![Just(Side::I), Just(Side::C), Just(Side::O), Just(Side::M)]
}

fn port_ref() -> impl Strategy<Value = PortRef> {
    let code = prop_oneof![
        (1u8..=u8::MAX).prop_map(PortCode::Icom),
        (1u8..=u8::MAX).prop_map(PortCode::Border)
    ];
    (model(), node_id(), side(), code)
        .prop_map(|(m, n, s, c)| PortRef::new(m, n, s, c).expect("rank is not 0"))
}

proptest! {
    // Roadmap M1.1 acceptance: parse(display(x)) == x.
    #[test]
    fn node_id_roundtrip(n in node_id()) {
        prop_assert_eq!(n.to_string().parse::<NodeId>(), Ok(n.clone()));
        prop_assert_eq!(n.dotted().to_string().parse::<NodeId>(), Ok(n));
    }

    #[test]
    fn node_ref_roundtrip(m in model(), n in node_id(), s in sheet()) {
        let r = NodeRef::new(m, n, s);
        prop_assert_eq!(r.to_string().parse::<NodeRef>(), Ok(r));
    }

    // FR-ID-07: the canonical slash form and the FIPS form both round-trip.
    #[test]
    fn port_ref_roundtrip(p in port_ref()) {
        prop_assert_eq!(p.to_string().parse::<PortRef>(), Ok(p.clone()));
        prop_assert_eq!(p.fips().to_string().parse::<PortRef>(), Ok(p));
    }

    // FR-ID-08, metamodel §3.6: `X.k` is `Xk`, and `A0.k` is `Ak`.
    #[test]
    fn dotted_child_equals_child(x in box_node(), k in 1u8..=7) {
        let dotted = format!("{x}.{k}");
        prop_assert_eq!(dotted.parse::<NodeId>().ok(), x.child(k));
    }

    // FR-ID-08: a fully dotted path (`A2.1.3`) normalizes left to right to the same node.
    #[test]
    fn fully_dotted_path(boxes in proptest::collection::vec(1u8..=7, 1..8)) {
        let mut text = String::from("A0");
        for k in &boxes {
            text.push('.');
            text.push(char::from(b'0' + k));
        }
        prop_assert_eq!(text.parse::<NodeId>().ok(), NodeId::from_boxes(&boxes));
    }

    // Parsing never panics, whatever the input.
    #[test]
    fn parse_total(s in "\\PC{0,24}") {
        let _ = s.parse::<NodeId>();
        let _ = s.parse::<NodeRef>();
        let _ = s.parse::<PortRef>();
    }
}
