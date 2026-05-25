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

