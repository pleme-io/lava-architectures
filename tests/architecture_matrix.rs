//! ★★ CLOSED-LOOP MASS-SYNTHESIS verification matrix.
//!
//! Per the org-wide directive: every substrate that ships N typed
//! variants must ship one matrix test that exercises every variant
//! and fails the build when a new variant lands without a matrix row.
//!
//! What this file proves for **every** bundled architecture:
//!
//! 1. The `.tlisp` source file is present on disk and parses cleanly.
//! 2. `eval_architecture` produces a typed `Architecture` value.
//! 3. The architecture renders to terraform.json that magma can apply.
//! 4. The rendered JSON contains at least the minimum expected number
//!    of `resource.<type>.<name>` entries.
//! 5. A typed `Interface` is registered via `interface_for` (so
//!    consumers can schema-gate the architecture).
//! 6. The registered Interface accepts a defaults-only / minimal-valid
//!    bag round-trip — proves the interface is self-consistent.
//!
//! Aggregate failure-reporting: every broken row is collected and
//! reported in one assert at the end. CI surface stays green/red
//! per-architecture-class, not first-failure-wins.

use indexmap::IndexMap;
use lava_architectures::{
    eval_architecture, interface_for, BUNDLED_ARCHITECTURES, ARCHITECTURE_DIR,
};
use lava_eval::InputBindings;

fn architecture_path(name: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(ARCHITECTURE_DIR)
        .join(format!("{name}.tlisp"))
}

fn count_resources(json: &serde_json::Value) -> usize {
    let Some(by_type) = json.get("resource").and_then(serde_json::Value::as_object) else {
        return 0;
    };
    by_type
        .values()
        .filter_map(serde_json::Value::as_object)
        .map(serde_json::Map::len)
        .sum()
}

