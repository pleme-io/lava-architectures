use std::collections::BTreeMap;

use lava_eval::InputBindings;

pub const SERVER_ID: &str = "111122223333444455";

pub type Row = BTreeMap<String, String>;

pub fn row(fields: &[(&str, &str)]) -> Row {
    fields
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect()
}

pub fn roles() -> Vec<Row> {
    vec![
        row(&[
            ("slug", "acme-founder"),
            ("name", "founder"),
            ("color", "15158332"),
            ("hoist", "true"),
            ("mentionable", "false"),
            ("permissions", "8"),
        ]),
        row(&[
            ("slug", "acme-maintainer"),
            ("name", "maintainer"),
            ("color", "3447003"),
            ("hoist", "true"),
            ("mentionable", "true"),
            ("permissions", "126016"),
        ]),
        row(&[
            ("slug", "acme-guest"),
            ("name", "guest"),
            ("color", "9807270"),
            ("hoist", "false"),
            ("mentionable", "false"),
            ("permissions", "66560"),
        ]),
    ]
}

pub fn categories() -> Vec<Row> {
    vec![
        row(&[
            ("slug", "acme-community"),
            ("name", "community"),
            ("position", "0"),
        ]),
        row(&[
            ("slug", "acme-lounge"),
            ("name", "lounge"),
            ("position", "1"),
        ]),
    ]
}

pub fn channels() -> Vec<Row> {
    let base = |slug: &str, name: &str, kind: &str, category: &str, position: &str| {
        row(&[
            ("slug", slug),
            ("name", name),
            ("topic", "acme example channel"),
            ("nsfw", "false"),
            ("position", position),
            ("category_slug", category),
            ("is_text", if kind == "text" { "true" } else { "false" }),
            ("is_voice", if kind == "voice" { "true" } else { "false" }),
            ("is_forum", if kind == "forum" { "true" } else { "false" }),
        ])
    };
    vec![
        base("acme-welcome", "welcome", "text", "acme-community", "0"),
        base("acme-general", "general", "text", "acme-community", "1"),
        base("acme-roadmap", "roadmap", "forum", "acme-community", "2"),
        base("acme-hangout", "hangout", "voice", "acme-lounge", "0"),
    ]
}

pub fn permission(slug: &str, kind: &str, channel: &str, role: &str, allow: &str, deny: &str) -> Row {
    row(&[
        ("slug", slug),
        ("channel_slug", channel),
        ("role_slug", role),
        ("allow", allow),
        ("deny", deny),
        ("on_text", if kind == "text" { "true" } else { "false" }),
        ("on_voice", if kind == "voice" { "true" } else { "false" }),
        ("on_forum", if kind == "forum" { "true" } else { "false" }),
        ("on_category", if kind == "category" { "true" } else { "false" }),
    ])
}

pub fn permissions() -> Vec<Row> {
    vec![
        permission("acme-welcome-guest", "text", "acme-welcome", "acme-guest", "66560", "2048"),
        permission("acme-hangout-guest", "voice", "acme-hangout", "acme-guest", "3146752", "0"),
        permission("acme-roadmap-maintainer", "forum", "acme-roadmap", "acme-maintainer", "126016", "0"),
    ]
}

pub fn bindings(
    roles: Vec<Row>,
    categories: Vec<Row>,
    channels: Vec<Row>,
    permissions: Vec<Row>,
) -> InputBindings {
    let mut b = InputBindings::new();
    b.set_str("server-id", SERVER_ID);
    b.set_records("roles", roles);
    b.set_records("categories", categories);
    b.set_records("channels", channels);
    b.set_records("permissions", permissions);
    b
}

pub fn example_bindings() -> InputBindings {
    bindings(roles(), categories(), channels(), permissions())
}

pub const APPLICATION_ID: &str = "555566667777888899";
pub const USER_ID: &str = "222233334444555566";

pub fn extra_text_channels() -> Vec<Row> {
    [
        ("acme-rules", "rules", "3"),
        ("acme-faq", "faq", "4"),
        ("acme-help", "help", "5"),
        ("acme-showcase", "showcase", "6"),
        ("acme-offtopic", "offtopic", "7"),
    ]
    .iter()
    .map(|(slug, name, position)| {
        row(&[
            ("slug", slug),
            ("name", name),
            ("topic", "acme example channel"),
            ("nsfw", "false"),
            ("position", position),
            ("category_slug", "acme-community"),
            ("is_text", "true"),
            ("is_voice", "false"),
            ("is_forum", "false"),
        ])
    })
    .collect()
}

pub fn full_channels() -> Vec<Row> {
    let mut v = channels();
    v.extend(extra_text_channels());
    v
}

