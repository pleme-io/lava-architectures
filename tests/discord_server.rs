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

fn collect_refs(value: &serde_json::Value, out: &mut Vec<String>) {
    match value {
        serde_json::Value::String(s) => {
            if s.starts_with("${") && s.ends_with('}') {
                out.push(s.clone());
            }
        }
        serde_json::Value::Array(items) => items.iter().for_each(|v| collect_refs(v, out)),
        serde_json::Value::Object(map) => map.values().for_each(|v| collect_refs(v, out)),
        _ => {}
    }
}

fn references(json: &serde_json::Value) -> Vec<String> {
    let mut out = Vec::new();
    for instances in json["resource"].as_object().expect("resources").values() {
        collect_refs(instances, &mut out);
    }
    out
}

fn dangling(json: &serde_json::Value) -> Vec<String> {
    let resources = json["resource"].as_object().expect("resources");
    references(json)
        .into_iter()
        .filter(|r| {
            let inner = &r[2..r.len() - 1];
            let parts: Vec<&str> = inner.split('.').collect();
            parts.len() != 3
                || resources
                    .get(parts[0])
                    .and_then(|m| m.get(parts[1]))
                    .is_none_or(serde_json::Value::is_null)
        })
        .collect()
}

#[test]
fn every_cross_reference_resolves() {
    let json = render();
    assert_eq!(dangling(&json), Vec::<String>::new());
    assert_eq!(references(&json).len(), 3 * 2 + 3 * 2 - 2, "reference count drifted");
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

fn render_full() -> serde_json::Value {
    render_with(&fx::full_bindings())
}

fn rows_with(f: impl FnOnce(&mut InputBindings)) -> serde_json::Value {
    let mut b = fx::full_bindings();
    f(&mut b);
    render_with(&b)
}

#[test]
fn the_original_five_lists_alone_still_render_the_original_twelve() {
    let json = render();
    for ty in [
        "discord_managed_server",
        "discord_role_everyone",
        "discord_server_widget",
        "discord_system_channel",
        "discord_news_channel",
        "discord_webhook",
        "discord_invite",
        "discord_server_onboarding",
        "discord_role_connection_metadata",
        "discord_member_roles",
        "discord_guild_sticker",
        "discord_message",
    ] {
        assert!(names(&json, ty).is_empty(), "{ty} appeared with no rows");
    }
}

#[test]
fn the_full_fixture_renders_every_new_resource_type() {
    let json = render_full();
    assert_eq!(names(&json, "discord_text_channel").len(), 7);
    assert_eq!(names(&json, "discord_managed_server"), sorted(&["acme-settings"]));
    assert_eq!(names(&json, "discord_role_everyone"), sorted(&["acme-everyone"]));
    assert_eq!(names(&json, "discord_server_widget"), sorted(&["acme-widget"]));
    assert_eq!(names(&json, "discord_system_channel"), sorted(&["acme-system"]));
    assert_eq!(names(&json, "discord_news_channel"), sorted(&["acme-announcements"]));
    assert_eq!(names(&json, "discord_webhook"), sorted(&["acme-alerts", "acme-releases"]));
    assert_eq!(names(&json, "discord_invite"), sorted(&["acme-invite"]));
    assert_eq!(names(&json, "discord_server_onboarding"), sorted(&["acme-onboarding"]));
    assert_eq!(
        names(&json, "discord_role_connection_metadata"),
        sorted(&["acme-linked"])
    );
    assert_eq!(names(&json, "discord_member_roles"), sorted(&["acme-founder-grant"]));
    assert_eq!(names(&json, "discord_guild_sticker"), sorted(&["acme-wave"]));
    assert_eq!(names(&json, "discord_message"), sorted(&["acme-rules-post"]));
    let total: usize = json["resource"]
        .as_object()
        .expect("resources")
        .values()
        .map(|m| m.as_object().expect("instances").len())
        .sum();
    assert_eq!(total, 30);
}

#[test]
fn every_reference_in_the_full_fixture_resolves_including_lists_and_blocks() {
    let json = render_full();
    assert_eq!(dangling(&json), Vec::<String>::new());
    assert_eq!(references(&json).len(), 9 + 6 + 15, "reference count drifted");
}

#[test]
fn a_row_naming_a_missing_channel_is_reported_dangling() {
    let json = rows_with(|b| {
        let mut hooks = fx::webhooks();
        hooks.push(fx::webhook("acme-ghost", "ghost", "text", "acme-no-such-channel"));
        b.set_records("webhooks", hooks);
    });
    assert_eq!(
        dangling(&json),
        vec!["${discord_text_channel.acme-no-such-channel.id}".to_string()]
    );
}

#[test]
fn a_row_naming_a_missing_role_is_reported_dangling() {
    let json = rows_with(|b| {
        let mut grants = fx::member_roles();
        grants[0].insert("role_slug".into(), "acme-no-such-role".into());
        b.set_records("member-roles", grants);
    });
    assert_eq!(
        dangling(&json),
        vec!["${discord_role.acme-no-such-role.id}".to_string()]
    );
}

#[test]
fn webhooks_and_invites_follow_their_channel_kind() {
    let json = rows_with(|b| {
        let mut invites = fx::invites();
        invites.push(fx::invite("acme-hangout-invite", "voice", "acme-hangout"));
        b.set_records("invites", invites);
    });
    let hooks = &json["resource"]["discord_webhook"];
    assert_eq!(hooks["acme-alerts"]["channel_id"], "${discord_text_channel.acme-general.id}");
    assert_eq!(
        hooks["acme-releases"]["channel_id"],
        "${discord_news_channel.acme-announcements.id}"
    );
    let invites = &json["resource"]["discord_invite"];
    assert_eq!(invites["acme-invite"]["channel_id"], "${discord_text_channel.acme-welcome.id}");
    assert_eq!(
        invites["acme-hangout-invite"]["channel_id"],
        "${discord_voice_channel.acme-hangout.id}"
    );
}

#[test]
fn guild_level_rows_render_with_the_provider_types() {
    let json = render_full();
    let r = &json["resource"];
    let settings = &r["discord_managed_server"]["acme-settings"];
    assert_eq!(settings["server_id"], fx::SERVER_ID);
    assert_eq!(settings["verification_level"], 1);
    assert_eq!(settings["explicit_content_filter"], 2);
    assert_eq!(settings["afk_timeout"], 300);
    assert_eq!(settings["description"], "acme example server");
    assert_eq!(r["discord_role_everyone"]["acme-everyone"]["permissions"], 1024);
    assert_eq!(r["discord_server_widget"]["acme-widget"]["enabled"], true);
    assert_eq!(
        r["discord_server_widget"]["acme-widget"]["channel_id"],
        "${discord_text_channel.acme-welcome.id}"
    );
    assert_eq!(r["discord_system_channel"]["acme-system"]["system_channel_flags"], 6);
    let invite = &r["discord_invite"]["acme-invite"];
    assert_eq!(invite["max_age"], 0);
    assert_eq!(invite["temporary"], false);
    assert_eq!(invite["unique"], false);
}

#[test]
fn onboarding_lists_seven_default_channels_as_references() {
    let json = render_full();
    let ob = &json["resource"]["discord_server_onboarding"]["acme-onboarding"];
    let ids = ob["default_channel_ids"].as_array().expect("list");
    assert_eq!(ids.len(), 7);
    assert_eq!(ids[0], "${discord_text_channel.acme-welcome.id}");
    assert_eq!(ids[6], "${discord_text_channel.acme-offtopic.id}");
    assert_eq!(ob["enabled"], true);
    assert_eq!(ob["mode"], 0);
}

#[test]
fn linked_role_metadata_renders_one_or_two_entries() {
    let one = render_full();
    let entries = one["resource"]["discord_role_connection_metadata"]["acme-linked"]["metadata"]
        .as_array()
        .expect("list")
        .clone();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["key"], "acme_commits");
    assert_eq!(entries[0]["type"], 2);
    assert_eq!(
        one["resource"]["discord_role_connection_metadata"]["acme-linked"]["application_id"],
        serde_json::json!(555_566_667_777_888_899_i64)
    );

    let two = rows_with(|b| b.set_records("linked-roles", vec![fx::linked_role("acme-linked", true)]));
    let entries = two["resource"]["discord_role_connection_metadata"]["acme-linked"]["metadata"]
        .as_array()
        .expect("list")
        .clone();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[1]["key"], "acme_member");
    assert_eq!(entries[1]["type"], 7);
}

