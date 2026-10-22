//! The `idef0` command-line tool. Commands are specified in `docs/cli.md`.
//!
//! Implemented so far: `render --svg` (roadmap M4.5) on a model file given by path (model
//! resolution rule 1; workspace resolution arrives with M2.4).

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand, ValueEnum};
use idef0_core::diag::{Diagnostic, Severity};
use idef0_core::id::{NodeId, NodeRef};
use idef0_core::model::Model;
use idef0_layout::metrics::Metrics;
use idef0_layout::scene::{LayoutInput, ThemeId, layout};
use idef0_layout::svg::{SvgOptions, TextMode};

/// Exit codes (cli §4).
mod exit {
    pub(crate) const RULE: u8 = 1;
    pub(crate) const USAGE: u8 = 2;
    pub(crate) const IO: u8 = 3;
    pub(crate) const INTERNAL: u8 = 4;
}

#[derive(Parser)]
#[command(name = "idef0", version, about = "IDEF0 (FIPS PUB 183) modeling tool")]
struct Cli {
    /// Lowest severity that makes the command exit 1.
    #[arg(long, global = true, value_enum, default_value_t = FailOn::Error)]
    fail_on: FailOn,
    #[command(subcommand)]
    command: Command,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum FailOn {
    Info,
    Warning,
    Error,
    Never,
}

impl FailOn {
    fn fails(self, s: Severity) -> bool {
        let rank = |s: Severity| match s {
            Severity::Info => FailOn::Info,
            Severity::Warning => FailOn::Warning,
            Severity::Error => FailOn::Error,
        };
        self != Self::Never && rank(s) >= self
    }
}

#[derive(Subcommand)]
enum Command {
    /// Render diagrams (cli §6).
    Render(RenderArgs),
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Text {
    Embed,
    Paths,
    Ref,
}

#[derive(Args)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "one field per cli §5 flag: --svg, --txt, --png, --publication, --check"
)]
struct RenderArgs {
    /// `[MODEL] [NODE]`: a model file path and a node (`A0`, `A21`, `A-0`, `@key`).
    #[arg(num_args = 0..=2)]
    targets: Vec<String>,
    /// SVG output (the default).
    #[arg(long, group = "format")]
    svg: bool,
    /// Text output (M5.6).
    #[arg(long, group = "format")]
    txt: bool,
    /// PNG output (M5.9).
    #[arg(long, group = "format")]
    png: bool,
    /// Theme: modern, classic, fips183, retro (default: by the model's profile).
    #[arg(long)]
    theme: Option<String>,
    /// SVG text mode. Only `ref` is available until the bundled fonts arrive (M5.5).
    #[arg(long, value_enum, default_value_t = Text::Ref)]
    text: Text,
    /// Publication export (M5.9).
    #[arg(long)]
    publication: bool,
    /// Output file (one node) or directory (every diagram).
    #[arg(long)]
    out: Option<PathBuf>,
    /// Run the geometry checker (M4.6).
    #[arg(long)]
    check: bool,
}

/// A failed run: its exit code and what to tell the user.
struct Fail(u8, String);

fn fail<T>(code: u8, msg: impl Into<String>) -> Result<T, Fail> {
    Err(Fail(code, msg.into()))
}

/// A token is a path if it ends in `.idef0` or contains a path separator and is not a
/// model-qualified node (cli §3, rule 1).
fn is_path(t: &str) -> bool {
    Path::new(t)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("idef0"))
        || ((t.contains('/') || t.contains('\\')) && t.parse::<NodeRef>().is_err())
}

/// The node a `NODE` token names in `model`.
fn resolve_node(model: &Model, token: &str) -> Result<NodeId, Fail> {
    if let Some(key) = token.strip_prefix('@') {
        for (node, d) in &model.diagrams {
            if let Some(i) = d.boxes.iter().position(|b| b.key.as_str() == key) {
                let detail = match node {
                    NodeId::Context { level: 0 } => Some(NodeId::a0()),
                    NodeId::Context { .. } => None,
                    NodeId::Node(_) => u8::try_from(i + 1).ok().and_then(|k| node.child(k)),
                };
                if let Some(n) = detail.filter(|n| model.diagrams.contains_key(n)) {
                    return Ok(n);
                }
                return fail(exit::USAGE, format!("box @{key} has no child diagram"));
            }
        }
        return fail(
            exit::USAGE,
            format!("no box @{key} in model {}", model.meta.abbrev),
        );
    }
    let r: NodeRef = token
        .parse()
        .map_err(|e| Fail(exit::USAGE, format!("invalid NODE `{token}`: {e}")))?;
    if let Some(m) = r.model()
        && *m != model.meta.abbrev
    {
        return fail(
            exit::USAGE,
            format!(
                "{token} names model {m}, but the file is model {}",
                model.meta.abbrev
            ),
        );
    }
    let node = r.node().clone();
    if !model.diagrams.contains_key(&node) {
        return fail(
            exit::USAGE,
            format!("model {} has no diagram {node}", model.meta.abbrev),
        );
    }
    Ok(node)
}

