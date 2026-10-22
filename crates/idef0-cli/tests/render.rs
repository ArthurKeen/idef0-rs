//! `idef0 render` end to end (roadmap M4.5; cli §3, §4, §6).

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

fn idef0(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_idef0"))
        .args(args)
        .output()
        .expect("run idef0")
}

fn code(o: &Output) -> i32 {
    o.status.code().expect("exit code")
}

fn dogfood() -> String {
    fixtures()
        .join("dogfood/idef0-rs.idef0")
        .display()
        .to_string()
}

// The CLI writes exactly the library's SVG, and two separate runs give the same bytes (D6).
#[test]
fn svg_to_stdout_matches_the_library() {
    let a = idef0(&["render", &dogfood(), "A0", "--svg"]);
    assert_eq!(code(&a), 0, "{}", String::from_utf8_lossy(&a.stderr));
    let b = idef0(&["render", &dogfood(), "A0"]);
    assert_eq!(
        a.stdout, b.stdout,
        "--svg is the default and output is deterministic"
    );
    let src = std::fs::read_to_string(dogfood()).expect("dogfood");
    let model = idef0_core::dsl::parse(&src).model.expect("model");
    let input = idef0_layout::scene::LayoutInput {
        theme: idef0_layout::scene::ThemeId::Modern,
        metrics: &idef0_layout::metrics::Metrics::default(),
    };
    let scene = idef0_layout::scene::layout(&model, &idef0_core::id::NodeId::a0(), &input)
        .expect("layout")
        .scene;
    let want = idef0_layout::svg::render(&scene, &idef0_layout::svg::SvgOptions::default());
    assert_eq!(String::from_utf8(a.stdout).expect("utf-8"), want);
    let by_key = idef0(&["render", &dogfood(), "@model-system"]);
    assert_eq!(
        code(&by_key),
        0,
        "{}",
        String::from_utf8_lossy(&by_key.stderr)
    );
    assert_eq!(
        by_key.stdout, b.stdout,
        "a box key names the diagram that details it"
    );
}

// Without NODE, every diagram goes into --out DIR, one file per node.
#[test]
fn every_diagram_into_a_directory() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("render-all");
    let _ = std::fs::remove_dir_all(&dir);
    let model = fixtures()
        .join("valid/constructs.idef0")
        .display()
        .to_string();
    let o = idef0(&["render", &model, "--out", &dir.display().to_string()]);
    assert_eq!(code(&o), 0, "{}", String::from_utf8_lossy(&o.stderr));
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .expect("out dir")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, ["A-0.svg", "A-1.svg", "A0.svg", "A3.svg"]);
    let none = idef0(&["render", &model]);
    assert_eq!(code(&none), 2, "every diagram needs --out");
}

// cli §4 exit codes.
#[test]
fn exit_codes() {
    let f = |p: &str| fixtures().join(p).display().to_string();
    let cases: [(Vec<String>, i32); 9] = [
        (vec![f("invalid/E-PARSE-01/recovery.idef0"), "A0".into()], 2),
        (
            vec![f("invalid/E-CALL-01/missing-box.idef0"), "A0".into()],
            1,
        ),
        (vec![f("nope.idef0"), "A0".into()], 3),
        (vec!["A0".into()], 3),
        (vec![dogfood(), "A9".into()], 2),
        (vec![dogfood(), "A7".into()], 2),
        (vec![dogfood(), "A0".into(), "--txt".into()], 2),
        (
            vec![dogfood(), "A0".into(), "--theme".into(), "classic".into()],
            2,
        ),
        (
            vec![dogfood(), "A0".into(), "--text".into(), "embed".into()],
            2,
        ),
    ];
    for (args, want) in cases {
        let mut all = vec!["render"];
        all.extend(args.iter().map(String::as_str));
        let o = idef0(&all);
        assert_eq!(
            code(&o),
            want,
            "{all:?}: {}",
            String::from_utf8_lossy(&o.stderr)
        );
        assert!(o.stdout.is_empty(), "{all:?}: nothing on stdout");
    }
}

// --theme fips183 draws no shadows.
#[test]
fn theme_flag() {
    let o = idef0(&["render", &dogfood(), "A-0", "--theme", "fips183"]);
    assert_eq!(code(&o), 0);
    let svg = String::from_utf8(o.stdout).expect("utf-8");
    assert!(svg.contains("data-theme=\"fips183\"") && !svg.contains("class=\"shadow\""));
}
