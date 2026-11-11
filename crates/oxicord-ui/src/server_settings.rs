//! Server settings: pages down the left, the open page on the right. A page
//! only shows when the account may use it there. Edits are sent as they are
//! made and the gateway reports the result back into the store.

use chrono::Local;
use gpui_kit::{
    AnyElement, AppContext as _, ClickEvent, Context, Entity, EventEmitter, FontWeight, InteractiveElement as _,
    IntoElement, ParentElement as _, PathPromptOptions, Render, SharedString, StatefulInteractiveElement as _, Styled as _,
    StyledImage as _, Subscription, Window, div, px, rgb,
    component::input::{Input, InputState},
};
use oxicord_media::cdn;
use oxicord_model::{
    ChannelType, MAX_ONBOARDING_OPTIONS, Onboarding, OnboardingOption, OnboardingPrompt, Permissions, Snowflake,
    WelcomeChannel, WelcomeScreen,
    admin::{AutoModRule, IntegrationEntry, WebhookEntry, action_name, keyword_rule_body, parse_keywords},
    welcome::MAX_WELCOME_CHANNELS,
};
use serde_json::json;

fn mime_for(path: &std::path::Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        _ => "image/png",
    }
}

use crate::{
    account::Load,
    admin::AdminRequest,
    backend::Backend,
    icons::icon,
    media::{Media, avatar},
    profile::modal,
    session::Session,
    settings_view::widgets::{self, Tone, button, group, status_line, title},
    theme,
};

/// The inputs of one onboarding question: its title and each answer's.
struct PromptInputs {
    title: Entity<InputState>,
    options: Vec<OptionInputs>,
}

struct OptionInputs {
    title: Entity<InputState>,
    description: Entity<InputState>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Page {
    Overview,
    ServerProfile,
    ServerTag,
    Members,
    Access,
    Integrations,
    AutoMod,
    EnableCommunity,
    WelcomeScreen,
    Onboarding,
    BoostPerks,
    Stickers,
    Soundboard,
    Engagement,
    ServerTemplate,
    Widget,
    SafetySetup,
    ServerInsights,
    PartnerProgram,
    Monetization,
    Discovery,
    Roles,
    Channels,
    Emoji,
    Invites,
    Bans,
    AuditLog,
}

impl Page {
    const ALL: [Page; 27] =
        [Page::Overview, Page::ServerProfile, Page::ServerTag, Page::Members, Page::Access, Page::Integrations, Page::AutoMod, Page::EnableCommunity, Page::WelcomeScreen, Page::Onboarding, Page::BoostPerks, Page::Stickers, Page::Soundboard, Page::Engagement, Page::ServerTemplate, Page::Widget, Page::SafetySetup, Page::ServerInsights, Page::PartnerProgram, Page::Monetization, Page::Discovery, Page::Roles, Page::Channels, Page::Emoji, Page::Invites, Page::Bans, Page::AuditLog];

    fn label(self) -> &'static str {
        match self {
            Page::Overview => "Overview",
            Page::ServerProfile => "Server Profile",
            Page::ServerTag => "Server Tag",
            Page::Members => "Members",
            Page::Access => "Access",
            Page::Integrations => "Integrations",
            Page::AutoMod => "AutoMod",
            Page::EnableCommunity => "Enable Community",
            Page::WelcomeScreen => "Welcome Screen",
            Page::Onboarding => "Onboarding",
            Page::BoostPerks => "Boost Perks",
            Page::Stickers => "Stickers",
            Page::Soundboard => "Soundboard",
            Page::Engagement => "Engagement",
            Page::ServerTemplate => "Server Template",
            Page::Widget => "Widget",
            Page::SafetySetup => "Safety Setup",
            Page::ServerInsights => "Server Insights",
            Page::PartnerProgram => "Partner Program",
            Page::Monetization => "Monetization",
            Page::Discovery => "Discovery",
            Page::Roles => "Roles",
            Page::Channels => "Channels",
            Page::Emoji => "Emoji",
            Page::Invites => "Invites",
            Page::Bans => "Bans",
            Page::AuditLog => "Audit Log",
        }
    }

    /// What lets someone open the page.
    fn needs(self) -> Permissions {
        match self {
            Page::Overview | Page::Invites | Page::ServerProfile | Page::ServerTag | Page::Members | Page::Access | Page::Integrations | Page::AutoMod | Page::EnableCommunity | Page::WelcomeScreen | Page::Onboarding | Page::BoostPerks | Page::Stickers | Page::Soundboard | Page::Engagement | Page::ServerTemplate | Page::Widget | Page::SafetySetup | Page::ServerInsights | Page::PartnerProgram | Page::Monetization | Page::Discovery => Permissions::MANAGE_GUILD,
            Page::Roles => Permissions::MANAGE_ROLES,
            Page::Channels => Permissions::MANAGE_CHANNELS,
            Page::Emoji => Permissions::MANAGE_GUILD_EXPRESSIONS,
            Page::Bans => Permissions::BAN_MEMBERS,
            Page::AuditLog => Permissions::VIEW_AUDIT_LOG,
        }
    }
}

/// Which pages these permissions open.
fn pages_for(perms: Permissions) -> Vec<Page> {
    Page::ALL.into_iter().filter(|p| perms.contains(p.needs())).collect()
}

/// Whether someone with `perms` has anything to do in server settings.
pub fn can_manage(perms: Permissions) -> bool {
    !pages_for(perms).is_empty()
}

/// Role permissions as the editor lists them, with their group.
const ROLE_PERMISSIONS: &[(Permissions, &str, &str)] = &[
    (Permissions::ADMINISTRATOR, "Administrator", "General"),
    (Permissions::VIEW_AUDIT_LOG, "View Audit Log", "General"),
    (Permissions::MANAGE_GUILD, "Manage Server", "General"),
    (Permissions::MANAGE_ROLES, "Manage Roles", "General"),
    (Permissions::MANAGE_CHANNELS, "Manage Channels", "General"),
    (Permissions::MANAGE_GUILD_EXPRESSIONS, "Manage Expressions", "General"),
    (Permissions::MANAGE_WEBHOOKS, "Manage Webhooks", "General"),
    (Permissions::VIEW_CHANNEL, "View Channels", "General"),
    (Permissions::CREATE_INSTANT_INVITE, "Create Invite", "Membership"),
    (Permissions::CHANGE_NICKNAME, "Change Nickname", "Membership"),
    (Permissions::MANAGE_NICKNAMES, "Manage Nicknames", "Membership"),
    (Permissions::KICK_MEMBERS, "Kick Members", "Membership"),
    (Permissions::BAN_MEMBERS, "Ban Members", "Membership"),
    (Permissions::MODERATE_MEMBERS, "Timeout Members", "Membership"),
    (Permissions::SEND_MESSAGES, "Send Messages", "Text"),
    (Permissions::SEND_MESSAGES_IN_THREADS, "Send Messages in Threads", "Text"),
    (Permissions::CREATE_PUBLIC_THREADS, "Create Public Threads", "Text"),
    (Permissions::CREATE_PRIVATE_THREADS, "Create Private Threads", "Text"),
    (Permissions::EMBED_LINKS, "Embed Links", "Text"),
    (Permissions::ATTACH_FILES, "Attach Files", "Text"),
    (Permissions::ADD_REACTIONS, "Add Reactions", "Text"),
    (Permissions::USE_EXTERNAL_EMOJIS, "Use External Emoji", "Text"),
    (Permissions::USE_EXTERNAL_STICKERS, "Use External Stickers", "Text"),
    (Permissions::MENTION_EVERYONE, "Mention Everyone", "Text"),
    (Permissions::MANAGE_MESSAGES, "Manage Messages", "Text"),
    (Permissions::MANAGE_THREADS, "Manage Threads", "Text"),
    (Permissions::READ_MESSAGE_HISTORY, "Read Message History", "Text"),
    (Permissions::USE_APPLICATION_COMMANDS, "Use Application Commands", "Text"),
    (Permissions::CONNECT, "Connect", "Voice"),
    (Permissions::SPEAK, "Speak", "Voice"),
    (Permissions::STREAM, "Video", "Voice"),
    (Permissions::MUTE_MEMBERS, "Mute Members", "Voice"),
    (Permissions::DEAFEN_MEMBERS, "Deafen Members", "Voice"),
    (Permissions::MOVE_MEMBERS, "Move Members", "Voice"),
    (Permissions::USE_VAD, "Use Voice Activity", "Voice"),
];

/// "#5865f2" or "5865f2" as the number Discord stores. Empty means no color.
fn parse_color(text: &str) -> Option<u32> {
    let text = text.trim().trim_start_matches('#');
    if text.is_empty() {
        return Some(0);
    }
    if text.len() != 6 {
        return None;
    }
    u32::from_str_radix(text, 16).ok()
}

fn color_text(color: u32) -> String {
    if color == 0 { String::new() } else { format!("#{color:06x}") }
}

/// What the two step delete button is waiting on.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Armed {
    Role(Snowflake),
    Channel(Snowflake),
    Emoji(Snowflake),
    Webhook(Snowflake),
    Rule(Snowflake),
    Integration(Snowflake),
}

/// The line under an integration's name: its kind, the linked account, the
/// subscribers and who added it, as far as the server said.
fn integration_detail(entry: &IntegrationEntry) -> String {
    let mut parts = vec![entry.kind_name().to_owned()];
    if let Some(account) = &entry.account {
        parts.push(account.clone());
    }
    if entry.subscribers > 0 {
        parts.push(counted(entry.subscribers as usize, "subscriber", "subscribers"));
    }
    if !entry.enabled {
        parts.push("Disabled".to_owned());
    }
    if let Some(creator) = &entry.creator {
        parts.push(format!("Added by {creator}"));
    }
    parts.join(" \u{b7} ")
}

/// The webhooks grouped by the channel they post to, in the order each
/// channel first appears.
fn webhooks_by_channel(list: &[WebhookEntry]) -> Vec<(Option<Snowflake>, Vec<&WebhookEntry>)> {
    let mut groups: Vec<(Option<Snowflake>, Vec<&WebhookEntry>)> = Vec::new();
    for hook in list {
        match groups.iter_mut().find(|(channel, _)| *channel == hook.channel) {
            Some((_, hooks)) => hooks.push(hook),
            None => groups.push((hook.channel, vec![hook])),
        }
    }
    groups
}

/// Whether the editor holds something other than what the server has.
fn webhook_changed(hook: &WebhookEntry, name: &str, channel: Option<Snowflake>) -> bool {
    name.trim() != hook.name || channel.is_some_and(|c| Some(c) != hook.channel)
}

/// What a rule's card says under its name: what it watches for and does.
fn automod_detail(rule: &AutoModRule) -> String {
    let actions = rule.action_names();
    let mut detail = rule.trigger_name().to_owned();
    if !actions.is_empty() {
        detail = format!("{detail} \u{b7} {}", actions.join(", "));
    }
    let exempt = rule.exempt_channels + rule.exempt_roles;
    if exempt > 0 {
        detail = format!("{detail} \u{b7} {}", counted(exempt, "exemption", "exemptions"));
    }
    detail
}

