//! End-to-end tests for the `lava-render` binary.
//!
//! These drive the actual compiled binary rather than calling the library
//! functions it wraps. The distinction matters here: the whole claim being
//! made is "the bundle is regenerable from a clean checkout", and the
//! thing a clean checkout runs is a process with an exit code and a stdout
//! — not a `pub fn`. A library-level test would prove the render works
//! while leaving every way the CLI can silently do nothing (exit 0 on a
//! missing param, write the error to stdout, emit an empty document)
//! completely unmeasured.
//!
//! Sibling of `dashboard_matrix.rs`, which owns the *content* of the
//! catalogue. This file owns the *entry point*.

use std::collections::BTreeMap;
use std::process::{Command, Output};

use lava_architectures::{DASHBOARD_DIR, required_dashboard_params, unbound_placeholders};

/// The binary under test, path supplied by cargo.
const BIN: &str = env!("CARGO_BIN_EXE_lava-render");

fn dashboards_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(DASHBOARD_DIR)
}

fn render(args: &[&str]) -> Output {
    Command::new(BIN)
        .args(args)
        // Pin the resolution directory rather than relying on the test
        // harness's cwd, which cargo does not promise.
        .arg("--dashboards-dir")
        .arg(dashboards_dir())
        .output()
        .expect("spawn lava-render")
}

/// Bind every placeholder the source declares, with a value distinctive
/// enough that a substitution that did not happen is visible.
///
/// Derived from the source, never hand-listed — the same reason
/// `dashboard_matrix.rs` derives its leak check from its row rather than
/// naming placeholders: a hand-list passes forever after the seventh param
/// lands.
fn params_for(src: &str) -> BTreeMap<String, String> {
    required_dashboard_params(src)
        .into_iter()
        .map(|k| {
            let v = format!("{k}-bound");
            (k, v)
        })
        .collect()
}

fn param_args(params: &BTreeMap<String, String>) -> Vec<String> {
    let mut v = Vec::new();
    for (k, val) in params {
        v.push("--param".to_string());
        v.push(format!("{k}={val}"));
    }
    v
}

fn bundled_names() -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dashboards_dir())
        .expect("read dashboards dir")
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

/// The headline test: a real bundled dashboard renders to parseable
/// Grafana JSON carrying no unbound placeholder.
#[test]
fn a_bundled_dashboard_renders_to_parseable_json_with_no_unbound_placeholders() {
    let src = std::fs::read_to_string(dashboards_dir().join("workload-overview.tlisp")).unwrap();
    let params = params_for(&src);
    let mut args = vec!["workload-overview".to_string()];
    args.extend(param_args(&params));
    let argv: Vec<&str> = args.iter().map(String::as_str).collect();

    let out = render(&argv);
    assert!(
        out.status.success(),
        "exit {:?}\nstderr: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );

    let text = String::from_utf8(out.stdout).expect("stdout is utf-8");
    let json: serde_json::Value =
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("stdout is not JSON: {e}\n{text}"));

    assert_eq!(json["schemaVersion"], 39);
    assert!(
        !json["uid"].as_str().unwrap_or_default().is_empty(),
        "rendered without a uid"
    );

    // The property the CLI exists to guarantee. `unbound_placeholders`
    // masks `{{label}}` legends first, so a correctly-preserved Grafana
    // legend is not miscounted as a leak.
    let leaked = unbound_placeholders(&text);
    assert!(
        leaked.is_empty(),
        "unbound placeholder(s) reached the output: {leaked:?}"
    );

    // And the positive half: binding actually happened. Without this the
    // check above passes just as well on an empty document.
    assert!(
        text.contains("service-bound"),
        "no bound param value appears in the output — the leak check above would be vacuous"
    );
}

