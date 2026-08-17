//! Verification matrix for the dashboard catalogue.
//!
//! Sibling of `architecture_matrix.rs`, and the same discipline: every
//! catalogue entry is exercised, and the build fails when one lands
//! without a row. These live in `dashboards/` rather than
//! `architectures/` because a `(deflava-dashboard …)` renders a Grafana
//! document, not terraform.json — feeding one to the architecture matrix
//! would fail on a shape mismatch that says nothing useful.
//!
//! Aggregate failure reporting: every broken entry is collected and
//! reported in one assert, so CI shows the whole picture rather than
//! first-failure-wins.

use std::collections::BTreeMap;

use lava_core::Theme;
use lava_eval::render_dashboard_grafana_json;

/// Directory holding the catalogue, relative to the crate root.
const DASHBOARD_DIR: &str = "dashboards";

/// Every catalogue entry, with the parameters it requires.
///
/// A hand-maintained list is the point: an entry added without a row
/// here is invisible to the matrix, and `every_catalogue_file_has_a_row`
/// below is what makes that impossible rather than merely discouraged.
fn catalogue() -> Vec<(&'static str, BTreeMap<&'static str, &'static str>)> {
    vec![
        (
            "workload-overview",
            BTreeMap::from([
                ("env", "camelot"),
                ("service", "auth"),
                ("job", "auth"),
                ("namespace", "default"),
                ("datasource", "mimir"),
                ("logs_datasource", "vlogs"),
            ]),
        ),
        (
            "homeostasis-control",
            BTreeMap::from([("env", "camelot"), ("board", "homeostasis-control"), ("datasource", "mimir")]),
        ),
        (
            "nervous-system-self-health",
            BTreeMap::from([
                ("env", "camelot"),
                ("board", "nervous-system-self-health"),
                ("namespace", "default"),
                ("datasource", "mimir"),
            ]),
        ),
        (
            "audit-explorer",
            BTreeMap::from([
                ("env", "camelot"),
                ("board", "audit-explorer"),
                ("namespace", "default"),
                ("datasource", "vlogs"),
            ]),
        ),
        (
            "log-explorer",
            BTreeMap::from([
                ("env", "camelot"),
                ("board", "log-explorer"),
                ("namespace", "default"),
                ("logs_datasource", "vlogs"),
            ]),
        ),
        // The fold of audit-explorer + log-explorer. Binds BOTH
        // datasources on purpose: `declared_datasources_and_query_
        // references_agree` below is what proves every LogsQL panel
        // resolves to `vlogs` and every PromQL panel to `mimir`, which is
        // exactly the defect (GAPS.md D1) the fold exists to fix — the
        // audit board declared the logs plugin and bound the metrics uid.
        // A row binding only one datasource could not have caught it.
        (
            "audit-logs-overview",
            BTreeMap::from([
                ("env", "camelot"),
                ("namespace", "default"),
                ("datasource", "mimir"),
                ("logs_datasource", "vlogs"),
            ]),
        ),
        (
            "services-overview",
            BTreeMap::from([("namespace", "default"), ("datasource", "mimir")]),
        ),
        // service-detail is the DUAL-DATASOURCE board (metrics + logs), so it
        // is the one this matrix must carry to keep the datasource-agreement
        // check honest — that is the defect audit-explorer shipped as D1.
        (
            "service-detail",
            BTreeMap::from([
                ("namespace", "default"),
                ("datasource", "mimir"),
                ("logs_datasource", "vlogs"),
            ]),
        ),
    ]
}

fn dashboards_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(DASHBOARD_DIR)
}

