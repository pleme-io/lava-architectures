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
    vec![(
        "workload-overview",
        BTreeMap::from([
            ("env", "camelot"),
            ("service", "auth"),
            ("job", "auth"),
            ("namespace", "default"),
            ("datasource", "mimir"),
        ]),
    )]
}

fn dashboards_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(DASHBOARD_DIR)
}

/// Bind `{key}` placeholders — the same scalar substitution the operator's
/// lava backend performs, reproduced here so the test exercises the real
/// path rather than a pre-substituted fixture.
fn bind(src: &str, params: &BTreeMap<&str, &str>) -> String {
    let mut out = src.to_string();
    for (k, v) in params {
        out = out.replace(&format!("{{{k}}}"), v);
    }
    out
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
                let text = json.to_string();
                if text.contains("{env}")
                    || text.contains("{service}")
                    || text.contains("{namespace}")
                    || text.contains("{datasource}")
                    || text.contains("{job}")
                {
                    failures.push(format!("{name}: an unbound placeholder reached the output"));
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

/// A rendered board must read a series something actually emits.
///
/// Measured 2026-08-11: 4 of the 6 akeyless microservices expose no
/// domain metric at all, so a board built on application series renders
/// empty for most of the fleet. Every catalogue entry therefore keys on
/// scraper/kube-state series, and this asserts it rather than trusting
/// the author to remember.
#[test]
fn catalogue_entries_read_series_the_scraper_emits() {
    let dir = dashboards_dir();
    for (name, params) in catalogue() {
        let raw = std::fs::read_to_string(dir.join(format!("{name}.tlisp"))).unwrap();
        let json = render_dashboard_grafana_json(&bind(&raw, &params), &Theme::tundra())
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        let text = json.to_string();
        assert!(
            text.contains("up{") || text.contains("kube_pod_container_status"),
            "{name} reads no scraper-emitted series — it will render empty on a service \
             with no application metrics"
        );
    }
}
