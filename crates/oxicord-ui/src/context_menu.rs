//! The menu for a server or a channel: mute, notification level, mark as
//! read, copy link, leave. It opens where it was asked for and closes on any
//! click outside.

use chrono::Utc;
use gpui_kit::{
    AnyElement, ClipboardItem, ClickEvent, Context, Entity, EventEmitter, InteractiveElement as _, IntoElement, ParentElement as _, Pixels, Point, Render, SharedString,
    StatefulInteractiveElement as _, Styled as _, Window, div, px, rgb, rgba,
};
use oxicord_model::{Channel, NotifyLevel, Snowflake};

use crate::{
    backend::{Backend, Command},
    header_menu::{self, Facts, Row},
    icons::icon,
    media::Media,
    member_list::server_tag_chip,
    menus::{Action, MuteFor, Place, channel_link, guild_of, settings_body},
    server_settings,
    session::{
        OpenEvents, OpenInviteDialog, OpenPrivacySettings, OpenRulesScreening, OpenServerGuide, OpenServerProfile, OpenServerSettings,
        Session,
    },
    theme,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Guild(Snowflake),
    Channel(Snowflake),
    /// The dropdown under the server name, drawn as the live client does.
    /// Right clicking a server icon opens `Guild` instead.
    GuildHeader(Snowflake),
}

pub struct CloseMenu;

impl EventEmitter<CloseMenu> for ContextMenu {}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    Mute,
    Notify,
}

pub struct ContextMenu {
    session: Entity<Session>,
    media: Entity<Media>,
    backend: Backend,
    target: Option<Target>,
    at: Point<Pixels>,
    open: Option<Section>,
    confirm_leave: bool,
}

impl ContextMenu {
    pub fn new(session: Entity<Session>, media: Entity<Media>, backend: Backend) -> Self {
        Self {
            session,
            media,
            backend,
            target: None,
            at: Point::default(),
            open: None,
            confirm_leave: false,
        }
    }

    pub fn show(&mut self, target: Target, at: Point<Pixels>, cx: &mut Context<Self>) {
        self.target = Some(target);
        self.at = at;
        self.open = None;
        self.confirm_leave = false;
        cx.notify();
    }

    /// Whether the dropdown under the server name is the menu showing.
    pub fn is_header(&self) -> bool {
        matches!(self.target, Some(Target::GuildHeader(_)))
    }

    fn channel(&self, cx: &gpui_kit::App) -> Option<Channel> {
        match self.target? {
            Target::Channel(id) => self.session.read(cx).store.channel(id).cloned(),
            Target::Guild(_) | Target::GuildHeader(_) => None,
        }
    }

    fn place(&self, cx: &gpui_kit::App) -> Option<Place> {
        match self.target? {
            Target::Guild(g) | Target::GuildHeader(g) => Some(Place::Guild(g)),
            Target::Channel(_) => {
                let c = self.channel(cx)?;
                Some(Place::Channel { guild: c.guild_id?, channel: c.id })
            }
        }
    }

    fn close(&mut self, cx: &mut Context<Self>) {
        self.target = None;
        cx.emit(CloseMenu);
        cx.notify();
    }

    /// Carries out one choice, then closes the menu.
    fn run(&mut self, action: Action, cx: &mut Context<Self>) {
        if let Some(place) = self.place(cx)
            && let Some(body) = settings_body(place, action, Utc::now())
        {
            self.backend.send(Command::PatchGuildSettings { guild: guild_of(place), body });
        } else {
            match (action, self.target) {
                (Action::MarkRead, Some(Target::Guild(guild))) => self.mark_guild_read(guild, cx),
                (Action::MarkRead, Some(Target::Channel(channel))) => self.mark_channel_read(channel, cx),
                (Action::CopyLink, Some(Target::Channel(channel))) => {
                    let guild = self.channel(cx).and_then(|c| c.guild_id);
                    cx.write_to_clipboard(ClipboardItem::new_string(channel_link(guild, channel)));
                }
                (Action::CopyId, Some(Target::Channel(channel))) => {
                    cx.write_to_clipboard(ClipboardItem::new_string(channel.to_string()));
                }
                (Action::CopyGuildId, Some(Target::Guild(guild) | Target::GuildHeader(guild))) => {
                    cx.write_to_clipboard(ClipboardItem::new_string(guild.to_string()));
                }
                (Action::Leave, Some(Target::Guild(guild) | Target::GuildHeader(guild))) => {
                    self.backend.send(Command::LeaveGuild(guild));
                }
                _ => {}
            }
        }
        self.close(cx);
    }

