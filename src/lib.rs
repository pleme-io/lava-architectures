//! lava-architectures — reusable infrastructure compositions for the
//! lava suite. Pangea-architectures analog.
//!
//! Every architecture is authored as a `.tlisp` source file in
//! `architectures/`. The interpreter lives in
//! [`lava_eval`](https://crates.io/crates/lava-eval) — this crate is
//! the architecture catalog + loader, not the engine.
//!
//! ## Pipeline
//!
//! ```text
//! architectures/<name>.tlisp        ← author here
//!         │
//!         ▼  lava_eval::eval_architecture
//! lava_core::Architecture           ← typed Rust value
//!         │
//!         ▼  Architecture::render_terraform_json
//! serde_json::Value                 ← in-memory terraform.json
//!         │
//!         ▼  magma plan/apply
//! cloud state
//! ```
//!
//! ## Correctness bar: byte-equivalent terraform.json
//!
//! Each ported architecture has a test that loads the .tlisp source,
//! renders it, and compares the JSON AST-equivalent to what pangea
//! emits. State files produced by `tofu apply` against pangea's JSON
//! and `magma apply` against lava's JSON are byte-equivalent (modulo
//! timestamps Terraform records).

#![allow(clippy::module_name_repetitions)]

use std::collections::{BTreeMap, BTreeSet};

pub use lava_core::{Architecture, ResourceRef, Value};
pub use lava_eval::{
    eval_architecture, eval_architecture_with_schema, interfaces_in_source, parse, Atom,
    EvalError, InputBindings, InterfaceParseError, ParseError, Sx,
};
pub use lava_schema::{Field, Interface, SchemaError};
pub use lava_types::Type;

/// Closed list of every bundled architecture this crate ships. Drives
/// the per-★★ CLOSED-LOOP MASS-SYNTHESIS verification matrix in
/// `tests/architecture_matrix.rs` — adding a new architecture without
/// extending this list fails the build.
///
/// Each entry: `(architecture-name, expected-minimum-resource-count)`.
pub const BUNDLED_ARCHITECTURES: &[(&str, usize)] = &[
    ("aws-vpc-network", 10),       // 1 vpc + 1 igw + 3 public + 3 private + 1 eip + 1 nat + 1 sg
    ("cloudflare-dns-records", 5), // root + www + api + wildcard + acme
    ("discord-server-baseline", 3), // moderator role + category + text channel
    ("pleme-io-server", 44), // the whole server: 13 resource types, 44 resources
    ("akeyless-secrets", 6),       // ci_auth + k8s_auth + db_pwd + api_key + db_target + role
    ("cloudflare-r2-bucket", 4),   // bucket + cors + custom-domain + cname
    ("public-dns", 6),             // zone + kms + ksk + dnssec + log-group + query-log
    ("akeyless-platform", 8),      // acm + validation + complete + 3 svc cnames + role + attach
    ("cloudflare-tunnel", 3),      // tunnel + config + cname (cname conditional via :when)
    ("cloudflare-zone", 2),        // zone + settings-override
    ("aws-eks-cluster", 4),        // cluster role + node role + cluster + node-group
    ("split-horizon-dns", 4),      // 2 zones + 2 apex records
    ("dns-record-set", 2),         // apex + www records
    ("cilium-irsa", 3),            // role + policy + attachment
    ("cluster-autoscaler-iam", 2), // role + inline policy
    ("backup-recovery", 3),        // vault + plan + selection
    ("ami-production-iam", 3),     // role + policy attachment + instance profile
    ("cloudflare-tunnel-ingress", 3), // tunnel + 2 cnames
    ("cloudflare-headless-blog", 3),  // bucket + worker + workers-domain
    ("akeyless-aws-integration", 3),  // aws-target + dynamic-secret + bridge role
    ("convergence-dashboard", 4),     // dashboard + 3 monitors
    ("drill-network", 3),             // vpc + subnet + sg
    ("cloudflare-zero-trust-access", 2), // app + policy
    ("akeyless-dev-cluster", 2),         // eks + node-role
    ("ami-test-iam", 2),                 // role + ssm attach
    ("drill-iam-role", 2),               // role + inline policy
    ("continuous-monitoring", 3),        // log-group + alarm + sns
    ("akeyless-target-attested-infra", 2), // target + role
    ("cloudflare-domain", 2),            // zone + root
    ("cloudflare-dns-security", 3),      // dmarc + spf + dkim
    ("aws-sg-ingress-rules", 2),         // ssh + api ingress on an externally-owned sg
    ("github-org-repos", 1),             // one resource set per catalogued repo
    ("akeyless-dev-packer", 2),          // role + bucket
    ("akeyless-dev-workspace", 2),       // vpc + subnet
    ("azure-aks-cluster", 1),            // aks
    ("burst-forge-dashboard", 2),        // dashboard + monitor
    ("dns-record-set-typed", 4),         // apex + www + mx + verify
    ("backup-recovery-multi-region", 3), // primary + secondary + plan
    ("cilium-irsa-variants", 3),         // 3 IRSA roles
];