fn report(diags: &[Diagnostic]) {
    let mut err = std::io::stderr().lock();
    for d in diags {
        let _ = writeln!(err, "{d}");
    }
}

/// Parse and type-check `path`. Syntax errors exit 2 (nothing else can run); other errors
/// are reported and exit 1, since a model with type errors cannot be laid out.
fn load(path: &Path, fail_on: FailOn) -> Result<(Model, bool), Fail> {
    let src = std::fs::read_to_string(path)
        .map_err(|e| Fail(exit::IO, format!("{}: {e}", path.display())))?;
    let parsed = idef0_core::dsl::parse(&src);
    let syntax = parsed.diagnostics.iter().any(|d| {
        ["E-PARSE", "E-VERSION", "E-LIMIT"]
            .iter()
            .any(|p| d.code.as_str().starts_with(p))
    });
    let Some(model) = parsed.model.filter(|_| !syntax) else {
        report(&parsed.diagnostics);
        return fail(exit::USAGE, format!("{}: syntax errors", path.display()));
    };
    let mut diags = parsed.diagnostics;
    diags.extend(idef0_core::typecheck::typecheck(&model));
    report(&diags);
    if diags.iter().any(|d| d.severity == Severity::Error) {
        return fail(
            exit::RULE,
            format!("{}: the model has errors", path.display()),
        );
    }
    Ok((model, diags.iter().any(|d| fail_on.fails(d.severity))))
}

fn render(args: &RenderArgs, fail_on: FailOn) -> Result<u8, Fail> {
    if args.txt {
        return fail(exit::USAGE, "--txt arrives with the ASCII renderer (M5.6)");
    }
    if args.png {
        return fail(exit::USAGE, "--png arrives with M5.9");
    }
    if args.publication {
        return fail(exit::USAGE, "--publication arrives with M5.9");
    }
    if args.check {
        return fail(
            exit::USAGE,
            "--check arrives with the geometry checker (M4.6)",
        );
    }
    if args.text != Text::Ref {
        return fail(
            exit::USAGE,
            "--text embed and paths need the bundled fonts (M5.5); use --text ref",
        );
    }
    let (path, node) = match args.targets.as_slice() {
        [m] | [m, _] if is_path(m) => (PathBuf::from(m), args.targets.get(1)),
        [] | [_] | [_, _] => {
            return fail(
                exit::IO,
                "no model file given: pass a path to a .idef0 file (workspace models arrive with M2.4)",
            );
        }
        _ => return fail(exit::USAGE, "render takes at most [MODEL] [NODE]"),
    };
    let (model, flagged) = load(&path, fail_on)?;
    let theme = match &args.theme {
        Some(t) => t.parse::<ThemeId>().map_err(|e| Fail(exit::USAGE, e))?,
        None => ThemeId::default_for(model.meta.profile),
    };
    let metrics = Metrics::default();
    let input = LayoutInput {
        theme,
        metrics: &metrics,
    };
    let opts = SvgOptions {
        text: TextMode::Ref,
    };
    let mut fails = flagged;
    let mut svg_of = |node: &NodeId| -> Result<String, Fail> {
        let r = layout(&model, node, &input).map_err(|e| Fail(exit::USAGE, e.to_string()))?;
        report(&r.diagnostics);
        fails |= r.diagnostics.iter().any(|d| fail_on.fails(d.severity));
        Ok(idef0_layout::svg::render(&r.scene, &opts))
    };
    let write = |to: &Path, text: &str| {
        std::fs::write(to, text).map_err(|e| Fail(exit::IO, format!("{}: {e}", to.display())))
    };
    if let Some(token) = node {
        let node = resolve_node(&model, token)?;
        let out = svg_of(&node)?;
        if let Some(to) = &args.out {
            write(to, &out)?;
        } else {
            let mut stdout = std::io::stdout().lock();
            stdout
                .write_all(out.as_bytes())
                .map_err(|e| Fail(exit::IO, format!("stdout: {e}")))?;
        }
    } else {
        let Some(dir) = &args.out else {
            return fail(exit::USAGE, "rendering every diagram needs --out DIR");
        };
        std::fs::create_dir_all(dir)
            .map_err(|e| Fail(exit::IO, format!("{}: {e}", dir.display())))?;
        let nodes: Vec<NodeId> = model.diagrams.keys().cloned().collect();
        for node in &nodes {
            let out = svg_of(node)?;
            write(&dir.join(format!("{node}.svg")), &out)?;
        }
    }
    Ok(if fails { exit::RULE } else { 0 })
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let run = std::panic::catch_unwind(|| match &cli.command {
        Command::Render(args) => render(args, cli.fail_on),
    });
    match run {
        Ok(Ok(code)) => ExitCode::from(code),
        Ok(Err(Fail(code, msg))) => {
            eprintln!("idef0: {msg}");
            ExitCode::from(code)
        }
        Err(_) => {
            eprintln!("idef0: internal error; please report it as a bug");
            ExitCode::from(exit::INTERNAL)
        }
    }
}