/// Returns the per-architecture (bindings, bag-projection) pair.
/// The bag projection matches what lava-runtime feeds to
/// `Interface::validate_inputs` — kept here so the matrix doesn't
/// need to depend on lava-runtime.
fn minimal_bindings(
    arch_name: &str,
) -> (InputBindings, IndexMap<String, String>) {
    let mut b = InputBindings::new();
    let mut bag: IndexMap<String, String> = IndexMap::new();
    match arch_name {
        "cloudflare-dns-records" => {
            b.set_str("zone-id", "11112222333344445555666677778888");
            bag.insert(
                "zone-id".to_string(),
                "11112222333344445555666677778888".to_string(),
            );
        }
        // ── ★ THE ONLY RECORD-VALUED ARCHITECTURE IN THE MATRIX ─────────
        // Every value here is an :input, so with an empty bag the for-each
        // loops iterate nothing and the whole architecture renders ZERO
        // resources — which lava reports as SUCCESS, not as an error. That
        // is the shape this matrix exists to catch: an architecture can be
        // perfectly valid, evaluate cleanly, and emit nothing.
        //
        // The fixture therefore supplies ONE repo with every `:when`
        // predicate true, so each conditional resource family is actually
        // exercised rather than skipped. Predicates are strings because
        // that is what a record scalar is; truthy spellings are
        // "true"/"#t"/"1".
        "github-org-repos" => {
            b.set_str("owner", "pleme-io");
            bag.insert("owner".to_string(), "pleme-io".to_string());
            // The evaluator reads `repo_count` (the architecture's own input
            // spelling); the interface declares `repo-count`. Only the latter
            // belongs in the bag — seeding both makes the interface reject an
            // unknown input.
            b.set_str("repo_count", "1");

            let mut repo = std::collections::BTreeMap::new();
            for (k, v) in [
                ("name", "matrix-repo"),
                ("description", "matrix fixture repository"),
                ("visibility", "public"),
                ("archived", "false"),
                ("default_branch", "main"),
                ("has_issues", "true"),
                ("delete_branch_on_merge", "true"),
                ("actions_enabled", "true"),
                ("standard_labels", "true"),
                ("has_branch_protection", "true"),
                ("bp_strict", "true"),
                ("bp_enforce_admins", "true"),
                ("has_ci_shim", "true"),
                ("ci_shim_path", ".github/workflows/ci.yml"),
                ("ci_shim_slug", "ci"),
                ("ci_shim_content", "name: ci\non: [push]\n"),
                ("exists_on_github", "true"),
            ] {
                repo.insert(k.to_string(), v.to_string());
            }
            b.set_records("repos", vec![repo]);

            let mut label = std::collections::BTreeMap::new();
            for (k, v) in [
                ("name", "bug"),
                ("slug", "bug"),
                ("color", "d73a4a"),
                ("description", "Something is not working"),
            ] {
                label.insert(k.to_string(), v.to_string());
            }
            b.set_records("labels", vec![label]);

            // ── ★ THE BAG IS A SECOND, FLAT VIEW and it is NOT optional ─────
            // `bindings` is what the evaluator reads; `bag` is what the typed
            // Interface validates. Records live only in the former, so a
            // record-valued input is absent from the latter and the interface
            // rejects the architecture as missing a required input — even
            // though it just rendered correctly. Both views must be fed.
            for (k, v) in [("repos", "matrix-repo"), ("labels", "bug"), ("repo-count", "1")] {
                bag.insert(k.to_string(), v.to_string());
            }
        }
        "akeyless-secrets" => {
            b.set_str("name-prefix", "matrix-test");
            bag.insert("name-prefix".to_string(), "matrix-test".to_string());
        }
        "cloudflare-r2-bucket" => {
            for (k, v) in [
                ("account-id", "abcd1234abcd1234abcd1234abcd1234"),
                ("bucket-name", "matrix-bucket"),
                ("zone-id", "ffff1234ffff1234ffff1234ffff1234"),
                ("domain", "cdn.example.com"),
            ] {
                b.set_str(k, v);
                bag.insert(k.to_string(), v.to_string());
            }
        }
        "public-dns" => {
            for (k, v) in [("name", "matrix"), ("domain", "matrix.example.com")] {
                b.set_str(k, v);
                bag.insert(k.to_string(), v.to_string());
            }
        }
        "akeyless-platform" => {
            for (k, v) in [
                ("zone-id", "ffff1234ffff1234ffff1234ffff1234"),
                ("domain", "akeyless.example.com"),
            ] {
                b.set_str(k, v);
                bag.insert(k.to_string(), v.to_string());
            }
        }
        "cloudflare-tunnel" => {
            for (k, v) in [
                ("account-id", "00000000000000000000000000000000"),
                ("tunnel-name", "matrix-tunnel"),
                ("tunnel-secret", "placeholder-32-byte-secret-value-x"),
                ("zone-id", "11111111111111111111111111111111"),
                ("hostname", "matrix.example.com"),
            ] {
                b.set_str(k, v);
                bag.insert(k.to_string(), v.to_string());
            }
        }
        "cloudflare-zone" => {
            for (k, v) in [
                ("account-id", "00000000000000000000000000000000"),
                ("domain", "matrix.example.com"),
            ] {
                b.set_str(k, v);
                bag.insert(k.to_string(), v.to_string());
            }
        }
        // ★ Deliberately synthetic values. This architecture's parity oracle
        // is a PRIVATE workspace carrying a live security-group id and an
        // operator's home CIDR; the real differential runs where that data
        // already lives. What belongs in this public matrix is proof the
        // STRUCTURE renders, and nothing that identifies an environment.
        "aws-sg-ingress-rules" => {
            for (k, v) in [
                ("name", "matrix"),
                ("security-group-id", "sg-00000000000000000"),
                ("ssh-description", "matrix operator — SSH"),
                ("api-description", "matrix operator — K3s API"),
            ] {
                b.set_str(k, v);
                bag.insert(k.to_string(), v.to_string());
            }
            b.set_list("operator-cidrs", vec!["203.0.113.0/32".into()]);
            bag.insert("operator-cidrs".to_string(), "203.0.113.0/32".to_string());
        }
        "aws-eks-cluster" => {
            for (k, v) in [("name", "matrix")] {
                b.set_str(k, v);
                bag.insert(k.to_string(), v.to_string());
            }
            b.set_list("subnet-ids", vec!["subnet-aaa".into(), "subnet-bbb".into()]);
            bag.insert("subnet-ids".to_string(), "subnet-aaa,subnet-bbb".to_string());
        }
        "split-horizon-dns" => {
            for (k, v) in [
                ("name", "matrix"),
                ("domain", "matrix.example.com"),
                ("vpc-id", "vpc-aaaaaaaa"),
            ] {
                b.set_str(k, v);
                bag.insert(k.to_string(), v.to_string());
            }
        }
        "dns-record-set" => {
            for (k, v) in [
                ("zone-id", "Z0000000000000000000"),
                ("name", "matrix"),
                ("domain", "matrix.example.com"),
            ] {
                b.set_str(k, v);
                bag.insert(k.to_string(), v.to_string());
            }
        }
        "cilium-irsa" => {
            for (k, v) in [
                ("cluster-name", "matrix"),
                ("oidc-provider-arn", "arn:aws:iam::000:oidc-provider/x"),
            ] {
                b.set_str(k, v);
                bag.insert(k.to_string(), v.to_string());
            }
        }
        "cluster-autoscaler-iam" => {
            for (k, v) in [("cluster-name", "matrix")] {
                b.set_str(k, v);
                bag.insert(k.to_string(), v.to_string());
            }
        }
        "cloudflare-tunnel-ingress" => {
            for (k, v) in [
                ("account-id", "00000000000000000000000000000000"),
                ("tunnel-name", "matrix"),
                ("tunnel-secret", "x"),
                ("zone-id", "11111111111111111111111111111111"),
                ("domain", "matrix.example.com"),
            ] {
                b.set_str(k, v);
                bag.insert(k.to_string(), v.to_string());
            }
        }
        "cloudflare-headless-blog" => {
            for (k, v) in [
                ("account-id", "00000000000000000000000000000000"),
                ("bucket-name", "matrix-blog"),
                ("zone-id", "11111111111111111111111111111111"),
                ("hostname", "matrix.example.com"),
            ] {
                b.set_str(k, v);
                bag.insert(k.to_string(), v.to_string());
            }
        }
        "akeyless-aws-integration" => {
            for (k, v) in [
                ("access-key-id", "AKIAxxxxxxxxxxxxxxxx"),
                ("secret-access-key", "placeholder-secret"),
            ] {
                b.set_str(k, v);
                bag.insert(k.to_string(), v.to_string());
            }
        }
        // backup-recovery, ami-production-iam, convergence-dashboard,
        // drill-network — defaults cover everything.
        "cloudflare-zero-trust-access" => {
            for (k, v) in [
                ("account-id", "00000000000000000000000000000000"),
                ("zone-id", "11111111111111111111111111111111"),
                ("app-domain", "matrix.example.com"),
            ] {
                b.set_str(k, v);
                bag.insert(k.to_string(), v.to_string());
            }
        }
        "cloudflare-domain" => {
            for (k, v) in [
                ("account-id", "00000000000000000000000000000000"),
                ("domain", "matrix.example.com"),
                ("zone-id", "11111111111111111111111111111111"),
            ] {
                b.set_str(k, v);
                bag.insert(k.to_string(), v.to_string());
            }
        }
        "cloudflare-dns-security" => {
            for (k, v) in [
                ("zone-id", "11111111111111111111111111111111"),
                ("domain", "matrix.example.com"),
            ] {
                b.set_str(k, v);
                bag.insert(k.to_string(), v.to_string());
            }
        }
        "azure-aks-cluster" => {
            for (k, v) in [("resource-group", "matrix-rg")] {
                b.set_str(k, v);
                bag.insert(k.to_string(), v.to_string());
            }
        }
        "cilium-irsa-variants" => {
            for (k, v) in [
                ("cluster-name", "matrix"),
                ("oidc-provider-arn", "arn:aws:iam::000:oidc-provider/x"),
            ] {
                b.set_str(k, v);
                bag.insert(k.to_string(), v.to_string());
            }
        }
        "dns-record-set-typed" => {
            for (k, v) in [
                ("zone-id", "Z0000000000000000000"),
                ("domain", "matrix.example.com"),
            ] {
                b.set_str(k, v);
                bag.insert(k.to_string(), v.to_string());
            }
        }
        "pleme-io-server" => {
            // The channel lists have no interface default, so the matrix must
            // supply them — and it supplies the REAL ones, not one element
            // each. The registry's resource floor is also what `lava ls`
            // reports to an operator, so it has to describe the architecture
            // as written; a short list here would render 35 and force that
            // floor down to a number that understates the real server by nine
            // channels.
            for (k, vs) in [
                ("substrate-channels", ["magma", "lava", "sui", "nix"].as_slice()),
                ("languages-channels", ["tatara-lisp", "blue"].as_slice()),
                ("platform-channels", ["blackmatter", "camelot", "k8s"].as_slice()),
                ("products-channels", ["mado", "hiroba", "gpu-apps"].as_slice()),
                ("ops-channels", ["alerts", "releases"].as_slice()),
                ("voice-channels", ["general", "pairing"].as_slice()),
            ] {
                b.set_list(k, vs.iter().map(|s| (*s).to_string()).collect());
                bag.insert(k.to_string(), vs.join(","));
            }
        }
        "discord-server-baseline" => {
            // A Discord snowflake — 18 digits, string-typed on the wire even
            // though it is numeric, which is how the provider models every id.
            for (k, v) in [("server-id", "111122223333444455")] {
                b.set_str(k, v);
                bag.insert(k.to_string(), v.to_string());
            }
        }
        _ => {}
    }
    (b, bag)
}

