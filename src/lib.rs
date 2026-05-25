//! lava-architectures — reusable infrastructure compositions for the
//! lava suite. Pangea-architectures analog.
//!
//! Every architecture is authored as a `.tlisp` source file in
//! `architectures/`. The Rust side is a typed in-memory interpreter
//! (sexpr parser + evaluator) that magma consumes directly — no
//! intermediate JSON file written to disk.
//!
//! ## Pipeline
//!
//! ```text
//! architectures/<name>.tlisp        ← author here
//!         │
//!         ▼  sexpr::parse → eval::eval_architecture
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

pub mod eval;
pub mod sexpr;

pub use eval::{eval_architecture, EvalError, InputBindings};
pub use sexpr::{parse, Atom, Sx};

/// Built-in path for the bundled `.tlisp` architectures. Magma reads
/// these at runtime; users can also load their own architectures from
/// any path via `eval_architecture(&fs::read_to_string(path)?, ...)`.
pub const ARCHITECTURE_DIR: &str = "architectures";

/// Convenience: load + evaluate one of the bundled architectures by
/// name. Looks up `architectures/<name>.tlisp` relative to CARGO_MANIFEST_DIR.
pub fn load_bundled(
    name: &str,
    bindings: &InputBindings,
) -> Result<lava_core::Architecture, EvalError> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(ARCHITECTURE_DIR)
        .join(format!("{name}.tlisp"));
    let src = std::fs::read_to_string(&path).map_err(|e| {
        EvalError::NotArchForm(format!("io: {} — {e}", path.display()))
    })?;
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
