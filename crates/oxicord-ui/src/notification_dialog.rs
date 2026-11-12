//! The Notification Settings modal behind the server menu's row: mute, the
//! server's level, the suppress switches, Mute New Events, Mobile Push and a
//! table of per channel overrides. Every control sends one settings patch
//! (`menus::settings_body`), which is built and unit tested but never sent
//! from the real account while checking this.

use chrono::Utc;
use gpui_kit::{
    AnyElement, ClickEvent, Context, Div, Entity, EventEmitter, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled as _, Window, div, px, rgb, rgba,
};
use gpui_kit::prelude::FluentBuilder as _;
use oxicord_model::{ChannelType, NotifyLevel, Snowflake, mute_active};

use crate::{
    backend::{Backend, Command},
    icons::icon,
    menus::{Action, MuteFor, Place, settings_body},
    profile::modal,
    session::Session,
    theme,
};

const CARD_WIDTH: f32 = 480.;
/// The modal never grows past this, then its body scrolls.
const CARD_MAX_HEIGHT: f32 = 800.;
/// The live link text color.
const LINK: u32 = 0x2781e7;
const HIGHLIGHTS_ARTICLE: &str = "https://support.discord.com/hc/en-us/articles/5304469213079";
/// The hairline the live modal draws between its sections.
const HAIRLINE: u32 = 0x97979f33;

pub struct CloseNotificationSettings;

impl EventEmitter<CloseNotificationSettings> for NotificationDialog {}

pub struct NotificationDialog {
    session: Entity<Session>,
    backend: Backend,
    guild: Option<Snowflake>,
    /// Channels picked for an override this time. A row exists from the pick,
    /// before anything is saved, like the live table.
    added: Vec<Snowflake>,
    picking: bool,
    durations: bool,
}

/// One channel or category in the override table and its picker.
struct Entry {
    id: Snowflake,
    name: String,
    category: bool,
    /// The category a channel sits under, shown beside it in the picker.
    parent: Option<String>,
    /// "in Static", "No Category" or "3 channels".
    sub: String,
}

impl NotificationDialog {
    pub fn new(session: Entity<Session>, backend: Backend) -> Self {
        Self { session, backend, guild: None, added: Vec::new(), picking: false, durations: false }
    }

    pub fn opened(&mut self, guild: Snowflake, cx: &mut Context<Self>) {
        self.guild = Some(guild);
        self.added.clear();
        self.picking = false;
        self.durations = false;
        cx.notify();
    }

    fn change(&self, place: Place, action: Action) {
        let Some(guild) = self.guild else { return };
        if let Some(body) = settings_body(place, action, Utc::now()) {
            self.backend.send(Command::PatchGuildSettings { guild, body });
        }
    }

    fn change_guild(&self, action: Action) {
        if let Some(guild) = self.guild {
            self.change(Place::Guild(guild), action);
        }
    }

    fn change_channel(&self, channel: Snowflake, action: Action) {
        if let Some(guild) = self.guild {
            self.change(Place::Channel { guild, channel }, action);
        }
    }

    /// The server's categories and channels in sidebar order, a category
    /// followed by what it holds. A category with nothing in it is left out,
    /// as it is in the live picker.
    fn entries(&self, guild: Snowflake, cx: &Context<Self>) -> Vec<Entry> {
        let store = &self.session.read(cx).store;
        let mut entries = Vec::new();
        for group in store.channel_groups(guild) {
            let category = group.category.map(|c| c.name.clone().unwrap_or_default());
            if let Some(c) = group.category
                && !group.channels.is_empty()
            {
                let count = group.channels.len();
                entries.push(Entry {
                    id: c.id,
                    name: category.clone().unwrap_or_default(),
                    category: true,
                    parent: None,
                    sub: format!("{count} channel{}", if count == 1 { "" } else { "s" }),
                });
            }
            for channel in group.channels {
                if channel.kind == ChannelType::GuildCategory {
                    continue;
                }
                entries.push(Entry {
                    id: channel.id,
                    name: channel.name.clone().unwrap_or_default(),
                    category: false,
                    parent: category.clone(),
                    sub: match &category {
                        Some(name) => format!("in {name}"),
                        None => "No Category".to_owned(),
                    },
                });
            }
        }
        entries
    }
}

fn hairline() -> Div {
    div().w_full().h(px(2.)).flex_none().border_t_1().border_color(rgba(HAIRLINE))
}

