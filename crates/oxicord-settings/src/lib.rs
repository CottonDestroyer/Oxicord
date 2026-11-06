//! Discord keeps account preferences in one protobuf blob, served at
//! `/users/@me/settings-proto/1`. This crate reads the values the client
//! shows and builds the partial blob that changes them.
//!
//! The schema (section and field numbers) was read from the public web
//! client's generated definitions. We only know the fields listed in
//! [`DEFS`]; everything else in the blob is carried through untouched.

pub mod wire;

use std::collections::{BTreeMap, BTreeSet};

use base64::{Engine as _, engine::general_purpose::STANDARD};

/// A response or patch bigger than this is not something we will chew on.
const MAX_BLOB: usize = 1 << 20;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("settings data is malformed: {0}")]
    Malformed(&'static str),
    #[error("settings data is not valid base64")]
    Base64,
    #[error("settings data is too large")]
    TooLarge,
}

/// Every preference this client can show or change.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Key {
    RenderEmbeds,
    InlineAttachmentMedia,
    InlineEmbedMedia,
    RenderReactions,
    /// `ALWAYS`, `ON_CLICK` or `IF_MODERATOR`.
    RenderSpoilers,
    /// 0 off, 1 members without roles, 2 everyone.
    ExplicitContentFilter,
    /// 1 off, 2 non-friends, 3 everyone.
    DmSpamFilter,
    /// Bit mask: 2 friends of friends, 4 server members, 8 everyone else.
    FriendSourceFlags,
    /// True when new servers may not message us.
    DefaultGuildsRestricted,
    ShowLocalTime,
    ShowCurrentGame,
    DeveloperMode,
    /// 0 follow the system, 1 twelve hour, 2 twenty-four hour.
    HourCycle,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    Bool(bool),
    Number(u32),
    Text(String),
}

impl Value {
    pub fn as_bool(&self) -> bool {
        match self {
            Self::Bool(b) => *b,
            Self::Number(n) => *n != 0,
            Self::Text(t) => !t.is_empty(),
        }
    }

    pub fn as_number(&self) -> u32 {
        match self {
            Self::Number(n) => *n,
            Self::Bool(b) => u32::from(*b),
            Self::Text(_) => 0,
        }
    }

    pub fn as_text(&self) -> &str {
        match self {
            Self::Text(t) => t,
            _ => "",
        }
    }
}

/// How a value is laid out on the wire.
#[derive(Clone, Copy, Debug)]
enum Encoding {
    /// `google.protobuf.BoolValue`: a message holding one bool.
    BoolWrapper,
    UIntWrapper,
    StringWrapper,
    /// An enum or plain scalar, a bare varint.
    Varint,
}

struct Def {
    key: Key,
    section: u32,
    field: u32,
    encoding: Encoding,
    default: fn() -> Value,
}

const TEXT_AND_IMAGES: u32 = 6;
const PRIVACY: u32 = 8;
const STATUS: u32 = 11;
const APPEARANCE: u32 = 13;

macro_rules! def {
    ($key:ident, $section:expr, $field:expr, $enc:ident, $default:expr) => {
        Def {
            key: Key::$key,
            section: $section,
            field: $field,
            encoding: Encoding::$enc,
            default: || $default,
        }
    };
}

const DEFS: &[Def] = &[
    def!(RenderSpoilers, TEXT_AND_IMAGES, 4, StringWrapper, Value::Text("ON_CLICK".into())),
    def!(InlineAttachmentMedia, TEXT_AND_IMAGES, 9, BoolWrapper, Value::Bool(true)),
    def!(InlineEmbedMedia, TEXT_AND_IMAGES, 10, BoolWrapper, Value::Bool(true)),
    def!(RenderEmbeds, TEXT_AND_IMAGES, 12, BoolWrapper, Value::Bool(true)),
    def!(RenderReactions, TEXT_AND_IMAGES, 13, BoolWrapper, Value::Bool(true)),
    def!(ExplicitContentFilter, TEXT_AND_IMAGES, 19, UIntWrapper, Value::Number(1)),
    def!(DmSpamFilter, TEXT_AND_IMAGES, 27, Varint, Value::Number(0)),
    def!(DefaultGuildsRestricted, PRIVACY, 4, Varint, Value::Bool(false)),
    def!(FriendSourceFlags, PRIVACY, 11, UIntWrapper, Value::Number(14)),
    def!(ShowLocalTime, PRIVACY, 31, BoolWrapper, Value::Bool(false)),
    def!(ShowCurrentGame, STATUS, 3, BoolWrapper, Value::Bool(true)),
    def!(DeveloperMode, APPEARANCE, 2, Varint, Value::Bool(false)),
    def!(HourCycle, APPEARANCE, 9, Varint, Value::Number(0)),
];