/// One row per supported architecture. Failures aggregate; CI sees
/// every broken architecture in one report.
#[test]
fn every_bundled_architecture_passes_the_matrix() {
    let mut failures: Vec<String> = Vec::new();

    for (name, min_resources) in BUNDLED_ARCHITECTURES {
        // 1) source exists
        let path = architecture_path(name);
        let src = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) => {
                failures.push(format!("{name}: missing source ({e}) at {}", path.display()));
                continue;
            }
        };
        // 2) eval — interpret the tlisp body.
        let (bindings, bag) = minimal_bindings(name);
        let arch = match eval_architecture(&src, &bindings) {
            Ok(a) => a,
            Err(e) => {
                failures.push(format!("{name}: eval failed: {e}"));
                continue;
            }
        };
        // 3) render — produce terraform.json magma can apply.
        let json = match arch.render_terraform_json() {
            Ok(j) => j,
            Err(e) => {
                failures.push(format!("{name}: render failed: {e}"));
                continue;
            }
        };
        // 4) resource-count gate.
        let n = count_resources(&json);
        if n < *min_resources {
            failures.push(format!(
                "{name}: rendered {n} resources, expected >= {min_resources}"
            ));
            continue;
        }
        // 5) typed Interface registered.
        let Some(iface) = interface_for(name) else {
            failures.push(format!("{name}: no Interface registered via interface_for"));
            continue;
        };
        // 6) interface accepts the same minimal binding bag.
        if let Err(errors) = iface.validate_inputs(&bag) {
            failures.push(format!(
                "{name}: interface rejected minimal bag: {} error(s); first = {}",
                errors.len(),
                errors[0]
            ));
            continue;
        }
    }

    assert!(
        failures.is_empty(),
        "{} architecture(s) failed the matrix:\n  - {}",
        failures.len(),
        failures.join("\n  - ")
    );
}

