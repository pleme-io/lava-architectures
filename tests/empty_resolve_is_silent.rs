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
// HAZARD 2 — the ported branch-protection surface is NARROWER than the
// Ruby's profile table, and the gap is silent.
//
// `open_source_repo.rb`'s PROFILES carries FIVE fields per profile:
//   required_reviews, dismiss_stale_reviews, require_signed_commits,
//   require_linear_history, enforce_admins
// The lava port emits TWO attributes on `github-branch-protection`:
//   required_status_checks_strict, enforce_admins
//
// ★ STATE THE DENOMINATOR: what the RUBY finally emits to terraform is
// UNVERIFIED here. The emission goes through
// `Pangea::Helpers::Github.protect_default_branch`, which lives in
// pangea-github — not cloned locally, gem not installed, and this token
// cannot use code search. So "the Ruby emits all five" is PLAUSIBLE, not
// measured, and this comment must not be read as proof of a regression.
//
// Why it still matters enough to pin: the org doctrine explicitly requires
// `dismiss_stale_reviews: true` and `require_last_push_approval: true` on
// the `standard` tier, citing a real incident where two post-approval
// commits merged unreviewed. If the Ruby does emit those and the port does
// not, migrating silently drops a mandated control — and a repo whose
// protection quietly weakened looks identical to one that was migrated
// correctly.
//
// The test below pins only the half that IS measurable: the exact attribute
// set the port emits. If someone widens it, this fails and they update it
// deliberately. Verifying the Ruby side is the open task.

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
