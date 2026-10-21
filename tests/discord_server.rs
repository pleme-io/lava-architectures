mod common {
    pub mod discord_server_fixture;
}

use common::discord_server_fixture as fx;
use lava_architectures::{ARCHITECTURE_DIR, Architecture, InputBindings, eval_architecture, interface_for};

fn render_with(b: &InputBindings) -> serde_json::Value {
    let src = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(ARCHITECTURE_DIR)
            .join("discord-server.tlisp"),
    )
    .expect("source");
    let arch: Architecture = eval_architecture(&src, b).expect("evaluates");
    arch.render_terraform_json().expect("renders")
}

fn render() -> serde_json::Value {
    render_with(&fx::example_bindings())
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
    assert_eq!(names(&json, "discord_role").len(), 3);
    assert_eq!(names(&json, "discord_category_channel").len(), 2);
    assert_eq!(names(&json, "discord_text_channel").len(), 2);
    assert_eq!(names(&json, "discord_voice_channel").len(), 1);
    assert_eq!(names(&json, "discord_forum_channel").len(), 1);
    assert_eq!(names(&json, "discord_channel_permission").len(), 3);
    let total: usize = json["resource"]
        .as_object()
        .expect("resources")
        .values()
        .map(|m| m.as_object().expect("instances").len())
        .sum();
    assert_eq!(total, 12);
}

#[test]
fn addresses_come_from_the_slug_fields() {
    let json = render();
    assert_eq!(
        names(&json, "discord_role"),
        sorted(&["acme-founder", "acme-maintainer", "acme-guest"])
    );
    assert_eq!(
        names(&json, "discord_category_channel"),
        sorted(&["acme-community", "acme-lounge"])
    );
    assert_eq!(
        names(&json, "discord_text_channel"),
        sorted(&["acme-welcome", "acme-general"])
    );
    assert_eq!(names(&json, "discord_voice_channel"), sorted(&["acme-hangout"]));
    assert_eq!(names(&json, "discord_forum_channel"), sorted(&["acme-roadmap"]));
    assert_eq!(
        names(&json, "discord_channel_permission"),
        sorted(&["acme-welcome-guest", "acme-hangout-guest", "acme-roadmap-maintainer"])
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
                let parts: Vec<&str> = inner.split('.').collect();
                assert_eq!(parts.len(), 3, "{ty}.{label}.{attr}: odd reference {s}");
                let target = &resources
                    .get(parts[0])
                    .unwrap_or_else(|| panic!("{ty}.{label}.{attr}: no resource type {}", parts[0]))[parts[1]];
                assert!(
                    !target.is_null(),
                    "{ty}.{label}.{attr}: dangling reference {s}"
                );
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 3 * 2 + 3 * 2 - 2, "reference count drifted: {checked}");
}

#[test]
fn channels_reference_their_category_and_permissions_their_channel_and_role() {
    let json = render();
    assert_eq!(
        json["resource"]["discord_text_channel"]["acme-welcome"]["category"],
        "${discord_category_channel.acme-community.id}"
    );
    assert_eq!(
        json["resource"]["discord_voice_channel"]["acme-hangout"]["category"],
        "${discord_category_channel.acme-lounge.id}"
    );
    assert_eq!(
        json["resource"]["discord_forum_channel"]["acme-roadmap"]["category"],
        "${discord_category_channel.acme-community.id}"
    );
    let perms = &json["resource"]["discord_channel_permission"];
    assert_eq!(
        perms["acme-welcome-guest"]["channel_id"],
        "${discord_text_channel.acme-welcome.id}"
    );
    assert_eq!(
        perms["acme-hangout-guest"]["channel_id"],
        "${discord_voice_channel.acme-hangout.id}"
    );
    assert_eq!(
        perms["acme-roadmap-maintainer"]["channel_id"],
        "${discord_forum_channel.acme-roadmap.id}"
    );
    assert_eq!(
        perms["acme-roadmap-maintainer"]["overwrite_id"],
        "${discord_role.acme-maintainer.id}"
    );
    assert_eq!(perms["acme-welcome-guest"]["type"], "role");
}

#[test]
fn scalars_render_with_the_provider_types() {
    let json = render();
    let role = &json["resource"]["discord_role"]["acme-founder"];
    assert_eq!(role["server_id"], fx::SERVER_ID);
    assert_eq!(role["permissions"], 8);
    assert_eq!(role["hoist"], true);
    assert_eq!(role["mentionable"], false);
    let perm = &json["resource"]["discord_channel_permission"]["acme-welcome-guest"];
    assert_eq!(perm["allow"], 66560);
    assert_eq!(perm["deny"], 2048);
}

#[test]
fn a_category_row_targets_the_category() {
    let mut perms = fx::permissions();
    perms.push(fx::permission(
        "acme-lounge-guest",
        "category",
        "acme-lounge",
        "acme-guest",
        "66560",
        "0",
    ));
    let json = render_with(&fx::bindings(fx::roles(), fx::categories(), fx::channels(), perms));
    assert_eq!(
        json["resource"]["discord_channel_permission"]["acme-lounge-guest"]["channel_id"],
        "${discord_category_channel.acme-lounge.id}"
    );
}

#[test]
fn empty_lists_render_no_resources() {
    let json = render_with(&fx::bindings(vec![], vec![], vec![], vec![]));
    let n = json["resource"].as_object().map_or(0, serde_json::Map::len);
    assert_eq!(n, 0);
}

#[test]
fn the_interface_is_registered() {
    let i = interface_for("discord-server").expect("interface");
    assert_eq!(i.name, "discord-server");
}
