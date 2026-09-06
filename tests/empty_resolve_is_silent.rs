//! The hazard the github-org-repos MIGRATION has to design around.
//!
//! `github-org-repos` takes every repository as an `:input` record. So a
//! caller whose RESOLVE step returns nothing — a failed org.yaml parse, a
//! 404'd GitHub listing, a shard filter that matched zero rows, a typo in a
//! variable name — does not produce an error. It produces a **valid,
//! successful, EMPTY** render.
//!
//! That matters because of what consumes it. The `pleme-io-opensource`
//! shards hold on the order of a thousand `github_repository` resources in
//! tofu state. A config that declares none of them is not "no change" — it
//! is a plan to DESTROY every one, and it arrives looking like a clean
//! compile.
//!
//! The CR's `destroyProtection: true` + `defaultDecision: requireApproval`
//! are what stand between that plan and the estate today. Both are
//! MITIGATION: they catch the plan after it is formed, at a review step a
//! human has to actually read. Nothing makes the empty render
//! unrepresentable, and this test does not pretend otherwise — it PINS the
//! behaviour so the property is visible to whoever builds the resolver,
//! rather than being discovered from a plan diff.
//!
//! ★ The rule this implies for the resolver: emit the DENOMINATOR beside
//! the records and cross-check it. The architecture already carries
//! `repo_count` for exactly this shape — a resolve that yields 0 records
//! while the catalogue declares ~250 must fail at the CALLER, because by
//! the time it reaches lava the information needed to tell "no repos" from
//! "resolve broke" is gone.

use lava_architectures::{ARCHITECTURE_DIR, eval_architecture};
use lava_eval::InputBindings;

fn render_with_no_repos() -> serde_json::Value {
    let src = std::fs::read_to_string(
        std::path::Path::new(ARCHITECTURE_DIR).join("github-org-repos.tlisp"),
    )
    .expect("github-org-repos source");
    let mut b = InputBindings::default();
    b.set_str("owner", "pleme-io");
    b.set_str("repo_count", "0");
    eval_architecture(&src, &b)
        .expect("an empty bag still EVALUATES — that is the hazard")
        .render_terraform_json()
        .expect("an empty bag still RENDERS — that is the hazard")
}

#[test]
fn an_empty_repo_bag_is_a_successful_render_of_nothing() {
    let json = render_with_no_repos();
    let types = json
        .get("resource")
        .and_then(|r| r.as_object())
        .map_or(0, |o| o.len());
    assert_eq!(
        types, 0,
        "expected the empty bag to render zero resource types, got {types}"
    );
}

