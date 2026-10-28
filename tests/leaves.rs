use std::path::{Path, PathBuf};

use lava_architectures::leaves::{
    Declaration, derive, parse_declarations, render_workspace, tree_drift, union_gate,
};
use lava_architectures::{ARCHITECTURE_DIR, bundled_source};
use lava_eval::leaf_key_in_source;
use serde_json::{Map, Value, json};

fn arch() -> String {
    bundled_source("github-org-repos").expect("github-org-repos source")
}

fn repo(name: &str, slug: &str, exists: bool, shim: bool) -> Value {
    json!({
        "name": name, "slug": slug, "description": format!("{name} repo"),
        "visibility": "private", "live_visibility": "private", "archived": "false",
        "default_branch": "main", "has_issues": "true", "delete_branch_on_merge": "true",
        "actions_enabled": "true", "actions_disabled": "false", "standard_labels": "true",
        "has_branch_protection": "true", "bp_strict": "false", "bp_enforce_admins": "false",
        "has_ci_shim": if shim { "true" } else { "false" },
        "ci_shim_slug": if shim { "ci" } else { "" },
        "ci_shim_path": if shim { ".github/workflows/ci.yml" } else { "" },
        "ci_shim_content": if shim { "on: push" } else { "" },
        "exists_on_github": if exists { "true" } else { "false" },
        "renamed_from": ""
    })
}

fn records() -> Map<String, Value> {
    json!({
        "owner": "pleme-io",
        "repo_count": "3",
        "labels": [{"name": "bug", "slug": "bug", "color": "d73a4a", "description": "broken"}],
        "repos": [
            repo("alpha", "alpha", true, false),
            repo("beta-two", "beta_two", false, true),
            repo("9lives", "r_9lives", true, false)
        ],
        "moves": [
            {"slug": "beta_two", "from": "github_repository.beta", "to": "github_repository.beta_two"},
            {"slug": "beta_two", "from": "github_actions_repository_permissions.beta_actions", "to": "github_actions_repository_permissions.beta_two_actions"}
        ]
    })
    .as_object()
    .unwrap()
    .clone()
}

fn declaration(enabled: bool) -> Value {
    json!({
        "leaves": "org-repos",
        "enabled": enabled,
        "from": {"data": "org.yaml", "collection": "repos", "field": "name"},
        "key": "slug",
        "architecture": "github-org-repos",
        "records": "leaves-records.json",
        "shared": ["owner", "labels"],
        "count": "repo_count",
        "namespace": "default",
        "spec": {"pangeaNamespace": "pleme-io-github", "destroyProtection": true},
        "catalog": {"source": {}},
        "migrate_from": "org-repos-fat"
    })
}

fn decl() -> Declaration {
    serde_json::from_value(declaration(true)).unwrap()
}

fn names() -> Vec<String> {
    vec!["alpha".into(), "beta-two".into(), "9lives".into()]
}

#[test]
fn github_org_repos_declares_its_leaf_key() {
    let key = leaf_key_in_source(&arch(), "github-org-repos").unwrap();
    let key = key.expect("github-org-repos carries a :leaf clause");
    assert_eq!(key.key, "slug");
    assert_eq!(key.each, vec!["repos".to_string(), "moves".to_string()]);
}

#[test]
fn the_union_of_one_repo_renders_equals_the_whole_render() {
    let set = derive(&decl(), &arch(), &records(), &names()).unwrap();
    assert_eq!(set.leaves.len(), 3);
    let beta = set.leaves.iter().find(|l| l.key == "beta_two").unwrap();
    assert_eq!(beta.name, "org-repos-beta-two");
    assert_eq!(beta.variables["moves"].as_array().unwrap().len(), 2);
    assert_eq!(beta.variables["repo_count"], json!("1"));
    assert!(!beta.variables.contains_key("owner"));
    assert_eq!(union_gate(&set, &arch(), &records()).unwrap(), 3);
}

#[test]
fn a_leaf_that_drops_a_row_fails_the_union() {
    let mut set = derive(&decl(), &arch(), &records(), &names()).unwrap();
    set.leaves.retain(|l| l.key != "alpha");
    assert!(union_gate(&set, &arch(), &records()).is_err());
}

#[test]
fn a_row_in_two_leaves_fails_the_union() {
    let mut set = derive(&decl(), &arch(), &records(), &names()).unwrap();
    let twin = set.leaves[0].clone();
    set.leaves.push(twin);
    let err = union_gate(&set, &arch(), &records()).unwrap_err();
    assert!(err.contains("more than one leaf"), "{err}");
}

