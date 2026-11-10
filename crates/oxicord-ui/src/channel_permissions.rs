//! What the channel permissions screen lists and how an overwrite reads. The
//! names, groups and descriptions are the live client's, scraped from its
//! Permissions page for a text channel and for a voice channel (stable 670026).

use oxicord_model::{ChannelType, Overwrite, OverwriteKind, Permissions, Snowflake};

/// One permission row.
pub struct Perm {
    pub flag: Permissions,
    pub name: &'static str,
    pub description: &'static str,
}

/// A titled run of rows, a fieldset on the live page.
pub struct Group {
    pub title: &'static str,
    pub perms: &'static [Perm],
}

/// Where an overwrite stands on one permission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tri {
    Deny,
    Inherit,
    Allow,
}

const fn perm(flag: Permissions, name: &'static str, description: &'static str) -> Perm {
    Perm { flag, name, description }
}

const MANAGE_PERMISSIONS: Perm =
    perm(Permissions::MANAGE_ROLES, "Manage Permissions", "Allows members to change this channel\u{2019}s permissions.");
const MANAGE_WEBHOOKS: Perm = perm(
    Permissions::MANAGE_WEBHOOKS,
    "Manage Webhooks",
    "Allows members to create, edit, or delete webhooks in this channel, which can post messages from other apps or sites.",
);
const EMBED_LINKS: Perm = perm(
    Permissions::EMBED_LINKS,
    "Embed Links",
    "Allows links that members share to show embedded content in this channel.",
);
const ATTACH_FILES: Perm =
    perm(Permissions::ATTACH_FILES, "Attach Files", "Allows members to upload files or media in this channel.");
const EXTERNAL_EMOJI: Perm = perm(
    Permissions::USE_EXTERNAL_EMOJIS,
    "Use External Emoji",
    "Allows members to use emoji from other servers in this channel, if they\u{2019}re a Discord Nitro member.",
);
const EXTERNAL_STICKERS: Perm = perm(
    Permissions::USE_EXTERNAL_STICKERS,
    "Use External Stickers",
    "Allows members to use stickers from other servers in this channel, if they\u{2019}re a Discord Nitro member.",
);
const MENTION_EVERYONE: Perm = perm(
    Permissions::MENTION_EVERYONE,
    "Mention @everyone, @here, and All Roles",
    "Allows members to use @everyone or @here (only online members) in this channel. They can also @mention all \
     roles in this channel, even if the role\u{2019}s \u{201c}Allow anyone to mention this role\u{201d} permission \
     is disabled.",
);
const MANAGE_MESSAGES: Perm = perm(
    Permissions::MANAGE_MESSAGES,
    "Manage Messages",
    "Allows members to delete or remove embeds from messages by other members in this channel.",
);
const BYPASS_SLOWMODE: Perm = perm(
    Permissions::BYPASS_SLOWMODE,
    "Bypass Slowmode",
    "Allows members to send messages in this channel without being affected by slowmode.",
);
const READ_HISTORY: Perm = perm(
    Permissions::READ_MESSAGE_HISTORY,
    "Read Message History",
    "Allows members to read previous messages sent in this channel. If this permission is disabled, members only see \
     messages sent when they are online.",
);
const SEND_TTS: Perm = perm(
    Permissions::SEND_TTS_MESSAGES,
    "Send Text-to-Speech Messages",
    "Allows members to send text-to-speech messages in this channel by starting a message with /tts. These messages \
     can be heard by anyone focused on this channel.",
);
const SEND_VOICE: Perm = perm(
    Permissions::SEND_VOICE_MESSAGES,
    "Send Voice Messages",
    "Allows members to send voice messages in this channel. Requires the \"Attach Files\" permission as well.",
);
const CREATE_POLLS: Perm =
    perm(Permissions::SEND_POLLS, "Create Polls", "Allows members to create polls in this channel.");
const SEND_MESSAGES: Perm =
    perm(Permissions::SEND_MESSAGES, "Send Messages", "Allows members to send messages in this channel.");

