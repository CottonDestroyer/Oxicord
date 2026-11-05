//! The menu for a server or a channel: mute, notification level, mark as
//! read, copy link, leave. It opens where it was asked for and closes on any
//! click outside.

use chrono::Utc;
use gpui_kit::{
    AnyElement, ClipboardItem, ClickEvent, Context, Entity, EventEmitter, InteractiveElement as _, IntoElement, ParentElement as _, Pixels, Point, Render, SharedString,
    StatefulInteractiveElement as _, Styled as _, Window, div, px, rgb,
};
use oxicord_model::{Channel, NotifyLevel, Snowflake};

use crate::{
    backend::{Backend, Command},
    icons::icon,
    menus::{Action, MuteFor, Place, channel_link, guild_of, settings_body},
    server_settings,
    session::{OpenInviteDialog, OpenServerSettings, Session},
    theme,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Guild(Snowflake),
    Channel(Snowflake),
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
    backend: Backend,
    target: Option<Target>,
    at: Point<Pixels>,
    open: Option<Section>,
    confirm_leave: bool,
}

impl ContextMenu {
    pub fn new(session: Entity<Session>, backend: Backend) -> Self {
        Self {
            session,
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

    fn channel(&self, cx: &gpui_kit::App) -> Option<Channel> {
        match self.target? {
            Target::Channel(id) => self.session.read(cx).store.channel(id).cloned(),
            Target::Guild(_) => None,
        }
    }

    fn place(&self, cx: &gpui_kit::App) -> Option<Place> {
        match self.target? {
            Target::Guild(g) => Some(Place::Guild(g)),
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
                (Action::Leave, Some(Target::Guild(guild))) => self.backend.send(Command::LeaveGuild(guild)),
                _ => {}
            }
        }
        self.close(cx);
    }

    fn mark_guild_read(&mut self, guild: Snowflake, cx: &mut Context<Self>) {
        let unread = self.session.read(cx).store.unread_in_guild(guild);
        self.ack(unread, cx);
    }

    fn mark_channel_read(&mut self, channel: Snowflake, cx: &mut Context<Self>) {
        let newest = self
            .session
            .read(cx)
            .store
            .channel(channel)
            .and_then(|c| c.last_message_id.map(|m| (channel, m)));
        self.ack(newest.into_iter().collect(), cx);
    }

    /// Clears the badges at once and tells the server.
    fn ack(&mut self, channels: Vec<(Snowflake, Snowflake)>, cx: &mut Context<Self>) {
        if channels.is_empty() {
            return;
        }
        self.session.update(cx, |session, cx| {
            for (channel, message) in &channels {
                let changes = session.store.mark_read(*channel, *message);
                cx.emit(changes);
            }
            cx.notify();
        });
        self.backend.send(Command::AckBulk(channels));
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
        let (muted, level, suppress, own_level, in_guild) = {
            let store = &self.session.read(cx).store;
            match target {
                Target::Guild(g) => (
                    store.guild_muted(g),
                    Some(store.guild_notify_level(g)),
                    store.suppresses_everyone(g),
                    None,
                    true,
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
                        )
                    })
                    .unwrap_or((false, None, false, None, false)),
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
            .p_1()
            .flex()
            .flex_col()
            .rounded(px(8.))
            .border_1()
            .border_color(rgb(theme::BORDER))
            .bg(rgb(theme::BG_PANEL));

        let invite_channel = match target {
            Target::Guild(g) => {
                let store = &self.session.read(cx).store;
                store
                    .channel_groups(g)
                    .iter()
                    .flat_map(|group| group.channels.iter())
                    .find(|c| {
                        !c.kind.is_thread() && store.permissions_in(c).contains(oxicord_model::Permissions::CREATE_INSTANT_INVITE)
                    })
                    .map(|c| c.id)
            }
            Target::Channel(_) => None,
        };
        if let Some(channel) = invite_channel {
            menu = menu.child(self.item("invite", "Invite People", theme::ACCENT, false, move |this, _, _, cx| {
                this.session.update(cx, |_, cx| cx.emit(OpenInviteDialog(channel)));
                this.close(cx);
            }, cx));
        }
        if let Target::Guild(guild) = target {
            let allowed = server_settings::can_manage(self.session.read(cx).store.guild_permissions(guild));
            if allowed {
                menu = menu.child(self.item("server-settings", "Server Settings", theme::TEXT, false, move |this, _, _, cx| {
                    this.session.update(cx, |_, cx| cx.emit(OpenServerSettings(guild)));
                    this.close(cx);
                }, cx));
            }
        }
        menu = menu.child(self.item("mark-read", "Mark as read", theme::TEXT, false, |this, _, _, cx| {
            this.run(Action::MarkRead, cx);
        }, cx));
        if matches!(target, Target::Channel(_)) {
            menu = menu.child(self.item("copy-link", "Copy link", theme::TEXT, false, |this, _, _, cx| {
                this.run(Action::CopyLink, cx);
            }, cx));
        }

        // Muting and levels are server settings, which conversations lack.
        if in_guild {
            menu = menu.child(div().my_1().h(px(1.)).bg(rgb(theme::BORDER)));
            if muted {
                menu = menu.child(self.item("unmute", format!("Unmute {noun}"), theme::TEXT, false, |this, _, _, cx| {
                    this.run(Action::Unmute, cx);
                }, cx));
            } else {
                let opened = self.open == Some(Section::Mute);
                menu = menu.child(self.item(
                    "mute",
                    format!("Mute {noun}"),
                    theme::TEXT,
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
            menu = menu.child(self.item("notify", "Notification settings", theme::TEXT, false, |this, _, _, cx| {
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
                    theme::TEXT,
                    suppress,
                    move |this, _, _, cx| this.run(Action::SuppressEveryone(!suppress), cx),
                    cx,
                ));
            }
        }

        if is_guild {
            menu = menu.child(div().my_1().h(px(1.)).bg(rgb(theme::BORDER)));
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