    fn mark_guild_read(&mut self, guild: Snowflake, cx: &mut Context<Self>) {
        mark_guild_read(&self.session, &self.backend, guild, cx);
    }

    fn mark_channel_read(&mut self, channel: Snowflake, cx: &mut Context<Self>) {
        mark_channel_read(&self.session, &self.backend, channel, cx);
    }

    /// A channel where this person may make an invite, if there is one.
    fn invite_channel(&self, guild: Snowflake, cx: &gpui_kit::App) -> Option<Snowflake> {
        let store = &self.session.read(cx).store;
        store
            .channel_groups(guild)
            .iter()
            .flat_map(|group| group.channels.iter())
            .find(|c| store.can_invite(c))
            .map(|c| c.id)
    }

    /// One row of the header menu: a 36px row with the picture at the left
    /// and the label after it, or the tag chip and its caption for the tag.
    fn header_row(
        &mut self,
        row: Row,
        lead: Option<AnyElement>,
        checked: Option<bool>,
        on_click: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let danger = row == Row::Leave;
        let label = if danger && self.confirm_leave { "Click again to leave" } else { row.label() };
        let group = SharedString::from(format!("header-row-{row:?}"));
        let color = if danger { theme::DANGER } else { theme::HEADER_MENU_TEXT };
        let lead = lead.unwrap_or_else(|| {
            icon(row.glyph(), 20., if danger { theme::DANGER } else { theme::TEXT_MUTED })
                .group_hover(group.clone(), move |s| s.text_color(rgb(if danger { theme::DANGER } else { theme::HEADER_MENU_TEXT })))
                .into_any_element()
        });
        div()
            .id(("header-row", row as u64))
            .group(group)
            .h(px(if row == Row::Tag { 34. } else { 36. }))
            .flex_none()
            .px(px(8.))
            .flex()
            .items_center()
            .gap(px(8.))
            .rounded(px(4.))
            .cursor_pointer()
            .text_size(px(14.))
            .line_height(px(18.))
            .font_weight(gpui_kit::FontWeight::MEDIUM)
            .text_color(rgb(color))
            .hover(move |s| s.bg(rgb(if danger { theme::HEADER_MENU_DANGER_HOVER } else { theme::HEADER_MENU_HOVER })))
            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| on_click(this, window, cx)))
            .child(lead)
            .child(div().flex_1().min_w_0().truncate().child(label))
            .children(checked.map(|on| {
                div()
                    .size(px(20.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(4.))
                    .border_1()
                    .border_color(rgba(if on { 0x5865f2ff } else { 0x96979ea3 }))
                    .bg(rgba(if on { 0x5865f2ff } else { 0x00000014 }))
                    .children(on.then(|| icon("check", 18., 0xffffff)))
            }))
            .into_any_element()
    }

    /// The dropdown under the server name: grouped rows with pictures, lines
    /// between the groups.
    fn render_header(&mut self, guild: Snowflake, cx: &mut Context<Self>) -> AnyElement {
        let (facts, hide_muted, tag) = {
            let session = self.session.read(cx);
            let store = &session.store;
            let perms = store.guild_permissions(guild);
            let owner = store.guild(guild).and_then(|g| g.owner_id).is_some_and(|owner| store.me().is_some_and(|me| me.id == owner));
            let tag = session.guild_tags.get(&guild).cloned().flatten();
            let facts = Facts {
                tag: tag.is_some(),
                manage: server_settings::can_access(perms),
                events: perms.intersects(oxicord_model::Permissions::CREATE_EVENTS | oxicord_model::Permissions::MANAGE_EVENTS),
                owner,
            };
            (facts, store.hide_muted_channels(guild), tag)
        };
        let invite_channel = self.invite_channel(guild, cx);

        let mut menu = div()
            .id("context-menu")
            .occlude()
            .absolute()
            .left(self.at.x)
            .top(self.at.y)
            .w(px(220.))
            .p(px(8.))
            .flex()
            .flex_col()
            .rounded(px(theme::RADIUS))
            .border_1()
            .border_color(rgb(theme::HEADER_MENU_BORDER))
            .bg(rgb(theme::BG_MENU));

        let layout = header_menu::groups(facts);
        let last = layout.len() - 1;
        for (index, group) in layout.into_iter().enumerate() {
            for row in group {
                let element = match row {
                    Row::Tag => {
                        let Some(tag) = tag.clone() else { continue };
                        let chip = server_tag_chip(&tag, 14., &self.media, cx).into_any_element();
                        self.header_row(row, Some(chip), None, |this, _, cx| this.close(cx), cx)
                    }
                    Row::Boost => self.header_row(row, None, None, |this, _, cx| {
                        this.session.update(cx, |session, cx| {
                            session.set_notice(Some("Server Boosts are bought in the Discord app.".to_owned()), cx);
                        });
                        this.close(cx);
                    }, cx),
                    Row::Invite => self.header_row(row, None, None, move |this, _, cx| {
                        // The live row is always there. Without the right to make an
                        // invite it opens a different sheet, which we don't have.
                        this.session.update(cx, |session, cx| match invite_channel {
                            Some(channel) => cx.emit(OpenInviteDialog(channel)),
                            None => session.set_notice(Some("You can't create invites in this server.".to_owned()), cx),
                        });
                        this.close(cx);
                    }, cx),
                    Row::Settings => self.header_row(row, None, None, move |this, _, cx| {
                        this.session.update(cx, |_, cx| cx.emit(OpenServerSettings(guild)));
                        this.close(cx);
                    }, cx),
                    // The creation form isn't built yet, so this opens the list of events.
                    Row::CreateEvent => self.header_row(row, None, None, move |this, _, cx| {
                        this.session.update(cx, |_, cx| cx.emit(OpenEvents(guild)));
                        this.close(cx);
                    }, cx),
                    Row::AppDirectory => self.header_row(row, None, None, |this, _, cx| {
                        cx.open_url("https://discord.com/application-directory");
                        this.close(cx);
                    }, cx),
                    // The notification modal isn't built yet, so this swaps to the
                    // older list, which has the mute and level choices.
                    Row::Notifications => self.header_row(row, None, None, move |this, _, cx| {
                        this.target = Some(Target::Guild(guild));
                        this.open = Some(Section::Notify);
                        cx.notify();
                    }, cx),
                    Row::Privacy => self.header_row(row, None, None, |this, _, cx| {
                        this.session.update(cx, |_, cx| cx.emit(OpenPrivacySettings));
                        this.close(cx);
                    }, cx),
                    Row::Profile => self.header_row(row, None, None, move |this, _, cx| {
                        this.session.update(cx, |_, cx| cx.emit(OpenServerProfile(guild)));
                        this.close(cx);
                    }, cx),
                    Row::HideMuted => self.header_row(row, None, Some(hide_muted), move |this, _, cx| {
                        this.run(Action::HideMutedChannels(!hide_muted), cx);
                    }, cx),
                    Row::Leave => self.header_row(row, None, None, |this, _, cx| {
                        if this.confirm_leave {
                            this.run(Action::Leave, cx);
                        } else {
                            this.confirm_leave = true;
                            cx.notify();
                        }
                    }, cx),
                    Row::CopyId => self.header_row(row, None, None, |this, _, cx| this.run(Action::CopyGuildId, cx), cx),
                };
                menu = menu.child(element);
            }
            if index != last {
                menu = menu.child(div().flex_none().my(px(8.)).h(px(1.)).bg(rgb(theme::HEADER_MENU_BORDER)));
            }
        }

        div()
            .id("context-scrim")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .on_click(cx.listener(|this, _, _, cx| this.close(cx)))
            .child(menu)
            .into_any_element()
    }

    fn item(
        &mut self,
        id: impl Into<gpui_kit::ElementId>,
        label: impl Into<SharedString>,
        tone: u32,
        checked: bool,
        on_click: impl Fn(&mut Self, &ClickEvent, &mut Window, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .id(id)
            .h(px(32.))
            .px_2()
            .flex()
            .items_center()
            .gap_2()
            .rounded(px(4.))
            .text_size(px(14.))
            .font_weight(gpui_kit::FontWeight::MEDIUM)
            .cursor_pointer()
            .text_color(rgb(tone))
            .hover(|s| s.bg(rgb(theme::BG_HOVER)))
            .on_click(cx.listener(move |this, event, window, cx| on_click(this, event, window, cx)))
            .child(div().flex_1().child(label.into()))
            .children(checked.then(|| icon("check", 14., theme::TEXT_BRIGHT)))
            .into_any_element()
    }

}

