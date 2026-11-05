//! Interactive message parts that bots attach: buttons, menus and the newer
//! layout components. Parsing is deliberately forgiving. Discord keeps adding
//! kinds, and one we don't know must never stop the message around it from
//! showing, so unknown kinds become [`Component::Unknown`].

use serde::{Deserialize, Deserializer};
use serde_json::Value;

use crate::{Emoji, Snowflake};

/// `flags` bit on a message whose body is made entirely of components.
pub const FLAG_COMPONENTS_V2: u64 = 1 << 15;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonStyle {
    Primary,
    Secondary,
    Success,
    Danger,
    /// Opens a web address instead of calling the bot.
    Link,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectOption {
    pub label: String,
    pub value: String,
    pub description: Option<String>,
    pub emoji: Option<Emoji>,
    pub default: bool,
}

/// A picture or file reference inside a layout component.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MediaRef {
    pub url: String,
    pub proxy_url: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub description: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Component {
    ActionRow(Vec<Component>),
    Button {
        style: ButtonStyle,
        label: Option<String>,
        custom_id: Option<String>,
        url: Option<String>,
        emoji: Option<Emoji>,
        disabled: bool,
    },
    /// Menus. `kind` is Discord's component type: 3 for text options, 5 to 8
    /// for users, roles, mentionables and channels.
    Select {
        kind: u8,
        custom_id: String,
        placeholder: Option<String>,
        options: Vec<SelectOption>,
        disabled: bool,
    },
    Section {
        texts: Vec<Component>,
        accessory: Option<Box<Component>>,
    },
    Text(String),
    Thumbnail(MediaRef),
    Gallery(Vec<MediaRef>),
    File {
        media: MediaRef,
        name: Option<String>,
    },
    Separator {
        divider: bool,
    },
    Container {
        children: Vec<Component>,
        accent: Option<u32>,
    },
    Unknown,
}

impl Component {
    /// Reads one component, with `Unknown` for anything unrecognised.
    pub fn from_value(value: &Value) -> Self {
        let text = |key: &str| value.get(key).and_then(Value::as_str).map(str::to_owned);
        let flag = |key: &str| value.get(key).and_then(Value::as_bool).unwrap_or(false);
        let list = |key: &str| -> Vec<Component> {
            value
                .get(key)
                .and_then(Value::as_array)
                .map(|items| items.iter().map(Component::from_value).collect())
                .unwrap_or_default()
        };
        let media = |key: &str| value.get(key).and_then(MediaRef::from_value);

        match value.get("type").and_then(Value::as_u64) {
            Some(1) => Self::ActionRow(list("components")),
            Some(2) => Self::Button {
                style: match value.get("style").and_then(Value::as_u64) {
                    Some(2) => ButtonStyle::Secondary,
                    Some(3) => ButtonStyle::Success,
                    Some(4) => ButtonStyle::Danger,
                    Some(5) => ButtonStyle::Link,
                    _ => ButtonStyle::Primary,
                },
                label: text("label"),
                custom_id: text("custom_id"),
                url: text("url"),
                emoji: value.get("emoji").and_then(|e| serde_json::from_value(e.clone()).ok()),
                disabled: flag("disabled"),
            },
            Some(kind @ (3 | 5..=8)) => Self::Select {
                kind: kind as u8,
                custom_id: text("custom_id").unwrap_or_default(),
                placeholder: text("placeholder"),
                options: value
                    .get("options")
                    .and_then(Value::as_array)
                    .map(|o| o.iter().filter_map(select_option).collect())
                    .unwrap_or_default(),
                disabled: flag("disabled"),
            },
            Some(9) => Self::Section {
                texts: list("components"),
                accessory: value.get("accessory").map(|a| Box::new(Component::from_value(a))),
            },
            Some(10) => Self::Text(text("content").unwrap_or_default()),
            Some(11) => media("media").map_or(Self::Unknown, Self::Thumbnail),
            Some(12) => Self::Gallery(
                value
                    .get("items")
                    .and_then(Value::as_array)
                    .map(|items| items.iter().filter_map(|i| i.get("media").and_then(MediaRef::from_value)).collect())
                    .unwrap_or_default(),
            ),
            Some(13) => media("file").map_or(Self::Unknown, |media| Self::File { media, name: text("name") }),
            Some(14) => Self::Separator { divider: value.get("divider").and_then(Value::as_bool).unwrap_or(true) },
            Some(17) => Self::Container {
                children: list("components"),
                accent: value.get("accent_color").and_then(Value::as_u64).map(|c| c as u32),
            },
            _ => Self::Unknown,
        }
    }
}

fn select_option(value: &Value) -> Option<SelectOption> {
    Some(SelectOption {
        label: value.get("label")?.as_str()?.to_owned(),
        value: value.get("value")?.as_str()?.to_owned(),
        description: value.get("description").and_then(Value::as_str).map(str::to_owned),
        emoji: value.get("emoji").and_then(|e| serde_json::from_value(e.clone()).ok()),
        default: value.get("default").and_then(Value::as_bool).unwrap_or(false),
    })
}

impl MediaRef {
    fn from_value(value: &Value) -> Option<Self> {
        let text = |key: &str| value.get(key).and_then(Value::as_str).map(str::to_owned);
        let number = |key: &str| value.get(key).and_then(Value::as_u64).map(|n| n as u32);
        Some(Self {
            url: text("url")?,
            proxy_url: text("proxy_url"),
            width: number("width"),
            height: number("height"),
            description: text("description"),
        })
    }
}

/// A message's components. Anything that isn't a list reads as empty.
pub fn components<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<Component>, D::Error> {
    Ok(match Option::<Value>::deserialize(deserializer)? {
        Some(Value::Array(items)) => items
            .iter()
            .map(Component::from_value)
            .filter(|c| *c != Component::Unknown)
            .collect(),
        _ => Vec::new(),
    })
}

/// Like [`components`] but keeps "not mentioned" apart from "now empty", which
/// matters for partial updates.
pub fn optional_components<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Vec<Component>>, D::Error> {
    Ok(match Option::<Value>::deserialize(deserializer)? {
        Some(Value::Array(items)) => Some(
            items
                .iter()
                .map(Component::from_value)
                .filter(|c| *c != Component::Unknown)
                .collect(),
        ),
        _ => None,
    })
}

/// The bot that owns a message's components: the application it came from,
/// or its author when that is a bot.
pub fn owner(application_id: Option<Snowflake>, author: Snowflake) -> Snowflake {
    application_id.unwrap_or(author)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn parse(value: Value) -> Vec<Component> {
        #[derive(Deserialize)]
        struct Holder {
            #[serde(default, deserialize_with = "components")]
            components: Vec<Component>,
        }
        serde_json::from_value::<Holder>(json!({ "components": value })).unwrap().components
    }

    #[test]
    fn buttons_and_menus_inside_rows() {
        let rows = parse(json!([{
            "type": 1, "components": [
                { "type": 2, "style": 3, "label": "Accept", "custom_id": "ok" },
                { "type": 2, "style": 5, "label": "Docs", "url": "https://example.com", "disabled": true },
                { "type": 3, "custom_id": "pick", "placeholder": "Choose",
                  "options": [{ "label": "A", "value": "a", "default": true }, { "label": "B" }] }
            ]
        }]));
        let Component::ActionRow(items) = &rows[0] else { panic!("not a row") };
        assert!(matches!(&items[0], Component::Button { style: ButtonStyle::Success, custom_id: Some(id), .. } if id == "ok"));
        assert!(matches!(&items[1], Component::Button { style: ButtonStyle::Link, disabled: true, .. }));
        // The second option has no value, so it is dropped rather than failing.
        let Component::Select { options, .. } = &items[2] else { panic!("not a menu") };
        assert_eq!(options.len(), 1);
        assert!(options[0].default);
    }

    #[test]
    fn layout_components_nest() {
        let parsed = parse(json!([{
            "type": 17, "accent_color": 16711680, "components": [
                { "type": 10, "content": "hello" },
                { "type": 14, "divider": false },
                { "type": 9, "components": [{ "type": 10, "content": "side" }],
                  "accessory": { "type": 11, "media": { "url": "https://cdn.example/a.png", "width": 10, "height": 20 } } },
                { "type": 12, "items": [{ "media": { "url": "https://cdn.example/b.png" } }] }
            ]
        }]));
        let Component::Container { children, accent } = &parsed[0] else { panic!("not a container") };
        assert_eq!(*accent, Some(0xff0000));
        assert_eq!(children[0], Component::Text("hello".into()));
        assert_eq!(children[1], Component::Separator { divider: false });
        assert!(matches!(&children[2], Component::Section { accessory: Some(a), .. } if matches!(**a, Component::Thumbnail(_))));
        assert!(matches!(&children[3], Component::Gallery(items) if items.len() == 1));
    }

    #[test]
    fn unknown_kinds_and_junk_are_skipped_not_fatal() {
        assert!(parse(json!([{ "type": 99 }, 5, "x", null])).is_empty());
        assert!(parse(json!("not a list")).is_empty());
        assert!(parse(Value::Null).is_empty());
    }
}
