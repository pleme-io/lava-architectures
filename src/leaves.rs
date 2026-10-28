use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use lava_eval::{InputBindings, LeafKey, leaf_key_in_source};
use serde::Deserialize;
use serde_json::{Map, Value};

pub const DECLARATION: &str = "leaves/declaration.json";
pub const TREE: &str = "leaves";
pub const MAX_NAME_LEN: usize = 63;
pub const WORKSPACE_LABEL: &str = "pangea.pleme.io/workspace";
pub const LEAF_KEY: &str = "pangea.pleme.io/leaf-key";
pub const LEAF_DIGEST: &str = "pangea.pleme.io/leaf-digest";
pub const LEAF_COLLECTION: &str = "pangea.pleme.io/leaf-collection";
pub const LEAF_KEY_FIELD: &str = "pangea.pleme.io/leaf-key-field";
pub const MIGRATE_FROM: &str = "pangea.pleme.io/migrate-from";

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub data: String,
    pub collection: String,
    pub field: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Declaration {
    pub leaves: String,
    #[serde(default = "on")]
    pub enabled: bool,
    pub from: Source,
    pub key: String,
    pub architecture: String,
    pub records: String,
    #[serde(default)]
    pub shared: Vec<String>,
    #[serde(default)]
    pub count: Option<String>,
    pub namespace: String,
    #[serde(default)]
    pub spec: Map<String, Value>,
    #[serde(default)]
    pub catalog: Map<String, Value>,
    #[serde(default)]
    pub migrate_from: Option<String>,
}

fn on() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq)]
pub struct Leaf {
    pub key: String,
    pub name: String,
    pub variables: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LeafSet {
    pub declaration: Declaration,
    pub rule: LeafKey,
    pub shared: Map<String, Value>,
    pub leaves: Vec<Leaf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    pub leaf_set: String,
    pub reason: String,
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.leaf_set, self.reason)
    }
}

pub fn parse_declarations(text: &str) -> Result<Vec<Result<Declaration, Refusal>>, String> {
    let raw: Vec<Value> =
        serde_json::from_str(text).map_err(|e| format!("declaration is not a JSON list: {e}"))?;
    Ok(raw
        .into_iter()
        .enumerate()
        .map(|(i, v)| {
            let label = v
                .get("leaves")
                .and_then(Value::as_str)
                .map_or_else(|| format!("entry {i}"), str::to_string);
            serde_json::from_value::<Declaration>(v).map_err(|e| Refusal {
                leaf_set: label,
                reason: format!("malformed entry: {e}"),
            })
        })
        .collect())
}

#[must_use]
pub fn dns_label(raw: &str) -> String {
    let mut out = String::new();
    let mut dash = true;
    for c in raw.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            dash = false;
        } else if !dash {
            out.push('-');
            dash = true;
        }
    }
    let trimmed = out.trim_matches('-');
    if trimmed.is_empty() {
        "leaf".to_string()
    } else {
        trimmed.to_string()
    }
}