impl Render for ContextMenu {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(target) = self.target else { return div().into_any_element() };
        if let Target::GuildHeader(guild) = target {
            return self.render_header(guild, cx);
        }
        let (muted, level, suppress, own_level, in_guild, hide_muted) = {
            let store = &self.session.read(cx).store;
            match target {
                Target::Guild(g) | Target::GuildHeader(g) => (
                    store.guild_muted(g),
                    Some(store.guild_notify_level(g)),
                    store.suppresses_everyone(g),
                    None,
                    true,
                    store.hide_muted_channels(g),
                ),
                Target::Channel(id) => store
                    .channel(id)
                    .map(|c| {
                        (
                            store.is_muted(c),
                            Some(store.notify_level(c)),
                            false,
                            store.channel_level_override(c),
                            c.guild_id.is_some(),
                            false,
                        )
                    })
                    .unwrap_or((false, None, false, None, false, false)),
            }
        };
        let is_guild = matches!(target, Target::Guild(_));
        let noun = if is_guild { "server" } else { "channel" };

        let mut menu = div()
            .id("context-menu")
            .occlude()
            .absolute()
            .left(self.at.x)
            .top(self.at.y)
            .w(px(264.))
            .p(px(8.))
            .gap(px(4.))
            .flex()
            .flex_col()
            .rounded(px(theme::RADIUS))
            .border_1()
            .border_color(rgb(theme::BORDER))
            .bg(rgb(theme::BG_MENU));