/// Catches the next architecture landing without a registry row —
/// CLOSED-LOOP MASS-SYNTHESIS rule 1: "fail the build when a new
/// variant lands without a matrix row."
#[test]
fn matrix_covers_every_architecture_file_on_disk() {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(ARCHITECTURE_DIR);
    let mut on_disk: Vec<String> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("read {}: {e}", dir.display()))
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("tlisp") {
                path.file_stem()
                    .and_then(|s| s.to_str())
                    .map(std::string::ToString::to_string)
            } else {
                None
            }
        })
        .collect();
    on_disk.sort();

    let mut covered: Vec<String> = BUNDLED_ARCHITECTURES
        .iter()
        .map(|(name, _)| (*name).to_string())
        .collect();
    covered.sort();

    let missing: Vec<&String> = on_disk.iter().filter(|n| !covered.contains(n)).collect();
    let extra: Vec<&String> = covered.iter().filter(|n| !on_disk.contains(n)).collect();

    assert!(
        missing.is_empty() && extra.is_empty(),
        "BUNDLED_ARCHITECTURES is out of sync with architectures/*.tlisp on disk:\n\
         missing from registry: {missing:?}\n\
         extra in registry:     {extra:?}"
    );
}

/// Every registered Interface must also list every architecture in
/// BUNDLED_ARCHITECTURES — partition-completeness check.
#[test]
fn every_bundled_architecture_has_a_registered_interface() {
    let unregistered: Vec<&str> = BUNDLED_ARCHITECTURES
        .iter()
        .filter_map(
            |(name, _)| {
                if interface_for(name).is_none() {
                    Some(*name)
                } else {
                    None
                }
            },
        )
        .collect();
    assert!(
        unregistered.is_empty(),
        "architectures missing from interface_for(): {unregistered:?}"
    );
}