fn def_of(key: Key) -> &'static Def {
    DEFS.iter()
        .find(|d| d.key == key)
        .expect("every key has a definition")
}

fn decode_value(encoding: Encoding, field: &wire::Field<'_>) -> Result<Value, Error> {
    let inner = |payload: &[u8]| -> Result<Option<u64>, Error> {
        wire::fields(payload)?
            .iter()
            .find(|f| f.number == 1)
            .map(wire::Field::varint)
            .transpose()
    };
    Ok(match encoding {
        Encoding::BoolWrapper => Value::Bool(inner(field.message()?)?.unwrap_or(0) != 0),
        Encoding::UIntWrapper => {
            Value::Number(u32::try_from(inner(field.message()?)?.unwrap_or(0)).unwrap_or(0))
        }
        Encoding::StringWrapper => {
            let text = wire::fields(field.message()?)?
                .iter()
                .find(|f| f.number == 1)
                .map(wire::Field::message)
                .transpose()?
                .map(|bytes| String::from_utf8_lossy(bytes).into_owned())
                .unwrap_or_default();
            Value::Text(text)
        }
        Encoding::Varint => Value::Number(u32::try_from(field.varint()?).unwrap_or(0)),
    })
}

fn encode_value(def: &Def, value: &Value, out: &mut Vec<u8>) {
    match def.encoding {
        Encoding::BoolWrapper => {
            let mut inner = Vec::new();
            if value.as_bool() {
                wire::put_varint(1, 1, &mut inner);
            }
            wire::put_message(def.field, &inner, out);
        }
        Encoding::UIntWrapper => {
            let mut inner = Vec::new();
            if value.as_number() != 0 {
                wire::put_varint(1, u64::from(value.as_number()), &mut inner);
            }
            wire::put_message(def.field, &inner, out);
        }
        Encoding::StringWrapper => {
            let mut inner = Vec::new();
            if !value.as_text().is_empty() {
                wire::put_message(1, value.as_text().as_bytes(), &mut inner);
            }
            wire::put_message(def.field, &inner, out);
        }
        Encoding::Varint => wire::put_varint(def.field, u64::from(value.as_number()), out),
    }
}

/// A change ready to send.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Patch {
    /// Base64 of a settings message holding only the sections that changed.
    pub settings: String,
    /// The data version the change was built against, so the server can
    /// refuse it if another client got there first.
    pub required_data_version: u32,
}

/// Field of the status section that holds the custom status message.
const CUSTOM_STATUS: u32 = 2;

/// The words and emoji next to your name, with the moment they expire.
/// `emoji_id` is set for a server emoji, `emoji_name` alone is a unicode one.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CustomStatus {
    pub text: String,
    pub emoji_id: Option<u64>,
    pub emoji_name: Option<String>,
    pub expires_at_ms: Option<u64>,
}

impl CustomStatus {
    fn read(bytes: &[u8]) -> Result<Self, Error> {
        let mut status = Self::default();
        for field in wire::fields(bytes)? {
            match field.number {
                1 => status.text = field.text()?.to_owned(),
                2 => status.emoji_id = Some(field.fixed64()?).filter(|id| *id != 0),
                3 => status.emoji_name = Some(field.text()?.to_owned()).filter(|n| !n.is_empty()),
                4 => status.expires_at_ms = Some(field.fixed64()?).filter(|at| *at != 0),
                _ => {}
            }
        }
        Ok(status)
    }

    fn write(&self, created_at_ms: u64, out: &mut Vec<u8>) {
        if !self.text.is_empty() {
            wire::put_message(1, self.text.as_bytes(), out);
        }
        if let Some(id) = self.emoji_id {
            wire::put_fixed64(2, id, out);
        }
        if let Some(name) = self.emoji_name.as_deref().filter(|n| !n.is_empty()) {
            wire::put_message(3, name.as_bytes(), out);
        }
        if let Some(at) = self.expires_at_ms {
            wire::put_fixed64(4, at, out);
        }
        wire::put_fixed64(5, created_at_ms, out);
    }

    fn is_empty(&self) -> bool {
        self.text.is_empty() && self.emoji_id.is_none() && self.emoji_name.is_none()
    }
}

/// A group of servers in the rail. A server outside any folder shows up as
/// a folder without an id holding just itself.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GuildFolder {
    pub id: Option<i64>,
    pub name: Option<String>,
    pub color: Option<u32>,
    pub guilds: Vec<u64>,
}