        let invite_channel = match target {
            Target::Guild(g) | Target::GuildHeader(g) => self.invite_channel(g, cx),
            Target::Channel(_) => None,
        };
        if let Some(channel) = invite_channel {
            menu = menu.child(self.item("invite", "Invite People", theme::ACCENT, false, move |this, _, _, cx| {
                this.session.update(cx, |_, cx| cx.emit(OpenInviteDialog(channel)));
                this.close(cx);
            }, cx));
        }
        if matches!(target, Target::Guild(_)) {
            menu = menu.child(self.item("server-guide", "Server Guide", theme::TEXT_BRIGHT, false, |this, _, _, cx| {
                if let Some(Target::Guild(guild)) = this.target {
                    this.session.update(cx, |_, cx| cx.emit(OpenServerGuide(guild)));
                }
                this.close(cx);
            }, cx));
        }
        if let Target::Guild(guild) = target {
            let allowed = server_settings::can_manage(self.session.read(cx).store.guild_permissions(guild));
            if allowed {
                menu = menu.child(self.item("server-settings", "Server Settings", theme::TEXT_BRIGHT, false, move |this, _, _, cx| {
                    this.session.update(cx, |_, cx| cx.emit(OpenServerSettings(guild)));
                    this.close(cx);
                }, cx));
            }
        }
        if let Target::Guild(guild) = target {
            menu = menu.child(self.item("nickname", "Change Nickname", theme::TEXT_BRIGHT, false, move |this, _, _, cx| {
                this.session.update(cx, |_, cx| cx.emit(crate::session::OpenNickname(guild)));
                this.close(cx);
            }, cx));
        }
        menu = menu.child(self.item("mark-read", "Mark as read", theme::TEXT_BRIGHT, false, |this, _, _, cx| {
            this.run(Action::MarkRead, cx);
        }, cx));
        if matches!(target, Target::Channel(_))
            && self
                .channel(cx)
                .map(|c| matches!(c.kind, oxicord_model::ChannelType::GuildVoice | oxicord_model::ChannelType::GuildStageVoice))
                .unwrap_or(false)
        {
            menu = menu.child(self.item("open-chat", "Open Chat", theme::TEXT_BRIGHT, false, |this, _, _, cx| {
                if let Some(Target::Channel(channel)) = this.target {
                    this.session.update(cx, |s, cx| s.jump_to(channel, cx));
                }
                this.close(cx);
            }, cx));
            menu = menu.child(self.item("invite-to-voice", "Invite to Voice", theme::TEXT_BRIGHT, false, |this, _, _, cx| {
                if let Some(Target::Channel(channel)) = this.target {
                    this.session.update(cx, |_, cx| cx.emit(OpenInviteDialog(channel)));
                }
                this.close(cx);
            }, cx));
        }

