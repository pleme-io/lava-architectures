//! Lava-test suite — runs every committed .test.tlisp fixture
//! against its target bundled architecture. Failures aggregate per
//! ★★ CLOSED-LOOP MASS-SYNTHESIS into one assertion report.

use indexmap::IndexMap;
use lava_architectures::{eval_architecture, ARCHITECTURE_DIR};
use lava_eval::InputBindings;
use lava_test::{run_case_against, tests_in_source, AssertContext};

fn tests_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("architectures")
}

fn architecture_src(name: &str) -> std::io::Result<String> {
    std::fs::read_to_string(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(ARCHITECTURE_DIR)
            .join(format!("{name}.tlisp")),
    )
}

#[test]
fn every_committed_test_fixture_passes_against_its_architecture() {
    let dir = tests_dir();
    let entries = std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("read {}: {e}", dir.display()));

    let mut failures: Vec<String> = Vec::new();
    let mut total_cases = 0usize;
    let mut total_assertions = 0usize;

    for entry in entries.flatten() {
        let path = entry.path();
        if !path
            .file_name()
            .and_then(|s| s.to_str())
            .is_some_and(|s| s.ends_with(".test.tlisp"))
        {
            continue;
        }

        let src = std::fs::read_to_string(&path).unwrap();
        let cases = match tests_in_source(&src) {
            Ok(cs) => cs,
            Err(e) => {
                failures.push(format!("{}: parse: {e}", path.display()));
                continue;
            }
        };

        for case in cases {
            total_cases += 1;
            let arch_name = case.architecture.clone().unwrap_or_default();
            if arch_name.is_empty() {
                failures.push(format!("{}: case {} has no :architecture", path.display(), case.name));
                continue;
            }
            let arch_src = match architecture_src(&arch_name) {
                Ok(s) => s,
                Err(e) => {
                    failures.push(format!("{}: {arch_name} missing source: {e}", path.display()));
                    continue;
                }
            };
            let mut bindings = InputBindings::new();
            for (k, v) in &case.bindings {
                bindings.set_str(k.clone(), v.clone());
            }
            // Provide matrix-fallback bindings for architectures with
            // required inputs not covered by the test fixture itself.
            apply_required_bindings(&arch_name, &case.bindings, &mut bindings);

            let arch = match eval_architecture(&arch_src, &bindings) {
                Ok(a) => a,
                Err(e) => {
                    failures.push(format!("{arch_name}/{}: eval: {e}", case.name));
                    continue;
                }
            };
            let ctx = match AssertContext::from_architecture(&arch) {
                Ok(c) => c,
                Err(e) => {
                    failures.push(format!("{arch_name}/{}: render: {e}", case.name));
                    continue;
                }
            };
            let outcome = run_case_against(&case, &ctx);
            total_assertions += outcome.passed + outcome.failures.len();
            if !outcome.ok() {
                for f in &outcome.failures {
                    failures.push(format!(
                        "{arch_name}/{}: {} @ {} — {}",
                        outcome.name,
                        f.assertion,
                        f.pointer.as_deref().unwrap_or("-"),
                        f.message
                    ));
                }
            }
        }
    }

    assert!(
        failures.is_empty(),
        "{} assertion(s) failed across {total_cases} case(s) ({total_assertions} total):\n  - {}",
        failures.len(),
        failures.join("\n  - ")
    );
}

/// Layer per-architecture required-input bindings on top of what the
/// .test.tlisp itself sets. Keeps test files minimal — the fixture
/// declares only what it cares about; required inputs default to
/// stable placeholder values.
fn apply_required_bindings(
    arch: &str,
    declared: &IndexMap<String, String>,
    bindings: &mut InputBindings,
) {
    let pairs: &[(&str, &str)] = match arch {
        "cloudflare-r2-bucket" => &[
            ("account-id", "abcd1234abcd1234abcd1234abcd1234"),
            ("bucket-name", "smoke-bucket"),
            ("zone-id", "ffff1234ffff1234ffff1234ffff1234"),
            ("domain", "cdn.example.com"),
        ],
        "public-dns" => &[("name", "smoke"), ("domain", "smoke.example.com")],
        "akeyless-platform" => &[
            ("zone-id", "ffff1234ffff1234ffff1234ffff1234"),
            ("domain", "akeyless.example.com"),
        ],
        "cloudflare-dns-records" => &[("zone-id", "abcdef1234567890")],
        "akeyless-secrets" => &[("name-prefix", "smoke")],
        "cloudflare-tunnel" => &[
            ("account-id", "00000000000000000000000000000000"),
            ("tunnel-name", "smoke-tunnel"),
            ("tunnel-secret", "placeholder-secret"),
            ("zone-id", "ffffffffffffffffffffffffffffffff"),
            ("hostname", "smoke.example.com"),
        ],
        "cloudflare-zone" => &[
            ("account-id", "00000000000000000000000000000000"),
            ("domain", "smoke.example.com"),
        ],
        "aws-eks-cluster" => &[("name", "smoke")],
        "split-horizon-dns" => &[
            ("name", "smoke"),
            ("domain", "smoke.example.com"),
            ("vpc-id", "vpc-1234"),
        ],
        "dns-record-set" => &[
            ("zone-id", "Z0000000000000000000"),
            ("name", "smoke"),
            ("domain", "smoke.example.com"),
        ],
        "cilium-irsa" => &[
            ("cluster-name", "smoke"),
            ("oidc-provider-arn", "arn:aws:iam::000:oidc-provider/x"),
        ],
        "cluster-autoscaler-iam" => &[("cluster-name", "smoke")],
        "cloudflare-tunnel-ingress" => &[
            ("account-id", "00000000000000000000000000000000"),
            ("tunnel-name", "smoke"),
            ("tunnel-secret", "x"),
            ("zone-id", "ffffffffffffffffffffffffffffffff"),
            ("domain", "smoke.example.com"),
        ],
        "cloudflare-headless-blog" => &[
            ("account-id", "00000000000000000000000000000000"),
            ("bucket-name", "smoke-blog"),
            ("zone-id", "ffffffffffffffffffffffffffffffff"),
            ("hostname", "smoke.example.com"),
        ],
        "akeyless-aws-integration" => &[
            ("access-key-id", "AKIAxxxxxxxxxxxxxxxx"),
            ("secret-access-key", "x"),
        ],
        "cloudflare-zero-trust-access" => &[
            ("account-id", "00000000000000000000000000000000"),
            ("zone-id", "11111111111111111111111111111111"),
            ("app-domain", "smoke.example.com"),
        ],
        "cloudflare-domain" => &[
            ("account-id", "00000000000000000000000000000000"),
            ("domain", "smoke.example.com"),
            ("zone-id", "11111111111111111111111111111111"),
        ],
        "cloudflare-dns-security" => &[
            ("zone-id", "11111111111111111111111111111111"),
            ("domain", "smoke.example.com"),
        ],
        "azure-aks-cluster" => &[("resource-group", "smoke-rg")],
        "cilium-irsa-variants" => &[
            ("cluster-name", "smoke"),
            ("oidc-provider-arn", "arn:aws:iam::000:oidc-provider/x"),
        ],
        "dns-record-set-typed" => &[
            ("zone-id", "Z0000000000000000000"),
            ("domain", "smoke.example.com"),
        ],
        _ => &[],
    };
    for (k, v) in pairs {
        if !declared.contains_key(*k) {
            bindings.set_str((*k).to_string(), (*v).to_string());
        }
    }
}