/// What we know about the account's preferences.
#[derive(Clone, Debug, Default)]
pub struct AccountSettings {
    data_version: u32,
    /// Raw bytes of each section we understand, kept so a patch can repeat
    /// the fields it isn't changing.
    sections: BTreeMap<u32, Vec<u8>>,
    values: BTreeMap<Key, Value>,
    /// The rail layout, empty when the account never arranged it.
    folders: Vec<GuildFolder>,
}

impl AccountSettings {
    /// Reads the `settings` string of a settings-proto response.
    pub fn decode(base64: &str) -> Result<Self, Error> {
        if base64.len() > MAX_BLOB * 2 {
            return Err(Error::TooLarge);
        }
        let bytes = STANDARD.decode(base64).map_err(|_| Error::Base64)?;
        if bytes.len() > MAX_BLOB {
            return Err(Error::TooLarge);
        }

        let wanted: BTreeSet<u32> = DEFS.iter().map(|d| d.section).collect();
        let mut settings = Self::default();
        for field in wire::fields(&bytes)? {
            match field.number {
                // `versions`, whose field 3 is the data version.
                1 => {
                    for inner in wire::fields(field.message()?)? {
                        if inner.number == 3 {
                            settings.data_version = u32::try_from(inner.varint()?).unwrap_or(0);
                        }
                    }
                }
                14 => settings.folders = read_folders(field.message()?)?,
                n if wanted.contains(&n) => {
                    settings.sections.insert(n, field.message()?.to_vec());
                }
                _ => {}
            }
        }

        for def in DEFS {
            let Some(section) = settings.sections.get(&def.section) else {
                continue;
            };
            // The last occurrence wins, as protobuf specifies.
            if let Some(field) = wire::fields(section)?
                .iter()
                .rev()
                .find(|f| f.number == def.field)
            {
                let value = match ((def.default)(), decode_value(def.encoding, field)?) {
                    // A bare varint is a bool when that is what the setting holds.
                    (Value::Bool(_), Value::Number(n)) => Value::Bool(n != 0),
                    (_, value) => value,
                };
                settings.values.insert(def.key, value);
            }
        }
        Ok(settings)
    }

    /// Servers and folders in the order the user arranged them.
    pub fn folders(&self) -> &[GuildFolder] {
        &self.folders
    }

    pub fn data_version(&self) -> u32 {
        self.data_version
    }

    /// The stored value, or Discord's default when it was never set.
    pub fn get(&self, key: Key) -> Value {
        self.values
            .get(&key)
            .cloned()
            .unwrap_or_else(|| (def_of(key).default)())
    }

    /// Builds the patch for `changes` against what we last read.
    pub fn patch(&self, changes: &[(Key, Value)]) -> Result<Patch, Error> {
        let mut message = Vec::new();
        let sections: BTreeSet<u32> = changes.iter().map(|(k, _)| def_of(*k).section).collect();

        for section in sections {
            let edited: Vec<&(Key, Value)> = changes
                .iter()
                .filter(|(k, _)| def_of(*k).section == section)
                .collect();
            let edited_fields: BTreeSet<u32> =
                edited.iter().map(|(k, _)| def_of(*k).field).collect();

            // Repeat every field we aren't touching, so nothing the server
            // keeps in this section is wiped by the update.
            let mut subtree = Vec::new();
            if let Some(current) = self.sections.get(&section) {
                for field in wire::fields(current)? {
                    if !edited_fields.contains(&field.number) {
                        subtree.extend_from_slice(field.raw);
                    }
                }
            }
            for (key, value) in edited {
                encode_value(def_of(*key), value, &mut subtree);
            }
            wire::put_message(section, &subtree, &mut message);
        }

        Ok(Patch {
            settings: STANDARD.encode(message),
            required_data_version: self.data_version,
        })
    }

    /// The custom status in effect at `now_ms`. One that has expired, or
    /// holds nothing, counts as none.
    pub fn custom_status(&self, now_ms: u64) -> Option<CustomStatus> {
        let section = self.sections.get(&STATUS)?;
        let field = wire::fields(section).ok()?.into_iter().rev().find(|f| f.number == CUSTOM_STATUS)?;
        let status = CustomStatus::read(field.message().ok()?).ok()?;
        let expired = status.expires_at_ms.is_some_and(|at| at <= now_ms);
        (!status.is_empty() && !expired).then_some(status)
    }