const TEXT_GENERAL: &[Perm] = &[
    perm(
        Permissions::VIEW_CHANNEL,
        "View Channel",
        "Allows members to view this channel by default. Disabling this for @everyone will make this channel private.",
    ),
    perm(
        Permissions::MANAGE_CHANNELS,
        "Manage Channel",
        "Allows members to change this channel\u{2019}s name, description, and text settings. They can also delete \
         the channel.",
    ),
    MANAGE_PERMISSIONS,
    MANAGE_WEBHOOKS,
];

const TEXT_INVITE: Perm = perm(
    Permissions::CREATE_INSTANT_INVITE,
    "Create Invite",
    "Allows members to invite new people to this server via a direct invite link to this channel.",
);

const TEXT_MEMBERSHIP: &[Perm] = &[TEXT_INVITE];

const TEXT_TEXT: &[Perm] = &[
    SEND_MESSAGES,
    perm(
        Permissions::SEND_MESSAGES_IN_THREADS,
        "Send Messages in Threads",
        "Allow members to send messages in threads under this channel.",
    ),
    perm(
        Permissions::CREATE_PUBLIC_THREADS,
        "Create Public Threads",
        "Allow members to create threads that everyone in this channel can view.",
    ),
    perm(
        Permissions::CREATE_PRIVATE_THREADS,
        "Create Private Threads",
        "Allow members to create invite-only threads in this channel.",
    ),
    EMBED_LINKS,
    ATTACH_FILES,
    perm(
        Permissions::ADD_REACTIONS,
        "Add Reactions",
        "Allows members to add new emoji reactions to a message in this channel. If this permission is disabled, \
         members can still react using any existing reactions on a message.",
    ),
    EXTERNAL_EMOJI,
    EXTERNAL_STICKERS,
    MENTION_EVERYONE,
    MANAGE_MESSAGES,
    perm(Permissions::PIN_MESSAGES, "Pin Messages", "Allows members to pin or unpin any message in this channel."),
    BYPASS_SLOWMODE,
    perm(
        Permissions::MANAGE_THREADS,
        "Manage Threads",
        "Allows members to rename, delete, close, and turn on slow mode for threads in this channel. They can also \
         view private threads.",
    ),
    READ_HISTORY,
    SEND_TTS,
    SEND_VOICE,
    CREATE_POLLS,
];

const APPS: &[Perm] = &[
    perm(
        Permissions::USE_APPLICATION_COMMANDS,
        "Use Application Commands",
        "Allows members to use commands from applications in this channel, including slash commands and context menu \
         commands.",
    ),
    perm(
        Permissions::USE_EMBEDDED_ACTIVITIES,
        "Use Activities",
        "Allows members to use Activities in this channel.",
    ),
    perm(
        Permissions::USE_EXTERNAL_APPS,
        "Use External Apps",
        "Allows apps that members have added to their account to post messages in this channel. When disabled, the \
         messages will be private.",
    ),
];

const TEXT_GROUPS: &[Group] = &[
    Group { title: "General Channel Permissions", perms: TEXT_GENERAL },
    Group { title: "Membership Permissions", perms: TEXT_MEMBERSHIP },
    Group { title: "Text Channel Permissions", perms: TEXT_TEXT },
    Group { title: "Apps Permissions", perms: APPS },
];

const VOICE_GENERAL: &[Perm] = &[
    perm(
        Permissions::VIEW_CHANNEL,
        "View Channel",
        "Allows members to view this channel by default. Disabling this and the Connect permission for @everyone will \
         make this channel private.",
    ),
    perm(
        Permissions::MANAGE_CHANNELS,
        "Manage Channel",
        "Allows members to change this channel\u{2019}s name, description, and voice settings. They can also delete \
         the channel.",
    ),
    MANAGE_PERMISSIONS,
    MANAGE_WEBHOOKS,
];

const VOICE_MEMBERSHIP: &[Perm] = &[perm(
    Permissions::CREATE_INSTANT_INVITE,
    "Create Invite",
    "Allows members to invite new people to this server via a direct invite link to this channel. The recipient will \
     automatically join the voice channel if they have permission to connect.",
)];