pub fn derive(
    decl: &Declaration,
    architecture_source: &str,
    records: &Map<String, Value>,
    catalogue: &[String],
) -> Result<LeafSet, Refusal> {
    let refuse = |reason: String| Refusal {
        leaf_set: decl.leaves.clone(),
        reason,
    };
    let rule = leaf_key_in_source(architecture_source, &decl.architecture)
        .map_err(|e| refuse(e.to_string()))?
        .ok_or_else(|| {
            refuse(format!(
                "architecture {:?} declares no :leaf clause, so it has no keyed form",
                decl.architecture
            ))
        })?;
    if rule.key != decl.key {
        return Err(refuse(format!(
            "the declaration keys by {:?} but architecture {:?} keys by {:?}",
            decl.key, decl.architecture, rule.key
        )));
    }
    for s in &decl.shared {
        if rule.each.contains(s) {
            return Err(refuse(format!(
                "{s:?} is both shared and a keyed collection"
            )));
        }
    }
    let declared: BTreeSet<&str> = catalogue.iter().map(String::as_str).collect();
    let resolved: BTreeSet<&str> = records
        .get(rule.primary())
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(|r| r.get(&decl.from.field).and_then(Value::as_str))
                .collect()
        })
        .unwrap_or_default();
    if declared != resolved {
        let missing: Vec<&&str> = declared.difference(&resolved).take(5).collect();
        let extra: Vec<&&str> = resolved.difference(&declared).take(5).collect();
        return Err(refuse(format!(
            "records {:?} are stale against {}: {} declared rows have no record (first {:?}), {} records have no declared row (first {:?})",
            decl.records,
            decl.from.data,
            declared.difference(&resolved).count(),
            missing,
            resolved.difference(&declared).count(),
            extra
        )));
    }
    let keys = rule.keys(records).map_err(|e| refuse(e.to_string()))?;
    let mut shared = Map::new();
    for s in &decl.shared {
        let v = records
            .get(s)
            .ok_or_else(|| refuse(format!("shared variable {s:?} is not in the records")))?;
        shared.insert(s.clone(), v.clone());
    }
    let mut names = BTreeMap::new();
    let mut leaves = Vec::with_capacity(keys.len());
    for key in keys {
        let name = format!("{}-{}", decl.leaves, dns_label(&key));
        if name.len() > MAX_NAME_LEN {
            return Err(refuse(format!(
                "leaf {key:?} names {name:?}, longer than {MAX_NAME_LEN}"
            )));
        }
        if let Some(prev) = names.insert(name.clone(), key.clone()) {
            return Err(refuse(format!(
                "leaves {prev:?} and {key:?} both name {name:?}"
            )));
        }
        let sliced = rule
            .slice(records, &key)
            .map_err(|e| refuse(e.to_string()))?;
        let mut variables = Map::new();
        for each in &rule.each {
            if let Some(v) = sliced.get(each) {
                variables.insert(each.clone(), v.clone());
            }
        }
        if let Some(count) = &decl.count {
            let n = variables
                .get(rule.primary())
                .and_then(Value::as_array)
                .map_or(0, Vec::len);
            variables.insert(count.clone(), Value::String(n.to_string()));
        }
        leaves.push(Leaf {
            key,
            name,
            variables,
        });
    }
    Ok(LeafSet {
        declaration: decl.clone(),
        rule,
        shared,
        leaves,
    })
}

#[must_use]
pub fn canonical(v: &Value) -> Value {
    match v {
        Value::Object(m) => {
            let sorted: BTreeMap<&String, Value> =
                m.iter().map(|(k, x)| (k, canonical(x))).collect();
            Value::Object(sorted.into_iter().map(|(k, x)| (k.clone(), x)).collect())
        }
        Value::Array(xs) => {
            let mut items: Vec<Value> = xs.iter().map(canonical).collect();
            items.sort_by_cached_key(|x| serde_json::to_string(x).unwrap_or_default());
            Value::Array(items)
        }
        other => other.clone(),
    }
}

fn merge(into: &mut Value, from: &Value, path: &str) -> Result<(), String> {
    match (into, from) {
        (Value::Object(a), Value::Object(b)) => {
            for (k, v) in b {
                let here = format!("{path}.{k}");
                let addressed = path.starts_with(".resource.") && path.matches('.').count() == 2;
                match a.get_mut(k) {
                    None => {
                        a.insert(k.clone(), v.clone());
                    }
                    Some(_) if addressed => {
                        return Err(format!("{here} is emitted by more than one leaf"));
                    }
                    Some(existing) => merge(existing, v, &here)?,
                }
            }
            Ok(())
        }
        (Value::Array(a), Value::Array(b)) => {
            a.extend(b.iter().cloned());
            Ok(())
        }
        (a, b) if a == b => Ok(()),
        _ => Err(format!("{path} is emitted by more than one leaf")),
    }
}

pub fn render(architecture_source: &str, variables: &Map<String, Value>) -> Result<Value, String> {
    let b = InputBindings::from_variables(variables).map_err(|e| e.to_string())?;
    lava_eval::eval_architecture(architecture_source, &b)
        .map_err(|e| format!("lava evaluation failed: {e}"))?
        .render_terraform_json()
        .map_err(|e| format!("lava render failed: {e}"))
}

#[must_use]
pub fn leaf_inputs(set: &LeafSet, leaf: &Leaf) -> Map<String, Value> {
    let mut v = set.shared.clone();
    for (k, x) in &leaf.variables {
        v.insert(k.clone(), x.clone());
    }
    v
}