/// Derived view of the typed interface registered alongside each
/// bundled architecture. Reads the `.tlisp` source at runtime and
/// pulls out the matching `(deflava-interface …)` form via
/// `lava_eval::interfaces_in_source`.
///
/// The .tlisp source is the single source of truth — no hand-curated
/// Rust mirror to drift. Returns `None` when the name is unknown OR
/// the source ships no interface form for that name.
#[must_use]
pub fn interface_for(name: &str) -> Option<Interface> {
    let path = bundled_source_path(name);
    let src = std::fs::read_to_string(&path).ok()?;
    let ifaces = interfaces_in_source(&src).ok()?;
    ifaces.into_iter().find(|i| i.name == name)
}

fn bundled_source_path(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(ARCHITECTURE_DIR)
        .join(format!("{name}.tlisp"))
}

/// Read the raw `.tlisp` source for a bundled architecture. Returns
/// `None` when the name doesn't resolve. Lets consumers (e.g.
/// magma-lava's source-aware façade) drive evaluation from in-memory
/// source instead of needing the file on disk.
#[must_use]
pub fn bundled_source(name: &str) -> Option<String> {
    std::fs::read_to_string(bundled_source_path(name)).ok()
}

/// Built-in path for the bundled `.tlisp` architectures. Magma reads
/// these at runtime; users can also load their own architectures from
/// any path via `eval_architecture(&fs::read_to_string(path)?, ...)`.
pub const ARCHITECTURE_DIR: &str = "architectures";

/// Convenience: load + evaluate one of the bundled architectures by
/// name. Looks up `architectures/<name>.tlisp` relative to CARGO_MANIFEST_DIR.
///
/// # Errors
/// Returns [`EvalError::NotArchForm`] wrapping the I/O failure if the
/// file cannot be read; any evaluation failure bubbles up from
/// [`lava_eval::eval_architecture`].
pub fn load_bundled(
    name: &str,
    bindings: &InputBindings,
) -> Result<lava_core::Architecture, EvalError> {
    let path = bundled_source_path(name);
    let src = std::fs::read_to_string(&path)
        .map_err(|e| EvalError::NotArchForm(format!("io: {} — {e}", path.display())))?;
    eval_architecture(&src, bindings)
}

// ── dashboard parameterization ──────────────────────────────────────────
//
// A `(deflava-dashboard …)` source is parameterized by `{name}`
// placeholders that are bound TEXTUALLY before evaluation. That binding
// used to exist in exactly one place — a private `bind` helper inside
// `tests/dashboard_matrix.rs` — which was fine while the matrix was the
// only caller. It stopped being fine the moment `src/bin/render.rs`
// needed the same substitution: two copies of a textual binder are two
// copies free to disagree, and the disagreement would surface as the
// matrix passing on a document the CLI does not produce.
//
// So the binder lives here, and both the matrix and the CLI call it.
// What the matrix was written to catch is unchanged: it mirrors the
// pangea-operator Ruby `bind_params`, which is EXTERNAL to this crate and
// therefore still an independent implementation to diverge from.

/// Built-in path for the bundled `.tlisp` dashboards, relative to the
/// crate root. Sibling of [`ARCHITECTURE_DIR`]; separate because a
/// dashboard renders a Grafana document, not terraform.json.
pub const DASHBOARD_DIR: &str = "dashboards";

