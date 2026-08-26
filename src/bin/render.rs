//! `lava-render` — render a bundled `.tlisp` dashboard to Grafana JSON
//! without a running pangea-operator.
//!
//! ## Why this exists
//!
//! Until this binary, the only thing that could turn a
//! `(deflava-dashboard …)` source into Grafana JSON was the operator's
//! embedded Ruby. That made the shipped bundle **non-regenerable from a
//! clean checkout**: you could read the `.tlisp`, you could read the JSON,
//! and you could not derive the second from the first without standing up
//! a cluster. A delivery whose reproducibility argument depends on a
//! running controller does not have a reproducibility argument.
//!
//! The rendering API was already public — `tests/dashboard_matrix.rs` has
//! been calling `lava_eval::render_dashboard_grafana_json` since the
//! catalogue landed. What was missing was an entry point.
//!
//! ## Argument parsing: hand-rolled, deliberately
//!
//! This crate depends on clap NOWHERE, and adding it is not free here.
//! `Cargo.nix` / `Cargo.build-spec.json` / `Cargo.gen.lock` are
//! crate2nix-generated and gated on staleness, and the current dependency
//! closure is 10 crates of which every one is a `lava-*` or serde. clap
//! pulls roughly a dozen more (anstream, anstyle, colorchoice, strsim, …)
//! into a crate whose entire CLI surface is one positional, one repeated
//! flag and two options. That is a poor trade for `--help` formatting, so
//! the parse below is ~40 lines of match and the closure is unchanged.
//!
//! Revisit if this binary grows subcommands — at that point clap earns it.
//!
//! ## What it refuses to do
//!
//! Emit a document with an unbound `{placeholder}` in it. That failure is
//! silent by nature: the board renders, validates, uploads, and shows a
//! literal `{service}` in its title to whoever opens it. Both halves are
//! checked — the required set is derived from the source before rendering
//! (so the error names what you forgot, not what the evaluator tripped
//! over), and the rendered output is re-scanned after (so a placeholder
//! introduced by the renderer itself cannot slip through).

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use lava_architectures::{
    DASHBOARD_DIR, bind_dashboard_params, required_dashboard_params, unbound_placeholders,
};
use lava_core::Theme;

const HELP: &str = "\
lava-render — render a lava dashboard .tlisp to Grafana JSON

USAGE:
    lava-render <dashboard> [--param k=v ...] [--out FILE]

ARGS:
    <dashboard>    A bundled dashboard name (resolved against the
                   dashboards directory), or a path to a .tlisp file.
                   An argument containing a path separator or ending in
                   .tlisp is used as-is.

OPTIONS:
    -p, --param <k=v>        Bind a {k} placeholder. Repeatable.
    -o, --out <FILE>         Write to FILE instead of stdout.
        --dashboards-dir <D> Where to resolve a bare name.
                             Default: $LAVA_DASHBOARDS_DIR, else ./dashboards.
    -l, --list               List resolvable dashboard names and exit.
    -h, --help               Print this help and exit.
    -V, --version            Print version and exit.

EXAMPLE:
    lava-render workload-overview \\
      --param env=camelot --param service=auth --param job=auth \\
      --param namespace=default \\
      --param datasource=mimir --param logs_datasource=vlogs
";

/// Every way this binary declines to produce a document.
///
/// Typed rather than a `String`: each variant is a distinct thing the
/// caller did or a distinct thing the source is missing, and the operator
/// reading stderr at 3am should not have to tell them apart by wording.
#[derive(Debug, thiserror::Error)]
enum CliError {
    #[error("{0}\n\n{HELP}")]
    Usage(&'static str),

    #[error("--param {0:?} is not k=v")]
    MalformedParam(String),

    #[error("no dashboard given\n\n{HELP}")]
    NoDashboard,

    #[error(
        "unknown dashboard {name:?}\n  looked in: {dir}\n  available: {available}\n\
         (pass a path ending in .tlisp to render a file outside that directory)"
    )]
    UnknownDashboard {
        name: String,
        dir: String,
        available: String,
    },

    #[error("cannot read {path}: {source}")]
    Unreadable {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error(
        "missing --param for {missing:?}\n  {name} requires: {required}\n\
         every {{placeholder}} in the source must be bound, or the rendered \
         board ships a literal brace"
    )]
    MissingParams {
        name: String,
        missing: String,
        required: String,
    },

    #[error(
        "--param {unknown:?} is not used by {name}\n  it accepts: {required}\n\
         an unused param is a typo that would otherwise bind nothing and say nothing"
    )]
    UnknownParams {
        name: String,
        unknown: String,
        required: String,
    },

    #[error("{name}: evaluation failed: {source}")]
    Eval {
        name: String,
        #[source]
        source: lava_eval::EvalError,
    },

    #[error(
        "{name}: unbound placeholder(s) reached the rendered output: {leaked}\n\
         this board would ship a literal brace to whoever opens it"
    )]
    Leaked { name: String, leaked: String },

    #[error("cannot write {path}: {source}")]
    Unwritable {
        path: String,
        #[source]
        source: std::io::Error,
    },
}

