use std::collections::BTreeMap;

use lava_eval::InputBindings;

pub type Row = BTreeMap<String, String>;

pub const ZONE_PRIMARY: &str = "acme.example";
pub const ZONE_SECONDARY: &str = "acme-labs.example";
pub const ORIGIN_HOST: &str = "origin.acme.example";
pub const ORIGIN_PATH: &str = "/acme";
pub const HOSTS: &str = "acme.example,www.acme.example,acme-labs.example";

pub fn row(fields: &[(&str, &str)]) -> Row {
    fields
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect()
}

pub fn zones() -> Vec<Row> {
    vec![
        row(&[("key", "acme-example"), ("name", ZONE_PRIMARY)]),
        row(&[("key", "acme-labs-example"), ("name", ZONE_SECONDARY)]),
    ]
}

pub fn dns_records() -> Vec<Row> {
    let r = |slug: &str, zone: &str, name: &str, ty: &str, ttl: &str, values: &str, apply: &str| {
        row(&[
            ("slug", slug),
            ("zone", zone),
            ("name", name),
            ("type", ty),
            ("ttl", ttl),
            ("values", values),
            ("pending", if apply == "true" { "false" } else { "true" }),
            ("apply", apply),
        ])
    };
    vec![
        r("acme-mx", "acme-example", "acme.example", "MX", "3600", "10 mail.acme-mail.example|20 mail2.acme-mail.example", "true"),
        r("acme-txt", "acme-example", "acme.example", "TXT", "3600", "v=spf1 include:_spf.acme-mail.example ~all|acme-site-verification=abc123", "true"),
        r("acme-dkim", "acme-example", "sel1._domainkey.acme.example", "CNAME", "3600", "sel1.dkim.acme-mail.example", "true"),
        r("acme-dkim2", "acme-example", "sel2._domainkey.acme.example", "CNAME", "3600", "PENDING-acme-dkim2", "false"),
        r("acme-dmarc", "acme-example", "_dmarc.acme.example", "TXT", "3600", "v=DMARC1; p=quarantine; rua=mailto:dmarc@acme.example", "true"),
        r("acme-status", "acme-example", "status.acme.example", "A", "300", "192.0.2.10", "true"),
        r("labs-caa", "acme-labs-example", "acme-labs.example", "CAA", "3600", "0 issue \\\"amazon.com\\\"", "true"),
    ]
}

pub const API_HOST: &str = "api.acme.example";
pub const API_ORIGIN_HOST: &str = "api-origin.acme.example";
pub const API_ORIGIN_PATH: &str = "/v1";

pub const CACHING_OPTIMIZED: &str = "658327ea-f89d-4fab-a63d-7e88639e58f6";
pub const CACHING_DISABLED: &str = "4135ea2d-6df8-44a3-9df3-4b5a84be39ad";
pub const ALL_VIEWER_EXCEPT_HOST: &str = "b689b0a8-53d0-40ab-baf2-68738e2966ac";

pub fn edge_sites() -> Vec<Row> {
    vec![
        row(&[
            ("site", "acme"),
            ("origin_host", ORIGIN_HOST),
            ("origin_path", ORIGIN_PATH),
            ("hosts", HOSTS),
            ("kind", "static"),
            ("cache", "optimized"),
            ("all_methods", "false"),
            ("origin_request", "none"),
            (
                "validation_fqdns",
                "aws_route53_record.acme-apex-validation.fqdn,aws_route53_record.acme-www-validation.fqdn,aws_route53_record.acme-labs-validation.fqdn",
            ),
        ]),
        row(&[
            ("site", "acme-api"),
            ("origin_host", API_ORIGIN_HOST),
            ("origin_path", API_ORIGIN_PATH),
            ("hosts", API_HOST),
            ("kind", "service"),
            ("cache", "disabled"),
            ("all_methods", "true"),
            ("origin_request", "all_viewer_except_host"),
            ("validation_fqdns", "aws_route53_record.acme-api-validation.fqdn"),
        ]),
    ]
}

pub fn edge_aliases() -> Vec<Row> {
    vec![
        row(&[("slug", "acme-apex"), ("site", "acme"), ("host", "acme.example"), ("zone", ZONE_PRIMARY)]),
        row(&[("slug", "acme-www"), ("site", "acme"), ("host", "www.acme.example"), ("zone", ZONE_PRIMARY)]),
        row(&[("slug", "acme-labs"), ("site", "acme"), ("host", "acme-labs.example"), ("zone", ZONE_SECONDARY)]),
        row(&[("slug", "acme-api"), ("site", "acme-api"), ("host", API_HOST), ("zone", ZONE_PRIMARY)]),
    ]
}

pub fn dns_bindings(zones: Vec<Row>, records: Vec<Row>) -> InputBindings {
    let mut b = InputBindings::new();
    b.set_records("zones", zones);
    b.set_records("dns-records", records);
    b
}

pub fn edge_bindings(sites: Vec<Row>, aliases: Vec<Row>) -> InputBindings {
    let mut b = InputBindings::new();
    b.set_records("edge-sites", sites);
    b.set_records("edge-aliases", aliases);
    b
}

pub fn example_dns() -> InputBindings {
    dns_bindings(zones(), dns_records())
}

pub fn example_edge() -> InputBindings {
    edge_bindings(edge_sites(), edge_aliases())
}