        if matches!(target, Target::Channel(_)) {
            menu = menu.child(self.item("copy-link", "Copy link", theme::TEXT_BRIGHT, false, |this, _, _, cx| {
                this.run(Action::CopyLink, cx);
            }, cx));
        }

        if matches!(target, Target::Guild(_)) {
            menu = menu.child(self.item("copy-guild-id", "Copy server ID", theme::TEXT_BRIGHT, false, |this, _, _, cx| {
                this.run(Action::CopyGuildId, cx);
            }, cx));
        }

        if matches!(target, Target::Channel(_)) {
            menu = menu.child(self.item("copy-id", "Copy channel ID", theme::TEXT_BRIGHT, false, |this, _, _, cx| {
                this.run(Action::CopyId, cx);
            }, cx));
        }

        // Muting and levels are server settings, which conversations lack.
        if in_guild {
            menu = menu.child(div().my(px(2.)).h(px(1.)).bg(rgb(theme::BORDER)));
            if muted {
                menu = menu.child(self.item("unmute", format!("Unmute {noun}"), theme::TEXT_BRIGHT, false, |this, _, _, cx| {
                    this.run(Action::Unmute, cx);
                }, cx));
            } else {
                let opened = self.open == Some(Section::Mute);
                menu = menu.child(self.item(
                    "mute",
                    format!("Mute {noun}"),
                    theme::TEXT_BRIGHT,
                    false,
                    |this, _, _, cx| {
                        this.open = (this.open != Some(Section::Mute)).then_some(Section::Mute);
                        cx.notify();
                    },
                    cx,
                ));
                if opened {
                    for (i, (how, label)) in MuteFor::CHOICES.into_iter().enumerate() {
                        menu = menu.child(div().pl_3().child(self.item(("mute-for", i as u64), label, theme::TEXT_MUTED, false, move |this, _, _, cx| {
                            this.run(Action::Mute(how), cx);
                        }, cx)));
                    }
                }
            }

            let opened = self.open == Some(Section::Notify);
            menu = menu.child(self.item("notify", "Notification settings", theme::TEXT_BRIGHT, false, |this, _, _, cx| {
                this.open = (this.open != Some(Section::Notify)).then_some(Section::Notify);
                cx.notify();
            }, cx));
            if opened {
                let mut choices = vec![
                    (Some(NotifyLevel::All), "All messages"),
                    (Some(NotifyLevel::Mentions), "Only @mentions"),
                    (Some(NotifyLevel::Nothing), "Nothing"),
                ];
                if !is_guild {
                    choices.insert(0, (None, "Use category default"));
                }
                for (i, (choice, label)) in choices.into_iter().enumerate() {
                    let current = if is_guild { level == choice } else { own_level == choice };
                    menu = menu.child(div().pl_3().child(self.item(("level", i as u64), label, theme::TEXT_MUTED, current, move |this, _, _, cx| {
                        this.run(Action::SetLevel(choice), cx);
                    }, cx)));
                }
            }

            if is_guild {
                menu = menu.child(self.item(
                    "suppress",
                    "Suppress @everyone and @here",
                    theme::TEXT_BRIGHT,
                    suppress,
                    move |this, _, _, cx| this.run(Action::SuppressEveryone(!suppress), cx),
                    cx,
                ));
                menu = menu.child(self.item(
                    "hide-muted",
                    "Hide Muted Channels",
                    theme::TEXT_BRIGHT,
                    hide_muted,
                    move |this, _, _, cx| this.run(Action::HideMutedChannels(!hide_muted), cx),
                    cx,
                ));
            }
        }