/// Render the JSON to a byte string that is stable across runs and
/// pleasant in a diff.
///
/// Two-space indent (serde_json's pretty default) plus a trailing
/// newline. The newline is not decoration: without it every regeneration
/// shows up in `git diff` as a `\ No newline at end of file` marker on the
/// last line, which makes a one-panel change look like a two-line change.
fn to_stable_json(v: &serde_json::Value) -> Result<Vec<u8>, serde_json::Error> {
    let mut buf = serde_json::to_vec_pretty(v)?;
    buf.push(b'\n');
    Ok(buf)
}

/// Comma-join a set for an error message. Present so error construction
/// stays free of ad-hoc string building at each call site.
fn joined<'a>(items: impl IntoIterator<Item = &'a String>) -> String {
    let mut out = String::new();
    for i in items {
        if !out.is_empty() {
            out.push_str(", ");
        }
        out.push_str(i);
    }
    out
}

struct Args {
    dashboard: Option<String>,
    params: BTreeMap<String, String>,
    out: Option<PathBuf>,
    dashboards_dir: Option<PathBuf>,
    list: bool,
    help: bool,
    version: bool,
}

fn parse_args(argv: impl Iterator<Item = String>) -> Result<Args, CliError> {
    let mut a = Args {
        dashboard: None,
        params: BTreeMap::new(),
        out: None,
        dashboards_dir: None,
        list: false,
        help: false,
        version: false,
    };
    let mut it = argv.peekable();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "-h" | "--help" => a.help = true,
            "-V" | "--version" => a.version = true,
            "-l" | "--list" => a.list = true,
            "-p" | "--param" => {
                let kv = it.next().ok_or(CliError::Usage("--param needs k=v"))?;
                // split_once, not split('='): a value may legitimately
                // contain '=' (a PromQL matcher, a base64 tail).
                let (k, v) = kv
                    .split_once('=')
                    .ok_or(CliError::MalformedParam(kv.clone()))?;
                a.params.insert(k.to_string(), v.to_string());
            }
            "-o" | "--out" => {
                a.out = Some(PathBuf::from(
                    it.next().ok_or(CliError::Usage("--out needs a path"))?,
                ));
            }
            "--dashboards-dir" => {
                a.dashboards_dir = Some(PathBuf::from(
                    it.next()
                        .ok_or(CliError::Usage("--dashboards-dir needs a path"))?,
                ));
            }
            other if other.starts_with('-') && other.len() > 1 => {
                return Err(CliError::Usage("unknown option"));
            }
            other => {
                if a.dashboard.is_some() {
                    return Err(CliError::Usage("more than one dashboard given"));
                }
                a.dashboard = Some(other.to_string());
            }
        }
    }
    Ok(a)
}

/// Where a bare dashboard name is resolved.
///
/// CWD-relative `dashboards/` is the primary path because the whole point
/// of this binary is regenerating the bundle from a clean checkout, and a
/// clean checkout is what you are standing in when you do that.
///
/// `CARGO_MANIFEST_DIR` is the last resort and is a *dev-checkout*
/// convenience only — it is baked in at compile time, so in a binary
/// installed from the Nix store it points at a build sandbox that no
/// longer exists. It is listed among the tried paths rather than hidden,
/// so a confusing miss reads as a confusing miss.
fn dashboards_dir(explicit: Option<PathBuf>) -> PathBuf {
    if let Some(d) = explicit {
        return d;
    }
    if let Some(d) = std::env::var_os("LAVA_DASHBOARDS_DIR") {
        return PathBuf::from(d);
    }
    let cwd = PathBuf::from(DASHBOARD_DIR);
    if cwd.is_dir() {
        return cwd;
    }
    Path::new(env!("CARGO_MANIFEST_DIR")).join(DASHBOARD_DIR)
}

fn available_in(dir: &Path) -> Vec<String> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut names: Vec<String> = rd
        .filter_map(Result::ok)
        .filter_map(|e| {
            let p = e.path();
            (p.extension()?.to_str()? == "tlisp")
                .then(|| p.file_stem()?.to_str().map(ToString::to_string))?
        })
        .collect();
    names.sort();
    names
}

/// A bare name resolves against the dashboards directory; anything
/// carrying a path separator or a `.tlisp` suffix is a path.
fn looks_like_a_path(arg: &str) -> bool {
    arg.ends_with(".tlisp") || arg.contains(std::path::MAIN_SEPARATOR) || arg.contains('/')
}

fn resolve(arg: &str, dir: &Path) -> Result<(String, PathBuf), CliError> {
    if looks_like_a_path(arg) {
        let p = PathBuf::from(arg);
        let name = p
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(arg)
            .to_string();
        return Ok((name, p));
    }
    let p = dir.join([arg, ".tlisp"].concat());
    if p.is_file() {
        return Ok((arg.to_string(), p));
    }
    let avail = available_in(dir);
    Err(CliError::UnknownDashboard {
        name: arg.to_string(),
        dir: dir.display().to_string(),
        available: if avail.is_empty() {
            "(none found)".to_string()
        } else {
            joined(&avail)
        },
    })
}