/// The half that is easy to forget: it is not merely empty, it is *clean*.
/// No error key, no marker, nothing a downstream consumer could branch on
/// to tell this apart from an org that genuinely has no repositories.
#[test]
fn the_empty_render_carries_no_signal_that_resolve_failed() {
    let json = render_with_no_repos();
    for key in ["error", "errors", "warning", "warnings"] {
        assert!(
            json.get(key).is_none(),
            "if lava ever grows a {key:?} key on an empty render, this hazard \
             is downgraded and the resolver's denominator check can be relaxed \
             — until then it cannot"
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────
// HAZARD 2 — the ported branch-protection surface DIVERGES FROM THE RUBY
// IN BOTH DIRECTIONS, and the Ruby side is not what `open_source_repo.rb`
// appears to say.
//
// MEASURED 2026-08-26 against pangea-github. An earlier revision of this
// comment GUESSED, and guessed wrong: it assumed the five-field
// `OpenSourceRepo::PROFILES` was the policy source. It is not.
//
// What the RUBY emits, via `GithubPresets.protect_default_branch`:
//   repository_id, pattern,
//   allows_deletions: false, allows_force_pushes: false   <- EVERY profile
//   + BRANCH_PROTECTION_PROFILES[profile], exactly THREE fields:
//     enforce_admins, require_signed_commits, required_linear_history
//
// What the LAVA PORT emits:
//   repository_id, pattern, required_status_checks_strict, enforce_admins
//
// So the port DROPS four attributes the Ruby always sets — allows_deletions,
// allows_force_pushes, require_signed_commits, required_linear_history — and
// ADDS one the Ruby never sets, required_status_checks_strict. Migrating
// as-is would UNLOCK force-pushes and branch deletion on every repo that
// currently has them locked.
//
// ── ★ AND A FINDING THAT OUTLIVES THIS MIGRATION ─────────────────────────
// `open_source_repo.rb`'s PROFILES declares `required_reviews` and
// `dismiss_stale_reviews` per profile. Grepped exhaustively: those two keys
// appear ONLY at their definition sites (lines 41–56) and are read NOWHERE
// — not in pangea-architectures, not in pangea-github. That table is a
// validity whitelist for the `PROFILES.key?` check, not a policy source.
//
// So the org doctrine line requiring `dismiss_stale_reviews: true` on the
// `standard` tier — the one citing the incident where two post-approval
// commits merged unreviewed — is DECLARED BUT NOT EMITTED on the Ruby path
// today, independent of lava. Fixing it in the port alone would not fix it.
//
// The test below pins the port's exact attribute set, so any move in either
// direction is deliberate.

use std::collections::BTreeMap;

fn protected_repo_render() -> serde_json::Value {
    let src = std::fs::read_to_string(
        std::path::Path::new(ARCHITECTURE_DIR).join("github-org-repos.tlisp"),
    )
    .expect("github-org-repos source");
    let mut b = InputBindings::default();
    b.set_str("owner", "pleme-io");
    b.set_str("repo_count", "1");
    let mut repo = BTreeMap::new();
    for (k, v) in [
        ("name", "probe-repo"),
        // The ADDRESS component — slugged, while `name` stays the real repo.
        ("slug", "probe_repo"),
        ("description", "branch-protection surface probe"),
        ("visibility", "public"),
        ("archived", "false"),
        ("default_branch", "main"),
        ("has_issues", "true"),
        ("delete_branch_on_merge", "true"),
        ("actions_enabled", "true"),
        ("standard_labels", "false"),
        ("has_branch_protection", "true"),
        ("bp_strict", "true"),
        ("bp_enforce_admins", "true"),
        ("has_ci_shim", "false"),
        // Present-but-empty ON PURPOSE -- see HAZARD 3 below.
        ("ci_shim_slug", ""),
        ("ci_shim_path", ""),
        ("ci_shim_content", ""),
        ("exists_on_github", "false"),
    ] {
        repo.insert(k.to_string(), v.to_string());
    }
    b.set_records("repos", vec![repo]);
    eval_architecture(&src, &b)
        .expect("evaluates")
        .render_terraform_json()
        .expect("renders")
}

#[test]
fn branch_protection_emits_exactly_two_policy_attributes() {
    let json = protected_repo_render();
    let bp = json
        .get("resource")
        .and_then(|r| r.get("github_branch_protection"))
        .and_then(|b| b.as_object())
        .expect("a github_branch_protection resource");
    let body = bp
        .values()
        .next()
        .and_then(|v| v.as_object())
        .expect("one entry");

    let mut keys: Vec<&str> = body.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        vec![
            "enforce_admins",
            "pattern",
            "repository_id",
            "required_status_checks_strict"
        ],
        "the ported branch-protection surface changed. If this WIDENED toward \
         PROFILES (dismiss_stale_reviews, required_reviews, require_signed_commits, \
         require_linear_history) that is the migration gap closing — update this \
         list. If it NARROWED, a control was dropped."
    );
}

// ─────────────────────────────────────────────────────────────────────────
// HAZARD 3 — `:when` gates EMISSION, not NAME RESOLUTION. This is the one
// most likely to bite the resolver author, and it fails LOUDLY (good) but
// for a reason that reads as unrelated to the field you forgot.
//
// The CI-shim resource is addressed `"{repo_name}__{repo_ci_shim_slug}"`.
// The address is interpolated whether or not `:when` lets the resource
// through, so a repo with `has_ci_shim: false` and NO `ci_shim_slug` field
// does not quietly skip — it aborts the ENTIRE render:
//
//     UnknownVar("repo_ci_shim_slug")
//
// One repo missing one field for a resource it does not even want takes
// down all ~250 in the shard. Measured: adding ci_shim_slug/path/content as
// empty strings to the fixture above is what turned this suite green.
//
// ★ The rule for the resolver: emit EVERY field for EVERY record, present
// and possibly empty, never conditionally. "This repo has no CI shim so I
// will omit the shim fields" is the natural thing to write and it is wrong.
// The record shape is the union of every field the architecture names, not
// the subset a given repo happens to use.

#[test]
fn a_record_missing_a_gated_only_field_fails_the_whole_render() {
    let src = std::fs::read_to_string(
        std::path::Path::new(ARCHITECTURE_DIR).join("github-org-repos.tlisp"),
    )
    .expect("github-org-repos source");
    let mut b = InputBindings::default();
    b.set_str("owner", "pleme-io");
    b.set_str("repo_count", "1");

    // Everything a repo needs EXCEPT the ci_shim_* trio, with the CI shim
    // explicitly switched OFF — the shape a resolver would naturally emit.
    let mut repo = BTreeMap::new();
    for (k, v) in [
        ("name", "no-shim-repo"),
        // Address component. Present so the render reaches the field this
        // test is actually about, rather than tripping on the slug first.
        ("slug", "no_shim_repo"),
        (
            "description",
            "has_ci_shim is false, so surely the slug is unused",
        ),
        ("visibility", "public"),
        ("archived", "false"),
        ("default_branch", "main"),
        ("has_issues", "true"),
        ("delete_branch_on_merge", "true"),
        ("actions_enabled", "true"),
        ("standard_labels", "false"),
        ("has_branch_protection", "false"),
        ("bp_strict", "false"),
        ("bp_enforce_admins", "false"),
        ("has_ci_shim", "false"),
        ("exists_on_github", "true"),
    ] {
        repo.insert(k.to_string(), v.to_string());
    }
    b.set_records("repos", vec![repo]);

    let err = eval_architecture(&src, &b)
        .err()
        .expect("a record missing a gated-only field must FAIL, not skip");
    let msg = err.to_string();
    assert!(
        msg.contains("ci_shim_slug"),
        "expected the failure to name the missing field, got: {msg}"
    );
}