        if is_guild {
            menu = menu.child(self.item("edit-server-profile", "Edit Per-server Profile", theme::TEXT_BRIGHT, false, |this, _, _, cx| {
                if let Some(Target::Guild(guild)) = this.target {
                    this.session.update(cx, |_, cx| cx.emit(OpenServerProfile(guild)));
                }
                this.close(cx);
            }, cx));
            menu = menu.child(self.item("accept-rules", "Accept Server Rules", theme::TEXT_BRIGHT, false, |this, _, _, cx| {
                if let Some(Target::Guild(guild)) = this.target {
                    this.session.update(cx, |_, cx| cx.emit(OpenRulesScreening { guild }));
                }
                this.close(cx);
            }, cx));
            menu = menu.child(self.item("privacy-settings", "Privacy Settings", theme::TEXT_BRIGHT, false, |this, _, _, cx| {
                this.session.update(cx, |_, cx| cx.emit(OpenPrivacySettings));
                this.close(cx);
            }, cx));
            menu = menu.child(self.item("server-settings", "Server Settings", theme::TEXT_BRIGHT, false, |this, _, _, cx| {
                if let Some(Target::Guild(guild)) = this.target {
                    this.session.update(cx, |_, cx| cx.emit(OpenServerSettings(guild)));
                }
                this.close(cx);
            }, cx));
            menu = menu.child(div().my(px(2.)).h(px(1.)).bg(rgb(theme::BORDER)));
            let armed = self.confirm_leave;
            menu = menu.child(self.item(
                "leave",
                if armed { "Click again to leave" } else { "Leave server" },
                theme::DANGER,
                false,
                |this, _, _, cx| {
                    if this.confirm_leave {
                        this.run(Action::Leave, cx);
                    } else {
                        this.confirm_leave = true;
                        cx.notify();
                    }
                },
                cx,
            ));
        }

        // A transparent sheet behind the menu closes it on any outside click.
        div()
            .id("context-scrim")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .on_click(cx.listener(|this, _, _, cx| this.close(cx)))
            .child(menu)
            .into_any_element()
    }
}

pub(crate) fn mark_guild_read(session: &Entity<Session>, backend: &Backend, guild: Snowflake, cx: &mut gpui_kit::App) {
    let unread = session.read(cx).store.unread_in_guild(guild);
    ack(session, backend, unread, cx);
}

pub(crate) fn mark_channel_read(session: &Entity<Session>, backend: &Backend, channel: Snowflake, cx: &mut gpui_kit::App) {
    let newest = session
        .read(cx)
        .store
        .channel(channel)
        .and_then(|c| c.last_message_id.map(|m| (channel, m)));
    ack(session, backend, newest.into_iter().collect(), cx);
}

/// Clears the badges at once and tells the server.
fn ack(session: &Entity<Session>, backend: &Backend, channels: Vec<(Snowflake, Snowflake)>, cx: &mut gpui_kit::App) {
    if channels.is_empty() {
        return;
    }
    session.update(cx, |session, cx| {
        for (channel, message) in &channels {
            let changes = session.store.mark_read(*channel, *message);
            cx.emit(changes);
        }
        cx.notify();
    });
    backend.send(Command::AckBulk(channels));
}
