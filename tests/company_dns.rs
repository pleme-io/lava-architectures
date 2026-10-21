mod common {
    pub mod company_fixture;
}

use common::company_fixture as fx;
use lava_architectures::{ARCHITECTURE_DIR, Architecture, InputBindings, eval_architecture, interface_for};

fn render_with(b: &InputBindings) -> serde_json::Value {
    let src = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(ARCHITECTURE_DIR)
            .join("company-dns.tlisp"),
    )
    .expect("source");
    let arch: Architecture = eval_architecture(&src, b).expect("evaluates");
    arch.render_terraform_json().expect("renders")
}

fn render() -> serde_json::Value {
    render_with(&fx::example_dns())
}

fn names(json: &serde_json::Value, ty: &str) -> Vec<String> {
    let mut v: Vec<String> = json["resource"][ty]
        .as_object()
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default();
    v.sort();
    v
}

fn sorted(items: &[&str]) -> Vec<String> {
    let mut v: Vec<String> = items.iter().map(|s| (*s).to_string()).collect();
    v.sort();
    v
}

#[test]
fn counts_match_the_rows() {
    let json = render();
    assert_eq!(names(&json, "aws_route53_zone").len(), 2);
    assert_eq!(names(&json, "aws_route53_record").len(), 6);
    let total: usize = json["resource"]
        .as_object()
        .expect("resources")
        .values()
        .map(|m| m.as_object().expect("instances").len())
        .sum();
    assert_eq!(total, 8);
}

#[test]
fn addresses_come_from_the_key_and_slug_fields() {
    let json = render();
    assert_eq!(
        names(&json, "aws_route53_zone"),
        sorted(&["acme-example", "acme-labs-example"])
    );
    assert_eq!(
        names(&json, "aws_route53_record"),
        sorted(&["acme-mx", "acme-txt", "acme-dkim", "acme-dmarc", "acme-status", "labs-caa"])
    );
}

#[test]
fn every_cross_reference_resolves() {
    let json = render();
    let resources = json["resource"].as_object().expect("resources");
    let mut checked = 0usize;
    for (ty, instances) in resources {
        for (label, body) in instances.as_object().expect("instances") {
            for (attr, value) in body.as_object().expect("body") {
                let Some(s) = value.as_str() else { continue };
                let Some(inner) = s.strip_prefix("${").and_then(|r| r.strip_suffix('}')) else {
                    continue;
                };
                if inner.contains('(') {
                    continue;
                }
                let parts: Vec<&str> = inner.split('.').collect();
                assert_eq!(parts.len(), 3, "{ty}.{label}.{attr}: odd reference {s}");
                let target = &resources
                    .get(parts[0])
                    .unwrap_or_else(|| panic!("{ty}.{label}.{attr}: no resource type {}", parts[0]))[parts[1]];
                assert!(!target.is_null(), "{ty}.{label}.{attr}: dangling reference {s}");
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 6, "reference count drifted: {checked}");
}

#[test]
fn records_point_at_their_zone() {
    let json = render();
    let records = &json["resource"]["aws_route53_record"];
    assert_eq!(records["acme-mx"]["zone_id"], "${aws_route53_zone.acme-example.zone_id}");
    assert_eq!(records["labs-caa"]["zone_id"], "${aws_route53_zone.acme-labs-example.zone_id}");
}

#[test]
fn scalars_render_with_the_provider_types() {
    let json = render();
    let zones = &json["resource"]["aws_route53_zone"];
    assert_eq!(zones["acme-example"]["name"], fx::ZONE_PRIMARY);
    let records = &json["resource"]["aws_route53_record"];
    assert_eq!(records["acme-mx"]["type"], "MX");
    assert_eq!(records["acme-mx"]["ttl"], 3600);
    assert_eq!(
        records["acme-mx"]["records"],
        "${split(\"|\", \"10 mail.acme-mail.example|20 mail2.acme-mail.example\")}"
    );
    assert_eq!(
        records["acme-txt"]["records"],
        "${split(\"|\", \"v=spf1 include:_spf.acme-mail.example ~all|acme-site-verification=abc123\")}"
    );
    assert_eq!(
        records["acme-status"]["records"],
        "${split(\"|\", \"192.0.2.10\")}"
    );
    assert_eq!(records["acme-dkim"]["type"], "CNAME");
    assert_eq!(records["acme-dmarc"]["name"], "_dmarc.acme.example");
    assert_eq!(
        records["labs-caa"]["records"],
        "${split(\"|\", \"0 issue \\\"amazon.com\\\"\")}"
    );
    assert_eq!(records["acme-status"]["ttl"], 300);
}

#[test]
fn a_pending_row_is_never_emitted() {
    let json = render();
    let records = &json["resource"]["aws_route53_record"];
    assert!(records.get("acme-dkim2").is_none());
    assert!(!json.to_string().contains("PENDING-"));
}

#[test]
fn one_record_per_zone_name_and_type() {
    let json = render();
    let records = json["resource"]["aws_route53_record"].as_object().expect("records");
    let mut keys: Vec<String> = records
        .values()
        .map(|r| format!("{}|{}|{}", r["zone_id"], r["name"], r["type"]))
        .collect();
    let before = keys.len();
    keys.sort();
    keys.dedup();
    assert_eq!(keys.len(), before);
    assert_eq!(records["acme-mx"]["name"], records["acme-txt"]["name"]);
}

#[test]
fn the_default_provider_renders() {
    let json = render();
    assert_eq!(json["provider"]["aws"]["region"], "us-east-1");
}

#[test]
fn empty_lists_render_no_resources() {
    let json = render_with(&fx::dns_bindings(vec![], vec![]));
    let n = json["resource"].as_object().map_or(0, serde_json::Map::len);
    assert_eq!(n, 0);
}

#[test]
fn the_interface_is_registered() {
    let i = interface_for("company-dns").expect("interface");
    assert_eq!(i.name, "company-dns");
}
