//! SVG snapshots of every committed fixture diagram (roadmap M4.5; layout §14.1).

use std::path::{Path, PathBuf};

use idef0_core::dsl::parse;
use idef0_core::model::Model;
use idef0_core::typecheck::typecheck;
use idef0_layout::metrics::Metrics;
use idef0_layout::scene::{LayoutInput, ThemeId, layout};
use idef0_layout::svg::{SvgOptions, render};

fn fixtures() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let mut out = Vec::new();
    for dir in ["valid", "dogfood", "bench", "reference", "layout/templates"] {
        let mut files: Vec<PathBuf> = std::fs::read_dir(root.join(dir))
            .expect("fixture dir")
            .map(|e| e.expect("entry").path())
            .filter(|p| p.extension().is_some_and(|x| x == "idef0"))
            .collect();
        files.sort();
        out.extend(files);
    }
    out
}

fn load(path: &Path) -> Model {
    let p = parse(&std::fs::read_to_string(path).expect("fixture"));
    assert!(
        p.diagnostics.is_empty(),
        "{}: {:?}",
        path.display(),
        p.diagnostics
    );
    let m = p.model.expect("model");
    assert!(typecheck(&m).is_empty(), "{}", path.display());
    m
}

fn svg(model: &Model, node: &idef0_core::id::NodeId, theme: ThemeId) -> String {
    let input = LayoutInput {
        theme,
        metrics: &Metrics::default(),
    };
    render(
        &layout(model, node, &input).expect("layout").scene,
        &SvgOptions::default(),
    )
}

// Roadmap M4.5 acceptance: an insta SVG snapshot of every diagram of every fixture, in the
// theme its profile selects.
#[test]
fn svg_snapshots() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let mut count = 0;
    for path in fixtures() {
        let model = load(&path);
        let theme = ThemeId::default_for(model.meta.profile);
        let rel = path.strip_prefix(&root).expect("under fixtures");
        let stem = rel
            .with_extension("")
            .to_string_lossy()
            .replace(['/', '\\'], "-");
        for node in model.diagrams.keys() {
            let name = format!("{stem}@{node}");
            insta::assert_snapshot!(name, svg(&model, node, theme));
            count += 1;
        }
    }
    assert!(count >= 20, "{count} diagrams");
}

// FR-STYLE-06/-07: the fips183 theme drops the shadow and labels corners with box numbers.
#[test]
fn fips183_theme_snapshot() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/dogfood/idef0-rs.idef0");
    let model = load(&path);
    let node = idef0_core::id::NodeId::context(0);
    let out = svg(&model, &node, ThemeId::Fips183);
    assert!(!out.contains("class=\"shadow\""));
    assert!(svg(&model, &node, ThemeId::Modern).contains("class=\"shadow\""));
    insta::assert_snapshot!("dogfood-fips183@A-0", out);
}