pub const COMPARED_OUTSIDE: &[&str] = &["output"];

pub fn union_gate(
    set: &LeafSet,
    architecture_source: &str,
    records: &Map<String, Value>,
) -> Result<usize, String> {
    let mut whole = render(architecture_source, records)?;
    let mut union = Value::Object(Map::new());
    for leaf in &set.leaves {
        let one = render(architecture_source, &leaf_inputs(set, leaf))
            .map_err(|e| format!("leaf {}: {e}", leaf.name))?;
        merge(&mut union, &one, "").map_err(|e| format!("leaf {}: {e}", leaf.name))?;
    }
    for drop in COMPARED_OUTSIDE {
        if let Value::Object(m) = &mut whole {
            m.remove(*drop);
        }
        if let Value::Object(m) = &mut union {
            m.remove(*drop);
        }
    }
    let (w, u) = (canonical(&whole), canonical(&union));
    if w == u {
        return Ok(set.leaves.len());
    }
    let ws = serde_json::to_string(&w).unwrap_or_default();
    let us = serde_json::to_string(&u).unwrap_or_default();
    let at = ws
        .bytes()
        .zip(us.bytes())
        .position(|(a, b)| a != b)
        .unwrap_or(ws.len().min(us.len()));
    let from = at.saturating_sub(80);
    Err(format!(
        "the union of {} leaf renders differs from the whole render at byte {at}: whole …{}… union …{}…",
        set.leaves.len(),
        &ws[from..(at + 80).min(ws.len())],
        &us[from..(at + 80).min(us.len())]
    ))
}

fn digest(architecture_source: &str, inputs: &Map<String, Value>) -> String {
    let mut h = blake3::Hasher::new();
    h.update(architecture_source.as_bytes());
    h.update(&[0]);
    h.update(
        serde_json::to_string(&canonical(&Value::Object(inputs.clone())))
            .unwrap_or_default()
            .as_bytes(),
    );
    h.finalize().to_hex().to_string()
}

fn yaml(v: &Value) -> Result<String, String> {
    serde_norway::to_string(&canonical(v)).map_err(|e| e.to_string())
}

pub fn manifests(
    set: &LeafSet,
    architecture_source: &str,
) -> Result<BTreeMap<PathBuf, String>, String> {
    let d = &set.declaration;
    let dir = PathBuf::from(&d.leaves);
    let mut out = BTreeMap::new();
    let mut catalog_spec = d.catalog.clone();
    catalog_spec.insert("variables".into(), Value::Object(set.shared.clone()));
    let mut catalog_meta = serde_json::json!({"name": d.leaves, "namespace": d.namespace});
    if let Some(fat) = &d.migrate_from {
        catalog_meta["annotations"] = serde_json::json!({MIGRATE_FROM: fat});
    }
    let catalog = serde_json::json!({
        "apiVersion": "pangea.pleme.io/v1alpha1",
        "kind": "WorkspaceCatalog",
        "metadata": catalog_meta,
        "spec": catalog_spec,
    });
    out.insert(dir.join("catalog.yaml"), yaml(&catalog)?);
    for leaf in &set.leaves {
        let mut spec = d.spec.clone();
        spec.insert("templateName".into(), Value::String(d.architecture.clone()));
        spec.insert("dialect".into(), Value::String("lava".into()));
        spec.insert("variables".into(), Value::Object(leaf.variables.clone()));
        let doc = serde_json::json!({
            "apiVersion": "pangea.pleme.io/v1alpha1",
            "kind": "InfrastructureTemplate",
            "metadata": {
                "name": leaf.name,
                "namespace": d.namespace,
                "labels": {WORKSPACE_LABEL: d.leaves},
                "annotations": {
                    LEAF_KEY: leaf.key,
                    LEAF_COLLECTION: set.rule.primary(),
                    LEAF_KEY_FIELD: set.rule.key,
                    LEAF_DIGEST: digest(architecture_source, &leaf_inputs(set, leaf)),
                },
            },
            "spec": spec,
        });
        out.insert(
            dir.join("leaves").join(format!("{}.yaml", leaf.name)),
            yaml(&doc)?,
        );
    }
    Ok(out)
}