pub fn settings() -> Vec<Row> {
    vec![row(&[
        ("slug", "acme-settings"),
        ("name", "acme"),
        ("description", "acme example server"),
        ("verification_level", "1"),
        ("explicit_content_filter", "2"),
        ("default_message_notifications", "1"),
        ("afk_timeout", "300"),
    ])]
}

pub fn everyone() -> Vec<Row> {
    vec![row(&[("slug", "acme-everyone"), ("permissions", "1024")])]
}

pub fn widgets() -> Vec<Row> {
    vec![row(&[
        ("slug", "acme-widget"),
        ("enabled", "true"),
        ("channel_slug", "acme-welcome"),
    ])]
}

pub fn system_channels() -> Vec<Row> {
    vec![row(&[
        ("slug", "acme-system"),
        ("channel_slug", "acme-welcome"),
        ("flags", "6"),
    ])]
}

pub fn news_channels() -> Vec<Row> {
    vec![row(&[
        ("slug", "acme-announcements"),
        ("name", "announcements"),
        ("topic", "acme example announcements"),
        ("nsfw", "false"),
        ("position", "8"),
        ("category_slug", "acme-community"),
    ])]
}

pub fn webhook(slug: &str, name: &str, kind: &str, channel: &str) -> Row {
    row(&[
        ("slug", slug),
        ("name", name),
        ("channel_slug", channel),
        ("on_text", if kind == "text" { "true" } else { "false" }),
        ("on_news", if kind == "news" { "true" } else { "false" }),
    ])
}

pub fn webhooks() -> Vec<Row> {
    vec![
        webhook("acme-alerts", "alerts", "text", "acme-general"),
        webhook("acme-releases", "releases", "news", "acme-announcements"),
    ]
}

pub fn invite(slug: &str, kind: &str, channel: &str) -> Row {
    row(&[
        ("slug", slug),
        ("channel_slug", channel),
        ("max_age", "0"),
        ("max_uses", "0"),
        ("temporary", "false"),
        ("unique", "false"),
        ("on_text", if kind == "text" { "true" } else { "false" }),
        ("on_voice", if kind == "voice" { "true" } else { "false" }),
    ])
}

pub fn invites() -> Vec<Row> {
    vec![invite("acme-invite", "text", "acme-welcome")]
}

pub fn onboardings() -> Vec<Row> {
    vec![row(&[
        ("slug", "acme-onboarding"),
        ("enabled", "true"),
        ("mode", "0"),
        ("default_1", "acme-welcome"),
        ("default_2", "acme-general"),
        ("default_3", "acme-rules"),
        ("default_4", "acme-faq"),
        ("default_5", "acme-help"),
        ("default_6", "acme-showcase"),
        ("default_7", "acme-offtopic"),
    ])]
}

pub fn linked_role(slug: &str, double: bool) -> Row {
    row(&[
        ("slug", slug),
        ("application_id", APPLICATION_ID),
        ("single", if double { "false" } else { "true" }),
        ("double", if double { "true" } else { "false" }),
        ("key_1", "acme_commits"),
        ("name_1", "commits"),
        ("description_1", "acme commits merged"),
        ("type_1", "2"),
        ("key_2", "acme_member"),
        ("name_2", "member"),
        ("description_2", "acme member"),
        ("type_2", "7"),
    ])
}

pub fn linked_roles() -> Vec<Row> {
    vec![linked_role("acme-linked", false)]
}

pub fn member_roles() -> Vec<Row> {
    vec![row(&[
        ("slug", "acme-founder-grant"),
        ("user_id", USER_ID),
        ("role_slug", "acme-founder"),
        ("has_role", "true"),
    ])]
}

pub fn stickers() -> Vec<Row> {
    vec![row(&[
        ("slug", "acme-wave"),
        ("name", "wave"),
        ("tags", "wave,hello"),
        ("description", "acme wave sticker"),
        ("file", "stickers/acme-wave.png"),
    ])]
}

pub fn messages() -> Vec<Row> {
    vec![row(&[
        ("slug", "acme-rules-post"),
        ("channel_slug", "acme-rules"),
        ("content", "be kind"),
        ("pinned", "true"),
        ("tts", "false"),
    ])]
}

pub fn full_bindings() -> InputBindings {
    let mut b = bindings(roles(), categories(), full_channels(), permissions());
    b.set_records("settings", settings());
    b.set_records("everyone", everyone());
    b.set_records("widgets", widgets());
    b.set_records("system-channels", system_channels());
    b.set_records("news-channels", news_channels());
    b.set_records("webhooks", webhooks());
    b.set_records("invites", invites());
    b.set_records("onboardings", onboardings());
    b.set_records("linked-roles", linked_roles());
    b.set_records("member-roles", member_roles());
    b.set_records("stickers", stickers());
    b.set_records("messages", messages());
    b
}