/// Sentinels used to hide Grafana's `{{label}}` legend syntax from the
/// placeholder machinery. `\0` cannot occur in a `.tlisp` source, so the
/// mask is unambiguous.
const LEGEND_LB: &str = "\u{0}lava-lb\u{0}";
const LEGEND_RB: &str = "\u{0}lava-rb\u{0}";

/// Hide `{{…}}` so a param sharing a Grafana label's name is not
/// substituted inside a legend.
///
/// Without this, `{{namespace}}` binds to `{default}` — which surfaces as
/// the evaluator's `unknown var "default"` rather than as the broken
/// legend it actually is, sending the reader to the wrong file.
fn mask_legends(src: &str) -> String {
    src.replace("{{", LEGEND_LB).replace("}}", LEGEND_RB)
}

fn unmask_legends(src: &str) -> String {
    src.replace(LEGEND_LB, "{{").replace(LEGEND_RB, "}}")
}

/// Drop `;`-to-end-of-line comments, respecting string literals.
///
/// String-aware on purpose. lava's own lexer only treats `;` as a comment
/// outside a `"…"` literal (`lava_eval`'s `Parser::skip`), and a naive
/// stripper would truncate any description containing a semicolon. That
/// is not cosmetic here: [`required_dashboard_params`] reads the stripped
/// source, so a truncated description silently drops whatever
/// placeholders followed the `;` from the required set — and a required
/// param that the CLI never asks for is exactly the unbound-placeholder
/// leak the CLI exists to prevent, reintroduced by its own preflight.
fn strip_comments(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut chars = src.chars();
    let mut in_string = false;
    let mut escaped = false;
    while let Some(c) = chars.next() {
        if in_string {
            out.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
        } else if c == '"' {
            in_string = true;
            out.push(c);
        } else if c == ';' {
            // Consume to end of line, keeping the newline so line-based
            // reasoning about the stripped source still lines up.
            for c2 in chars.by_ref() {
                if c2 == '\n' {
                    out.push('\n');
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Collect every `{identifier}` span in already-legend-masked text.
///
/// The grammar is deliberately narrow — `[A-Za-z_][A-Za-z0-9_]*` — because
/// a brace in one of these documents is far more often NOT a placeholder:
/// `{namespace=\"camelot\"}` is a LogsQL stream selector and `up{job=…}`
/// is a PromQL matcher. Both are rejected by the grammar (they carry `=`
/// and quotes), so a scan for "any `{`" would report a leak on every
/// correctly-rendered board and get switched off within a week.
fn scan_placeholders(masked: &str, out: &mut BTreeSet<String>) {
    let b = masked.as_bytes();
    let mut i = 0;
    while i < b.len() {
        // ASCII `{` cannot appear inside a multi-byte UTF-8 sequence, and
        // the run below only advances over ASCII, so the slice at the end
        // is always on a char boundary.
        if b[i] == b'{' {
            let start = i + 1;
            let mut j = start;
            if j < b.len() && (b[j].is_ascii_alphabetic() || b[j] == b'_') {
                j += 1;
                while j < b.len() && (b[j].is_ascii_alphanumeric() || b[j] == b'_') {
                    j += 1;
                }
                if j < b.len() && b[j] == b'}' {
                    out.insert(masked[start..j].to_string());
                    i = j + 1;
                    continue;
                }
            }
        }
        i += 1;
    }
}

/// Every parameter a dashboard source requires, derived from the source
/// itself.
///
/// Derived, never hand-listed. A hand-list drifts the moment a param is
/// added — which already happened once in this crate, when the matrix's
/// leak check named five placeholders and went on passing after
/// `logs_datasource` became the sixth.
///
/// Comments are stripped first: `workload-overview.tlisp`'s header says
/// "bound as {placeholders}" in prose, and demanding a `placeholders`
/// param would make the preflight refuse a document that is entirely
/// correct.
#[must_use]
pub fn required_dashboard_params(src: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    scan_placeholders(&mask_legends(&strip_comments(src)), &mut out);
    out
}

/// Bind `{key}` placeholders in a dashboard source.
///
/// Scalar and textual, matching pangea-operator's `bind_params`. Textual
/// binding is why a param whose value can carry the delimiter of the
/// syntax it lands in — a double quote inside a tlisp string literal —
/// must not be a param at all; see `workload-overview.tlisp`'s header.
#[must_use]
pub fn bind_dashboard_params<K, V>(src: &str, params: &BTreeMap<K, V>) -> String
where
    K: AsRef<str>,
    V: AsRef<str>,
{
    let mut out = mask_legends(src);
    for (k, v) in params {
        let k = k.as_ref();
        let mut needle = String::with_capacity(k.len() + 2);
        needle.push('{');
        needle.push_str(k);
        needle.push('}');
        out = out.replace(&needle, v.as_ref());
    }
    unmask_legends(&out)
}

/// Placeholder spans still present in rendered output.
///
/// The failure this names is the quiet one: an unbound `{service}` renders
/// perfectly, validates, and ships a literal brace into a live dashboard
/// title. Legends are masked before the scan for the same reason the
/// binder masks them — a preserved `{{namespace}}` CONTAINS the literal
/// `{namespace}`, and reporting that as a leak would be reporting the one
/// case that was handled correctly.
#[must_use]
pub fn unbound_placeholders(rendered: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    scan_placeholders(&mask_legends(rendered), &mut out);
    out
}

#[cfg(test)]
mod dashboard_param_tests {
    use super::*;

    #[test]
    fn prose_braces_in_comments_are_not_required_params() {
        // The real line from workload-overview.tlisp's header.
        let src = ";; bound as {placeholders}\n(deflava-dashboard d :uid \"{env}\")";
        let req = required_dashboard_params(src);
        assert!(req.contains("env"));
        assert!(
            !req.contains("placeholders"),
            "a brace in prose became a required param: {req:?}"
        );
    }

    #[test]
    fn a_semicolon_inside_a_string_does_not_start_a_comment() {
        // Guards the string-awareness of strip_comments. A naive stripper
        // truncates at the `;` and loses `{job}` from the required set.
        let src = "(deflava-dashboard d :description \"a; b\" :uid \"{job}\")";
        assert!(
            required_dashboard_params(src).contains("job"),
            "a placeholder after an in-string semicolon was dropped"
        );
    }

    #[test]
    fn selectors_are_not_mistaken_for_placeholders() {
        // Both real: a LogsQL stream selector and a PromQL matcher.
        let src = r#"(:expr "{namespace=\"camelot\"} up{job=\"auth\"}")"#;
        assert!(
            required_dashboard_params(src).is_empty(),
            "a query selector was read as a placeholder"
        );
    }

    #[test]
    fn legends_survive_binding_and_are_not_reported_as_leaks() {
        let params = BTreeMap::from([("namespace", "camelot")]);
        let bound = bind_dashboard_params("{namespace} {{namespace}}", &params);
        assert_eq!(bound, "camelot {{namespace}}");
        assert!(
            unbound_placeholders(&bound).is_empty(),
            "a preserved Grafana legend was reported as an unbound placeholder"
        );
    }

    #[test]
    fn an_unbound_placeholder_is_named() {
        let params = BTreeMap::from([("env", "camelot")]);
        let bound = bind_dashboard_params("{env}-{service}", &params);
        let leaked = unbound_placeholders(&bound);
        assert_eq!(leaked.len(), 1);
        assert!(leaked.contains("service"), "got {leaked:?}");
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    /// End-to-end byte-equivalence test. Loads aws-vpc-network.tlisp,
    /// evaluates with default inputs, renders terraform.json, validates
    /// every field the pangea aws_vpc_network_spec asserts. Same state
    /// file emerges from `tofu apply` and `magma apply`.
    #[test]
    fn aws_vpc_network_tlisp_renders_byte_equivalent_to_pangea_spec() {
        let bindings = InputBindings::new();
        let arch = load_bundled("aws-vpc-network", &bindings).unwrap();
        let json = arch.render_terraform_json().unwrap();

        // VPC — pangea spec line 22-26.
        assert_eq!(json["resource"]["aws_vpc"]["main-vpc"]["cidr_block"], "10.0.0.0/16");
        assert_eq!(json["resource"]["aws_vpc"]["main-vpc"]["enable_dns_support"], true);
        assert_eq!(json["resource"]["aws_vpc"]["main-vpc"]["enable_dns_hostnames"], true);
        assert_eq!(json["resource"]["aws_vpc"]["main-vpc"]["tags"]["Name"], "main-vpc");
        assert_eq!(
            json["resource"]["aws_vpc"]["main-vpc"]["tags"]["Environment"],
            "production"
        );

        // IGW — pangea spec line 29-31.
        assert_eq!(
            json["resource"]["aws_internet_gateway"]["main-igw"]["vpc_id"],
            "${aws_vpc.main-vpc.id}"
        );

        // 3 public subnets — pangea spec line 33-40.
        for i in 0..3 {
            let name = format!("main-public-{i}");
            assert_eq!(
                json["resource"]["aws_subnet"][&name]["vpc_id"],
                "${aws_vpc.main-vpc.id}",
                "public subnet {i} vpc_id"
            );
            assert_eq!(
                json["resource"]["aws_subnet"][&name]["cidr_block"],
                format!("10.0.{i}.0/24"),
                "public subnet {i} cidr"
            );
            assert_eq!(
                json["resource"]["aws_subnet"][&name]["map_public_ip_on_launch"],
                true,
                "public subnet {i} map_public_ip"
            );
        }

        // 3 private subnets — pangea spec line 42-48.
        for i in 0..3 {
            let name = format!("main-private-{i}");
            assert_eq!(
                json["resource"]["aws_subnet"][&name]["vpc_id"],
                "${aws_vpc.main-vpc.id}",
                "private subnet {i} vpc_id"
            );
            assert_eq!(
                json["resource"]["aws_subnet"][&name]["cidr_block"],
                format!("10.0.{}.0/24", i + 10),
                "private subnet {i} cidr"
            );
        }

        // EIP for NAT — pangea spec line 50-53.
        assert_eq!(json["resource"]["aws_eip"]["main-nat-eip"]["domain"], "vpc");

        // NAT Gateway — pangea spec line 55-59.
        assert_eq!(
            json["resource"]["aws_nat_gateway"]["main-nat"]["subnet_id"],
            "${aws_subnet.main-public-0.id}"
        );
        assert_eq!(
            json["resource"]["aws_nat_gateway"]["main-nat"]["allocation_id"],
            "${aws_eip.main-nat-eip.allocation_id}"
        );

        // Default security group — pangea spec line 61-64.
        assert_eq!(
            json["resource"]["aws_security_group"]["main-default-sg"]["vpc_id"],
            "${aws_vpc.main-vpc.id}"
        );
    }

    /// Cloudflare DNS bundle renders all 5 records as terraform.json
    /// `resource.cloudflare_dns_record.*` entries with the expected
    /// proxied / type fields.
    #[test]
    fn cloudflare_dns_records_tlisp_renders_five_typed_records() {
        let bindings = InputBindings::new();
        let arch = load_bundled("cloudflare-dns-records", &bindings).unwrap();
        let json = arch.render_terraform_json().unwrap();

        let records = &json["resource"]["cloudflare_dns_record"];
        assert_eq!(records["root-cname"]["name"], "@");
        assert_eq!(records["root-cname"]["type"], "CNAME");
        assert_eq!(records["root-cname"]["proxied"], true);

        assert_eq!(records["www-cname"]["name"], "www");
        assert_eq!(records["api-cname"]["proxied"], true);
        assert_eq!(records["wildcard-staging-cname"]["name"], "*.staging");
        assert_eq!(records["wildcard-staging-cname"]["proxied"], false);

        assert_eq!(records["acme-challenge-txt"]["type"], "TXT");
        assert_eq!(records["acme-challenge-txt"]["name"], "_acme-challenge");
    }

    /// Akeyless secrets architecture emits all six resource types with
    /// the path-prefixed names pangea's akeyless_secrets_spec asserts.
    #[test]
    fn akeyless_secrets_tlisp_renders_full_resource_set() {
        let bindings = InputBindings::new();
        let arch = load_bundled("akeyless-secrets", &bindings).unwrap();
        let json = arch.render_terraform_json().unwrap();

        assert_eq!(
            json["resource"]["akeyless_auth_method_api_key"]["prod-ci-auth"]["name"],
            "/prod/ci-auth"
        );
        assert_eq!(
            json["resource"]["akeyless_auth_method_k8s"]["prod-k8s-auth"]["name"],
            "/prod/k8s-auth"
        );
        assert_eq!(
            json["resource"]["akeyless_static_secret"]["prod-db-password"]["name"],
            "/prod/database/password"
        );
        assert_eq!(
            json["resource"]["akeyless_static_secret"]["prod-api-key"]["name"],
            "/prod/service/api-key"
        );
        assert_eq!(
            json["resource"]["akeyless_target_db"]["prod-db-target"]["db_type"],
            "postgres"
        );
        assert_eq!(
            json["resource"]["akeyless_role"]["prod-app-role"]["name"],
            "/prod/roles/app"
        );
    }

    /// The hand-curated Interface registry rejects bad input *before*
    /// the architecture is evaluated — typed-gate at compose time.
    #[test]
    fn cloudflare_interface_rejects_missing_required_zone_id() {
        let iface = interface_for("cloudflare-dns-records").unwrap();
        let src = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join(ARCHITECTURE_DIR)
                .join("cloudflare-dns-records.tlisp"),
        )
        .unwrap();
        // Empty bindings — :zone-id is :required #t.
        let bindings = InputBindings::new();
        let err = eval_architecture_with_schema(&src, &bindings, &iface).unwrap_err();
        match err {
            EvalError::Schema {
                interface, errors, ..
            } => {
                assert_eq!(interface, "cloudflare-dns-records");
                assert!(errors.iter().any(|e| matches!(e, SchemaError::MissingRequired { name, .. } if name == "zone-id")));
            }
            other => panic!("expected EvalError::Schema, got {other:?}"),
        }
    }

    /// Akeyless interface accepts valid input + bag round-trips through
    /// evaluator + render.
    #[test]
    fn akeyless_interface_gates_valid_eval() {
        let iface = interface_for("akeyless-secrets").unwrap();
        let src = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join(ARCHITECTURE_DIR)
                .join("akeyless-secrets.tlisp"),
        )
        .unwrap();
        let mut bindings = InputBindings::new();
        bindings.set_str("name-prefix", "staging");
        let arch = eval_architecture_with_schema(&src, &bindings, &iface).unwrap();
        let json = arch.render_terraform_json().unwrap();
        assert_eq!(
            json["resource"]["akeyless_role"]["staging-app-role"]["name"],
            "/staging/roles/app"
        );
    }

    /// User-supplied input overrides flow through correctly: 2 AZs
    /// produce 2 public + 2 private subnets, not the default 3.
    #[test]
    fn user_inputs_override_defaults_in_tlisp_eval() {
        let mut b = InputBindings::new();
        b.set_str("name", "preview");
        b.set_list("availability-zones", vec!["us-west-2a".into(), "us-west-2b".into()]);
        let arch = load_bundled("aws-vpc-network", &b).unwrap();
        let json = arch.render_terraform_json().unwrap();

        // Name threads through.
        assert!(json["resource"]["aws_vpc"]["preview-vpc"].is_object());
        assert!(json["resource"]["aws_internet_gateway"]["preview-igw"].is_object());

        // 2 subnets only (matches override list length).
        assert!(json["resource"]["aws_subnet"]["preview-public-0"].is_object());
        assert!(json["resource"]["aws_subnet"]["preview-public-1"].is_object());
        assert!(json["resource"]["aws_subnet"]["preview-public-2"].is_null());

        // AZ value flowed through.
        assert_eq!(
            json["resource"]["aws_subnet"]["preview-public-0"]["availability_zone"],
            "us-west-2a"
        );
        assert_eq!(
            json["resource"]["aws_subnet"]["preview-public-1"]["availability_zone"],
            "us-west-2b"
        );
    }
}