/// Bind `{key}` placeholders — the same scalar substitution the operator's
/// lava backend performs, reproduced here so the test exercises the real
/// path rather than a pre-substituted fixture.
///
/// The binder itself moved into the library
/// (`lava_architectures::bind_dashboard_params`) when `lava-render` needed
/// the same substitution. Two textual binders are two binders free to
/// disagree, and the disagreement would surface as this matrix passing on
/// a document the CLI does not produce.
///
/// What this test was written to catch is unchanged. The implementation it
/// mirrors — pangea-operator's Ruby `bind_params`, INCLUDING its `{{…}}`
/// masking — is external to this crate, so it is still an independent
/// implementation to diverge from. A Grafana legend is `{{label}}`, and a
/// param sharing a label's name must not be substituted inside one: naive
/// replacement turns `{{namespace}}` into `{default}`, which surfaces as
/// the evaluator's `unknown var "default"` rather than as a broken legend.
/// It caught exactly this one.
fn bind(src: &str, params: &BTreeMap<&str, &str>) -> String {
    lava_architectures::bind_dashboard_params(src, params)
}

#[test]
fn every_catalogue_entry_renders_to_grafana_json() {
    let dir = dashboards_dir();
    let mut failures: Vec<String> = Vec::new();

    for (name, params) in catalogue() {
        let path = dir.join(format!("{name}.tlisp"));
        let Ok(raw) = std::fs::read_to_string(&path) else {
            failures.push(format!("{name}: missing at {}", path.display()));
            continue;
        };
        let src = bind(&raw, &params);
        match render_dashboard_grafana_json(&src, &Theme::tundra()) {
            Ok(json) => {
                if json["schemaVersion"] != 39 {
                    failures.push(format!("{name}: schemaVersion is {}", json["schemaVersion"]));
                }
                if json["uid"].as_str().unwrap_or_default().is_empty() {
                    failures.push(format!("{name}: rendered without a uid"));
                }
                // An unbound placeholder is the failure this catches: it
                // renders perfectly and ships a literal `{service}` into
                // a live dashboard title.
                //
                // DERIVED from the row's own params, never hand-listed.
                // A hand-list drifts the moment a param is added: this
                // check named five placeholders and went on passing when
                // `logs_datasource` became the sixth, which is the exact
                // failure the check exists to prevent, one level up.
                // Mask `{{…}}` before scanning, for the same reason the
                // binder does: a preserved Grafana legend `{{namespace}}`
                // CONTAINS the literal `{namespace}`, so an unmasked scan
                // reports a leak for a placeholder that was correctly left
                // alone. The check and the binder have to agree on what a
                // placeholder is, or one of them is always wrong.
                let text = json
                    .to_string()
                    .replace("{{", "\u{0}L\u{0}")
                    .replace("}}", "\u{0}R\u{0}");
                let leaked: Vec<&str> = params
                    .keys()
                    .copied()
                    .filter(|k| text.contains(&format!("{{{k}}}")))
                    .collect();
                if !leaked.is_empty() {
                    failures.push(format!(
                        "{name}: unbound placeholder(s) reached the output: {leaked:?}"
                    ));
                }
            }
            Err(e) => failures.push(format!("{name}: {e}")),
        }
    }

    assert!(
        failures.is_empty(),
        "{} catalogue entr{} failed:\n  {}",
        failures.len(),
        if failures.len() == 1 { "y" } else { "ies" },
        failures.join("\n  ")
    );
}

/// The matrix must cover the directory. Without this, adding a `.tlisp`
/// and forgetting the row leaves it untested while the suite stays green.
#[test]
fn every_catalogue_file_has_a_row() {
    let dir = dashboards_dir();
    let on_disk: Vec<String> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("read {}: {e}", dir.display()))
        .filter_map(Result::ok)
        .filter_map(|e| {
            let p = e.path();
            (p.extension()?.to_str()? == "tlisp")
                .then(|| p.file_stem()?.to_str().map(ToString::to_string))?
        })
        .collect();

    assert!(
        !on_disk.is_empty(),
        "no .tlisp files in {} — the matrix would pass vacuously",
        dir.display()
    );

    let rows: Vec<&str> = catalogue().into_iter().map(|(n, _)| n).collect();
    let missing: Vec<&String> = on_disk.iter().filter(|f| !rows.contains(&f.as_str())).collect();
    assert!(
        missing.is_empty(),
        "catalogue files with no matrix row: {missing:?} (add them to catalogue() in this file)"
    );
}