fn heading(text: impl Into<SharedString>) -> Div {
    div()
        .text_size(px(16.))
        .line_height(px(20.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(rgb(theme::TEXT_BRIGHT))
        .child(text.into())
}

fn hint(text: &'static str) -> Div {
    div().text_size(px(14.)).line_height(px(18.)).text_color(rgb(theme::TEXT_MUTED)).child(text)
}

/// The 48 by 24 switch the live client draws.
fn switch(on: bool) -> Div {
    div()
        .w(px(48.))
        .h(px(24.))
        .flex_none()
        .flex()
        .items_center()
        .px(px(3.))
        .rounded(px(16.))
        .border_1()
        .border_color(rgba(0x97979f33))
        .bg(if on { rgb(theme::ACCENT) } else { rgba(0x0000001f) })
        .when(on, |track| track.justify_end())
        .child(div().size(px(16.)).rounded_full().bg(rgb(0xffffff)))
}

/// A label with its switch at the right. With a description the switch sits
/// at the top, without one the two are centered on each other.
fn switch_row(
    id: &'static str,
    label: AnyElement,
    description: Option<AnyElement>,
    on: bool,
    disabled: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut gpui_kit::App) + 'static,
) -> AnyElement {
    let described = description.is_some();
    div()
        .id(id)
        .w_full()
        .flex()
        .gap(px(24.))
        .when(!described, |row| row.items_center())
        .when(disabled, |row| row.opacity(0.5))
        .when(!disabled, |row| row.cursor_pointer().on_click(on_click))
        .child(div().flex_1().min_w_0().flex().flex_col().gap(px(4.)).child(label).children(description))
        .child(switch(on))
        .into_any_element()
}

/// A 20px radio. The chosen one is filled with the brand color and a white dot.
fn radio(on: bool) -> Div {
    div()
        .size(px(20.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .border_2()
        .border_color(rgb(if on { theme::ACCENT } else { theme::TEXT_FAINT }))
        .when(on, |dot| dot.bg(rgb(theme::ACCENT)).child(div().size(px(8.)).rounded_full().bg(rgb(0xffffff))))
}

fn cell_check(on: bool) -> Div {
    div()
        .size(px(20.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(4.))
        .border_1()
        .border_color(if on { rgba(0x5865f2ff) } else { rgba(0x96979ea3) })
        .bg(if on { rgba(0x5865f2ff) } else { rgba(0x00000014) })
        .children(on.then(|| icon("check", 16., 0xffffff)))
}

impl Render for NotificationDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(guild) = self.guild else { return div().into_any_element() };
        let (name, settings, level) = {
            let store = &self.session.read(cx).store;
            (
                store.guild(guild).map(|g| g.name.clone()).unwrap_or_default(),
                store.guild_settings(guild),
                store.guild_notify_level(guild),
            )
        };
        let entries = self.entries(guild, cx);
        let now = Utc::now();
        let muted = mute_active(settings.muted, settings.mute_config.as_ref(), now);

        // Mute, with the duration choice once it is on.
        let mut mute = div().flex().flex_col().gap(px(20.)).child(switch_row(
            "notify-mute",
            heading(format!("Mute {name}")).into_any_element(),
            Some(hint("Muting a server prevents unread indicators and notifications from appearing unless you are mentioned.").into_any_element()),
            muted,
            false,
            cx.listener(move |this, _: &ClickEvent, _, _| this.change_guild(if muted { Action::Unmute } else { Action::Mute(MuteFor::Forever) })),
        ));
        if muted {
            let chosen = MuteFor::from_window(settings.mute_config.as_ref().and_then(|c| c.selected_time_window));
            let mut select = div().flex().flex_col().gap(px(8.)).child(heading("Mute duration")).child(
                div()
                    .id("notify-duration")
                    .h(px(40.))
                    .px(px(12.))
                    .flex()
                    .items_center()
                    .justify_between()
                    .rounded(px(8.))
                    .border_1()
                    .border_color(rgba(HAIRLINE))
                    .bg(rgba(0x0000001f))
                    .text_size(px(16.))
                    .text_color(rgb(theme::TEXT_BRIGHT))
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.durations = !this.durations;
                        cx.notify();
                    }))
                    .child(chosen.label())
                    .child(icon(if self.durations { "chevron-up" } else { "chevron-down" }, 18., theme::TEXT_MUTED)),
            );
            if self.durations {
                for (index, (how, label)) in MuteFor::CHOICES.into_iter().enumerate() {
                    select = select.child(
                        div()
                            .id(("notify-duration-choice", index))
                            .h(px(36.))
                            .px(px(12.))
                            .flex()
                            .items_center()
                            .justify_between()
                            .rounded(px(4.))
                            .text_size(px(16.))
                            .text_color(rgb(theme::TEXT_BRIGHT))
                            .cursor_pointer()
                            .hover(|row| row.bg(rgb(theme::BG_HOVER)))
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.change_guild(Action::Mute(how));
                                this.durations = false;
                                cx.notify();
                            }))
                            .child(label)
                            .children((how == chosen).then(|| icon("check", 16., theme::TEXT_BRIGHT))),
                    );
                }
            }
            mute = mute.child(select);
        }

        // The server's own level.
        let mut levels = div().flex().flex_col().gap(px(16.)).pt(px(8.)).pb(px(4.));
        for (index, (choice, label)) in [
            (NotifyLevel::All, "All Messages"),
            (NotifyLevel::Mentions, "Only @mentions"),
            (NotifyLevel::Nothing, "Nothing"),
        ]
        .into_iter()
        .enumerate()
        {
            levels = levels.child(
                div()
                    .id(("notify-level", index))
                    .h(px(20.))
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .when(muted, |row| row.opacity(0.5))
                    .when(!muted, |row| {
                        row.cursor_pointer().on_click(cx.listener(move |this, _: &ClickEvent, _, _| this.change_guild(Action::SetLevel(Some(choice)))))
                    })
                    .child(radio(!muted && level == choice))
                    .child(div().text_size(px(16.)).line_height(px(20.)).text_color(rgb(theme::TEXT_BRIGHT)).child(label)),
            );
        }

        let bold = |text: &'static str| div().font_weight(FontWeight::SEMIBOLD).child(text);
        let suppress_everyone = div()
            .flex()
            .text_size(px(16.))
            .line_height(px(20.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(rgb(theme::TEXT_BRIGHT))
            .child("Suppress ")
            .child(bold("@everyone"))
            .child(" and ")
            .child(bold("@here"))
            .into_any_element();

        let highlights_off = muted || settings.highlights_off();
        let highlights_description = div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(hint("Highlights provide occasional updates when your friends are chatting in busy servers, and more."))
            .child(
                div()
                    .id("notify-highlights-link")
                    .text_size(px(14.))
                    .line_height(px(18.))
                    .text_color(rgb(LINK))
                    .cursor_pointer()
                    .on_click(|_: &ClickEvent, _, cx| cx.open_url(HIGHLIGHTS_ARTICLE))
                    .child("Learn more about Highlights"),
            )
            .into_any_element();

        let switches = div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .child(switch_row("notify-everyone", suppress_everyone, None, settings.suppress_everyone, false, {
                let on = settings.suppress_everyone;
                cx.listener(move |this, _: &ClickEvent, _, _| this.change_guild(Action::SuppressEveryone(!on)))
            }))
            .child(hairline())
            .child(switch_row("notify-roles", heading("Suppress All Role @mentions").into_any_element(), None, settings.suppress_roles, false, {
                let on = settings.suppress_roles;
                cx.listener(move |this, _: &ClickEvent, _, _| this.change_guild(Action::SuppressRoles(!on)))
            }))
            .child(hairline())
            .child(switch_row(
                "notify-highlights",
                heading("Suppress Highlights").into_any_element(),
                Some(highlights_description),
                highlights_off,
                muted,
                cx.listener(move |this, _: &ClickEvent, _, _| this.change_guild(Action::SuppressHighlights(!highlights_off))),
            ))
            .child(hairline())
            .child(switch_row("notify-events", heading("Mute New Events").into_any_element(), None, settings.mute_scheduled_events, false, {
                let on = settings.mute_scheduled_events;
                cx.listener(move |this, _: &ClickEvent, _, _| this.change_guild(Action::MuteEvents(!on)))
            }))
            .child(hairline())
            .child(switch_row(
                "notify-push",
                heading("Mobile Push Notifications").into_any_element(),
                None,
                !muted && settings.mobile_push,
                muted,
                {
                    let on = settings.mobile_push;
                    cx.listener(move |this, _: &ClickEvent, _, _| this.change_guild(Action::MobilePush(!on)))
                },
            ));

        // The picker for a new override, then the table of the ones in force.
        let shown: Vec<&Entry> = entries
            .iter()
            .filter(|entry| {
                self.added.contains(&entry.id)
                    || settings.channel_overrides.iter().any(|o| o.channel_id == entry.id && o.is_set())
            })
            .collect();
        let mut picker = div().flex().flex_col().gap(px(8.)).child(
            div()
                .id("notify-picker")
                .h(px(40.))
                .px(px(12.))
                .flex()
                .items_center()
                .justify_between()
                .rounded(px(8.))
                .border_1()
                .border_color(rgb(if self.picking { theme::ACCENT } else { 0x25252a }))
                .bg(rgba(0x0000001f))
                .text_size(px(16.))
                .text_color(rgb(theme::TEXT_MUTED))
                .cursor_pointer()
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.picking = !this.picking;
                    cx.notify();
                }))
                .child("Select a channel or category\u{2026}")
                .child(icon(if self.picking { "chevron-up" } else { "chevron-down" }, 18., theme::TEXT_MUTED)),
        );
        if self.picking {
            let mut list = div()
                .id("notify-picker-list")
                .max_h(px(264.))
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .rounded(px(8.))
                .border_1()
                .border_color(rgb(0x25252a))
                .bg(rgb(theme::BG_MENU));
            for (index, entry) in entries.iter().enumerate() {
                let id = entry.id;
                list = list.child(
                    div()
                        .id(("notify-picker-entry", index))
                        .h(px(44.))
                        .flex_none()
                        .px(px(12.))
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .cursor_pointer()
                        .hover(|row| row.bg(rgb(theme::BG_HOVER)))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            if !this.added.contains(&id) {
                                this.added.push(id);
                            }
                            this.picking = false;
                            cx.notify();
                        }))
                        .child(icon(if entry.category { "folder-open" } else { "hash" }, 16., theme::TEXT_MUTED))
                        .child(div().text_size(px(16.)).text_color(rgb(theme::TEXT_BRIGHT)).child(entry.name.clone()))
                        .children(entry.parent.clone().map(|parent| {
                            div().text_size(px(14.)).text_color(rgb(theme::TEXT_MUTED)).child(parent)
                        })),
                );
            }
            picker = picker.child(list);
        }

        let column = |text: &'static str| {
            div().w(px(75.)).flex_none().flex().justify_center().text_size(px(14.)).line_height(px(18.)).font_weight(FontWeight::MEDIUM).child(text)
        };
        let mut table = div().flex().flex_col().child(
            div()
                .h(px(44.))
                .pl(px(8.))
                .flex()
                .items_start()
                .text_color(rgb(theme::TEXT))
                .child(div().flex_1().h(px(36.)).flex().items_center().text_size(px(14.)).line_height(px(18.)).font_weight(FontWeight::MEDIUM).child("Channel or Category"))
                .child(column("All"))
                .child(column("Mentions"))
                .child(column("Nothing"))
                .child(column("Mute")),
        );
        if shown.is_empty() {
            table = table.child(
                div()
                    .h(px(58.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(3.))
                    .border_1()
                    .border_dashed()
                    .border_color(rgb(0x121214))
                    .text_size(px(14.))
                    .line_height(px(18.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(theme::TEXT))
                    .child("Add a channel to override its default notification settings"),
            );
        }
        for (index, entry) in shown.into_iter().enumerate() {
            let id = entry.id;
            let own = settings.channel_overrides.iter().find(|o| o.channel_id == id);
            let own_level = own.and_then(|o| NotifyLevel::from_wire(o.message_notifications));
            let own_muted = own.is_some_and(|o| mute_active(o.muted, o.mute_config.as_ref(), now));
            let mut options = div().flex().flex_none();
            for (choice_index, choice) in [NotifyLevel::All, NotifyLevel::Mentions, NotifyLevel::Nothing].into_iter().enumerate() {
                // A channel with no level of its own shows its server's.
                let on = !muted && own_level.unwrap_or(level) == choice;
                options = options.child(
                    div()
                        .id(("notify-override-level", index * 3 + choice_index))
                        .w(px(75.))
                        .h(px(40.))
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_center()
                        .when(muted, |cell| cell.opacity(0.5))
                        .when(!muted, |cell| {
                            cell.cursor_pointer().on_click(cx.listener(move |this, _: &ClickEvent, _, _| {
                                this.change_channel(id, Action::SetLevel(Some(choice)));
                            }))
                        })
                        .child(radio(on)),
                );
            }
            options = options.child(
                div()
                    .id(("notify-override-mute", index))
                    .w(px(75.))
                    .h(px(40.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, _| {
                        this.change_channel(id, if own_muted { Action::Unmute } else { Action::Mute(MuteFor::Forever) });
                    }))
                    .child(cell_check(own_muted)),
            );
            let group = SharedString::from(format!("notify-override-{index}"));
            table = table.child(
                div()
                    .group(group.clone())
                    .relative()
                    .h(px(58.))
                    .pl(px(8.))
                    .flex()
                    .items_center()
                    .rounded(px(3.))
                    .border_1()
                    .border_dashed()
                    .border_color(rgb(0x121214))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(icon(if entry.category { "folder-open" } else { "hash" }, 16., theme::TEXT_MUTED))
                            .child(
                                div()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .child(div().truncate().text_size(px(14.)).line_height(px(18.)).font_weight(FontWeight::SEMIBOLD).text_color(rgb(theme::TEXT_BRIGHT)).child(entry.name.clone()))
                                    .child(div().truncate().text_size(px(12.)).line_height(px(16.)).text_color(rgb(theme::TEXT_MUTED)).child(entry.sub.clone())),
                            ),
                    )
                    .child(options)
                    .child(
                        div()
                            .id(("notify-override-remove", index))
                            .absolute()
                            .top(px(-8.))
                            .right(px(-8.))
                            .size(px(20.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded_full()
                            .bg(rgb(theme::BG_ACTIVE))
                            .cursor_pointer()
                            .invisible()
                            .group_hover(group, |button| button.visible())
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.added.retain(|added| *added != id);
                                this.change_channel(id, Action::ClearOverride);
                                cx.notify();
                            }))
                            .child(icon("x", 14., theme::TEXT_BRIGHT)),
                    ),
            );
        }

        let channels = div()
            .flex()
            .flex_col()
            .gap(px(30.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(div().flex().flex_col().gap(px(4.)).child(heading("Select a channel or category\u{2026}")).child(hint("Add a channel to override its default notification settings")))
                    .child(picker),
            )
            .child(table);

        let body = div()
            .id("notify-body")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .px(px(24.))
            .py(px(12.))
            .flex()
            .flex_col()
            .gap(px(20.))
            .child(mute)
            .child(hairline())
            .child(
                div()
                    .mt(px(10.))
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(heading("Server Notification Settings"))
                    .child(levels),
            )
            .child(div().mt(px(10.)).child(hairline()))
            .child(switches)
            .child(div().mt(px(10.)).child(channels));

        let header = div()
            .relative()
            .h(px(48.))
            .flex_none()
            .child(
                div()
                    .absolute()
                    .top(px(16.))
                    .left(px(24.))
                    .text_size(px(20.))
                    .line_height(px(24.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(theme::TEXT_BRIGHT))
                    .child("Notification Settings"),
            )
            .child(
                div()
                    .id("notify-close")
                    .absolute()
                    .top_0()
                    .right(px(8.))
                    .size(px(40.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(8.))
                    .cursor_pointer()
                    .hover(|button| button.bg(rgb(theme::BG_HOVER)))
                    .on_click(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(CloseNotificationSettings)))
                    .child(icon("x", 20., theme::TEXT_MUTED)),
            );

        let footer = div().flex_none().px(px(24.)).pt(px(16.)).pb(px(8.)).child(
            div()
                .id("notify-done")
                .h(px(40.))
                .w_full()
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(8.))
                .bg(rgb(theme::ACCENT))
                .text_size(px(16.))
                .line_height(px(20.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(rgb(0xffffff))
                .cursor_pointer()
                .hover(|button| button.bg(rgb(theme::ACCENT_HOVER)))
                .on_click(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(CloseNotificationSettings)))
                .child("Done"),
        );

        let ceiling = (window.viewport_size().height - px(80.)).min(px(CARD_MAX_HEIGHT));
        let card = div()
            .id("notification-settings-card")
            .occlude()
            .w(px(CARD_WIDTH))
            .max_h(ceiling)
            .pt(px(8.))
            .pb(px(16.))
            .flex()
            .flex_col()
            .rounded(px(12.))
            .border_1()
            .border_color(rgba(HAIRLINE))
            .bg(rgb(theme::BG_PANEL))
            .child(header)
            .child(div().h(px(16.)).flex_none())
            .child(div().h(px(1.)).flex_none().bg(rgba(HAIRLINE)))
            .child(body)
            .child(div().h(px(1.)).flex_none().bg(rgba(HAIRLINE)))
            .child(div().h(px(8.)).flex_none())
            .child(footer);
        modal(card.into_any_element(), Box::new(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(CloseNotificationSettings))))
    }
}