/// "2 webhooks", "1 webhook".
fn counted(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

pub struct CloseServerSettings;

impl EventEmitter<CloseServerSettings> for ServerSettings {}

pub struct ServerSettings {
    session: Entity<Session>,
    media: Entity<Media>,
    backend: Backend,
    guild: Option<Snowflake>,
    page: Page,
    name: Entity<InputState>,
    new_role: Entity<InputState>,
    role_name: Entity<InputState>,
    role_color: Entity<InputState>,
    new_channel: Entity<InputState>,
    channel_name: Entity<InputState>,
    channel_topic: Entity<InputState>,
    emoji_name: Entity<InputState>,
    webhook_name: Entity<InputState>,
    /// The Webhooks page inside Integrations is open.
    webhooks_open: bool,
    /// The webhook whose editor is open, and the channel picked in it.
    webhook: Option<Snowflake>,
    webhook_channel: Option<Snowflake>,
    channel_menu: bool,
    /// The AutoMod form that makes a rule, and the editor of the open rule.
    new_rule_name: Entity<InputState>,
    new_rule_words: Entity<InputState>,
    rule_name: Entity<InputState>,
    rule_words: Entity<InputState>,
    rule: Option<Snowflake>,
    /// The Welcome Screen draft, its description input, one input per featured
    /// channel and whether the channel menu is open.
    welcome: Option<WelcomeScreen>,
    welcome_description: Entity<InputState>,
    welcome_inputs: Vec<Entity<InputState>>,
    welcome_menu: bool,
    /// The onboarding draft, the inputs of each question, whether the default
    /// channel menu is open and a counter for throwaway ids.
    onboarding: Option<Onboarding>,
    onboarding_inputs: Vec<PromptInputs>,
    onboarding_menu: bool,
    onboarding_seq: u64,
    role: Option<Snowflake>,
    role_perms: Permissions,
    role_hoist: bool,
    role_mentionable: bool,
    channel: Option<Snowflake>,
    emoji: Option<Snowflake>,
    new_kind: u8,
    armed: Option<Armed>,
    /// A color the editor could not read.
    color_error: bool,
    _subscriptions: Vec<Subscription>,
}

impl ServerSettings {
    pub fn new(
        session: Entity<Session>,
        media: Entity<Media>,
        backend: Backend,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let input = |cx: &mut Context<Self>, window: &mut Window, hint: &'static str| {
            cx.new(|cx| InputState::new(window, cx).placeholder(hint))
        };
        let subscriptions = vec![cx.observe(&session, |_, _, cx| cx.notify()), cx.observe(&media, |_, _, cx| cx.notify())];
        Self {
            name: input(cx, window, "Server name"),
            new_role: input(cx, window, "New role name"),
            role_name: input(cx, window, "Role name"),
            role_color: input(cx, window, "#rrggbb"),
            new_channel: input(cx, window, "new-channel"),
            channel_name: input(cx, window, "Channel name"),
            channel_topic: input(cx, window, "Topic"),
            emoji_name: input(cx, window, "Emoji name"),
            webhook_name: input(cx, window, "Webhook name"),
            webhooks_open: false,
            webhook: None,
            webhook_channel: None,
            channel_menu: false,
            new_rule_name: input(cx, window, "Rule name"),
            new_rule_words: input(cx, window, "Words to block, separated by commas"),
            rule_name: input(cx, window, "Rule name"),
            rule_words: input(cx, window, "Words to block, separated by commas"),
            rule: None,
            welcome: None,
            welcome_description: input(cx, window, "Tell newcomers what the server is about"),
            welcome_inputs: Vec::new(),
            welcome_menu: false,
            onboarding: None,
            onboarding_inputs: Vec::new(),
            onboarding_menu: false,
            onboarding_seq: 0,
            session,
            media,
            backend,
            guild: None,
            page: Page::Overview,
            role: None,
            role_perms: Permissions::empty(),
            role_hoist: false,
            role_mentionable: false,
            channel: None,
            emoji: None,
            new_kind: 0,
            armed: None,
            color_error: false,
            _subscriptions: subscriptions,
        }
    }

    /// Opens the server settings straight on the per-server profile page.
    pub fn opened_profile(&mut self, guild: Snowflake, window: &mut Window, cx: &mut Context<Self>) {
        self.page = Page::ServerProfile;
        self.opened(guild, window, cx);
    }

    /// Opens straight on the Webhooks page, for checking it from the dev control.
    pub fn opened_webhooks(&mut self, guild: Snowflake, window: &mut Window, cx: &mut Context<Self>) {
        self.opened(guild, window, cx);
        self.select(Page::Integrations, cx);
        self.webhooks_open = true;
    }

    /// Opens straight on the AutoMod page, for checking it from the dev control.
    pub fn opened_automod(&mut self, guild: Snowflake, window: &mut Window, cx: &mut Context<Self>) {
        self.opened(guild, window, cx);
        self.select(Page::AutoMod, cx);
    }

    pub fn opened(&mut self, guild: Snowflake, window: &mut Window, cx: &mut Context<Self>) {
        self.guild = Some(guild);
        self.role = None;
        self.channel = None;
        self.emoji = None;
        self.armed = None;
        let (name, first) = {
            let s = self.session.read(cx);
            (
                s.store.guild(guild).map(|g| g.name.clone()).unwrap_or_default(),
                pages_for(s.store.guild_permissions(guild)).first().copied(),
            )
        };
        self.name.update(cx, |i, cx| i.set_value(name, window, cx));
        self.select(first.unwrap_or(Page::Overview), cx);
    }

    fn select(&mut self, page: Page, cx: &mut Context<Self>) {
        self.page = page;
        self.armed = None;
        self.webhooks_open = false;
        self.webhook = None;
        self.rule = None;
        self.channel_menu = false;
        self.welcome = None;
        self.welcome_menu = false;
        self.onboarding = None;
        self.onboarding_menu = false;
        let Some(guild) = self.guild else { return };
        let request = match page {
            Page::Integrations => Some(AdminRequest::LoadWebhooks(guild)),
            Page::AutoMod => Some(AdminRequest::LoadAutomod(guild)),
            Page::WelcomeScreen => Some(AdminRequest::LoadWelcome(guild)),
            Page::Onboarding => Some(AdminRequest::LoadOnboardingSetup(guild)),
            Page::Invites => Some(AdminRequest::LoadInvites(guild)),
            Page::Bans => Some(AdminRequest::LoadBans(guild)),
            Page::AuditLog => Some(AdminRequest::LoadAudit(guild)),
            _ => None,
        };
        if let Some(request) = request {
            let backend = self.backend.clone();
            self.session.update(cx, |s, cx| s.request_admin(&backend, request, cx));
        }
        if page == Page::Integrations {
            self.send(AdminRequest::LoadIntegrations(guild), cx);
        }
        cx.notify();
    }

    fn send(&mut self, request: AdminRequest, cx: &mut Context<Self>) {
        let backend = self.backend.clone();
        self.session.update(cx, |s, cx| s.request_admin(&backend, request, cx));
    }

    /// Two clicks to delete: the first arms, the second sends.
    fn confirm(&mut self, what: Armed, cx: &mut Context<Self>) -> bool {
        if self.armed == Some(what) {
            self.armed = None;
            true
        } else {
            self.armed = Some(what);
            cx.notify();
            false
        }
    }

    fn pick_role(&mut self, id: Snowflake, window: &mut Window, cx: &mut Context<Self>) {
        let Some(guild) = self.guild else { return };
        let role = self
            .session
            .read(cx)
            .store
            .guild(guild)
            .and_then(|g| g.roles.iter().find(|r| r.id == id))
            .cloned();
        let Some(role) = role else { return };
        self.role = Some(id);
        self.role_perms = role.permissions;
        self.role_hoist = role.hoist;
        self.role_mentionable = role.mentionable;
        self.color_error = false;
        self.armed = None;
        self.role_name.update(cx, |i, cx| i.set_value(role.name, window, cx));
        self.role_color.update(cx, |i, cx| i.set_value(color_text(role.color), window, cx));
        cx.notify();
    }

    fn save_role(&mut self, cx: &mut Context<Self>) {
        let (Some(guild), Some(role)) = (self.guild, self.role) else { return };
        let mut fields = json!({
            "permissions": self.role_perms.bits().to_string(),
        });
        // @everyone has no name, color or flags of its own to change.
        if role != guild {
            let color_text = self.role_color.read(cx).value().to_string();
            let Some(color) = parse_color(&color_text) else {
                self.color_error = true;
                cx.notify();
                return;
            };
            fields["name"] = json!(self.role_name.read(cx).value().trim());
            fields["color"] = json!(color);
            fields["hoist"] = json!(self.role_hoist);
            fields["mentionable"] = json!(self.role_mentionable);
        }
        self.color_error = false;
        self.send(AdminRequest::EditRole { guild, role, fields }, cx);
    }

    fn pick_channel(&mut self, id: Snowflake, window: &mut Window, cx: &mut Context<Self>) {
        let channel = self.session.read(cx).store.channel(id).cloned();
        let Some(channel) = channel else { return };
        self.channel = Some(id);
        self.armed = None;
        self.channel_name.update(cx, |i, cx| i.set_value(channel.name.unwrap_or_default(), window, cx));
        self.channel_topic.update(cx, |i, cx| i.set_value(channel.topic.unwrap_or_default(), window, cx));
        cx.notify();
    }

    fn pick_emoji(&mut self, id: Snowflake, name: String, window: &mut Window, cx: &mut Context<Self>) {
        self.emoji = Some(id);
        self.armed = None;
        self.emoji_name.update(cx, |i, cx| i.set_value(name, window, cx));
        cx.notify();
    }

    fn clear(input: &Entity<InputState>, window: &mut Window, cx: &mut Context<Self>) {
        input.update(cx, |i, cx| i.set_value("", window, cx));
    }

    // ---- pages -------------------------------------------------------

    fn overview(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Some(guild) = self.guild else { return div().into_any_element() };
        let info = {
            let s = self.session.read(cx);
            s.store.guild(guild).map(|g| {
                let owner = g
                    .owner_id
                    .and_then(|o| s.store.user(o))
                    .map_or_else(|| "Unknown".to_owned(), |u| u.display_name().to_owned());
                (
                    g.id.to_string(),
                    owner,
                    g.member_count.map_or_else(|| "Unknown".to_owned(), |n| n.to_string()),
                    g.id.datetime().format("%b %d, %Y").to_string(),
                )
            })
        };
        let (id, owner, members, created) = info.unwrap_or_default();

        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(title("Server Overview"))
            .child(group(
                "Server name",
                vec![
                    Input::new(&self.name).into_any_element(),
                    button("save-name", "Save", Tone::Primary, {
                        let view = cx.weak_entity();
                        move |_, _, cx| {
                            view.update(cx, |this, cx| {
                                let name = this.name.read(cx).value().trim().to_owned();
                                if !name.is_empty() {
                                    this.send(AdminRequest::EditGuild { guild, fields: json!({ "name": name }) }, cx);
                                }
                            })
                            .ok();
                        }
                    }),
                ],
            ))
            .child(group(
                "About",
                vec![
                    widgets::info_row("Server ID", id.clone()),
                    button("copy-server-id", "Copy Server ID", Tone::Plain, {
                        let id = id.clone();
                        move |_, _, cx| {
                            cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(id.clone()));
                        }
                    }),
                    widgets::info_row("Owner", owner),
                    widgets::info_row("Members", members),
                    widgets::info_row("Created", created),
                ],
            ))
            .into_any_element()
    }

    fn server_profile(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Some(guild) = self.guild else { return div().into_any_element() };
        let name = self
            .session
            .read(cx)
            .store
            .guild(guild)
            .map(|g| g.name.clone())
            .unwrap_or_default();

        let name_for_avatar = self.session.read(cx).store.me().map(|m| m.display_name().to_owned()).unwrap_or_default();
        let avatar_url = String::new();
        let has_avatar = false;
        let banner_url: Option<String> = None;
        let has_banner = false;

        let media = self.media.clone();
        let guild_for_upload = guild;

        // avatar upload column
        let mut avatar_col = div().flex().flex_col().gap_2().child(button("upload-server-avatar", "Upload Avatar", Tone::Primary, cx.listener(
            move |_, _: &ClickEvent, _, cx| {
                let chosen = cx.prompt_for_paths(PathPromptOptions {
                    files: true,
                    directories: false,
                    multiple: false,
                    prompt: None,
                });
                let guild = guild_for_upload;
                cx.spawn(async move |this, cx| {
                    if let Ok(Ok(Some(paths))) = chosen.await
                        && let Some(path) = paths.first()
                        && let Ok(data) = std::fs::read(path) {
                        let mime = mime_for(path);
                        let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &data);
                        let uri = format!("data:{};base64,{}", mime, b64);
                        this.update(cx, |this, cx| {
                            this.send(AdminRequest::SetServerAvatar { guild, avatar: Some(uri) }, cx);
                        })
                        .ok();
                    }
                })
                .detach();
            },
        )));
        if has_avatar {
            let guild = guild_for_upload;
            avatar_col = avatar_col.child(button("remove-server-avatar", "Remove", Tone::Plain, cx.listener(
                move |this, _: &ClickEvent, _, cx| {
                    this.send(AdminRequest::SetServerAvatar { guild, avatar: None }, cx);
                },
            )));
        }

        let guild_for_upload2 = guild;
        let mut banner_col = div().flex().flex_col().gap_2().child(button("upload-server-banner", "Upload Banner", Tone::Primary, cx.listener(
            move |_, _: &ClickEvent, _, cx| {
                let chosen = cx.prompt_for_paths(PathPromptOptions {
                    files: true,
                    directories: false,
                    multiple: false,
                    prompt: None,
                });
                let guild = guild_for_upload2;
                cx.spawn(async move |this, cx| {
                    if let Ok(Ok(Some(paths))) = chosen.await
                        && let Some(path) = paths.first()
                        && let Ok(data) = std::fs::read(path) {
                        let mime = mime_for(path);
                        let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &data);
                        let uri = format!("data:{};base64,{}", mime, b64);
                        this.update(cx, |this, cx| {
                            this.send(AdminRequest::SetServerBanner { guild, banner: Some(uri) }, cx);
                        })
                        .ok();
                    }
                })
                .detach();
            },
        )));
        if has_banner {
            let guild = guild_for_upload2;
            banner_col = banner_col.child(button("remove-server-banner", "Remove", Tone::Plain, cx.listener(
                move |this, _: &ClickEvent, _, cx| {
                    this.send(AdminRequest::SetServerBanner { guild, banner: None }, cx);
                },
            )));
        }

        let banner_preview = match banner_url.as_deref().and_then(|url| media.update(cx, |m, _| m.get(url))) {
            Some(image) => div()
                .h(px(80.))
                .w(px(240.))
                .rounded(px(theme::RADIUS))
                .overflow_hidden()
                .child(gpui_kit::img(gpui_kit::ImageSource::Image(image)).size_full().object_fit(gpui_kit::ObjectFit::Cover))
                .into_any_element(),
            None => div()
                .h(px(80.))
                .w(px(240.))
                .rounded(px(theme::RADIUS))
                .bg(rgb(theme::BG_ACTIVE))
                .into_any_element(),
        };

        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(title("Server Profile"))
            .child(group(
                "Profile",
                vec![
                    widgets::info_row("Server Name", name),
                    widgets::info_row("Server ID", guild.to_string()),
                ],
            ))
            .child(group("Avatar", vec![
                div().flex().items_center().gap_3().child(crate::media::avatar(&media, &avatar_url, 80., &name_for_avatar, cx)).child(avatar_col).into_any_element(),
            ]))
            .child(group("Banner", vec![
                div().flex().items_center().gap_3().child(banner_preview).child(banner_col).into_any_element(),
            ]))
            .into_any_element()
    }

    fn server_tag(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Some(guild) = self.guild else { return div().into_any_element() };
        let name = self
            .session
            .read(cx)
            .store
            .guild(guild)
            .map(|g| g.name.clone())
            .unwrap_or_default();

        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(title("Server Tag"))
            .child(group(
                "Tag",
                vec![
                    widgets::info_row("Server Name", name),
                    widgets::info_row("Server ID", guild.to_string()),
                ],
            ))
            .into_any_element()
    }

    fn members(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Some(guild) = self.guild else { return div().into_any_element() };
        let members = self
            .session
            .read(cx)
            .store
            .guild(guild)
            .map(|g| g.member_count.map_or_else(|| "Unknown".to_owned(), |n| n.to_string()))
            .unwrap_or_default();

        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(title("Members"))
            .child(group(
                "Members",
                vec![widgets::info_row("Member Count", members)],
            ))
            .into_any_element()
    }

    fn access(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Some(guild) = self.guild else { return div().into_any_element() };
        let name = self
            .session
            .read(cx)
            .store
            .guild(guild)
            .map(|g| g.name.clone())
            .unwrap_or_default();

        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(title("Access"))
            .child(group(
                "Access",
                vec![widgets::info_row("Server Name", name)],
            ))
            .into_any_element()
    }

    fn integrations(&mut self, cx: &mut Context<Self>) -> AnyElement {
        if self.webhooks_open {
            return self.webhooks(cx);
        }
        let (hooks, followed) = match &self.session.read(cx).admin.webhooks {
            Load::Ready(list) => {
                let followed = list.iter().filter(|w| w.kind == 2).count();
                (counted(list.len() - followed, "webhook", "webhooks"), counted(followed, "channel", "channels"))
            }
            _ => ("...".to_owned(), "...".to_owned()),
        };
        let card = |glyph: &str, name: &'static str, count: String| {
            div()
                .h(px(88.))
                .px_4()
                .flex()
                .items_center()
                .gap_4()
                .rounded(px(theme::RADIUS))
                .bg(rgb(theme::BG_PANEL))
                .child(icon(glyph, 24., theme::TEXT_MUTED))
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .child(div().font_weight(FontWeight::MEDIUM).text_color(rgb(theme::TEXT_BRIGHT)).child(name))
                        .child(div().text_size(px(12.)).text_color(rgb(theme::TEXT)).child(count)),
                )
        };

        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(title("Integrations"))
            .child(widgets::blurb(
                "Customize your server with integrations. Manage webhooks, followed channels, and apps, as well as Twitch and YouTube settings for creators.",
            ))
            .child(div().h(px(1.)).mb_2().bg(rgb(theme::BORDER)))
            .child(
                card("webhook", "Webhooks", hooks)
                    .id("open-webhooks")
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(theme::BG_HOVER)))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.webhooks_open = true;
                        cx.notify();
                    }))
                    .child(icon("chevron-right", 16., theme::TEXT_MUTED)),
            )
            .child(card("arrow-left-right", "Channels Followed", followed))
            .children(self.integration_cards(cx))
            .into_any_element()
    }

    /// The Twitch, YouTube and app integrations below the two cards. Nothing
    /// shows while the list loads, fails or is empty.
    fn integration_cards(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let Some(guild) = self.guild else { return Vec::new() };
        let Load::Ready(list) = self.session.read(cx).admin.integrations.clone() else { return Vec::new() };
        list.into_iter()
            .map(|entry| {
                let id = entry.id;
                let armed = self.armed == Some(Armed::Integration(id));
                let detail = integration_detail(&entry);
                div()
                    .h(px(88.))
                    .px_4()
                    .flex()
                    .items_center()
                    .gap_4()
                    .rounded(px(theme::RADIUS))
                    .bg(rgb(theme::BG_PANEL))
                    .child(icon("bot", 24., theme::TEXT_MUTED))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .child(div().font_weight(FontWeight::MEDIUM).text_color(rgb(theme::TEXT_BRIGHT)).child(entry.name.clone()))
                            .child(div().text_size(px(12.)).text_color(rgb(theme::TEXT)).child(detail)),
                    )
                    .child(button(
                        ("remove-integration", id.0),
                        if armed { "Click again to remove" } else { "Remove" },
                        Tone::Danger,
                        {
                            let view = cx.weak_entity();
                            move |_, _, cx| {
                                view.update(cx, |this, cx| {
                                    if this.confirm(Armed::Integration(id), cx) {
                                        this.send(AdminRequest::DeleteIntegration { guild, integration: id }, cx);
                                    }
                                })
                                .ok();
                            }
                        },
                    ))
                    .into_any_element()
            })
            .collect()
    }

    /// Opens one webhook's editor with what the server has for it.
    fn pick_webhook(&mut self, hook: &WebhookEntry, window: &mut Window, cx: &mut Context<Self>) {
        self.webhook = Some(hook.id);
        self.webhook_channel = hook.channel;
        self.channel_menu = false;
        self.armed = None;
        self.webhook_name.update(cx, |i, cx| i.set_value(hook.name.clone(), window, cx));
        cx.notify();
    }

    fn webhooks(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Some(guild) = self.guild else { return div().into_any_element() };
        let state = self.session.read(cx).admin.webhooks.clone();
        let channel_name = |this: &Self, id: Option<Snowflake>, cx: &Context<Self>| -> String {
            id.and_then(|id| this.session.read(cx).store.channel(id).and_then(|c| c.name.clone())).unwrap_or_default()
        };

        let crumbs = div()
            .mb_4()
            .flex()
            .items_center()
            .gap_2()
            .text_size(px(24.))
            .font_weight(FontWeight::MEDIUM)
            .child(
                div()
                    .id("webhooks-back")
                    .cursor_pointer()
                    .text_color(rgb(theme::TEXT_MUTED))
                    .hover(|s| s.text_color(rgb(theme::TEXT_BRIGHT)))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.webhooks_open = false;
                        this.webhook = None;
                        cx.notify();
                    }))
                    .child("Integrations"),
            )
            .child(icon("chevron-right", 18., theme::TEXT_MUTED))
            .child(div().text_color(rgb(theme::TEXT_BRIGHT)).child("Webhooks"));

        // A new webhook goes in the first text channel, named at random the way
        // the web client does it.
        let first_text = {
            let store = &self.session.read(cx).store;
            store
                .channel_groups(guild)
                .into_iter()
                .flat_map(|g| g.channels)
                .find(|c| c.kind == ChannelType::GuildText)
                .map(|c| c.id)
        };
        let new_webhook = button("new-webhook", "New Webhook", Tone::Primary, {
            let view = cx.weak_entity();
            move |_, _, cx| {
                let Some(channel) = first_text else { return };
                let name = if std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.subsec_nanos())
                    % 2
                    == 0
                {
                    "Spidey Bot"
                } else {
                    "Captain Hook"
                };
                view.update(cx, |this, cx| {
                    this.send(AdminRequest::CreateWebhook { guild, channel, name: name.to_owned() }, cx);
                })
                .ok();
            }
        });

        let mut body = div().flex().flex_col().gap_4();
        match state {
            Load::Ready(list) => {
                for (channel, hooks) in webhooks_by_channel(&list) {
                    let heading = format!("Posting to #{}", channel_name(self, channel, cx));
                    let mut section = div().flex().flex_col().gap_3().child(
                        div().text_color(rgb(theme::TEXT_BRIGHT)).font_weight(FontWeight::MEDIUM).child(heading),
                    );
                    for hook in hooks {
                        section = section.child(self.webhook_card(guild, hook, cx));
                    }
                    body = body.child(section);
                }
            }
            Load::Failed(error) => body = body.child(status_line("circle-alert", error, theme::DANGER)),
            _ => body = body.child(status_line("loader", "Loading webhooks...".into(), theme::TEXT_MUTED)),
        }

        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(crumbs)
            .child(widgets::blurb(
                "Webhooks are a simple way to post messages from other apps and websites into Discord using internet magic.",
            ))
            .child(div().h(px(1.)).mb_2().bg(rgb(theme::BORDER)))
            .child(new_webhook)
            .child(body)
            .into_any_element()
    }

    fn webhook_card(&mut self, guild: Snowflake, hook: &WebhookEntry, cx: &mut Context<Self>) -> AnyElement {
        let id = hook.id;
        let open = self.webhook == Some(id);
        let avatar_url = cdn::user_avatar(id, hook.avatar.as_deref(), 64);
        let media = self.media.clone();
        let made = format!(
            "Created on {}{}",
            id.datetime().format("%b %-d, %Y"),
            hook.creator.as_ref().map_or_else(String::new, |c| format!(" by {c}")),
        );
        let header = div()
            .id(("webhook", id.0))
            .h(px(88.))
            .px_4()
            .flex()
            .items_center()
            .gap_4()
            .cursor_pointer()
            .child(crate::media::avatar(&media, &avatar_url, 48., &hook.name, cx))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(div().font_weight(FontWeight::MEDIUM).text_color(rgb(theme::TEXT_BRIGHT)).child(hook.name.clone()))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .text_size(px(12.))
                            .text_color(rgb(theme::TEXT))
                            .child(icon("clock", 14., theme::TEXT_MUTED))
                            .child(made),
                    ),
            )
            .child(icon(if open { "chevron-down" } else { "chevron-right" }, 16., theme::TEXT_MUTED))
            .on_click({
                let hook = hook.clone();
                cx.listener(move |this, _: &ClickEvent, window, cx| {
                    if this.webhook == Some(hook.id) {
                        this.webhook = None;
                        cx.notify();
                    } else {
                        this.pick_webhook(&hook, window, cx);
                    }
                })
            });

        let mut card = div().flex().flex_col().rounded(px(theme::RADIUS)).bg(rgb(theme::BG_PANEL)).child(header);
        if open {
            card = card.child(self.webhook_editor(guild, hook, cx));
        }
        card.into_any_element()
    }

    fn webhook_editor(&mut self, guild: Snowflake, hook: &WebhookEntry, cx: &mut Context<Self>) -> AnyElement {
        let id = hook.id;
        let chosen = self.webhook_channel.or(hook.channel);
        let chosen_name = chosen
            .and_then(|c| self.session.read(cx).store.channel(c).and_then(|c| c.name.clone()))
            .unwrap_or_default();
        let typed = self.webhook_name.read(cx).value().to_string();
        let changed = webhook_changed(hook, &typed, self.webhook_channel);
        let armed = self.armed == Some(Armed::Webhook(id));
        let label = |text: &'static str| {
            div().font_weight(FontWeight::MEDIUM).text_color(rgb(theme::TEXT_BRIGHT)).child(text)
        };

        let select = div()
            .id("webhook-channel")
            .h(px(40.))
            .px_3()
            .flex()
            .items_center()
            .justify_between()
            .rounded(px(theme::RADIUS))
            .border_1()
            .border_color(rgb(theme::BORDER))
            .cursor_pointer()
            .text_color(rgb(theme::TEXT))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.channel_menu = !this.channel_menu;
                cx.notify();
            }))
            .child(format!("#{chosen_name}"))
            .child(icon("chevron-down", 16., theme::TEXT_MUTED));

        let mut channel_column = div().flex_1().flex().flex_col().gap_2().child(label("Channel")).child(select);
        if self.channel_menu {
            let options: Vec<(Snowflake, String)> = {
                let store = &self.session.read(cx).store;
                store
                    .channel_groups(guild)
                    .into_iter()
                    .flat_map(|g| g.channels)
                    .filter(|c| !matches!(c.kind, ChannelType::GuildCategory))
                    .map(|c| (c.id, c.name.clone().unwrap_or_default()))
                    .collect()
            };
            let mut menu = div()
                .id("webhook-channel-menu")
                .max_h(px(200.))
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .rounded(px(theme::RADIUS))
                .border_1()
                .border_color(rgb(theme::BORDER))
                .bg(rgb(theme::BG_CHAT));
            for (option, name) in options {
                menu = menu.child(
                    div()
                        .id(("webhook-channel-option", option.0))
                        .h(px(32.))
                        .px_3()
                        .flex()
                        .items_center()
                        .cursor_pointer()
                        .text_color(rgb(theme::TEXT))
                        .hover(|s| s.bg(rgb(theme::BG_HOVER)))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.webhook_channel = Some(option);
                            this.channel_menu = false;
                            cx.notify();
                        }))
                        .child(format!("#{name}")),
                );
            }
            channel_column = channel_column.child(menu);
        }

        let mut actions = div().flex().gap_2();
        if changed {
            actions = actions.child(button("save-webhook", "Save Changes", Tone::Primary, {
                let view = cx.weak_entity();
                move |_, _, cx| {
                    view.update(cx, |this, cx| {
                        let name = this.webhook_name.read(cx).value().trim().to_owned();
                        let Some(channel) = this.webhook_channel.or(chosen) else { return };
                        if !name.is_empty() {
                            this.send(AdminRequest::EditWebhook { guild, webhook: id, name, channel }, cx);
                        }
                    })
                    .ok();
                }
            }));
        }
        // Application webhooks come without a token, so there is nothing to copy
        // for them, but the live client still shows the button.
        let url = hook.url();
        actions = actions.child(button("copy-webhook", "Copy Webhook URL", Tone::Plain, move |_, _, cx| {
            if let Some(url) = &url {
                cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(url.clone()));
            }
        }));
        actions = actions.child(button(
            "delete-webhook",
            if armed { "Click again to delete" } else { "Delete Webhook" },
            Tone::Danger,
            {
                let view = cx.weak_entity();
                move |_, _, cx| {
                    view.update(cx, |this, cx| {
                        if this.confirm(Armed::Webhook(id), cx) {
                            this.webhook = None;
                            this.send(AdminRequest::DeleteWebhook { guild, webhook: id }, cx);
                        }
                    })
                    .ok();
                }
            },
        ));

        div()
            .mx_4()
            .pt_4()
            .pb_4()
            .flex()
            .flex_col()
            .gap_4()
            .border_t_1()
            .border_color(rgb(theme::BORDER))
            .child(
                div()
                    .flex()
                    .gap_4()
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(label("Name"))
                            .child(Input::new(&self.webhook_name)),
                    )
                    .child(channel_column),
            )
            .child(actions)
            .into_any_element()
    }

    fn enable_community(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Some(guild) = self.guild else { return div().into_any_element() };
        let name = self
            .session
            .read(cx)
            .store
            .guild(guild)
            .map(|g| g.name.clone())
            .unwrap_or_default();

        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(title("Enable Community"))
            .child(group(
                "Community",
                vec![widgets::info_row("Server Name", name)],
            ))
            .into_any_element()
    }

    /// Copies the loaded welcome screen into the draft the first time it is on
    /// screen, with an input for the description and one for each channel.
    fn seed_welcome(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.page != Page::WelcomeScreen || self.welcome.is_some() {
            return;
        }
        let Load::Ready(screen) = self.session.read(cx).admin.welcome.clone() else { return };
        let text = screen.description.clone().unwrap_or_default();
        self.welcome_description.update(cx, |i, cx| i.set_value(text, window, cx));
        let inputs: Vec<_> = screen.welcome_channels.iter().map(|c| self.welcome_input(&c.description, window, cx)).collect();
        self.welcome_inputs = inputs;
        self.welcome = Some(screen);
    }

    fn welcome_input(&self, text: &str, window: &mut Window, cx: &mut Context<Self>) -> Entity<InputState> {
        let text = text.to_owned();
        cx.new(|cx| {
            let mut state = InputState::new(window, cx).placeholder("What can they do here?");
            state.set_value(text, window, cx);
            state
        })
    }

    /// The draft with what is typed into its inputs.
    fn welcome_current(&self, cx: &Context<Self>) -> Option<WelcomeScreen> {
        let mut screen = self.welcome.clone()?;
        screen.description = Some(self.welcome_description.read(cx).value().to_string());
        for (channel, input) in screen.welcome_channels.iter_mut().zip(&self.welcome_inputs) {
            channel.description = input.read(cx).value().to_string();
        }
        Some(screen)
    }

    /// One featured channel: its name, what it is for and a way to drop it.
    fn welcome_row(&mut self, index: usize, channel: &WelcomeChannel, cx: &mut Context<Self>) -> AnyElement {
        let name = self
            .session
            .read(cx)
            .store
            .channel(channel.channel_id)
            .and_then(|c| c.name.clone())
            .unwrap_or_else(|| "unknown-channel".to_owned());
        let mut row = div()
            .p_3()
            .flex()
            .items_center()
            .gap_3()
            .rounded(px(theme::RADIUS))
            .bg(rgb(theme::BG_PANEL))
            .child(
                div()
                    .w(px(180.))
                    .flex_none()
                    .truncate()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(theme::TEXT_BRIGHT))
                    .child(format!("#{name}")),
            );
        if let Some(input) = self.welcome_inputs.get(index) {
            row = row.child(div().flex_1().child(Input::new(input)));
        }
        row.child(button(("welcome-remove", index as u64), "Remove", Tone::Danger, {
            let view = cx.weak_entity();
            move |_, _, cx| {
                view.update(cx, |this, cx| {
                    if let Some(mut screen) = this.welcome_current(cx) {
                        screen.welcome_channels.remove(index);
                        this.welcome_inputs.remove(index);
                        this.welcome = Some(screen);
                        cx.notify();
                    }
                })
                .ok();
            }
        }))
        .into_any_element()
    }

    /// The channels not featured yet that the screen can point to.
    fn welcome_choices(&self, guild: Snowflake, featured: &[WelcomeChannel], cx: &Context<Self>) -> Vec<(Snowflake, String)> {
        self.session
            .read(cx)
            .store
            .channel_groups(guild)
            .into_iter()
            .flat_map(|g| g.channels)
            .filter(|c| matches!(c.kind, ChannelType::GuildText | ChannelType::GuildAnnouncement))
            .filter(|c| !featured.iter().any(|f| f.channel_id == c.id))
            .map(|c| (c.id, c.name.clone().unwrap_or_default()))
            .collect()
    }

    fn welcome_screen(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Some(guild) = self.guild else { return div().into_any_element() };
        let header = div().flex().flex_col().gap_3().child(title("Welcome Screen")).child(widgets::blurb(
            "Greet new members with a short note about your server and a few channels to start in.",
        ));
        let loaded = match self.session.read(cx).admin.welcome.clone() {
            Load::Ready(screen) => screen,
            Load::Failed(error) => return header.child(status_line("circle-alert", error, theme::DANGER)).into_any_element(),
            _ => return header.child(status_line("loader", "Loading the welcome screen...".into(), theme::TEXT_MUTED)).into_any_element(),
        };
        let Some(current) = self.welcome_current(cx) else { return header.into_any_element() };
        let changed = current.body() != loaded.body();
        let label = |text: &'static str| div().font_weight(FontWeight::MEDIUM).text_color(rgb(theme::TEXT_BRIGHT)).child(text);

        let mut channels = div().flex().flex_col().gap_2();
        for (index, channel) in current.welcome_channels.iter().enumerate() {
            channels = channels.child(self.welcome_row(index, channel, cx));
        }
        if current.welcome_channels.len() < MAX_WELCOME_CHANNELS {
            channels = channels.child(div().flex().child(button("welcome-add", "Add Channel", Tone::Plain, {
                let view = cx.weak_entity();
                move |_, _, cx| {
                    view.update(cx, |this, cx| {
                        this.welcome_menu = !this.welcome_menu;
                        cx.notify();
                    })
                    .ok();
                }
            })));
        }
        if self.welcome_menu {
            let mut menu = div()
                .id("welcome-channel-menu")
                .max_h(px(200.))
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .rounded(px(theme::RADIUS))
                .border_1()
                .border_color(rgb(theme::BORDER))
                .bg(rgb(theme::BG_CHAT));
            for (option, name) in self.welcome_choices(guild, &current.welcome_channels, cx) {
                menu = menu.child(
                    div()
                        .id(("welcome-channel-option", option.0))
                        .h(px(32.))
                        .px_3()
                        .flex()
                        .items_center()
                        .cursor_pointer()
                        .text_color(rgb(theme::TEXT))
                        .hover(|s| s.bg(rgb(theme::BG_HOVER)))
                        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                            this.welcome_menu = false;
                            if let Some(mut screen) = this.welcome_current(cx)
                                && screen.add_channel(option)
                            {
                                let input = this.welcome_input("", window, cx);
                                this.welcome_inputs.push(input);
                                this.welcome = Some(screen);
                            }
                            cx.notify();
                        }))
                        .child(format!("#{name}")),
                );
            }
            channels = channels.child(menu);
        }

        let mut page = header
            .child(widgets::switch_row("welcome-enabled", "Enable the welcome screen", None, current.enabled, {
                let view = cx.weak_entity();
                move |_, _, cx| {
                    view.update(cx, |this, cx| {
                        if let Some(mut screen) = this.welcome_current(cx) {
                            screen.enabled = !screen.enabled;
                            this.welcome = Some(screen);
                        }
                        cx.notify();
                    })
                    .ok();
                }
            }))
            .child(div().flex().flex_col().gap_2().child(label("Server description")).child(Input::new(&self.welcome_description)))
            .child(div().flex().flex_col().gap_2().child(label("Recommended channels")).child(channels));
        if changed {
            page = page.child(div().flex().child(button("save-welcome", "Save Changes", Tone::Primary, {
                let view = cx.weak_entity();
                move |_, _, cx| {
                    view.update(cx, |this, cx| {
                        if let Some(screen) = this.welcome_current(cx) {
                            this.welcome = None;
                            this.send(AdminRequest::SaveWelcome { guild, body: screen.body() }, cx);
                        }
                    })
                    .ok();
                }
            })));
        }
        page.into_any_element()
    }

    /// Copies the loaded onboarding setup into the draft the first time it is
    /// on screen, with an input for each question and answer.
    fn seed_onboarding(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.page != Page::Onboarding || self.onboarding.is_some() {
            return;
        }
        let Load::Ready(setup) = self.session.read(cx).admin.onboarding.clone() else { return };
        self.onboarding_inputs = setup
            .prompts
            .iter()
            .map(|prompt| PromptInputs {
                title: self.onboarding_input(&prompt.title, "Write a question", window, cx),
                options: prompt.options.iter().map(|o| self.option_inputs(o, window, cx)).collect(),
            })
            .collect();
        self.onboarding = Some(setup);
    }

    fn onboarding_input(&self, text: &str, hint: &'static str, window: &mut Window, cx: &mut Context<Self>) -> Entity<InputState> {
        let text = text.to_owned();
        cx.new(|cx| {
            let mut state = InputState::new(window, cx).placeholder(hint);
            state.set_value(text, window, cx);
            state
        })
    }

    fn option_inputs(&self, option: &OnboardingOption, window: &mut Window, cx: &mut Context<Self>) -> OptionInputs {
        OptionInputs {
            title: self.onboarding_input(&option.title, "Answer", window, cx),
            description: self.onboarding_input(&option.description, "Description (optional)", window, cx),
        }
    }

    /// The draft with what is typed into its inputs.
    fn onboarding_current(&self, cx: &Context<Self>) -> Option<Onboarding> {
        let mut setup = self.onboarding.clone()?;
        for (prompt, inputs) in setup.prompts.iter_mut().zip(&self.onboarding_inputs) {
            prompt.title = inputs.title.read(cx).value().to_string();
            for (option, inputs) in prompt.options.iter_mut().zip(&inputs.options) {
                option.title = inputs.title.read(cx).value().to_string();
                option.description = inputs.description.read(cx).value().to_string();
            }
        }
        Some(setup)
    }

    /// Changes the draft, keeping what is typed, and redraws.
    fn edit_onboarding(&mut self, cx: &mut Context<Self>, change: impl FnOnce(&mut Self, &mut Onboarding, &mut Context<Self>)) {
        if let Some(mut setup) = self.onboarding_current(cx) {
            change(self, &mut setup, cx);
            self.onboarding = Some(setup);
        }
        cx.notify();
    }

    /// A throwaway id for a new question or answer, as the web client makes
    /// them from the clock. The server replaces it on save.
    fn fresh_id(&mut self) -> u64 {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64);
        self.onboarding_seq += 1;
        now + self.onboarding_seq
    }

    fn onboarding_switch(
        &self,
        id: &'static str,
        label: &'static str,
        on: bool,
        cx: &mut Context<Self>,
        flip: impl Fn(&mut Onboarding) + 'static,
    ) -> AnyElement {
        let view = cx.weak_entity();
        widgets::switch_row(id, label, None, on, move |_, _, cx| {
            view.update(cx, |this, cx| this.edit_onboarding(cx, |_, setup, _| flip(setup))).ok();
        })
    }

    /// One question: its title, how it is answered and each answer.
    fn onboarding_prompt(&mut self, p: usize, prompt: &OnboardingPrompt, cx: &mut Context<Self>) -> AnyElement {
        let label = |text: &'static str| div().font_weight(FontWeight::MEDIUM).text_color(rgb(theme::TEXT_BRIGHT)).child(text);
        let mut card = div().p_3().flex().flex_col().gap_3().rounded(px(theme::RADIUS)).bg(rgb(theme::BG_PANEL));
        if let Some(inputs) = self.onboarding_inputs.get(p) {
            card = card.child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(div().flex_1().child(Input::new(&inputs.title)))
                    .child(button(("onboarding-prompt-remove", p as u64), "Remove", Tone::Danger, {
                        let view = cx.weak_entity();
                        move |_, _, cx| {
                            view.update(cx, |this, cx| {
                                this.edit_onboarding(cx, |this, setup, cx| {
                                    setup.prompts.remove(p);
                                    this.onboarding_inputs.remove(p);
                                });
                            })
                            .ok();
                        }
                    })),
            );
        }
        let on_flag = |id: &'static str, label: &'static str, on: bool, cx: &mut Context<Self>, flip: fn(&mut OnboardingPrompt)| {
            let view = cx.weak_entity();
            widgets::switch_row(id, label, None, on, move |_, _, cx| {
                view.update(cx, |this, cx| {
                    this.edit_onboarding(cx, |_, setup, _| {
                        if let Some(prompt) = setup.prompts.get_mut(p) {
                            flip(prompt);
                        }
                    });
                })
                .ok();
            })
        };
        card = card
            .child(on_flag("onboarding-single", "Members can pick one answer", prompt.single_select, cx, |p| p.single_select = !p.single_select))
            .child(on_flag("onboarding-required", "Required", prompt.required, cx, |p| p.required = !p.required))
            .child(on_flag("onboarding-shown", "Ask when someone joins", prompt.in_onboarding, cx, |p| p.in_onboarding = !p.in_onboarding));

        let mut answers = div().flex().flex_col().gap_2().child(label("Answers"));
        for (o, option) in prompt.options.iter().enumerate() {
            let Some(inputs) = self.onboarding_inputs.get(p).and_then(|i| i.options.get(o)) else { continue };
            let mut row = div()
                .flex()
                .items_center()
                .gap_2()
                .child(div().flex_1().child(Input::new(&inputs.title)))
                .child(div().flex_1().child(Input::new(&inputs.description)));
            let attached = option.channel_ids.len() + option.role_ids.len();
            if attached > 0 {
                row = row.child(div().flex_none().text_color(rgb(theme::TEXT_MUTED)).child(format!("{attached} attached")));
            }
            answers = answers.child(row.child(button(("onboarding-option-remove", (p * 1000 + o) as u64), "Remove", Tone::Danger, {
                let view = cx.weak_entity();
                move |_, _, cx| {
                    view.update(cx, |this, cx| {
                        this.edit_onboarding(cx, |this, setup, cx| {
                            setup.prompts[p].options.remove(o);
                            this.onboarding_inputs[p].options.remove(o);
                        });
                    })
                    .ok();
                }
            })));
        }
        if prompt.options.len() < MAX_ONBOARDING_OPTIONS {
            answers = answers.child(div().flex().child(button(("onboarding-option-add", p as u64), "Add Answer", Tone::Plain, {
                let view = cx.weak_entity();
                move |_, window, cx| {
                    view.update(cx, |this, cx| {
                        let id = this.fresh_id();
                        this.edit_onboarding(cx, |this, setup, cx| {
                            if setup.prompts[p].add_option(id) {
                                let option = setup.prompts[p].options.last().expect("an option was just added").clone();
                                let inputs = this.option_inputs(&option, window, cx);
                                this.onboarding_inputs[p].options.push(inputs);
                            }
                        });
                    })
                    .ok();
                }
            })));
        }
        card.child(answers).into_any_element()
    }

    /// The text and announcement channels that are not default yet.
    fn onboarding_choices(&self, guild: Snowflake, defaults: &[Snowflake], cx: &Context<Self>) -> Vec<(Snowflake, String)> {
        self.session
            .read(cx)
            .store
            .channel_groups(guild)
            .into_iter()
            .flat_map(|g| g.channels)
            .filter(|c| matches!(c.kind, ChannelType::GuildText | ChannelType::GuildAnnouncement))
            .filter(|c| !defaults.contains(&c.id))
            .map(|c| (c.id, c.name.clone().unwrap_or_default()))
            .collect()
    }

    fn onboarding_page(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Some(guild) = self.guild else { return div().into_any_element() };
        let header = div().flex().flex_col().gap_3().child(title("Onboarding")).child(widgets::blurb(
            "Welcome new members with questions that open the channels and roles that fit them.",
        ));
        let loaded = match self.session.read(cx).admin.onboarding.clone() {
            Load::Ready(setup) => setup,
            Load::Failed(error) => return header.child(status_line("circle-alert", error, theme::DANGER)).into_any_element(),
            _ => return header.child(status_line("loader", "Loading onboarding...".into(), theme::TEXT_MUTED)).into_any_element(),
        };
        let Some(current) = self.onboarding_current(cx) else { return header.into_any_element() };
        let changed = current.edit_body() != loaded.edit_body();
        let label = |text: &'static str| div().font_weight(FontWeight::MEDIUM).text_color(rgb(theme::TEXT_BRIGHT)).child(text);

        let mut defaults = div().flex().flex_col().gap_2().child(label("Default channels"));
        for (index, id) in current.default_channel_ids.iter().enumerate() {
            let name = self.session.read(cx).store.channel(*id).and_then(|c| c.name.clone()).unwrap_or_else(|| "unknown-channel".to_owned());
            let id = *id;
            defaults = defaults.child(
                div()
                    .p_3()
                    .flex()
                    .items_center()
                    .justify_between()
                    .rounded(px(theme::RADIUS))
                    .bg(rgb(theme::BG_PANEL))
                    .child(div().truncate().text_color(rgb(theme::TEXT_BRIGHT)).child(format!("#{name}")))
                    .child(button(("onboarding-default-remove", index as u64), "Remove", Tone::Danger, {
                        let view = cx.weak_entity();
                        move |_, _, cx| {
                            view.update(cx, |this, cx| this.edit_onboarding(cx, |_, setup, _| setup.toggle_default_channel(id))).ok();
                        }
                    })),
            );
        }
        defaults = defaults.child(div().flex().child(button("onboarding-default-add", "Add Channel", Tone::Plain, {
            let view = cx.weak_entity();
            move |_, _, cx| {
                view.update(cx, |this, cx| {
                    this.onboarding_menu = !this.onboarding_menu;
                    cx.notify();
                })
                .ok();
            }
        })));
        if self.onboarding_menu {
            let mut menu = div()
                .id("onboarding-channel-menu")
                .max_h(px(200.))
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .rounded(px(theme::RADIUS))
                .border_1()
                .border_color(rgb(theme::BORDER))
                .bg(rgb(theme::BG_CHAT));
            for (option, name) in self.onboarding_choices(guild, &current.default_channel_ids, cx) {
                menu = menu.child(
                    div()
                        .id(("onboarding-channel-option", option.0))
                        .h(px(32.))
                        .px_3()
                        .flex()
                        .items_center()
                        .cursor_pointer()
                        .text_color(rgb(theme::TEXT))
                        .hover(|s| s.bg(rgb(theme::BG_HOVER)))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.onboarding_menu = false;
                            this.edit_onboarding(cx, |_, setup, _| setup.toggle_default_channel(option));
                        }))
                        .child(format!("#{name}")),
                );
            }
            defaults = defaults.child(menu);
        }

        let mut questions = div().flex().flex_col().gap_3().child(label("Questions"));
        for (p, prompt) in current.prompts.iter().enumerate() {
            questions = questions.child(self.onboarding_prompt(p, prompt, cx));
        }
        questions = questions.child(div().flex().child(button("onboarding-prompt-add", "Add Question", Tone::Plain, {
            let view = cx.weak_entity();
            move |_, window, cx| {
                view.update(cx, |this, cx| {
                    let id = this.fresh_id();
                    this.edit_onboarding(cx, |this, setup, cx| {
                        if setup.add_prompt(id) {
                            let inputs = PromptInputs { title: this.onboarding_input("", "Write a question", window, cx), options: Vec::new() };
                            this.onboarding_inputs.push(inputs);
                        }
                    });
                })
                .ok();
            }
        })));

        let mut page = header
            .child(self.onboarding_switch("onboarding-enabled", "Enable onboarding", current.enabled, cx, |setup| setup.enabled = !setup.enabled))
            .child(defaults)
            .child(questions);
        if changed {
            page = page.child(div().flex().child(button("save-onboarding", "Save Changes", Tone::Primary, {
                let view = cx.weak_entity();
                move |_, _, cx| {
                    view.update(cx, |this, cx| {
                        if let Some(setup) = this.onboarding_current(cx) {
                            this.onboarding = None;
                            this.send(AdminRequest::SaveOnboardingSetup { guild, body: setup.edit_body() }, cx);
                        }
                    })
                    .ok();
                }
            })));
        }
        page.into_any_element()
    }

    fn boost_perks(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Some(guild) = self.guild else { return div().into_any_element() };
        let name = self
            .session
            .read(cx)
            .store
            .guild(guild)
            .map(|g| g.name.clone())
            .unwrap_or_default();

        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(title("Boost Perks"))
            .child(group(
                "Boost Perks",
                vec![widgets::info_row("Server Name", name)],
            ))
            .into_any_element()
    }

    /// Opens one rule's editor with what the server has for it.
    fn pick_rule(&mut self, rule: &AutoModRule, window: &mut Window, cx: &mut Context<Self>) {
        self.rule = Some(rule.id);
        self.armed = None;
        self.rule_name.update(cx, |i, cx| i.set_value(rule.name.clone(), window, cx));
        self.rule_words.update(cx, |i, cx| i.set_value(rule.keywords.join(", "), window, cx));
        cx.notify();
    }

    /// The form that makes a custom word rule: a name and the words to block.
    fn new_rule_form(&mut self, guild: Snowflake, cx: &mut Context<Self>) -> AnyElement {
        let name = self.new_rule_name.read(cx).value().trim().to_owned();
        let words = parse_keywords(self.new_rule_words.read(cx).value().as_ref());
        let ready = !name.is_empty() && !words.is_empty();
        let label = |text: &'static str| div().font_weight(FontWeight::MEDIUM).text_color(rgb(theme::TEXT_BRIGHT)).child(text);
        let mut form = div()
            .p_4()
            .flex()
            .flex_col()
            .gap_3()
            .rounded(px(theme::RADIUS))
            .bg(rgb(theme::BG_PANEL))
            .child(label("Create a custom word rule"))
            .child(
                div()
                    .flex()
                    .gap_4()
                    .child(div().flex_1().child(Input::new(&self.new_rule_name)))
                    .child(div().flex_1().child(Input::new(&self.new_rule_words))),
            );
        if ready {
            form = form.child(div().flex().child(button("create-rule", "Create Rule", Tone::Primary, {
                let view = cx.weak_entity();
                move |_, window, cx| {
                    view.update(cx, |this, cx| {
                        let name = this.new_rule_name.read(cx).value().trim().to_owned();
                        let words = parse_keywords(this.new_rule_words.read(cx).value().as_ref());
                        if name.is_empty() || words.is_empty() {
                            return;
                        }
                        this.new_rule_name.update(cx, |i, cx| i.set_value("", window, cx));
                        this.new_rule_words.update(cx, |i, cx| i.set_value("", window, cx));
                        this.send(AdminRequest::CreateAutomod { guild, body: keyword_rule_body(&name, &words) }, cx);
                    })
                    .ok();
                }
            })));
        }
        form.into_any_element()
    }

    /// One AutoMod rule: its name, what it watches for and does, and whether it
    /// is on. Clicking it opens the editor.
    fn automod_card(&mut self, guild: Snowflake, rule: &AutoModRule, cx: &mut Context<Self>) -> AnyElement {
        let id = rule.id;
        let open = self.rule == Some(id);
        let (state, color) = if rule.enabled { ("On", theme::ONLINE) } else { ("Off", theme::TEXT_MUTED) };
        let header = div()
            .id(("automod-rule", id.0))
            .h(px(72.))
            .px_4()
            .flex()
            .items_center()
            .gap_4()
            .cursor_pointer()
            .child(icon("shield-check", 24., theme::TEXT_MUTED))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(div().font_weight(FontWeight::MEDIUM).text_color(rgb(theme::TEXT_BRIGHT)).child(rule.name.clone()))
                    .child(div().text_size(px(12.)).text_color(rgb(theme::TEXT)).child(automod_detail(rule))),
            )
            .child(div().text_size(px(12.)).font_weight(FontWeight::MEDIUM).text_color(rgb(color)).child(state))
            .child(icon(if open { "chevron-down" } else { "chevron-right" }, 16., theme::TEXT_MUTED))
            .on_click({
                let rule = rule.clone();
                cx.listener(move |this, _: &ClickEvent, window, cx| {
                    if this.rule == Some(rule.id) {
                        this.rule = None;
                        cx.notify();
                    } else {
                        this.pick_rule(&rule, window, cx);
                    }
                })
            });

        let mut card = div().flex().flex_col().rounded(px(theme::RADIUS)).bg(rgb(theme::BG_PANEL)).child(header);
        if open {
            card = card.child(self.automod_editor(guild, rule, cx));
        }
        card.into_any_element()
    }

    fn automod_editor(&mut self, guild: Snowflake, rule: &AutoModRule, cx: &mut Context<Self>) -> AnyElement {
        let id = rule.id;
        let name = self.rule_name.read(cx).value().to_string();
        let words = parse_keywords(self.rule_words.read(cx).value().as_ref());
        let changed = rule.changed(&name, &words) && !name.trim().is_empty();
        let armed = self.armed == Some(Armed::Rule(id));
        let label = |text: &'static str| div().font_weight(FontWeight::MEDIUM).text_color(rgb(theme::TEXT_BRIGHT)).child(text);
        let column = |title: &'static str, input: &Entity<InputState>| {
            div().flex_1().flex().flex_col().gap_2().child(label(title)).child(Input::new(input))
        };

        let mut fields = div().flex().gap_4().child(column("Name", &self.rule_name));
        if rule.trigger == 1 {
            fields = fields.child(column("Words", &self.rule_words));
        }

        let mut actions = div().flex().gap_2();
        if changed {
            actions = actions.child(button("save-rule", "Save Changes", Tone::Primary, {
                let view = cx.weak_entity();
                move |_, _, cx| {
                    view.update(cx, |this, cx| {
                        let name = this.rule_name.read(cx).value().trim().to_owned();
                        let words = parse_keywords(this.rule_words.read(cx).value().as_ref());
                        // Only word rules own a word list to send.
                        let fields = if words.is_empty() { json!({ "name": name }) } else { AutoModRule::edit_body(&name, &words) };
                        this.send(AdminRequest::EditAutomod { guild, rule: id, fields }, cx);
                    })
                    .ok();
                }
            }));
        }
        let enabled = rule.enabled;
        actions = actions.child(button(
            "toggle-rule",
            if enabled { "Turn Off" } else { "Turn On" },
            Tone::Plain,
            {
                let view = cx.weak_entity();
                move |_, _, cx| {
                    view.update(cx, |this, cx| {
                        this.send(AdminRequest::EditAutomod { guild, rule: id, fields: json!({ "enabled": !enabled }) }, cx);
                    })
                    .ok();
                }
            },
        ));
        actions = actions.child(button(
            "delete-rule",
            if armed { "Click again to delete" } else { "Delete Rule" },
            Tone::Danger,
            {
                let view = cx.weak_entity();
                move |_, _, cx| {
                    view.update(cx, |this, cx| {
                        if this.confirm(Armed::Rule(id), cx) {
                            this.rule = None;
                            this.send(AdminRequest::DeleteAutomod { guild, rule: id }, cx);
                        }
                    })
                    .ok();
                }
            },
        ));

        div()
            .mx_4()
            .pt_4()
            .pb_4()
            .flex()
            .flex_col()
            .gap_4()
            .border_t_1()
            .border_color(rgb(theme::BORDER))
            .child(fields)
            .child(actions)
            .into_any_element()
    }

    fn automod(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Some(guild) = self.guild else { return div().into_any_element() };
        let state = self.session.read(cx).admin.automod.clone();
        let mut body = div().flex().flex_col().gap_3();
        match state {
            Load::Ready(rules) if rules.is_empty() => {
                body = body.child(status_line("shield-check", "No AutoMod rules yet.".into(), theme::TEXT_MUTED));
            }
            Load::Ready(rules) => {
                for rule in &rules {
                    body = body.child(self.automod_card(guild, rule, cx));
                }
            }
            Load::Failed(error) => body = body.child(status_line("circle-alert", error, theme::DANGER)),
            _ => body = body.child(status_line("loader", "Loading rules...".into(), theme::TEXT_MUTED)),
        }

        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(title("AutoMod"))
            .child(widgets::blurb(
                "Set up automated moderation to keep your community safe. AutoMod rules can block messages, send alerts to moderators and time out members.",
            ))
            .child(div().h(px(1.)).mb_2().bg(rgb(theme::BORDER)))
            .child(self.new_rule_form(guild, cx))
            .child(body)
            .into_any_element()
    }

    fn stickers(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Some(guild) = self.guild else { return div().into_any_element() };
        let name = self
            .session
            .read(cx)
            .store
            .guild(guild)
            .map(|g| g.name.clone())
            .unwrap_or_default();

        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(title("Stickers"))
            .child(group(
                "Stickers",
                vec![widgets::info_row("Server Name", name)],
            ))
            .into_any_element()
    }

    fn soundboard(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Some(guild) = self.guild else { return div().into_any_element() };
        let name = self
            .session
            .read(cx)
            .store
            .guild(guild)
            .map(|g| g.name.clone())
            .unwrap_or_default();

        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(title("Soundboard"))
            .child(group(
                "Soundboard",
                vec![widgets::info_row("Server Name", name)],
            ))
            .into_any_element()
    }

    fn engagement(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Some(guild) = self.guild else { return div().into_any_element() };
        let name = self
            .session
            .read(cx)
            .store
            .guild(guild)
            .map(|g| g.name.clone())
            .unwrap_or_default();

        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(title("Engagement"))
            .child(group(
                "Engagement",
                vec![widgets::info_row("Server Name", name)],
            ))
            .into_any_element()
    }

    fn server_template(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Some(guild) = self.guild else { return div().into_any_element() };
        let name = self
            .session
            .read(cx)
            .store
            .guild(guild)
            .map(|g| g.name.clone())
            .unwrap_or_default();

        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(title("Server Template"))
            .child(group(
                "Server Template",
                vec![widgets::info_row("Server Name", name)],
            ))
            .into_any_element()
    }

    fn widget(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Some(guild) = self.guild else { return div().into_any_element() };
        let name = self
            .session
            .read(cx)
            .store
            .guild(guild)
            .map(|g| g.name.clone())
            .unwrap_or_default();

        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(title("Widget"))
            .child(group("Widget", vec![widgets::info_row("Server", name)]))
            .into_any_element()
    }


    fn safety_setup(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Some(guild) = self.guild else { return div().into_any_element() };
        let name = self
            .session
            .read(cx)
            .store
            .guild(guild)
            .map(|g| g.name.clone())
            .unwrap_or_default();

        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(title("Safety Setup"))
            .child(group("Safety Setup", vec![widgets::info_row("Server", name)]))
            .into_any_element()
    }


    fn server_insights(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Some(guild) = self.guild else { return div().into_any_element() };
        let name = self
            .session
            .read(cx)
            .store
            .guild(guild)
            .map(|g| g.name.clone())
            .unwrap_or_default();

        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(title("Server Insights"))
            .child(group("Server Insights", vec![widgets::info_row("Server", name)]))
            .into_any_element()
    }


    fn partner_program(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Some(guild) = self.guild else { return div().into_any_element() };
        let name = self
            .session
            .read(cx)
            .store
            .guild(guild)
            .map(|g| g.name.clone())
            .unwrap_or_default();

        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(title("Partner Program"))
            .child(group("Partner Program", vec![widgets::info_row("Server", name)]))
            .into_any_element()
    }


    fn monetization(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Some(guild) = self.guild else { return div().into_any_element() };
        let name = self
            .session
            .read(cx)
            .store
            .guild(guild)
            .map(|g| g.name.clone())
            .unwrap_or_default();

        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(title("Monetization"))
            .child(group("Monetization", vec![widgets::info_row("Server", name)]))
            .into_any_element()
    }


    fn discovery(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Some(guild) = self.guild else { return div().into_any_element() };
        let name = self
            .session
            .read(cx)
            .store
            .guild(guild)
            .map(|g| g.name.clone())
            .unwrap_or_default();

        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(title("Discovery"))
            .child(group("Discovery", vec![widgets::info_row("Server", name)]))
            .into_any_element()
    }


    fn roles(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Some(guild) = self.guild else { return div().into_any_element() };
        let mut roles: Vec<_> = self
            .session
            .read(cx)
            .store
            .guild(guild)
            .map(|g| g.roles.clone())
            .unwrap_or_default();
        roles.sort_by_key(|r| std::cmp::Reverse(r.position));

        let mut list = div().w(px(200.)).flex_none().flex().flex_col().gap(px(1.));
        for r in &roles {
            let (id, selected) = (r.id, self.role == Some(r.id));
            list = list.child(
                div()
                    .id(("role", id.0))
                    .h(px(30.))
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .rounded(px(4.))
                    .cursor_pointer()
                    .bg(rgb(if selected { theme::BG_ACTIVE } else { theme::BG_CHAT }))
                    .hover(|s| s.bg(rgb(theme::BG_HOVER)))
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| this.pick_role(id, window, cx)))
                    .child(div().size(px(12.)).flex_none().rounded_full().bg(rgb(if r.color == 0 { theme::OFFLINE } else { r.color })))
                    .child(div().flex_1().min_w_0().truncate().text_color(rgb(theme::TEXT)).child(r.name.clone())),
            );
        }

        let create = div()
            .flex()
            .flex_col()
            .gap_2()
            .mt_2()
            .child(Input::new(&self.new_role))
            .child(button("create-role", "Create Role", Tone::Plain, {
                let view = cx.weak_entity();
                move |_, window, cx| {
                    view.update(cx, |this, cx| {
                        let name = this.new_role.read(cx).value().trim().to_owned();
                        if !name.is_empty() {
                            this.send(AdminRequest::CreateRole { guild, name }, cx);
                            Self::clear(&this.new_role, window, cx);
                        }
                    })
                    .ok();
                }
            }));

        let editor = self.role_editor(guild, cx);
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(title("Roles"))
            .child(div().flex().gap_4().child(div().flex().flex_col().child(list).child(create)).child(editor))
            .into_any_element()
    }

    fn role_editor(&mut self, guild: Snowflake, cx: &mut Context<Self>) -> AnyElement {
        let Some(role) = self.role else {
            return div().text_color(rgb(theme::TEXT_MUTED)).child("Pick a role to edit it.").into_any_element();
        };
        let everyone = role == guild;
        let managed = self
            .session
            .read(cx)
            .store
            .guild(guild)
            .and_then(|g| g.roles.iter().find(|r| r.id == role))
            .is_some_and(|r| r.managed);

        let mut column = div().flex_1().min_w_0().flex().flex_col().gap_3();
        if !everyone {
            column = column.child(group(
                "Appearance",
                vec![
                    Input::new(&self.role_name).into_any_element(),
                    Input::new(&self.role_color).into_any_element(),
                    div()
                        .text_size(px(12.))
                        .text_color(rgb(if self.color_error { theme::DANGER } else { theme::TEXT_MUTED }))
                        .child(if self.color_error { "Use a color like #5865f2." } else { "Leave the color empty for none." })
                        .into_any_element(),
                    widgets::switch_row("role-hoist", "Show separately in the member list", None, self.role_hoist, {
                        let view = cx.weak_entity();
                        move |_, _, cx| {
                            view.update(cx, |this, cx| {
                                this.role_hoist = !this.role_hoist;
                                cx.notify();
                            })
                            .ok();
                        }
                    }),
                    widgets::switch_row("role-mentionable", "Allow anyone to mention this role", None, self.role_mentionable, {
                        let view = cx.weak_entity();
                        move |_, _, cx| {
                            view.update(cx, |this, cx| {
                                this.role_mentionable = !this.role_mentionable;
                                cx.notify();
                            })
                            .ok();
                        }
                    }),
                ],
            ));
        }

        let mut heading = "";
        let mut rows: Vec<AnyElement> = Vec::new();
        let mut groups: Vec<AnyElement> = Vec::new();
        let flush = |heading: &'static str, rows: Vec<AnyElement>, groups: &mut Vec<AnyElement>| {
            if !rows.is_empty() {
                groups.push(group(heading, rows).into_any_element());
            }
        };
        for (flag, label, section) in ROLE_PERMISSIONS {
            if *section != heading {
                flush(if heading.is_empty() { "General" } else { heading_static(heading) }, std::mem::take(&mut rows), &mut groups);
                heading = section;
            }
            let (flag, on) = (*flag, self.role_perms.contains(*flag));
            let view = cx.weak_entity();
            rows.push(widgets::switch_row(label, label, None, on, move |_, _, cx| {
                view.update(cx, |this, cx| {
                    this.role_perms.toggle(flag);
                    cx.notify();
                })
                .ok();
            }));
        }
        flush(heading_static(heading), rows, &mut groups);
        column = column.children(groups);

        let armed = self.armed == Some(Armed::Role(role));
        column = column.child(
            div()
                .flex()
                .gap_2()
                .child(button("save-role", "Save Changes", Tone::Primary, {
                    let view = cx.weak_entity();
                    move |_, _, cx| {
                        view.update(cx, |this, cx| this.save_role(cx)).ok();
                    }
                }))
                .children((!everyone && !managed).then(|| {
                    button("delete-role", if armed { "Click again to delete" } else { "Delete Role" }, Tone::Danger, {
                        let view = cx.weak_entity();
                        move |_, _, cx| {
                            view.update(cx, |this, cx| {
                                if this.confirm(Armed::Role(role), cx) {
                                    this.send(AdminRequest::DeleteRole { guild, role }, cx);
                                    this.role = None;
                                }
                            })
                            .ok();
                        }
                    })
                })),
        );
        column.into_any_element()
    }

    fn channels(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Some(guild) = self.guild else { return div().into_any_element() };
        let mut rows = div().flex().flex_col().gap(px(1.));
        let groups: Vec<_> = {
            let store = &self.session.read(cx).store;
            store
                .channel_groups(guild)
                .into_iter()
                .map(|g| {
                    (
                        g.category.map(|c| (c.id, c.name.clone().unwrap_or_default())),
                        g.channels.into_iter().map(|c| (c.id, c.name.clone().unwrap_or_default(), c.kind)).collect::<Vec<_>>(),
                    )
                })
                .collect()
        };
        for (category, channels) in groups {
            if let Some((id, name)) = category {
                rows = rows.child(self.channel_row(id, name, ChannelType::GuildCategory, cx));
                if self.channel == Some(id) {
                    rows = rows.child(self.channel_editor(guild, id, ChannelType::GuildCategory, cx));
                }
            }
            for (id, name, kind) in channels {
                rows = rows.child(self.channel_row(id, name, kind, cx));
                if self.channel == Some(id) {
                    rows = rows.child(self.channel_editor(guild, id, kind, cx));
                }
            }
        }

        let mut kinds = div().flex().gap_1();
        for (kind, label) in [(0u8, "Text"), (2, "Voice"), (4, "Category")] {
            let on = self.new_kind == kind;
            kinds = kinds.child(
                div()
                    .id(SharedString::from(format!("new-kind-{kind}")))
                    .h(px(26.))
                    .px_2()
                    .flex()
                    .items_center()
                    .rounded(px(theme::RADIUS))
                    .cursor_pointer()
                    .text_size(px(12.))
                    .text_color(rgb(theme::TEXT_BRIGHT))
                    .bg(rgb(if on { theme::ACCENT } else { theme::BG_ACTIVE }))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.new_kind = kind;
                        cx.notify();
                    }))
                    .child(label),
            );
        }

        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(title("Channels"))
            .child(group(
                "Create channel",
                vec![
                    Input::new(&self.new_channel).into_any_element(),
                    kinds.into_any_element(),
                    button("create-channel", "Create Channel", Tone::Primary, {
                        let view = cx.weak_entity();
                        move |_, window, cx| {
                            view.update(cx, |this, cx| {
                                let name = this.new_channel.read(cx).value().trim().to_owned();
                                if !name.is_empty() {
                                    let kind = this.new_kind;
                                    this.send(AdminRequest::CreateChannel { guild, name, kind, parent: None }, cx);
                                    Self::clear(&this.new_channel, window, cx);
                                }
                            })
                            .ok();
                        }
                    }),
                ],
            ))
            .child(rows)
            .into_any_element()
    }

    fn channel_row(&mut self, id: Snowflake, name: String, kind: ChannelType, cx: &mut Context<Self>) -> AnyElement {
        let glyph = match kind {
            ChannelType::GuildCategory => "chevron-down",
            ChannelType::GuildVoice | ChannelType::GuildStageVoice => "volume-2",
            ChannelType::GuildAnnouncement => "megaphone",
            ChannelType::GuildForum => "messages-square",
            _ => "hash",
        };
        let selected = self.channel == Some(id);
        div()
            .id(("manage-channel", id.0))
            .h(px(32.))
            .px_2()
            .flex()
            .items_center()
            .gap_2()
            .rounded(px(4.))
            .cursor_pointer()
            .bg(rgb(if selected { theme::BG_ACTIVE } else { theme::BG_CHAT }))
            .hover(|s| s.bg(rgb(theme::BG_HOVER)))
            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                if this.channel == Some(id) {
                    this.channel = None;
                    cx.notify();
                } else {
                    this.pick_channel(id, window, cx);
                }
            }))
            .child(icon(glyph, 16., theme::TEXT_MUTED))
            .child(div().text_color(rgb(theme::TEXT)).child(name))
            .into_any_element()
    }

    fn channel_editor(&mut self, guild: Snowflake, id: Snowflake, kind: ChannelType, cx: &mut Context<Self>) -> AnyElement {
        let armed = self.armed == Some(Armed::Channel(id));
        let has_topic = matches!(kind, ChannelType::GuildText | ChannelType::GuildAnnouncement | ChannelType::GuildForum);
        let mut rows = vec![Input::new(&self.channel_name).into_any_element()];
        if has_topic {
            rows.push(Input::new(&self.channel_topic).into_any_element());
        }
        rows.push(
            div()
                .flex()
                .gap_2()
                .child(button("save-channel", "Save", Tone::Primary, {
                    let view = cx.weak_entity();
                    move |_, _, cx| {
                        view.update(cx, |this, cx| {
                            let mut fields = json!({ "name": this.channel_name.read(cx).value().trim() });
                            if has_topic {
                                fields["topic"] = json!(this.channel_topic.read(cx).value().trim());
                            }
                            this.send(AdminRequest::EditChannel { channel: id, fields }, cx);
                        })
                        .ok();
                    }
                }))
                .child(button(
                    "delete-channel",
                    if armed { "Click again to delete" } else { "Delete" },
                    Tone::Danger,
                    {
                        let view = cx.weak_entity();
                        move |_, _, cx| {
                            view.update(cx, |this, cx| {
                                if this.confirm(Armed::Channel(id), cx) {
                                    this.send(AdminRequest::DeleteChannel(id), cx);
                                    this.channel = None;
                                }
                            })
                            .ok();
                            let _ = guild;
                        }
                    },
                ))
                .into_any_element(),
        );
        div().ml_6().my_1().flex().flex_col().gap_2().p_3().rounded(px(8.)).bg(rgb(theme::BG_PANEL)).children(rows).into_any_element()
    }

    fn emoji(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Some(guild) = self.guild else { return div().into_any_element() };
        let emojis: Vec<_> = self
            .session
            .read(cx)
            .store
            .guild(guild)
            .map(|g| g.emojis.iter().filter_map(|e| Some((e.id?, e.name.clone()?))).collect())
            .unwrap_or_default();

        let mut list = div().flex().flex_col().gap(px(1.));
        if emojis.is_empty() {
            list = list.child(status_line("info", "This server has no emoji yet.".into(), theme::TEXT_MUTED));
        }
        for (id, name) in emojis {
            let url = cdn::emoji(id, 48);
            let image = self.media.update(cx, |m, _| m.get(&url));
            let selected = self.emoji == Some(id);
            let armed = self.armed == Some(Armed::Emoji(id));
            let mut row = div()
                .id(("emoji-row", id.0))
                .px_2()
                .py_1()
                .flex()
                .flex_col()
                .gap_2()
                .rounded(px(4.))
                .bg(rgb(if selected { theme::BG_ACTIVE } else { theme::BG_CHAT }));
            let header_name = name.clone();
            row = row.child(
                div()
                    .id(("emoji-pick", id.0))
                    .flex()
                    .items_center()
                    .gap_3()
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        this.pick_emoji(id, header_name.clone(), window, cx)
                    }))
                    .child(div().size(px(32.)).flex_none().children(image.map(|i| {
                        gpui_kit::img(gpui_kit::ImageSource::Image(i)).size(px(32.))
                    })))
                    .child(div().text_color(rgb(theme::TEXT)).child(format!(":{name}:"))),
            );
            if selected {
                row = row.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(div().flex_1().child(Input::new(&self.emoji_name)))
                        .child(button("save-emoji", "Rename", Tone::Primary, {
                            let view = cx.weak_entity();
                            move |_, _, cx| {
                                view.update(cx, |this, cx| {
                                    let name = this.emoji_name.read(cx).value().trim().to_owned();
                                    if name.len() >= 2 {
                                        this.send(AdminRequest::RenameEmoji { guild, emoji: id, name }, cx);
                                    }
                                })
                                .ok();
                            }
                        }))
                        .child(button(
                            "delete-emoji",
                            if armed { "Click again" } else { "Delete" },
                            Tone::Danger,
                            {
                                let view = cx.weak_entity();
                                move |_, _, cx| {
                                    view.update(cx, |this, cx| {
                                        if this.confirm(Armed::Emoji(id), cx) {
                                            this.send(AdminRequest::DeleteEmoji { guild, emoji: id }, cx);
                                            this.emoji = None;
                                        }
                                    })
                                    .ok();
                                }
                            },
                        )),
                );
            }
            list = list.child(row);
        }
        div().flex().flex_col().gap_3().child(title("Emoji")).child(list).into_any_element()
    }

    fn invites(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Some(guild) = self.guild else { return div().into_any_element() };
        let state = self.session.read(cx).admin.invites.clone();
        let mut list = div().flex().flex_col().gap_1();
        match state {
            Load::Ready(items) if items.is_empty() => {
                list = list.child(status_line("info", "No active invites.".into(), theme::TEXT_MUTED));
            }
            Load::Ready(items) => {
                for invite in items {
                    let code = invite.code.clone();
                    let limit = if invite.max_uses == 0 { "unlimited".to_owned() } else { invite.max_uses.to_string() };
                    let expires = if invite.max_age == 0 { "never expires".to_owned() } else { format!("lasts {} h", invite.max_age / 3600) };
                    list = list.child(
                        div()
                            .p_2()
                            .flex()
                            .items_center()
                            .gap_3()
                            .rounded(px(theme::RADIUS))
                            .bg(rgb(theme::BG_PANEL))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .child(div().text_color(rgb(theme::TEXT_BRIGHT)).child(invite.code.clone()))
                                    .child(
                                        div().text_size(px(12.)).text_color(rgb(theme::TEXT_MUTED)).child(format!(
                                            "{} \u{b7} #{} \u{b7} {}/{} uses \u{b7} {}",
                                            invite.inviter.clone().unwrap_or_else(|| "Unknown".into()),
                                            invite.channel.clone().unwrap_or_default(),
                                            invite.uses,
                                            limit,
                                            expires,
                                        )),
                                    ),
                            )
                            .child(button(SharedString::from(format!("copy-{code}")), "Copy", Tone::Plain, {
                                let code = code.clone();
                                move |_, _, cx| {
                                    cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(code.clone()));
                                }
                            }))
                            .child(button(SharedString::from(format!("revoke-{code}")), "Revoke", Tone::Danger, {
                                let view = cx.weak_entity();
                                move |_, _, cx| {
                                    let code = code.clone();
                                    view.update(cx, |this, cx| this.send(AdminRequest::RevokeInvite { guild, code }, cx)).ok();
                                }
                            })),
                    );
                }
            }
            Load::Failed(error) => list = list.child(status_line("circle-alert", error, theme::DANGER)),
            _ => list = list.child(status_line("loader", "Loading invites...".into(), theme::TEXT_MUTED)),
        }
        div().flex().flex_col().gap_3().child(title("Invites")).child(list).into_any_element()
    }

    fn bans(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Some(guild) = self.guild else { return div().into_any_element() };
        let state = self.session.read(cx).admin.bans.clone();
        let mut list = div().flex().flex_col().gap_1();
        match state {
            Load::Ready(items) if items.is_empty() => {
                list = list.child(status_line("info", "Nobody is banned.".into(), theme::TEXT_MUTED));
            }
            Load::Ready(items) => {
                for ban in items {
                    let user = ban.user.id;
                    let url = cdn::user_avatar(user, ban.user.avatar.as_deref(), 64);
                    list = list.child(
                        div()
                            .p_2()
                            .flex()
                            .items_center()
                            .gap_3()
                            .rounded(px(theme::RADIUS))
                            .bg(rgb(theme::BG_PANEL))
                            .child(avatar(&self.media, &url, 32., ban.user.display_name(), cx))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .child(div().text_color(rgb(theme::TEXT_BRIGHT)).child(ban.user.display_name().to_owned()))
                                    .child(
                                        div()
                                            .text_size(px(12.))
                                            .text_color(rgb(theme::TEXT_MUTED))
                                            .child(ban.reason.clone().unwrap_or_else(|| "No reason given".into())),
                                    ),
                            )
                            .child(button(("unban", user.0), "Revoke Ban", Tone::Plain, {
                                let view = cx.weak_entity();
                                move |_, _, cx| {
                                    view.update(cx, |this, cx| this.send(AdminRequest::Unban { guild, user }, cx)).ok();
                                }
                            })),
                    );
                }
            }
            Load::Failed(error) => list = list.child(status_line("circle-alert", error, theme::DANGER)),
            _ => list = list.child(status_line("loader", "Loading bans...".into(), theme::TEXT_MUTED)),
        }
        div().flex().flex_col().gap_3().child(title("Bans")).child(list).into_any_element()
    }

    fn audit(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let state = self.session.read(cx).admin.audit.clone();
        let mut list = div().flex().flex_col().gap_1();
        match state {
            Load::Ready(log) if log.entries.is_empty() => {
                list = list.child(status_line("info", "Nothing in the log yet.".into(), theme::TEXT_MUTED));
            }
            Load::Ready(log) => {
                for entry in &log.entries {
                    let who = entry
                        .user_id
                        .and_then(|id| log.users.iter().find(|u| u.id == id))
                        .map_or_else(|| "Someone".to_owned(), |u| u.display_name().to_owned());
                    let when = entry.id.datetime().with_timezone(&Local).format("%d/%m/%Y %H:%M").to_string();
                    list = list.child(
                        div()
                            .p_2()
                            .flex()
                            .flex_col()
                            .rounded(px(theme::RADIUS))
                            .bg(rgb(theme::BG_PANEL))
                            .child(
                                div()
                                    .flex()
                                    .gap_2()
                                    .child(div().font_weight(FontWeight::SEMIBOLD).text_color(rgb(theme::TEXT_BRIGHT)).child(who))
                                    .child(div().text_color(rgb(theme::TEXT)).child(action_name(entry.action)))
                                    .child(div().flex_1())
                                    .child(div().text_size(px(12.)).text_color(rgb(theme::TEXT_FAINT)).child(when)),
                            )
                            .children(entry.reason.clone().map(|r| {
                                div().text_size(px(12.)).text_color(rgb(theme::TEXT_MUTED)).child(format!("Reason: {r}"))
                            })),
                    );
                }
            }
            Load::Failed(error) => list = list.child(status_line("circle-alert", error, theme::DANGER)),
            _ => list = list.child(status_line("loader", "Loading the log...".into(), theme::TEXT_MUTED)),
        }
        div().flex().flex_col().gap_3().child(title("Audit Log")).child(list).into_any_element()
    }
}