#[test]
fn stale_records_are_refused_by_name() {
    let mut more = names();
    more.push("gamma".into());
    let r = derive(&decl(), &arch(), &records(), &more).unwrap_err();
    assert!(r.reason.contains("stale"), "{}", r.reason);
    assert!(r.reason.contains("gamma"), "{}", r.reason);
}

#[test]
fn a_key_that_disagrees_with_the_architecture_is_refused() {
    let mut d = decl();
    d.key = "name".into();
    assert!(derive(&d, &arch(), &records(), &names()).is_err());
}

#[test]
fn an_architecture_without_a_leaf_clause_is_refused() {
    let src = arch().replace(":leaf (:key :slug :each (:repos :moves))", "");
    let r = derive(&decl(), &src, &records(), &names()).unwrap_err();
    assert!(r.reason.contains("no :leaf clause"), "{}", r.reason);
}

#[test]
fn a_malformed_entry_is_refused_alone() {
    let text =
        serde_json::to_string(&json!([declaration(true), {"leaves": "bad", "shard": 4}])).unwrap();
    let parsed = parse_declarations(&text).unwrap();
    assert!(parsed[0].is_ok());
    assert_eq!(parsed[1].as_ref().unwrap_err().leaf_set, "bad");
}

fn workspace(enabled: bool) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "lava-leaves-{}-{enabled}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(dir.join("leaves")).unwrap();
    std::fs::write(
        dir.join("org.yaml"),
        "repos:\n- name: alpha\n- name: beta-two\n- name: 9lives\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("leaves-records.json"),
        serde_json::to_string(&records()).unwrap(),
    )
    .unwrap();
    std::fs::write(
        dir.join("leaves/declaration.json"),
        serde_json::to_string(&json!([declaration(enabled)])).unwrap(),
    )
    .unwrap();
    dir
}

fn write(ws: &Path, files: &std::collections::BTreeMap<PathBuf, String>) {
    for (p, body) in files {
        std::fs::create_dir_all(ws.join(p).parent().unwrap()).unwrap();
        std::fs::write(ws.join(p), body).unwrap();
    }
}

#[test]
fn a_rendered_tree_is_fresh_and_a_hand_edit_is_stale() {
    let ws = workspace(true);
    let archs = Path::new(env!("CARGO_MANIFEST_DIR")).join(ARCHITECTURE_DIR);
    let out = render_workspace(&ws, &archs, true).unwrap();
    assert!(out.refusals.is_empty(), "{:?}", out.refusals);
    assert_eq!(out.files.len(), 4);
    assert!(!tree_drift(&ws, &out.files, &out.sets).is_empty());
    write(&ws, &out.files);
    assert!(tree_drift(&ws, &out.files, &out.sets).is_empty());
    let leaf = ws.join("leaves/org-repos/leaves/org-repos-alpha.yaml");
    let body = std::fs::read_to_string(&leaf).unwrap();
    assert!(
        body.contains("pangea.pleme.io/workspace: org-repos"),
        "{body}"
    );
    assert!(body.contains("pangea.pleme.io/leaf-key: alpha"), "{body}");
    assert!(
        body.contains("pangea.pleme.io/leaf-collection: repos"),
        "{body}"
    );
    assert!(
        body.contains("pangea.pleme.io/leaf-key-field: slug"),
        "{body}"
    );
    assert!(
        body.contains("pangea.pleme.io/leaf-addresses: github_actions_repository_permissions.alpha_actions,github_branch_protection.alpha_protection,github_issue_label.alpha-label-bug,github_repository.alpha"),
        "{body}"
    );
    let catalog = std::fs::read_to_string(ws.join("leaves/org-repos/catalog.yaml")).unwrap();
    assert!(!catalog.contains("namespace:"), "{catalog}");
    assert!(
        catalog.contains("pangea.pleme.io/migrate-from: org-repos-fat"),
        "{catalog}"
    );
    std::fs::write(&leaf, body.replace("alpha repo", "edited")).unwrap();
    std::fs::write(ws.join("leaves/org-repos/leaves/extra.yaml"), "x").unwrap();
    assert_eq!(tree_drift(&ws, &out.files, &out.sets).len(), 2);
    std::fs::remove_dir_all(&ws).unwrap();
}

#[test]
fn a_disabled_set_renders_no_leaves() {
    let ws = workspace(false);
    let archs = Path::new(env!("CARGO_MANIFEST_DIR")).join(ARCHITECTURE_DIR);
    let out = render_workspace(&ws, &archs, true).unwrap();
    assert!(out.files.is_empty());
    assert_eq!(out.sets, vec![("org-repos".to_string(), 0)]);
    std::fs::remove_dir_all(&ws).unwrap();
}