const VOICE_VOICE: &[Perm] = &[
    perm(
        Permissions::CONNECT,
        "Connect",
        "Allows members to join this voice channel and hear others. Disabling this and the View Channel permissions \
         for @everyone will make this channel private.",
    ),
    perm(
        Permissions::SPEAK,
        "Speak",
        "Allows members to talk in this voice channel. If this permission is disabled, members are default muted \
         until somebody with the \u{201c}Mute Members\u{201d} permission un-mutes them.",
    ),
    perm(
        Permissions::STREAM,
        "Video",
        "Allows members to share their video, screen share, or stream a game in this voice channel.",
    ),
    perm(
        Permissions::USE_SOUNDBOARD,
        "Use Soundboard",
        "Allows members to send sounds from server soundboard in this voice channel.",
    ),
    perm(
        Permissions::USE_EXTERNAL_SOUNDS,
        "Use External Sounds",
        "Allows members to use sounds from other servers, if they're a Discord Nitro member.",
    ),
    perm(
        Permissions::USE_VAD,
        "Use Voice Activity",
        "Allows members to speak in this voice channel by simply talking. If this permission is disabled, members \
         are required to use Push-to-talk. Good for controlling background noise or noisy members.",
    ),
    perm(
        Permissions::PRIORITY_SPEAKER,
        "Priority Speaker",
        "Allows members to be more easily heard in this voice channel. When activated, the volume of others without \
         this permission will be automatically lowered. Priority Speaker is activated by using the Push to Talk \
         (Priority) keybind.",
    ),
    perm(
        Permissions::MUTE_MEMBERS,
        "Mute Members",
        "Allows members to mute other members in this voice channel for everyone.",
    ),
    perm(
        Permissions::DEAFEN_MEMBERS,
        "Deafen Members",
        "Allows members to deafen other members in this voice channel, which means they won\u{2019}t be able to \
         speak or hear others.",
    ),
    perm(
        Permissions::MOVE_MEMBERS,
        "Move Members",
        "Allows members to disconnect other members from this channel. They will also be able to move somebody into \
         other channels that they have this permission in.",
    ),
    perm(
        Permissions::SET_VOICE_CHANNEL_STATUS,
        "Set Voice Channel Status",
        "Allows members to create and edit voice channel status.",
    ),
];

const VOICE_CHAT: &[Perm] = &[
    SEND_MESSAGES,
    EMBED_LINKS,
    ATTACH_FILES,
    perm(
        Permissions::ADD_REACTIONS,
        "Add Reactions",
        "Allows members to add new emoji reactions to a message in this channel and send reactions in voice \
         channels. If this permission is disabled, members can still react using any existing reactions on a message.",
    ),
    EXTERNAL_EMOJI,
    EXTERNAL_STICKERS,
    MENTION_EVERYONE,
    MANAGE_MESSAGES,
    BYPASS_SLOWMODE,
    READ_HISTORY,
    SEND_TTS,
    SEND_VOICE,
    CREATE_POLLS,
];

const VOICE_EVENTS: &[Perm] = &[
    perm(Permissions::CREATE_EVENTS, "Create Events", "Allows members to create events in this channel."),
    perm(Permissions::MANAGE_EVENTS, "Manage Events", "Allows members to edit and cancel events in this channel."),
];

const VOICE_GROUPS: &[Group] = &[
    Group { title: "General Channel Permissions", perms: VOICE_GENERAL },
    Group { title: "Membership Permissions", perms: VOICE_MEMBERSHIP },
    Group { title: "Voice Channel Permissions", perms: VOICE_VOICE },
    Group { title: "Voice Channel Chat Permissions", perms: VOICE_CHAT },
    Group { title: "Events Permissions", perms: VOICE_EVENTS },
    Group { title: "Apps Permissions", perms: APPS },
];

/// The groups a channel of this kind lists. Other kinds (categories, forums,
/// announcements) take the text list, which is what the live client draws for
/// a text channel and the nearest thing for the rest.
pub fn catalog(kind: ChannelType) -> &'static [Group] {
    match kind {
        ChannelType::GuildVoice | ChannelType::GuildStageVoice => VOICE_GROUPS,
        _ => TEXT_GROUPS,
    }
}

/// How an overwrite reads on one permission. No overwrite is inheriting.
pub fn state_of(overwrite: Option<&Overwrite>, flag: Permissions) -> Tri {
    match overwrite {
        Some(o) if o.deny.contains(flag) => Tri::Deny,
        Some(o) if o.allow.contains(flag) => Tri::Allow,
        _ => Tri::Inherit,
    }
}

