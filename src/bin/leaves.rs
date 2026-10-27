use std::path::{Path, PathBuf};
use std::process::ExitCode;

use lava_architectures::leaves::{TREE, render_workspace, tree_drift};

const HELP: &str = "\
lava-leaves — render and gate a workspace's leaf sets

USAGE:
    lava-leaves render <workspace> --architectures <dir>
    lava-leaves check  <workspace> --architectures <dir>

render writes <workspace>/leaves/<set>/ from <workspace>/leaves/declaration.json.
check renders in memory, proves the union of the leaf renders equals the whole
render, and fails on any file that differs from its render.
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut verb = None;
    let mut workspace = None;
    let mut architectures = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-h" | "--help" => {
                print!("{HELP}");
                return ExitCode::SUCCESS;
            }
            "--architectures" => architectures = it.next().map(PathBuf::from),
            v if verb.is_none() => verb = Some(v.to_string()),
            w if workspace.is_none() => workspace = Some(PathBuf::from(w)),
            other => {
                eprintln!("lava-leaves: unexpected argument {other:?}\n\n{HELP}");
                return ExitCode::from(2);
            }
        }
    }
    let (Some(verb), Some(workspace), Some(architectures)) = (verb, workspace, architectures)
    else {
        eprint!("{HELP}");
        return ExitCode::from(2);
    };
    match verb.as_str() {
        "render" => run(&workspace, &architectures, false),
        "check" => run(&workspace, &architectures, true),
        other => {
            eprintln!("lava-leaves: unknown verb {other:?}\n\n{HELP}");
            ExitCode::from(2)
        }
    }
}

fn run(workspace: &Path, architectures: &Path, check: bool) -> ExitCode {
    let outcome = match render_workspace(workspace, architectures, check) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("lava-leaves: {e}");
            return ExitCode::FAILURE;
        }
    };
    let mut failed = !outcome.refusals.is_empty();
    for r in &outcome.refusals {
        eprintln!("refused {r}");
    }
    if check {
        let drift = tree_drift(workspace, &outcome.files, &outcome.sets);
        failed |= !drift.is_empty();
        for d in &drift {
            eprintln!("stale {d}");
        }
    } else {
        for (set, _) in &outcome.sets {
            let dir = workspace.join(TREE).join(set);
            if dir.exists() {
                if let Err(e) = std::fs::remove_dir_all(&dir) {
                    eprintln!("lava-leaves: {}: {e}", dir.display());
                    return ExitCode::FAILURE;
                }
            }
        }
        for (path, body) in &outcome.files {
            let full = workspace.join(path);
            if let Some(parent) = full.parent() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    eprintln!("lava-leaves: {}: {e}", parent.display());
                    return ExitCode::FAILURE;
                }
            }
            if let Err(e) = std::fs::write(&full, body) {
                eprintln!("lava-leaves: {}: {e}", full.display());
                return ExitCode::FAILURE;
            }
        }
    }
    for (set, n) in &outcome.sets {
        let gate = if check {
            ", union equals the whole render"
        } else {
            ""
        };
        println!("{set}: {n} leaves{gate}");
    }
    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
