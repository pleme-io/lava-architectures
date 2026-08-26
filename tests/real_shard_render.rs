//! PROOF: lava renders the REAL pleme-io-opensource shard-0 data.
//!
//! Fixture is produced by pangea-architectures/bin/lava-resolve-org and
//! supplied via LAVA_SHARD_FIXTURE. Skips when absent so CI stays green
//! without it — the point is the local end-to-end proof, not a pinned blob.
use lava_architectures::{ARCHITECTURE_DIR, eval_architecture};
use lava_eval::InputBindings;
use std::collections::BTreeMap;

#[test]
fn renders_the_real_shard_zero() {
    let Ok(path) = std::env::var("LAVA_SHARD_FIXTURE") else {
        eprintln!("LAVA_SHARD_FIXTURE unset — skipping");
        return;
    };
    let raw = std::fs::read_to_string(&path).expect("fixture");
    let v: serde_json::Value = serde_json::from_str(&raw).expect("json");

    let repos: Vec<BTreeMap<String, String>> = v["repos"]
        .as_array()
        .expect("repos")
        .iter()
        .map(|r| {
            r.as_object()
                .unwrap()
                .iter()
                .map(|(k, x)| (k.clone(), x.as_str().unwrap_or_default().to_string()))
                .collect()
        })
        .collect();

    let declared = repos.len();
    let mut b = InputBindings::default();
    b.set_str("owner", v["owner"].as_str().unwrap_or("pleme-io"));
    b.set_str("repo_count", v["repo_count"].as_str().unwrap_or("0"));
    b.set_records("repos", repos);

    let src = std::fs::read_to_string(
        std::path::Path::new(ARCHITECTURE_DIR).join("github-org-repos.tlisp"),
    )
    .expect("source");
    let json = eval_architecture(&src, &b)
        .expect("REAL shard-0 data evaluates")
        .render_terraform_json()
        .expect("REAL shard-0 data renders");

    let res = json["resource"].as_object().expect("resources");
    let repos_out = res["github_repository"].as_object().expect("repos").len();
    let perms = res["github_actions_repository_permissions"]
        .as_object()
        .map_or(0, |o| o.len());

    println!(
        "REAL RENDER: declared={declared} github_repository={repos_out} actions_perms={perms}"
    );
    println!("  resource types: {:?}", res.keys().collect::<Vec<_>>());

    // HAZARD 1 in assertion form: the denominator must survive the render.
    assert_eq!(
        repos_out, declared,
        "every declared repo must reach the plan"
    );
    assert!(declared > 200, "expected the real shard, got {declared}");
}