// ── pleme-io-server structural gates ──────────────────────────────────
//
// The matrix above proves the architecture EVALUATES and renders enough
// resources. Neither of those notices if a permission bitfield is off by a
// bit, or if a whole category is left with no overwrite at all — and both of
// those are silent, security-relevant, and exactly the sort of thing a
// declaration is supposed to make reviewable.

fn render_pleme_io_server() -> serde_json::Value {
    let src = std::fs::read_to_string(architecture_path("pleme-io-server")).expect("source");
    let (bindings, _) = minimal_bindings("pleme-io-server");
    eval_architecture(&src, &bindings)
        .expect("evaluates")
        .render_terraform_json()
        .expect("renders")
}

/// Every permission constant in the file, recomputed from its bits.
///
/// The architecture cannot hold a numeric constant table — a numeric value in
/// an architecture's `:inputs` fails evaluation, so the bitfields are written
/// as literals at each site with their derivation in a comment. A comment
/// cannot be wrong-and-noticed, so this recomputes each one from the shifts
/// and asserts the rendered JSON matches. A mistyped bit is a red test rather
/// than a channel that is quietly world-readable.
#[test]
fn pleme_io_server_permission_bits_are_correct() {
    const VIEW_CHANNEL: u64 = 1 << 10;
    const READ_MESSAGE_HISTORY: u64 = 1 << 16;
    const SEND_MESSAGES: u64 = 1 << 11;
    const ADD_REACTIONS: u64 = 1 << 6;
    const EMBED_LINKS: u64 = 1 << 14;
    const ATTACH_FILES: u64 = 1 << 15;
    const MANAGE_MESSAGES: u64 = 1 << 13;
    const CONNECT: u64 = 1 << 20;
    const SPEAK: u64 = 1 << 21;
    const ADMINISTRATOR: u64 = 1 << 3;

    let read_only = VIEW_CHANNEL | READ_MESSAGE_HISTORY;
    let participate =
        read_only | SEND_MESSAGES | ADD_REACTIONS | EMBED_LINKS | ATTACH_FILES;
    let moderate = participate | MANAGE_MESSAGES;
    let voice = VIEW_CHANNEL | CONNECT | SPEAK;

    // The values the file documents. If a shift above is edited, these fail
    // first and name which composition moved.
    assert_eq!(read_only, 66560, "READ_ONLY");
    assert_eq!(participate, 117824, "PARTICIPATE");
    assert_eq!(moderate, 126016, "MODERATE");
    assert_eq!(voice, 3_146_752, "VOICE");
    assert_eq!(ADMINISTRATOR, 8, "ADMINISTRATOR");

    let json = render_pleme_io_server();
    let roles = &json["resource"]["discord_role"];
    assert_eq!(roles["founder"]["permissions"], ADMINISTRATOR);
    assert_eq!(roles["maintainer"]["permissions"], moderate);
    assert_eq!(roles["contributor"]["permissions"], participate);
    assert_eq!(roles["bot"]["permissions"], participate);

    // @everyone grants nothing server-wide; visibility is per channel.
    assert_eq!(
        json["resource"]["discord_role_everyone"]["everyone"]["permissions"], 0,
        "@everyone must grant nothing at the server level"
    );

    let perms = &json["resource"]["discord_channel_permission"];
    assert_eq!(perms["welcome-everyone"]["allow"], read_only);
    assert_eq!(perms["substrate-contributor"]["allow"], participate);
    assert_eq!(perms["voice-contributor"]["allow"], voice);
    // ops is read-only for humans, and SEND_MESSAGES is DENIED rather than
    // merely ungranted — the difference matters when a second overwrite or a
    // role grant would otherwise re-add it.
    assert_eq!(perms["ops-contributor"]["allow"], read_only);
    assert_eq!(perms["ops-contributor"]["deny"], SEND_MESSAGES);

    // Bitfields must render as JSON numbers. A string here would be coerced by
    // some consumers and rejected by others, and the difference would only
    // surface at apply time.
    assert!(
        perms["substrate-contributor"]["allow"].is_number(),
        "permission must be a number, not a string"
    );
}

