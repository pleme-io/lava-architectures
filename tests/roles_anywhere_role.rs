use lava_architectures::{ARCHITECTURE_DIR, Architecture, InputBindings, eval_architecture};

const PEM: &str = "-----BEGIN CERTIFICATE-----\nLOCK\n-----END CERTIFICATE-----\n";
const TRUST: &str = r#"{"Version":"2012-10-17","Statement":[{"Effect":"Allow","Principal":{"Service":"rolesanywhere.amazonaws.com"},"Action":["sts:AssumeRole","sts:TagSession","sts:SetSourceIdentity"],"Condition":{"StringEquals":{"aws:PrincipalTag/x509Subject/CN":"op.node.pleme.internal"}}}]}"#;
const POLICY: &str = r#"{"Version":"2012-10-17","Statement":[{"Sid":"All","Effect":"Allow","Action":"*","Resource":"*"}]}"#;

fn render() -> serde_json::Value {
    let src = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(ARCHITECTURE_DIR)
            .join("roles-anywhere-role.tlisp"),
    )
    .expect("source");
    let mut b = InputBindings::new();
    for (k, v) in [
        ("name", "op"),
        ("role_name", "op-role"),
        ("ca_cert_pem", PEM),
        ("trust_policy_json", TRUST),
        ("policy_json", POLICY),
    ] {
        b.set_str(k, v);
    }
    let arch: Architecture = eval_architecture(&src, &b).expect("evaluates");
    arch.render_terraform_json().expect("renders")
}

#[test]
fn the_trust_anchor_carries_the_ca_bundle_as_a_nested_source_block() {
    let json = render();
    let anchor = &json["resource"]["aws_rolesanywhere_trust_anchor"]["op-anchor"];
    assert_eq!(anchor["enabled"], true);
    assert_eq!(anchor["source"]["source_type"], "CERTIFICATE_BUNDLE");
    assert_eq!(anchor["source"]["source_data"]["x509_certificate_data"], PEM);
}

#[test]
fn the_iam_json_inputs_pass_through_byte_for_byte() {
    let json = render();
    let role = &json["resource"]["aws_iam_role"]["op-role"];
    assert_eq!(role["name"], "op-role");
    assert_eq!(role["assume_role_policy"], TRUST);
    assert_eq!(role["max_session_duration"], 3600);
    assert_eq!(json["resource"]["aws_iam_policy"]["op-policy"]["policy"], POLICY);
}

#[test]
fn the_attachment_and_profile_reference_the_role_and_policy() {
    let json = render();
    let att = &json["resource"]["aws_iam_role_policy_attachment"]["op-policy-attachment"];
    assert_eq!(att["role"], "${aws_iam_role.op-role.name}");
    assert_eq!(att["policy_arn"], "${aws_iam_policy.op-policy.arn}");
    let profile = &json["resource"]["aws_rolesanywhere_profile"]["op-profile"];
    assert_eq!(profile["role_arns"], serde_json::json!(["${aws_iam_role.op-role.arn}"]));
    assert_eq!(profile["duration_seconds"], 3600);
    assert_eq!(profile["enabled"], true);
}
