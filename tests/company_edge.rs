mod common {
    pub mod company_fixture;
}

use common::company_fixture as fx;
use lava_architectures::{ARCHITECTURE_DIR, Architecture, InputBindings, eval_architecture, interface_for};

fn source() -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(ARCHITECTURE_DIR)
            .join("company-edge.tlisp"),
    )
    .expect("source")
}

fn render_src(src: &str, b: &InputBindings) -> serde_json::Value {
    let arch: Architecture = eval_architecture(src, b).expect("evaluates");
    arch.render_terraform_json().expect("renders")
}

fn render_with(b: &InputBindings) -> serde_json::Value {
    render_src(&source(), b)
}

fn render() -> serde_json::Value {
    render_with(&fx::example_edge())
}

fn names(json: &serde_json::Value, ty: &str) -> Vec<String> {
    let mut v: Vec<String> = json["resource"][ty]
        .as_object()
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default();
    v.sort();
    v
}

fn data_names(json: &serde_json::Value, ty: &str) -> Vec<String> {
    let mut v: Vec<String> = json["data"][ty]
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
    assert_eq!(names(&json, "aws_acm_certificate").len(), 2);
    assert_eq!(names(&json, "aws_acm_certificate_validation").len(), 2);
    assert_eq!(names(&json, "aws_cloudfront_distribution").len(), 2);
    assert_eq!(names(&json, "aws_route53_record").len(), 12);
    assert_eq!(data_names(&json, "aws_route53_zone").len(), 4);
    let total: usize = json["resource"]
        .as_object()
        .expect("resources")
        .values()
        .map(|m| m.as_object().expect("instances").len())
        .sum();
    assert_eq!(total, 18);
}

#[test]
fn addresses_come_from_the_site_and_slug_fields() {
    let json = render();
    assert_eq!(names(&json, "aws_acm_certificate"), sorted(&["acme", "acme-api"]));
    assert_eq!(names(&json, "aws_cloudfront_distribution"), sorted(&["acme", "acme-api"]));
    assert_eq!(
        names(&json, "aws_route53_record"),
        sorted(&[
            "acme-apex-validation",
            "acme-www-validation",
            "acme-labs-validation",
            "acme-api-validation",
            "acme-apex-a",
            "acme-www-a",
            "acme-labs-a",
            "acme-api-a",
            "acme-apex-aaaa",
            "acme-www-aaaa",
            "acme-labs-aaaa",
            "acme-api-aaaa",
        ])
    );
    assert_eq!(
        data_names(&json, "aws_route53_zone"),
        sorted(&["acme-apex", "acme-www", "acme-labs", "acme-api"])
    );
}