/// No category may be left with no overwrite at all.
///
/// With the `@everyone` baseline at 0, a category nobody is granted access to
/// is invisible to every role below founder. That fails safe, which is why the
/// baseline is 0 — but silently, and a category that nobody can see is a bug
/// rather than a policy. Every category is therefore required to be reachable
/// by some role, and `welcome` is the one deliberate public exception.
#[test]
fn every_pleme_io_category_is_reachable_by_some_role() {
    let json = render_pleme_io_server();
    let categories: Vec<String> = json["resource"]["discord_category_channel"]
        .as_object()
        .expect("categories")
        .keys()
        .cloned()
        .collect();
    assert!(!categories.is_empty(), "no categories rendered — vacuous");

    // Which channel each overwrite targets, as the rendered "${...id}" ref.
    let targeted: Vec<String> = json["resource"]["discord_channel_permission"]
        .as_object()
        .expect("overwrites")
        .values()
        .filter_map(|p| p["channel_id"].as_str().map(str::to_owned))
        .collect();

    for cat in &categories {
        let want = format!("${{discord_category_channel.{cat}.id}}");
        assert!(
            targeted.iter().any(|t| t == &want),
            "category `{cat}` has no permission overwrite — with @everyone at 0 \
             it is invisible to every role below founder"
        );
    }

    // And the public front door really is public.
    let welcome = format!("${{discord_text_channel.welcome.id}}");
    assert!(
        targeted.iter().any(|t| t == &welcome),
        "#welcome carries no @everyone overwrite — the server would have no \
         readable channel at all"
    );
}

/// Every attribute this architecture renders must actually exist on that
/// resource, and every required one must be present.
///
/// lava renders whatever attribute name it is handed — it never checks the
/// name against the provider's schema. So `:enable` instead of `:enabled`
/// produced perfectly well-formed JSON that Discord's provider would have
/// rejected at APPLY, with both "unsupported argument" and "missing required
/// argument". That is exactly the failure a plan-time gate exists to move
/// earlier, and it is how the real typo in this file was found.
///
/// The fixture is generated from lava-discord's schema.json, which magma read
/// out of the provider binary:
///
///   python3 - <<'PY'  (see the generator in this repo's history)
///   json.dump(...)  ->  tests/fixtures/discord-provider-attributes.json
///
/// Regenerate it whenever the provider version in lava-discord moves.
#[test]
fn pleme_io_server_attributes_conform_to_the_provider_schema() {
    let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/discord-provider-attributes.json");
    let schema: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&fixture).expect("fixture present"))
            .expect("fixture parses");

    let json = render_pleme_io_server();
    let resources = json["resource"].as_object().expect("resources");

    // Non-vacuity: this must actually be checking something.
    assert!(resources.len() >= 13, "expected >=13 resource types, got {}", resources.len());
    let mut checked = 0usize;

    let mut problems: Vec<String> = Vec::new();
    for (ty, instances) in resources {
        let Some(known) = schema.get(ty) else {
            problems.push(format!("{ty}: not a resource this provider ships"));
            continue;
        };
        let attrs = known["attributes"].as_object().expect("attributes");
        for (label, body) in instances.as_object().expect("instances") {
            let rendered = body.as_object().expect("body");
            for name in rendered.keys() {
                checked += 1;
                match attrs.get(name) {
                    None => problems.push(format!(
                        "{ty}.{label}: `{name}` is not an attribute of {ty}"
                    )),
                    Some(a) if !a["settable"].as_bool().unwrap_or(false) => problems.push(
                        format!("{ty}.{label}: `{name}` is computed and cannot be set"),
                    ),
                    Some(_) => {}
                }
            }
            for (name, a) in attrs {
                if a["required"].as_bool().unwrap_or(false) && !rendered.contains_key(name) {
                    problems.push(format!(
                        "{ty}.{label}: required attribute `{name}` is missing"
                    ));
                }
            }
        }
    }

    assert!(checked >= 60, "only {checked} attributes checked — gate looks vacuous");
    assert!(
        problems.is_empty(),
        "{} attribute problem(s) the provider would reject at apply:\n  - {}",
        problems.len(),
        problems.join("\n  - ")
    );
}