/// Every datasource a board declares must be used by some panel query,
/// and every query must name a datasource the board declared.
///
/// This is the well-formedness half of the quote-injection hazard
/// documented in `workload-overview.tlisp`. Scalar param binding is
/// TEXTUAL: a value carrying the delimiter of the syntax it lands in —
/// a double quote inside a tlisp string literal — silently ends the
/// literal early, and what comes out is a different document that still
/// parses. The visible symptom is a query pointing at a datasource that
/// was never declared, or a declared datasource nothing references.
///
/// It is a real hazard, not a hypothetical: camelot's chart passes
/// `logsStream: '{namespace="camelot"}'`, and binding that as a param
/// would have done exactly this.
#[test]
fn declared_datasources_and_query_references_agree() {
    let dir = dashboards_dir();
    for (name, params) in catalogue() {
        let raw = std::fs::read_to_string(dir.join(format!("{name}.tlisp"))).unwrap();
        let json = render_dashboard_grafana_json(&bind(&raw, &params), &Theme::tundra())
            .unwrap_or_else(|e| panic!("{name}: {e}"));

        // Grafana nests a collapsed row's children under `row.panels`, so
        // walk instead of assuming one flat list.
        let mut used: std::collections::BTreeSet<String> = Default::default();
        let mut queries = 0usize;
        fn walk(
            v: &serde_json::Value,
            used: &mut std::collections::BTreeSet<String>,
            queries: &mut usize,
        ) {
            match v {
                serde_json::Value::Object(m) => {
                    if let Some(targets) = m.get("targets").and_then(|t| t.as_array()) {
                        for t in targets {
                            *queries += 1;
                            if let Some(uid) =
                                t.pointer("/datasource/uid").and_then(|u| u.as_str())
                            {
                                used.insert(uid.to_string());
                            }
                        }
                    }
                    for x in m.values() {
                        walk(x, used, queries);
                    }
                }
                serde_json::Value::Array(a) => {
                    for x in a {
                        walk(x, used, queries);
                    }
                }
                _ => {}
            }
        }
        walk(&json, &mut used, &mut queries);

        // Carry the denominator: a walk that found nothing would satisfy
        // every set comparison below and prove exactly nothing.
        assert!(
            queries > 0,
            "{name}: walked the rendered board and found 0 queries — this check would pass vacuously"
        );

        let declared: std::collections::BTreeSet<String> = params
            .iter()
            .filter(|(k, _)| k.ends_with("datasource"))
            .map(|(_, v)| (*v).to_string())
            .collect();
        assert!(
            !declared.is_empty(),
            "{name}: no *datasource param in the matrix row — nothing to compare"
        );
        assert_eq!(
            used, declared,
            "{name}: panels reference {used:?} but the row binds {declared:?} — a query is \
             pointing at a datasource nobody declared, or a declared one is unreferenced"
        );
    }
}

/// Signal families we have EVIDENCE are emitted, each carrying its evidence.
///
/// The point of the second column is that it exists. A board reading a
/// series nothing produces renders perfectly, ships, and shows an empty
/// panel an operator reads as "nothing wrong" — the most expensive
/// failure a dashboard has, because it is indistinguishable from health.
/// Requiring a written justification per family is what stops the list
/// growing by assumption: you cannot add a row without saying where you
/// checked.
///
/// This replaced a two-prefix hand-list (`up{` or `kube_pod_container_status`)
/// that was correct for the one entry it was written against and failed
/// the four added after it — for the wrong reason, since those read real
/// series it simply did not name. Same drift class as the placeholder
/// check above.
///
/// Measured 2026-08-11 and the reason this table is not longer: the
/// akeyless gateway metric family (`gateway_auth_total` and friends, which
/// SecretsPlatformOverview / AuthMethodHealth / SecurityPostureBoard are
/// built on) returns an EMPTY series on akeyless's own Datadog over a 2h
/// window. Those three boards are therefore NOT in the catalogue — porting
/// them faithfully would ship exactly the empty-panel failure above, three
/// times over, on a board whose whole job is to say whether security is OK.
const PROVEN_SERIES: &[(&str, &str)] = &[
    (
        "up{",
        "the scraper's own liveness series; exists wherever a target is scraped at all",
    ),
    (
        "scrape_",
        "scrape_duration_seconds / scrape_samples_scraped — same origin as `up`",
    ),
    (
        "kube_pod_",
        "kube-state-metrics; deployed cluster-wide, independent of workload instrumentation",
    ),
    (
        "container_",
        "container_cpu_usage_seconds_total / container_memory_working_set_bytes, from \
         cAdvisor inside the kubelet — present on any node, no workload opt-in",
    ),
    (
        "breathe_band_",
        "registered by our own controller — breathe-runtime/src/lib.rs `metrics_for`, \
         with the label set (dim, namespace, name) read from the same function",
    ),
    (
        "vector_component_",
        "vector's own component metrics; absent until the first error/drop, which is why \
         every panel using them is marked event_driven",
    ),
    (
        "{namespace=",
        "a VictoriaLogs stream selector. LogsQL boards read no metric at all, so a \
         metric-name check would fail them for the wrong reason",
    ),
];