/// `group` wants a static heading, and the permission table's are.
fn heading_static(heading: &str) -> &'static str {
    ["General", "Membership", "Text", "Voice"].into_iter().find(|h| *h == heading).unwrap_or("General")
}

impl Render for ServerSettings {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(guild) = self.guild else { return div().into_any_element() };
        self.seed_welcome(window, cx);
        self.seed_onboarding(window, cx);
        let (name, perms) = {
            let s = self.session.read(cx);
            (s.store.guild(guild).map(|g| g.name.clone()).unwrap_or_default(), s.store.guild_permissions(guild))
        };
        let pages = pages_for(perms);

        let mut nav = div().flex().flex_col().gap(px(1.));
        for page in pages {
            let selected = self.page == page;
            nav = nav.child(
                div()
                    .id(SharedString::from(format!("server-page-{}", page.label())))
                    .h(px(32.))
                    .px_2()
                    .flex()
                    .items_center()
                    .rounded(px(theme::RADIUS))
                    .cursor_pointer()
                    .text_color(rgb(if selected { theme::TEXT_BRIGHT } else { theme::TEXT_MUTED }))
                    .bg(rgb(if selected { theme::BG_ACTIVE } else { theme::BG_SIDEBAR }))
                    .hover(|s| s.bg(rgb(theme::BG_HOVER)))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.select(page, cx)))
                    .child(page.label()),
            );
        }

        let body = match self.page {
            Page::Overview => self.overview(cx),
            Page::ServerProfile => self.server_profile(cx),
            Page::ServerTag => self.server_tag(cx),
            Page::Members => self.members(cx),
            Page::Access => self.access(cx),
            Page::Integrations => self.integrations(cx),
            Page::AutoMod => self.automod(cx),
            Page::EnableCommunity => self.enable_community(cx),
            Page::WelcomeScreen => self.welcome_screen(cx),
            Page::Onboarding => self.onboarding_page(cx),
            Page::BoostPerks => self.boost_perks(cx),
            Page::Stickers => self.stickers(cx),
            Page::Soundboard => self.soundboard(cx),
            Page::Engagement => self.engagement(cx),
            Page::ServerTemplate => self.server_template(cx),
            Page::Widget => self.widget(cx),
            Page::SafetySetup => self.safety_setup(cx),
            Page::ServerInsights => self.server_insights(cx),
            Page::PartnerProgram => self.partner_program(cx),
            Page::Monetization => self.monetization(cx),
            Page::Discovery => self.discovery(cx),
            Page::Roles => self.roles(cx),
            Page::Channels => self.channels(cx),
            Page::Emoji => self.emoji(cx),
            Page::Invites => self.invites(cx),
            Page::Bans => self.bans(cx),
            Page::AuditLog => self.audit(cx),
        };

        let sidebar = div()
            .w(px(240.))
            .h_full()
            .flex_none()
            .flex()
            .flex_col()
            .gap_2()
            .p_3()
            .bg(rgb(theme::BG_SIDEBAR))
            .child(
                div()
                    .px_2()
                    .pt_2()
                    .pb_1()
                    .truncate()
                    .text_size(px(12.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(theme::TEXT_MUTED))
                    .child(name.to_uppercase()),
            )
            .child(div().id("server-nav").flex_1().overflow_y_scroll().child(nav));

        let close_button = div()
            .id("server-settings-close")
            .absolute()
            .top(px(12.))
            .right(px(12.))
            .size(px(32.))
            .flex()
            .items_center()
            .justify_center()
            .rounded_full()
            .border_1()
            .border_color(rgb(theme::BORDER))
            .cursor_pointer()
            .hover(|s| s.bg(rgb(theme::BG_HOVER)))
            .on_click(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(CloseServerSettings)))
            .child(icon("x", 16., theme::TEXT_MUTED));

        let content = div()
            .id("server-settings-window")
            .occlude()
            .relative()
            .m_4()
            .w_full()
            .max_w(px(960.))
            .h_full()
            .max_h(px(660.))
            .flex()
            .overflow_hidden()
            .rounded(px(theme::RADIUS))
            .border_1()
            .border_color(rgb(theme::BORDER))
            .bg(rgb(theme::BG_CHAT))
            .child(sidebar)
            .child(div().id("server-page-body").flex_1().min_w_0().h_full().overflow_y_scroll().px_8().py_6().child(body))
            .child(close_button)
            .into_any_element();

        modal(content, Box::new(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(CloseServerSettings))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors_are_read_with_or_without_the_hash() {
        assert_eq!(parse_color("#5865f2"), Some(0x5865f2));
        assert_eq!(parse_color("5865F2"), Some(0x5865f2));
        assert_eq!(parse_color("  "), Some(0));
        assert_eq!(parse_color("#12"), None);
        assert_eq!(parse_color("#zzzzzz"), None);
    }

    #[test]
    fn a_color_round_trips_through_its_text() {
        assert_eq!(parse_color(&color_text(0x00a8fc)), Some(0x00a8fc));
        assert_eq!(color_text(0), "");
    }

    fn hook(id: u64, channel: u64, name: &str) -> WebhookEntry {
        WebhookEntry {
            id: Snowflake(id),
            name: name.into(),
            channel: Some(Snowflake(channel)),
            avatar: None,
            token: None,
            kind: 1,
            creator: None,
        }
    }

    #[test]
    fn webhooks_group_by_channel_in_first_seen_order() {
        let list = [hook(1, 20, "a"), hook(2, 10, "b"), hook(3, 20, "c")];
        let groups = webhooks_by_channel(&list);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].0, Some(Snowflake(20)));
        assert_eq!(groups[0].1.iter().map(|h| h.id).collect::<Vec<_>>(), vec![Snowflake(1), Snowflake(3)]);
        assert_eq!(groups[1].0, Some(Snowflake(10)));
    }

    #[test]
    fn a_webhook_counts_as_changed_only_when_name_or_channel_differ() {
        let h = hook(1, 20, "Wick");
        assert!(!webhook_changed(&h, " Wick ", None));
        assert!(!webhook_changed(&h, "Wick", Some(Snowflake(20))));
        assert!(webhook_changed(&h, "Wick 2", None));
        assert!(webhook_changed(&h, "Wick", Some(Snowflake(21))));
    }

    #[test]
    fn an_integration_line_names_its_kind_account_and_adder() {
        let mut entry = IntegrationEntry {
            id: Snowflake(1),
            name: "My Channel".into(),
            kind: "twitch".into(),
            enabled: true,
            creator: Some("Ada".into()),
            account: Some("adaplays".into()),
            subscribers: 1,
        };
        assert_eq!(integration_detail(&entry), "Twitch \u{b7} adaplays \u{b7} 1 subscriber \u{b7} Added by Ada");
        entry.kind = "discord".into();
        entry.account = None;
        entry.subscribers = 0;
        entry.enabled = false;
        assert_eq!(integration_detail(&entry), "App \u{b7} Disabled \u{b7} Added by Ada");
    }

    #[test]
    fn counts_say_one_or_many() {
        assert_eq!(counted(1, "webhook", "webhooks"), "1 webhook");
        assert_eq!(counted(0, "channel", "channels"), "0 channels");
    }

    #[test]
    fn pages_follow_permissions() {
        assert!(pages_for(Permissions::empty()).is_empty());
        let mods = pages_for(Permissions::BAN_MEMBERS | Permissions::VIEW_AUDIT_LOG);
        assert_eq!(mods, vec![Page::Bans, Page::AuditLog]);
        assert_eq!(pages_for(Permissions::all()).len(), Page::ALL.len());
    }

    #[test]
    fn every_listed_permission_names_a_known_group() {
        for (_, _, group) in ROLE_PERMISSIONS {
            assert_eq!(heading_static(group), *group);
        }
    }
}