fn collect_expressions(value: &serde_json::Value, out: &mut Vec<String>) {
    match value {
        serde_json::Value::String(s) => {
            let mut rest = s.as_str();
            while let Some(start) = rest.find("${") {
                let after = &rest[start + 2..];
                let mut depth = 1usize;
                let mut end = None;
                for (i, c) in after.char_indices() {
                    match c {
                        '{' => depth += 1,
                        '}' => {
                            depth -= 1;
                            if depth == 0 {
                                end = Some(i);
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                let end = end.unwrap_or_else(|| panic!("unclosed expression in {s}"));
                out.push(after[..end].to_string());
                rest = &after[end + 1..];
            }
        }
        serde_json::Value::Array(items) => items.iter().for_each(|v| collect_expressions(v, out)),
        serde_json::Value::Object(m) => m.values().for_each(|v| collect_expressions(v, out)),
        _ => {}
    }
}

fn ident_after<'a>(expr: &'a str, prefix: &str) -> Vec<&'a str> {
    let mut found = Vec::new();
    let mut from = 0;
    while let Some(off) = expr[from..].find(prefix) {
        let at = from + off;
        let before_ok = at == 0
            || !expr[..at]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.');
        let tail = &expr[at..];
        let len = tail
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.'))
            .unwrap_or(tail.len());
        if before_ok {
            found.push(&tail[..len]);
        }
        from = at + len.max(1);
    }
    found
}

fn dangling_references(json: &serde_json::Value) -> Vec<String> {
    let mut exprs = Vec::new();
    collect_expressions(json, &mut exprs);
    let mut bad = Vec::new();
    for expr in &exprs {
        for root in ident_after(expr, "data.aws_") {
            let p: Vec<&str> = root.split('.').collect();
            if json["data"][p[1]][p[2]].is_null() {
                bad.push(format!("data.{}.{}", p[1], p[2]));
            }
        }
        for root in ident_after(expr, "aws_") {
            let p: Vec<&str> = root.split('.').collect();
            if p.len() >= 2 && json["resource"][p[0]][p[1]].is_null() {
                bad.push(format!("{}.{}", p[0], p[1]));
            }
        }
        for root in ident_after(expr, "local.") {
            let name = root.trim_start_matches("local.");
            if json["locals"][name].is_null() {
                bad.push(format!("local.{name}"));
            }
        }
    }
    bad
}

fn reference_count(json: &serde_json::Value) -> usize {
    let mut exprs = Vec::new();
    collect_expressions(json, &mut exprs);
    exprs
        .iter()
        .map(|e| ident_after(e, "data.aws_").len() + ident_after(e, "aws_").len() + ident_after(e, "local.").len())
        .sum()
}

#[test]
fn every_cross_reference_resolves() {
    let json = render();
    assert_eq!(dangling_references(&json), Vec::<String>::new());
    assert_eq!(reference_count(&json), 54, "reference count drifted");
}

#[test]
fn a_broken_reference_is_caught() {
    let broken = source().replace(
        "(ref aws-acm-certificate-validation \"{site_site}\" certificate_arn)",
        "(ref aws-acm-certificate-validation \"{site_site}-gone\" certificate_arn)",
    );
    assert_ne!(broken, source(), "the replacement must hit");
    let json = render_src(&broken, &fx::example_edge());
    let bad = dangling_references(&json);
    assert!(
        bad.iter().any(|b| b == "aws_acm_certificate_validation.acme-gone"),
        "dangling reference not detected: {bad:?}"
    );
}

#[test]
fn the_certificate_is_in_us_east_1_through_the_aliased_provider() {
    let json = render();
    let providers = json["provider"]["aws"].as_array().expect("two aws configs");
    assert_eq!(providers.len(), 2);
    assert_eq!(providers[1]["alias"], "us_east_1");
    assert_eq!(providers[1]["region"], "us-east-1");
    for site in ["acme", "acme-api"] {
        assert_eq!(json["resource"]["aws_acm_certificate"][site]["provider"], "aws.us_east_1");
        assert_eq!(
            json["resource"]["aws_acm_certificate_validation"][site]["provider"],
            "aws.us_east_1"
        );
        assert_eq!(json["resource"]["aws_acm_certificate"][site]["validation_method"], "DNS");
    }
}

#[test]
fn raw_terraform_expressions_pass_through_with_fields_substituted() {
    let json = render();
    let rec = &json["resource"]["aws_route53_record"]["acme-www-validation"];
    let lookup = "{for dvo in aws_acm_certificate.acme.domain_validation_options : dvo.domain_name => dvo}[\"www.acme.example\"]";
    assert_eq!(rec["name"], format!("${{{lookup}.resource_record_name}}"));
    assert_eq!(rec["type"], format!("${{{lookup}.resource_record_type}}"));
    assert_eq!(
        rec["records"],
        serde_json::json!([format!("${{{lookup}.resource_record_value}}")])
    );
    assert_eq!(rec["zone_id"], "${data.aws_route53_zone.acme-www.zone_id}");
    assert_eq!(rec["ttl"], 60);
    assert_eq!(rec["allow_overwrite"], true);
    let api = &json["resource"]["aws_route53_record"]["acme-api-validation"];
    assert!(api["name"].as_str().expect("name").contains("aws_acm_certificate.acme-api."));
    assert!(api["name"].as_str().expect("name").contains("[\"api.acme.example\"]"));
    let cert = &json["resource"]["aws_acm_certificate"]["acme"];
    assert_eq!(cert["domain_name"], "${split(\",\", \"acme.example,www.acme.example,acme-labs.example\")[0]}");
    assert_eq!(
        cert["subject_alternative_names"],
        "${split(\",\", \"acme.example,www.acme.example,acme-labs.example\")}"
    );
}

#[test]
fn each_distribution_has_one_https_origin_with_its_input_path() {
    let json = render();
    let d = &json["resource"]["aws_cloudfront_distribution"];
    for (site, host, path) in [
        ("acme", fx::ORIGIN_HOST, fx::ORIGIN_PATH),
        ("acme-api", fx::API_ORIGIN_HOST, fx::API_ORIGIN_PATH),
    ] {
        let s = &d[site];
        assert_eq!(s["origin"]["domain_name"], host);
        assert_eq!(s["origin"]["origin_path"], path);
        assert_eq!(s["origin"]["origin_id"], site);
        assert_eq!(s["origin"]["custom_origin_config"]["origin_protocol_policy"], "https-only");
        assert_eq!(s["origin"]["custom_origin_config"]["https_port"], 443);
        assert_eq!(s["default_cache_behavior"]["target_origin_id"], site);
        assert_eq!(s["default_cache_behavior"]["viewer_protocol_policy"], "redirect-to-https");
        assert_eq!(s["is_ipv6_enabled"], true);
        assert_eq!(s["enabled"], true);
    }
    assert_eq!(
        d["acme"]["aliases"],
        "${split(\",\", \"acme.example,www.acme.example,acme-labs.example\")}"
    );
    assert_eq!(d["acme-api"]["aliases"], "${split(\",\", \"api.acme.example\")}");
    assert_eq!(d["acme"]["comment"], "acme static");
    assert_eq!(d["acme-api"]["comment"], "acme-api service");
}

fn resolve_local(json: &serde_json::Value, expr: &str) -> Option<serde_json::Value> {
    if let Some(rest) = expr.strip_prefix("lookup(local.") {
        let (name, tail) = rest.split_once(',')?;
        let key = tail.split('"').nth(1)?;
        return json["locals"][name].get(key).cloned();
    }
    let rest = expr.strip_prefix("local.")?;
    let (name, tail) = rest.split_once("[\"")?;
    let key = tail.strip_suffix("\"]")?;
    json["locals"][name].get(key).cloned()
}

fn behaviour_value(json: &serde_json::Value, site: &str, attr: &str) -> Option<serde_json::Value> {
    let raw = json["resource"]["aws_cloudfront_distribution"][site]["default_cache_behavior"][attr]
        .as_str()?;
    let expr = raw.strip_prefix("${")?.strip_suffix('}')?;
    resolve_local(json, expr)
}

#[test]
fn locals_hold_the_aws_managed_policy_ids() {
    let json = render();
    assert_eq!(json["locals"]["cache_policy"]["optimized"], fx::CACHING_OPTIMIZED);
    assert_eq!(json["locals"]["cache_policy"]["disabled"], fx::CACHING_DISABLED);
    assert_eq!(
        json["locals"]["origin_request_policy"]["all_viewer_except_host"],
        fx::ALL_VIEWER_EXCEPT_HOST
    );
}

#[test]
fn the_static_site_caches_and_allows_get_head_options() {
    let json = render();
    let b = &json["resource"]["aws_cloudfront_distribution"]["acme"]["default_cache_behavior"];
    assert_eq!(b["cache_policy_id"], "${local.cache_policy[\"optimized\"]}");
    assert_eq!(
        behaviour_value(&json, "acme", "cache_policy_id"),
        Some(serde_json::json!(fx::CACHING_OPTIMIZED))
    );
    assert_eq!(
        behaviour_value(&json, "acme", "allowed_methods"),
        Some(serde_json::json!(["GET", "HEAD", "OPTIONS"]))
    );
    assert_eq!(
        b["origin_request_policy_id"],
        "${lookup(local.origin_request_policy, \"none\", null)}"
    );
    assert_eq!(resolve_local(&json, "lookup(local.origin_request_policy, \"none\", null)"), None);
    assert_eq!(b["cached_methods"], serde_json::json!(["GET", "HEAD"]));
    assert_eq!(b["compress"], true);
}

#[test]
fn the_service_site_disables_caching_allows_all_methods_and_drops_the_host_header() {
    let json = render();
    let b = &json["resource"]["aws_cloudfront_distribution"]["acme-api"]["default_cache_behavior"];
    assert_eq!(b["cache_policy_id"], "${local.cache_policy[\"disabled\"]}");
    assert_eq!(
        behaviour_value(&json, "acme-api", "cache_policy_id"),
        Some(serde_json::json!(fx::CACHING_DISABLED))
    );
    assert_eq!(
        behaviour_value(&json, "acme-api", "allowed_methods"),
        Some(serde_json::json!(["GET", "HEAD", "OPTIONS", "PUT", "POST", "PATCH", "DELETE"]))
    );
    assert_eq!(
        behaviour_value(&json, "acme-api", "origin_request_policy_id"),
        Some(serde_json::json!(fx::ALL_VIEWER_EXCEPT_HOST))
    );
    assert_eq!(b["cached_methods"], serde_json::json!(["GET", "HEAD"]));
}

#[test]
fn the_viewer_certificate_comes_from_the_validated_acm_certificate() {
    let json = render();
    for site in ["acme", "acme-api"] {
        let v = &json["resource"]["aws_cloudfront_distribution"][site]["viewer_certificate"];
        assert_eq!(
            v["acm_certificate_arn"],
            format!("${{aws_acm_certificate_validation.{site}.certificate_arn}}")
        );
        assert_eq!(v["ssl_support_method"], "sni-only");
        assert_eq!(
            json["resource"]["aws_acm_certificate_validation"][site]["certificate_arn"],
            format!("${{aws_acm_certificate.{site}.arn}}")
        );
    }
}

#[test]
fn every_host_has_a_and_aaaa_alias_records_including_the_apex() {
    let json = render();
    let records = &json["resource"]["aws_route53_record"];
    for (slug, site, host) in [
        ("acme-apex", "acme", "acme.example"),
        ("acme-www", "acme", "www.acme.example"),
        ("acme-labs", "acme", "acme-labs.example"),
        ("acme-api", "acme-api", "api.acme.example"),
    ] {
        for (suffix, ty) in [("a", "A"), ("aaaa", "AAAA")] {
            let r = &records[format!("{slug}-{suffix}")];
            assert_eq!(r["name"], host);
            assert_eq!(r["type"], ty);
            assert_eq!(r["alias"]["name"], format!("${{aws_cloudfront_distribution.{site}.domain_name}}"));
            assert_eq!(
                r["alias"]["zone_id"],
                format!("${{aws_cloudfront_distribution.{site}.hosted_zone_id}}")
            );
            assert_eq!(r["alias"]["evaluate_target_health"], false);
            assert_eq!(r["zone_id"], format!("${{data.aws_route53_zone.{slug}.zone_id}}"));
        }
    }
    assert_eq!(records["acme-apex-a"]["name"], fx::ZONE_PRIMARY);
}

#[test]
fn each_zone_lookup_names_the_zone_of_its_host() {
    let json = render();
    let z = &json["data"]["aws_route53_zone"];
    assert_eq!(z["acme-apex"]["name"], fx::ZONE_PRIMARY);
    assert_eq!(z["acme-labs"]["name"], fx::ZONE_SECONDARY);
    assert_eq!(z["acme-api"]["name"], fx::ZONE_PRIMARY);
    assert_eq!(z["acme-www"]["private_zone"], false);
}

#[test]
fn the_host_lists_cover_every_alias_row() {
    let sites = fx::edge_sites();
    for a in fx::edge_aliases() {
        let site = sites.iter().find(|s| s["site"] == a["site"]).expect("site row");
        assert!(
            site["hosts"].split(',').any(|h| h == a["host"]),
            "{} not in the host list of {}",
            a["host"],
            a["site"]
        );
    }
    let listed: usize = sites.iter().map(|s| s["hosts"].split(',').count()).sum();
    assert_eq!(listed, fx::edge_aliases().len());
}

#[test]
fn empty_lists_render_no_resources() {
    let json = render_with(&fx::edge_bindings(vec![], vec![]));
    let n = json["resource"].as_object().map_or(0, serde_json::Map::len);
    assert_eq!(n, 0);
}

#[test]
fn the_interface_is_registered() {
    let i = interface_for("company-edge").expect("interface");
    assert_eq!(i.name, "company-edge");
}

#[test]
fn the_certificate_validation_waits_for_its_validation_records() {
    let json = render();
    let v = &json["resource"]["aws_acm_certificate_validation"];
    assert_eq!(
        v["acme"]["validation_record_fqdns"],
        "${[aws_route53_record.acme-apex-validation.fqdn,aws_route53_record.acme-www-validation.fqdn,aws_route53_record.acme-labs-validation.fqdn]}"
    );
    assert_eq!(
        v["acme-api"]["validation_record_fqdns"],
        "${[aws_route53_record.acme-api-validation.fqdn]}"
    );
    assert_eq!(dangling_references(&json), Vec::<String>::new());
}

#[test]
fn a_validation_fqdn_naming_no_record_is_caught() {
    let mut sites = fx::edge_sites();
    sites[1].insert(
        "validation_fqdns".to_string(),
        "aws_route53_record.acme-api-gone.fqdn".to_string(),
    );
    let json = render_with(&fx::edge_bindings(sites, fx::edge_aliases()));
    assert!(dangling_references(&json).iter().any(|b| b == "aws_route53_record.acme-api-gone"));
}