/// Every bundled dashboard, not just the one above. Covers the directory
/// so a `.tlisp` added later cannot arrive unrendered.
#[test]
fn every_bundled_dashboard_renders_clean_through_the_cli() {
    let names = bundled_names();
    assert!(
        !names.is_empty(),
        "no .tlisp files in {} — this test would pass vacuously",
        dashboards_dir().display()
    );

    let mut failures: Vec<String> = Vec::new();
    for name in &names {
        let src = std::fs::read_to_string(dashboards_dir().join(format!("{name}.tlisp"))).unwrap();
        let params = params_for(&src);
        if params.is_empty() {
            failures.push(format!("{name}: source declares no params at all"));
            continue;
        }
        let mut args = vec![name.clone()];
        args.extend(param_args(&params));
        let argv: Vec<&str> = args.iter().map(String::as_str).collect();

        let out = render(&argv);
        if !out.status.success() {
            failures.push(format!(
                "{name}: exit {:?} — {}",
                out.status.code(),
                String::from_utf8_lossy(&out.stderr).trim()
            ));
            continue;
        }
        let text = String::from_utf8_lossy(&out.stdout).to_string();
        match serde_json::from_str::<serde_json::Value>(&text) {
            Ok(_) => {
                let leaked = unbound_placeholders(&text);
                if !leaked.is_empty() {
                    failures.push(format!("{name}: unbound placeholder(s) {leaked:?}"));
                }
            }
            Err(e) => failures.push(format!("{name}: stdout is not JSON: {e}")),
        }
    }

    assert!(
        failures.is_empty(),
        "{}/{} bundled dashboard(s) failed through the CLI:\n  {}",
        failures.len(),
        names.len(),
        failures.join("\n  ")
    );
}

/// 2-space indent and a trailing newline, so a regenerated bundle diffs
/// line-by-line instead of as one reflowed blob.
#[test]
fn output_is_pretty_printed_and_newline_terminated() {
    let src = std::fs::read_to_string(dashboards_dir().join("log-explorer.tlisp")).unwrap();
    let params = params_for(&src);
    let mut args = vec!["log-explorer".to_string()];
    args.extend(param_args(&params));
    let argv: Vec<&str> = args.iter().map(String::as_str).collect();

    let text = String::from_utf8(render(&argv).stdout).unwrap();
    assert!(text.ends_with("}\n"), "no trailing newline");
    assert!(
        text.contains("\n  \"uid\"") || text.contains("\n  \"title\""),
        "top-level keys are not indented by two spaces"
    );
    assert!(text.lines().count() > 20, "output looks un-pretty-printed");
}

/// Same inputs, same bytes. A regeneration that churns is a regeneration
/// nobody will run.
#[test]
fn rendering_is_deterministic() {
    let src = std::fs::read_to_string(dashboards_dir().join("audit-explorer.tlisp")).unwrap();
    let params = params_for(&src);
    let mut args = vec!["audit-explorer".to_string()];
    args.extend(param_args(&params));
    let argv: Vec<&str> = args.iter().map(String::as_str).collect();

    let a = render(&argv).stdout;
    let b = render(&argv).stdout;
    assert_eq!(a, b, "two renders of the same input differ");
}

/// `--out FILE` writes exactly what stdout would have carried.
#[test]
fn out_file_is_byte_identical_to_stdout() {
    let src = std::fs::read_to_string(dashboards_dir().join("homeostasis-control.tlisp")).unwrap();
    let params = params_for(&src);
    let mut args = vec!["homeostasis-control".to_string()];
    args.extend(param_args(&params));
    let argv: Vec<&str> = args.iter().map(String::as_str).collect();
    let via_stdout = render(&argv).stdout;

    let dir = std::env::temp_dir().join("lava-render-test-out");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("homeostasis-control.json");
    let mut with_out = argv.clone();
    let file_str = file.display().to_string();
    with_out.push("--out");
    with_out.push(&file_str);
    let out = render(&with_out);
    assert!(out.status.success(), "--out run failed");
    assert!(
        out.stdout.is_empty(),
        "--out still wrote the document to stdout"
    );

    let via_file = std::fs::read(&file).unwrap();
    assert_eq!(via_file, via_stdout, "--out and stdout disagree");
    let _ = std::fs::remove_file(&file);
}

// ── refusals ────────────────────────────────────────────────────────────
//
// Each of these is a way the CLI could plausibly have failed silently.
// A render CLI that exits 0 having emitted a broken board is worse than
// one that does not exist, because the broken board gets committed.

#[test]
fn an_unknown_dashboard_exits_non_zero_and_names_it() {
    let out = render(&["no-such-board", "--param", "env=x"]);
    assert!(!out.status.success(), "unknown dashboard exited 0");
    assert!(out.stdout.is_empty(), "wrote a document anyway");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("no-such-board"),
        "error does not name it: {err}"
    );
    // The available list is what turns "unknown" into "you meant this".
    assert!(
        err.contains("workload-overview"),
        "error does not list what IS available: {err}"
    );
}