fn run(a: Args) -> Result<Vec<u8>, CliError> {
    let dir = dashboards_dir(a.dashboards_dir);

    if a.list {
        let mut out = String::new();
        for n in available_in(&dir) {
            out.push_str(&n);
            out.push('\n');
        }
        return Ok(out.into_bytes());
    }

    let arg = a.dashboard.ok_or(CliError::NoDashboard)?;
    let (name, path) = resolve(&arg, &dir)?;

    let raw = std::fs::read_to_string(&path).map_err(|source| CliError::Unreadable {
        path: path.display().to_string(),
        source,
    })?;

    // Preflight. Doing this BEFORE the evaluator runs is the difference
    // between "missing --param for [\"job\"]" and the evaluator's own
    // complaint about a malformed selector three layers down — same root
    // cause, wildly different time-to-fix.
    let required = required_dashboard_params(&raw);
    let supplied: BTreeSet<String> = a.params.keys().cloned().collect();
    let missing: Vec<&String> = required.difference(&supplied).collect();
    if !missing.is_empty() {
        return Err(CliError::MissingParams {
            name,
            missing: joined(missing.into_iter()),
            required: joined(&required),
        });
    }
    let unknown: Vec<&String> = supplied.difference(&required).collect();
    if !unknown.is_empty() {
        return Err(CliError::UnknownParams {
            name,
            unknown: joined(unknown.into_iter()),
            required: joined(&required),
        });
    }

    let bound = bind_dashboard_params(&raw, &a.params);
    // Theme::default() is tundra, and tundra is the only theme lava-core
    // ships. A --theme flag over a one-element set would read as choice
    // where there is none; add it when a second theme exists.
    let json =
        lava_eval::render_dashboard_grafana_json(&bound, &Theme::default()).map_err(|source| {
            CliError::Eval {
                name: name.clone(),
                source,
            }
        })?;

    let bytes = to_stable_json(&json).map_err(|e| CliError::Eval {
        name: name.clone(),
        source: lava_eval::EvalError::Dashboard(e.to_string()),
    })?;

    // Scan the bytes that will actually be written, not the Value. A leak
    // is only a leak if it reaches the file.
    let text = String::from_utf8_lossy(&bytes);
    let leaked = unbound_placeholders(&text);
    if !leaked.is_empty() {
        return Err(CliError::Leaked {
            name,
            leaked: joined(&leaked),
        });
    }

    Ok(bytes)
}

fn main() -> ExitCode {
    let argv = std::env::args().skip(1);
    let a = match parse_args(argv) {
        Ok(a) => a,
        Err(e) => return fail(&e),
    };

    if a.help {
        print!("{HELP}");
        return ExitCode::SUCCESS;
    }
    if a.version {
        println!("lava-render {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }

    let out = a.out.clone();
    let bytes = match run(a) {
        Ok(b) => b,
        Err(e) => return fail(&e),
    };

    let write_result = match &out {
        Some(p) => std::fs::write(p, &bytes).map_err(|source| CliError::Unwritable {
            path: p.display().to_string(),
            source,
        }),
        None => std::io::stdout()
            .write_all(&bytes)
            .map_err(|source| CliError::Unwritable {
                path: "<stdout>".to_string(),
                source,
            }),
    };
    match write_result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => fail(&e),
    }
}

/// One place that turns an error into a non-zero exit, so no path can
/// report a failure on stdout or exit 0 having produced nothing.
fn fail(e: &CliError) -> ExitCode {
    let mut err = std::io::stderr();
    let _ = writeln!(err, "lava-render: {e}");
    ExitCode::FAILURE
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(v: &[&str]) -> Result<Args, CliError> {
        parse_args(v.iter().map(ToString::to_string))
    }

    #[test]
    fn params_split_on_the_first_equals_only() {
        let a = args(&["d", "--param", "expr=up{job=\"auth\"}"]).unwrap();
        assert_eq!(a.params.get("expr").unwrap(), "up{job=\"auth\"}");
    }

    #[test]
    fn a_param_without_an_equals_is_rejected() {
        assert!(matches!(
            args(&["d", "--param", "nope"]),
            Err(CliError::MalformedParam(_))
        ));
    }

    #[test]
    fn a_tlisp_suffix_or_a_separator_means_a_path() {
        assert!(looks_like_a_path("board.tlisp"));
        assert!(looks_like_a_path("./x/board.tlisp"));
        assert!(looks_like_a_path("some/dir/board"));
        assert!(!looks_like_a_path("workload-overview"));
    }

    #[test]
    fn stable_json_is_two_space_indented_and_newline_terminated() {
        let v = serde_json::json!({"a": {"b": 1}});
        let s = String::from_utf8(to_stable_json(&v).unwrap()).unwrap();
        assert!(s.ends_with("}\n"), "no trailing newline: {s:?}");
        assert!(s.contains("\n  \"a\""), "not 2-space indented: {s:?}");
        assert!(s.contains("\n    \"b\""), "not 2-space indented: {s:?}");
    }
}
