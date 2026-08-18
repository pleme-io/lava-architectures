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