pub fn catalogue_names(path: &Path, collection: &str, field: &str) -> Result<Vec<String>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let doc: Value =
        serde_norway::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    let rows = doc
        .get(collection)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{}: no list {collection:?}", path.display()))?;
    rows.iter()
        .enumerate()
        .map(|(i, r)| {
            r.get(field)
                .and_then(Value::as_str)
                .map(str::to_string)
                .ok_or_else(|| {
                    format!(
                        "{}: {collection}[{i}] has no string {field:?}",
                        path.display()
                    )
                })
        })
        .collect()
}

pub fn read_records(path: &Path) -> Result<Map<String, Value>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    match serde_json::from_str::<Value>(&text).map_err(|e| format!("{}: {e}", path.display()))? {
        Value::Object(m) => Ok(m),
        _ => Err(format!("{}: records are not a JSON object", path.display())),
    }
}

#[derive(Debug, Default)]
pub struct Outcome {
    pub files: BTreeMap<PathBuf, String>,
    pub refusals: Vec<Refusal>,
    pub sets: Vec<(String, usize)>,
}

pub fn render_workspace(
    workspace: &Path,
    architectures: &Path,
    gate: bool,
) -> Result<Outcome, String> {
    let text = std::fs::read_to_string(workspace.join(DECLARATION))
        .map_err(|e| format!("{}: {e}", workspace.join(DECLARATION).display()))?;
    let mut outcome = Outcome::default();
    for entry in parse_declarations(&text)? {
        let decl = match entry {
            Ok(d) => d,
            Err(r) => {
                outcome.refusals.push(r);
                continue;
            }
        };
        if !decl.enabled {
            outcome.sets.push((decl.leaves.clone(), 0));
            continue;
        }
        match one_set(workspace, architectures, &decl, gate) {
            Ok((files, n)) => {
                outcome.files.extend(files);
                outcome.sets.push((decl.leaves.clone(), n));
            }
            Err(reason) => outcome.refusals.push(Refusal {
                leaf_set: decl.leaves.clone(),
                reason,
            }),
        }
    }
    Ok(outcome)
}

fn one_set(
    workspace: &Path,
    architectures: &Path,
    decl: &Declaration,
    gate: bool,
) -> Result<(BTreeMap<PathBuf, String>, usize), String> {
    let arch_path = architectures.join(format!("{}.tlisp", decl.architecture));
    let source =
        std::fs::read_to_string(&arch_path).map_err(|e| format!("{}: {e}", arch_path.display()))?;
    let records = read_records(&workspace.join(&decl.records))?;
    let names = catalogue_names(
        &workspace.join(&decl.from.data),
        &decl.from.collection,
        &decl.from.field,
    )?;
    let set = derive(decl, &source, &records, &names).map_err(|r| r.reason)?;
    if gate {
        union_gate(&set, &source, &records)?;
    }
    let files = manifests(&set, &source)?
        .into_iter()
        .map(|(p, s)| (Path::new(TREE).join(p), s))
        .collect();
    Ok((files, set.leaves.len()))
}

#[must_use]
pub fn tree_drift(
    workspace: &Path,
    rendered: &BTreeMap<PathBuf, String>,
    sets: &[(String, usize)],
) -> Vec<String> {
    let mut drift = Vec::new();
    for (path, body) in rendered {
        match std::fs::read_to_string(workspace.join(path)) {
            Ok(on_disk) if &on_disk == body => {}
            Ok(_) => drift.push(format!("{} differs from its render", path.display())),
            Err(_) => drift.push(format!("{} is missing", path.display())),
        }
    }
    for (set, _) in sets {
        let dir = workspace.join(TREE).join(set);
        for found in walk(&dir) {
            let rel = found
                .strip_prefix(workspace)
                .unwrap_or(&found)
                .to_path_buf();
            if !rendered.contains_key(&rel) {
                drift.push(format!("{} is not rendered by any leaf", rel.display()));
            }
        }
    }
    drift
}

fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(dir) else {
        return out;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            out.extend(walk(&p));
        } else {
            out.push(p);
        }
    }
    out.sort();
    out
}