#[test]
fn stickers_and_messages_render_with_the_provider_types() {
    let json = render_full();
    let sticker = &json["resource"]["discord_guild_sticker"]["acme-wave"];
    assert_eq!(sticker["file"], "stickers/acme-wave.png");
    assert_eq!(sticker["tags"], "wave,hello");
    let message = &json["resource"]["discord_message"]["acme-rules-post"];
    assert_eq!(message["channel_id"], "${discord_text_channel.acme-rules.id}");
    assert_eq!(message["pinned"], true);
    assert_eq!(message["tts"], false);
}

#[test]
fn member_role_rows_render_the_role_block() {
    let json = render_full();
    let grant = &json["resource"]["discord_member_roles"]["acme-founder-grant"];
    assert_eq!(grant["user_id"], serde_json::json!(222_233_334_444_555_566_i64));
    let role = grant["role"].as_array().expect("block list");
    assert_eq!(role.len(), 1);
    assert_eq!(role[0]["role_id"], "${discord_role.acme-founder.id}");
    assert_eq!(role[0]["has_role"], true);
}

#[test]
fn the_full_render_conforms_to_the_provider_schema() {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/discord-provider-attributes.json");
    let schema: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(fixture).expect("fixture")).expect("parses");
    let json = render_full();
    let mut problems: Vec<String> = Vec::new();
    let mut checked = 0usize;
    for (ty, instances) in json["resource"].as_object().expect("resources") {
        let Some(known) = schema.get(ty) else {
            problems.push(format!("{ty}: not a provider resource"));
            continue;
        };
        let attrs = known["attributes"].as_object().expect("attributes");
        let blocks: Vec<&str> = known["block_types"]
            .as_array()
            .expect("blocks")
            .iter()
            .filter_map(serde_json::Value::as_str)
            .collect();
        for (label, body) in instances.as_object().expect("instances") {
            let rendered = body.as_object().expect("body");
            for name in rendered.keys() {
                checked += 1;
                let known_attr = attrs.get(name).is_some_and(|a| a["settable"].as_bool() == Some(true));
                if !known_attr && !blocks.contains(&name.as_str()) {
                    problems.push(format!("{ty}.{label}: `{name}` is not settable on {ty}"));
                }
            }
            for (name, a) in attrs {
                if a["required"].as_bool() == Some(true) && !rendered.contains_key(name) {
                    problems.push(format!("{ty}.{label}: required `{name}` missing"));
                }
            }
            for block in &blocks {
                if ["role", "metadata"].contains(block) && !rendered.contains_key(*block) {
                    problems.push(format!("{ty}.{label}: required block `{block}` missing"));
                }
            }
        }
    }
    assert!(checked >= 80, "only {checked} attributes checked");
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn the_interface_declares_every_list_the_architecture_reads() {
    let i = interface_for("discord-server").expect("interface");
    for list in [
        "settings",
        "everyone",
        "widgets",
        "system-channels",
        "news-channels",
        "webhooks",
        "invites",
        "onboardings",
        "linked-roles",
        "member-roles",
        "stickers",
        "messages",
    ] {
        assert!(i.inputs.contains_key(list), "interface lacks {list}");
    }
}