/// Rendered strings must survive the parser byte-for-byte.
///
/// The pinned lava-eval reads string literals with a byte-to-char cast rather
/// than a UTF-8 decode, so any multi-byte character is silently split into
/// Latin-1 lookalikes. An em dash in the guild description rendered as
/// `\u{e2}\u{80}\u{94}` — and that text would have become the server's actual
/// description. Until lava moves to lava-eval 0.2, rendered values stay ASCII
/// and this proves it.
#[test]
fn pleme_io_server_renders_no_mojibake() {
    let json = render_pleme_io_server();
    let text = serde_json::to_string(&json).expect("serialise");
    assert!(
        text.is_ascii(),
        "a rendered value contains non-ASCII, which the pinned parser mangles"
    );
    // And the description specifically, since it is the one prose value that
    // reaches Discord.
    let desc = json["resource"]["discord_server"]["pleme-io"]["description"]
        .as_str()
        .expect("description rendered");
    assert!(desc.is_ascii(), "guild description must be ASCII: {desc:?}");
    assert!(desc.contains("pleme-io"), "description lost its content");
}

/// Completeness, as a gate rather than a claim.
///
/// Every settable attribute on the four GUILD-level resources is either set by
/// the architecture or listed here with a reason. A new provider version that
/// adds a guild setting fails this test until somebody decides about it —
/// which is the only way "the complete set of settings" stays true after the
/// day it was written.
///
/// Omissions are deliberate and each is a judgement, not an oversight:
#[test]
fn every_guild_level_setting_is_set_or_deliberately_omitted() {
    // resource -> attribute -> why it is not set
    let omitted: &[(&str, &str, &str)] = &[
        ("discord_server", "icon_data_uri",
         "no asset pipeline here; a placeholder would set a broken icon"),
        ("discord_server", "icon_url",
         "same, and mutually exclusive with icon_data_uri"),
        ("discord_server", "splash_data_uri", "no asset pipeline"),
        ("discord_server", "splash_url", "no asset pipeline"),
        ("discord_server", "owner_id",
         "computed: the bot owns a bot-created guild, and writing an owner \
          here would assert something the API will not honour"),
        ("discord_server", "region",
         "deprecated by Discord — voice region moved to the channel"),
    ];

    let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/discord-provider-attributes.json");
    let schema: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&fixture).expect("fixture"))
            .expect("parses");
    let json = render_pleme_io_server();

    // The guild-level surface: the server itself and the three resources that
    // configure it. Channels and roles are structure, not settings.
    let guild_level = [
        ("discord_server", "pleme-io"),
        ("discord_system_channel", "system"),
        ("discord_server_widget", "widget"),
        ("discord_role_everyone", "everyone"),
    ];

    let mut unaccounted: Vec<String> = Vec::new();
    let mut set_count = 0usize;
    for (ty, label) in guild_level {
        let attrs = schema[ty]["attributes"].as_object().expect("attrs");
        let rendered = json["resource"][ty][label]
            .as_object()
            .unwrap_or_else(|| panic!("{ty}.{label} not rendered"));
        for (name, a) in attrs {
            if !a["settable"].as_bool().unwrap_or(false) {
                continue;
            }
            // `id` is Terraform's own identifier, not a provider setting.
            // Some resources declare it optional+computed, which makes it look
            // settable to the schema check — but an author writing one would
            // be asserting an id the provider is about to compute. Excluded by
            // name rather than listed as an "omission", because calling it a
            // decision would imply there was one to make.
            if name == "id" {
                continue;
            }
            if rendered.contains_key(name) {
                set_count += 1;
            } else if !omitted.iter().any(|(t, n, _)| *t == ty && n == name) {
                unaccounted.push(format!("{ty}.{name}"));
            }
        }
    }

    assert!(set_count >= 14, "only {set_count} guild settings set — looks vacuous");
    assert!(
        unaccounted.is_empty(),
        "{} guild setting(s) neither set nor explained — decide about each, \
         then either set it or add it to `omitted` with a reason:\n  - {}",
        unaccounted.len(),
        unaccounted.join("\n  - ")
    );

    // Every omission must name a real attribute — a stale entry here would
    // quietly excuse nothing while looking like diligence.
    for (ty, name, _) in omitted {
        assert!(
            schema[ty]["attributes"].get(name).is_some(),
            "omission list names {ty}.{name}, which the provider does not have"
        );
    }
}