#[test]
fn an_unreadable_path_exits_non_zero() {
    let out = render(&["/nonexistent/dir/board.tlisp", "--param", "env=x"]);
    assert!(!out.status.success(), "unreadable path exited 0");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("cannot read"), "unclear message: {err}");
}

#[test]
fn a_missing_param_exits_non_zero_and_names_the_param() {
    // workload-overview requires six; supply five.
    let out = render(&[
        "workload-overview",
        "--param",
        "env=camelot",
        "--param",
        "service=auth",
        "--param",
        "job=auth",
        "--param",
        "namespace=default",
        "--param",
        "datasource=mimir",
    ]);
    assert!(!out.status.success(), "missing param exited 0");
    assert!(out.stdout.is_empty(), "emitted a document anyway");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("logs_datasource"),
        "error does not name the missing param: {err}"
    );
}

#[test]
fn an_unused_param_exits_non_zero() {
    // A typo binds nothing and, without this, says nothing.
    let src = std::fs::read_to_string(dashboards_dir().join("workload-overview.tlisp")).unwrap();
    let params = params_for(&src);
    let mut args = vec!["workload-overview".to_string()];
    args.extend(param_args(&params));
    args.push("--param".to_string());
    args.push("namespcae=typo".to_string());
    let argv: Vec<&str> = args.iter().map(String::as_str).collect();

    let out = render(&argv);
    assert!(!out.status.success(), "an unused param exited 0");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("namespcae"), "error does not name it: {err}");
}

/// Both layers that can reject a source: the s-expression parser and the
/// dashboard evaluator above it. They are separate because they fail at
/// different times and a CLI that surfaced one but swallowed the other
/// would look correct in exactly half the tests you would think to write.
///
/// Neither fixture takes a param, so both reach the evaluator rather than
/// being stopped by the preflight — otherwise this would be testing the
/// preflight twice under another name.
#[test]
fn a_parse_or_eval_failure_exits_non_zero() {
    let dir = std::env::temp_dir().join("lava-render-test-bad");
    std::fs::create_dir_all(&dir).unwrap();

    let cases: [(&str, &str); 2] = [
        (
            "unknown-kind",
            // Parses fine; the evaluator rejects the panel kind. NOTE a
            // dashboard with no rows at all renders happily — an empty
            // board is a valid document — so "broken" has to mean broken.
            "(deflava-dashboard broken :uid \"u\" :title \"t\"\n  \
             :rows ((:title \"r\" :panels ((:id \"p\" :kind \"no-such-kind\" :title \"t\")))))",
        ),
        (
            "unclosed-list",
            "(deflava-dashboard broken :uid \"u\" :title \"t\"",
        ),
    ];

    for (label, src) in cases {
        let bad = dir.join(format!("{label}.tlisp"));
        std::fs::write(&bad, src).unwrap();
        let out = Command::new(BIN).arg(&bad).output().expect("spawn");
        assert!(
            !out.status.success(),
            "{label}: exited 0 on a broken source"
        );
        assert!(out.stdout.is_empty(), "{label}: emitted a document anyway");
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(
            err.starts_with("lava-render:"),
            "{label}: unprefixed error: {err}"
        );
        assert!(
            err.contains("evaluation failed"),
            "{label}: message does not say what went wrong: {err}"
        );
        let _ = std::fs::remove_file(&bad);
    }
}

#[test]
fn no_arguments_exits_non_zero_with_usage() {
    let out = Command::new(BIN).output().expect("spawn");
    assert!(!out.status.success(), "no args exited 0");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("USAGE"), "no usage on stderr: {err}");
}

#[test]
fn help_and_list_exit_zero() {
    let h = Command::new(BIN).arg("--help").output().expect("spawn");
    assert!(h.status.success());
    assert!(String::from_utf8_lossy(&h.stdout).contains("USAGE"));

    let l = render(&["--list"]);
    assert!(l.status.success());
    let listed = String::from_utf8_lossy(&l.stdout);
    for n in bundled_names() {
        assert!(listed.contains(&n), "--list omitted {n}");
    }
}
