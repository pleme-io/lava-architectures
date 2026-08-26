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