/// What camelot's own VictoriaMetrics actually held, 2026-08-11 23:40Z,
/// queried through a port-forward to
/// `vmsingle-lareira-vm-stack-victoria-metrics-k8s-stack` in `monitoring`.
///
/// Recorded because "the metric family exists" is NOT the property a board
/// needs — it needs the series to exist for the namespace and job it
/// queries — and checking the former while believing the latter is how a
/// board ships that renders empty. Two of these were found exactly that
/// way, after the boards were already written:
///
///   breathe_band_util_ratio                155 series
///   breathe_band_dry_run                   172 series  (all ==1; ZERO ==0)
///   kube_pod_info                         8071 series
///   container_memory_working_set_bytes      604
///   container_cpu_usage_seconds_total       604
///   scrape_duration_seconds               3739
///   scrape_samples_scraped                  59
///   vector_component_errors_total            0   <-- DEPLOYED, UNSCRAPED
///   vector_component_discarded_events_total  0   <-- same
///   gateway_auth_total                       0   <-- no instrumentation
///
///   count by (namespace) (up):  monitoring 8, breathe-system 9, keda 6,
///                               keda-http 2, camelot-nats 1, camelot 1
///   count by (job) (up{job=~"auth|authcert|bis|uam|uamop|kfm|sdr|gator|logan"}):
///                               ZERO ROWS — none of the nine akeyless
///                               services is scraped at all.
///
/// This is a DATED SNAPSHOT and nothing re-takes it. Re-measure before
/// acting on any figure here; do not infer today's state from it.
const _CAMELOT_SERIES_CENSUS_2026_08_11: () = ();

/// A rendered board must read a signal we have evidence something emits.
#[test]
fn catalogue_entries_read_signals_with_evidence() {
    let dir = dashboards_dir();
    for (name, params) in catalogue() {
        let raw = std::fs::read_to_string(dir.join(format!("{name}.tlisp"))).unwrap();
        let json = render_dashboard_grafana_json(&bind(&raw, &params), &Theme::tundra())
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        let text = json.to_string();
        let matched = PROVEN_SERIES
            .iter()
            .any(|(prefix, _)| text.contains(prefix));
        assert!(
            matched,
            "{name} reads no signal family with recorded evidence of being emitted. \
             Either it will render empty, or PROVEN_SERIES is missing a family — and \
             adding one there requires saying WHERE you checked, not merely that you \
             did. Known families: {:?}",
            PROVEN_SERIES.iter().map(|(p, _)| *p).collect::<Vec<_>>()
        );
    }
}

/// Every family in the evidence table must carry non-trivial evidence.
///
/// Without this, the table degrades into the hand-list it replaced: a
/// future prefix added with `""` or `"todo"` in the second column reads as
/// justified at a glance and is not.
#[test]
fn every_proven_series_family_states_its_evidence() {
    assert!(!PROVEN_SERIES.is_empty(), "the evidence table is empty");
    for (prefix, evidence) in PROVEN_SERIES {
        assert!(
            evidence.len() > 30,
            "{prefix:?} is listed with no real evidence ({evidence:?}) — say where you \
             checked that something emits it"
        );
    }
}