/// The overwrite `id` has in a channel, if any.
pub fn find(overwrites: &[Overwrite], id: Snowflake) -> Option<&Overwrite> {
    overwrites.iter().find(|o| o.id == id)
}

/// A channel is private when `@everyone` (the role with the server's id)
/// is denied viewing it.
pub fn is_private(overwrites: &[Overwrite], guild: Snowflake) -> bool {
    state_of(find(overwrites, guild), Permissions::VIEW_CHANNEL) == Tri::Deny
}

/// Whether a channel's overwrites are exactly its category's. A channel with
/// no category is never out of sync with one.
pub fn synced_with(own: &[Overwrite], category: &[Overwrite]) -> bool {
    let key = |o: &Overwrite| (o.id, o.kind as u8, o.allow.bits(), o.deny.bits());
    let mut a: Vec<_> = own.iter().map(key).collect();
    let mut b: Vec<_> = category.iter().map(key).collect();
    a.sort_unstable();
    b.sort_unstable();
    a == b
}

/// Roles come before members in the list the live page draws, then by name.
pub fn is_role(overwrite: &Overwrite) -> bool {
    overwrite.kind == OverwriteKind::Role
}

#[cfg(test)]
mod tests {
    use super::*;

    fn overwrite(id: u64, kind: OverwriteKind, allow: Permissions, deny: Permissions) -> Overwrite {
        Overwrite { id: Snowflake(id), kind, allow, deny }
    }

    #[test]
    fn a_permission_is_denied_allowed_or_inherited() {
        let o = overwrite(1, OverwriteKind::Role, Permissions::SEND_MESSAGES, Permissions::EMBED_LINKS);
        assert_eq!(state_of(Some(&o), Permissions::SEND_MESSAGES), Tri::Allow);
        assert_eq!(state_of(Some(&o), Permissions::EMBED_LINKS), Tri::Deny);
        assert_eq!(state_of(Some(&o), Permissions::ATTACH_FILES), Tri::Inherit);
        assert_eq!(state_of(None, Permissions::SEND_MESSAGES), Tri::Inherit);
    }

    #[test]
    fn everyone_denied_viewing_makes_a_channel_private() {
        let guild = Snowflake(100);
        let hidden = [overwrite(100, OverwriteKind::Role, Permissions::empty(), Permissions::VIEW_CHANNEL)];
        let open = [overwrite(200, OverwriteKind::Role, Permissions::empty(), Permissions::VIEW_CHANNEL)];
        assert!(is_private(&hidden, guild));
        assert!(!is_private(&open, guild));
        assert!(!is_private(&[], guild));
    }

    #[test]
    fn overwrites_match_their_category_in_any_order() {
        let a = overwrite(1, OverwriteKind::Role, Permissions::SEND_MESSAGES, Permissions::empty());
        let b = overwrite(2, OverwriteKind::Member, Permissions::empty(), Permissions::VIEW_CHANNEL);
        assert!(synced_with(&[a.clone(), b.clone()], &[b.clone(), a.clone()]));
        assert!(!synced_with(&[a.clone()], &[a, b]));
    }

    #[test]
    fn every_listed_permission_is_a_single_known_flag_and_none_repeat_in_a_kind() {
        for kind in [ChannelType::GuildText, ChannelType::GuildVoice] {
            let mut seen = Permissions::empty();
            for group in catalog(kind) {
                for p in group.perms {
                    assert_eq!(p.flag.bits().count_ones(), 1, "{} is one bit", p.name);
                    assert!(!p.description.is_empty(), "{} has words", p.name);
                    // A flag may show twice only across the chat and voice halves of a
                    // voice channel, which the live page does not do either.
                    assert!(!seen.contains(p.flag), "{} repeats in {kind:?}", p.name);
                    seen |= p.flag;
                }
            }
        }
    }

    #[test]
    fn voice_channels_list_voice_permissions_and_text_ones_do_not() {
        let has = |kind, name: &str| catalog(kind).iter().any(|g| g.perms.iter().any(|p| p.name == name));
        assert!(has(ChannelType::GuildVoice, "Connect"));
        assert!(!has(ChannelType::GuildText, "Connect"));
        assert!(has(ChannelType::GuildText, "Pin Messages"));
    }
}