    /// Sets or clears the custom status, keeping the rest of the status
    /// section as the server has it. The change shows here at once.
    pub fn set_custom_status(&mut self, status: Option<&CustomStatus>, now_ms: u64) -> Result<Patch, Error> {
        let mut subtree = Vec::new();
        if let Some(current) = self.sections.get(&STATUS) {
            for field in wire::fields(current)? {
                if field.number != CUSTOM_STATUS {
                    subtree.extend_from_slice(field.raw);
                }
            }
        }
        if let Some(status) = status.filter(|s| !s.is_empty()) {
            let mut inner = Vec::new();
            status.write(now_ms, &mut inner);
            wire::put_message(CUSTOM_STATUS, &inner, &mut subtree);
        }

        let mut message = Vec::new();
        wire::put_message(STATUS, &subtree, &mut message);
        self.sections.insert(STATUS, subtree);
        Ok(Patch {
            settings: STANDARD.encode(message),
            required_data_version: self.data_version,
        })
    }

    /// Records changes the server accepted.
    pub fn apply(&mut self, changes: &[(Key, Value)]) -> Result<(), Error> {
        // Re-decoding our own patch keeps the raw sections and the typed
        // values in step.
        let patch = self.patch(changes)?;
        let bytes = STANDARD.decode(&patch.settings).map_err(|_| Error::Base64)?;
        for field in wire::fields(&bytes)? {
            self.sections.insert(field.number, field.message()?.to_vec());
        }
        for (key, value) in changes {
            self.values.insert(*key, value.clone());
        }
        Ok(())
    }
}

/// The `GuildFolders` message: field 1 holds the folders in order.
fn read_folders(bytes: &[u8]) -> Result<Vec<GuildFolder>, Error> {
    let mut out = Vec::new();
    for field in wire::fields(bytes)? {
        if field.number != 1 {
            continue;
        }
        let mut folder = GuildFolder::default();
        for inner in wire::fields(field.message()?)? {
            match inner.number {
                1 => folder.guilds.extend(inner.fixed64s()?),
                // The wrappers below are protobuf's Int64Value, StringValue
                // and UInt32Value, each holding its value in field 1.
                2 => folder.id = wrapped(&inner)?.and_then(|f| f.varint().ok()).map(|v| v as i64),
                3 => folder.name = wrapped(&inner)?.and_then(|f| f.text().ok()).map(str::to_owned),
                4 => folder.color = wrapped(&inner)?.and_then(|f| f.varint().ok()).and_then(|v| u32::try_from(v).ok()),
                _ => {}
            }
        }
        out.push(folder);
    }
    Ok(out)
}

