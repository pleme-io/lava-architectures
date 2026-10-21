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
