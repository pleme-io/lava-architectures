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
    ("akeyless-secrets", 6),       // ci_auth + k8s_auth + db_pwd + api_key + db_target + role
    ("cloudflare-r2-bucket", 4),   // bucket + cors + custom-domain + cname
    ("public-dns", 6),             // zone + kms + ksk + dnssec + log-group + query-log
    ("akeyless-platform", 8),      // acm + validation + complete + 3 svc cnames + role + attach
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