fn wrapped<'a>(field: &wire::Field<'a>) -> Result<Option<wire::Field<'a>>, Error> {
    Ok(wire::fields(field.message()?)?.into_iter().find(|f| f.number == 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A blob with data version 7, an unrelated field 99 inside the text
    /// section, and render_embeds switched off.
    fn sample() -> String {
        let mut text = Vec::new();
        wire::put_message(12, &[], &mut text); // render_embeds = false
        wire::put_varint(99, 5, &mut text); // a field we know nothing about

        let mut versions = Vec::new();
        wire::put_varint(3, 7, &mut versions);

        let mut blob = Vec::new();
        wire::put_message(1, &versions, &mut blob);
        wire::put_message(TEXT_AND_IMAGES, &text, &mut blob);
        STANDARD.encode(blob)
    }

    #[test]
    fn reads_values_defaults_and_the_version() {
        let settings = AccountSettings::decode(&sample()).unwrap();
        assert_eq!(settings.data_version(), 7);
        assert!(!settings.get(Key::RenderEmbeds).as_bool());
        // Never set, so Discord's default.
        assert!(settings.get(Key::InlineAttachmentMedia).as_bool());
        assert_eq!(settings.get(Key::FriendSourceFlags).as_number(), 14);
        assert_eq!(settings.get(Key::RenderSpoilers).as_text(), "ON_CLICK");
    }

    #[test]
    fn the_rail_layout_is_read_from_field_14() {
        let wrap_varint = |v: u64| {
            let mut inner = Vec::new();
            wire::put_varint(1, v, &mut inner);
            inner
        };
        let mut folder = Vec::new();
        let mut ids = Vec::new();
        ids.extend(11u64.to_le_bytes());
        ids.extend(12u64.to_le_bytes());
        wire::put_message(1, &ids, &mut folder);
        wire::put_message(2, &wrap_varint(77), &mut folder);
        let mut name = Vec::new();
        wire::put_message(1, b"Friends", &mut name);
        wire::put_message(3, &name, &mut folder);
        wire::put_message(4, &wrap_varint(0x5865f2), &mut folder);

        let mut lone = Vec::new();
        wire::put_message(1, &13u64.to_le_bytes(), &mut lone);

        let mut folders = Vec::new();
        wire::put_message(1, &folder, &mut folders);
        wire::put_message(1, &lone, &mut folders);
        let mut blob = Vec::new();
        wire::put_message(14, &folders, &mut blob);

        let settings = AccountSettings::decode(&STANDARD.encode(blob)).unwrap();
        assert_eq!(
            settings.folders(),
            [
                GuildFolder { id: Some(77), name: Some("Friends".into()), color: Some(0x5865f2), guilds: vec![11, 12] },
                GuildFolder { id: None, name: None, color: None, guilds: vec![13] },
            ]
        );
    }

    #[test]
    fn a_patch_changes_only_its_field_and_repeats_the_rest() {
        let settings = AccountSettings::decode(&sample()).unwrap();
        let patch = settings
            .patch(&[(Key::RenderEmbeds, Value::Bool(true))])
            .unwrap();
        assert_eq!(patch.required_data_version, 7);

        // Reading the patch back as a settings blob shows the new value
        // and the unknown field still there.
        let bytes = STANDARD.decode(&patch.settings).unwrap();
        let top = wire::fields(&bytes).unwrap();
        assert_eq!(top.len(), 1, "only the touched section is sent");
        let section = wire::fields(top[0].message().unwrap()).unwrap();
        assert!(section.iter().any(|f| f.number == 99 && f.varint().unwrap() == 5));
        let decoded = AccountSettings::decode(&patch.settings).unwrap();
        assert!(decoded.get(Key::RenderEmbeds).as_bool());
    }

    #[test]
    fn every_encoding_round_trips() {
        let mut settings = AccountSettings::default();
        let changes = [
            (Key::RenderSpoilers, Value::Text("ALWAYS".into())),
            (Key::ExplicitContentFilter, Value::Number(2)),
            (Key::DmSpamFilter, Value::Number(3)),
            (Key::DefaultGuildsRestricted, Value::Bool(true)),
            (Key::ShowCurrentGame, Value::Bool(false)),
            (Key::HourCycle, Value::Number(2)),
            (Key::DeveloperMode, Value::Bool(true)),
        ];
        let patch = settings.patch(&changes).unwrap();
        settings.apply(&changes).unwrap();

        let reread = AccountSettings::decode(&patch.settings).unwrap();
        for (key, value) in &changes {
            assert_eq!(reread.get(*key), *value, "{key:?}");
            assert_eq!(settings.get(*key), *value, "{key:?}");
        }
    }

    #[test]
    fn a_custom_status_round_trips_and_leaves_its_neighbours_alone() {
        // The status section as the web client sends it: the online state,
        // a status we are about to replace, and show_current_game.
        let mut status = Vec::new();
        wire::put_message(1, b"invisible", &mut status);
        let mut old = Vec::new();
        wire::put_message(1, b"old words", &mut old);
        wire::put_message(CUSTOM_STATUS, &old, &mut status);
        wire::put_message(3, &[], &mut status);
        let mut blob = Vec::new();
        wire::put_message(STATUS, &status, &mut blob);
        let mut settings = AccountSettings::decode(&STANDARD.encode(blob)).unwrap();
        assert_eq!(settings.custom_status(0).unwrap().text, "old words");

        let new = CustomStatus {
            text: "busy".into(),
            emoji_id: None,
            emoji_name: Some("\u{1f525}".into()),
            expires_at_ms: Some(2_000),
        };
        let patch = settings.set_custom_status(Some(&new), 1_000).unwrap();
        assert_eq!(settings.custom_status(1_500), Some(new.clone()));
        // Gone once the clock passes its expiry.
        assert_eq!(settings.custom_status(2_000), None);

        let reread = AccountSettings::decode(&patch.settings).unwrap();
        assert_eq!(reread.custom_status(1_500), Some(new));
        let bytes = STANDARD.decode(&patch.settings).unwrap();
        let section = wire::fields(wire::fields(&bytes).unwrap()[0].message().unwrap()).unwrap();
        let numbers: Vec<u32> = section.iter().map(|f| f.number).collect();
        assert_eq!(numbers, [1, 3, 2], "the online state and show_current_game survive");

        let patch = settings.set_custom_status(None, 3_000).unwrap();
        assert_eq!(settings.custom_status(0), None);
        assert_eq!(AccountSettings::decode(&patch.settings).unwrap().custom_status(0), None);
    }

    #[test]
    fn bad_input_is_an_error_not_a_panic() {
        assert!(matches!(AccountSettings::decode("%%%"), Err(Error::Base64)));
        let garbage = STANDARD.encode([0xff, 0xff]);
        assert!(AccountSettings::decode(&garbage).is_err());
    }
}
