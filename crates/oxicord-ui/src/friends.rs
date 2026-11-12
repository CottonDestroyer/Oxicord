use gpui_kit::{
    AnyElement, AppContext as _, ClickEvent, Context, Entity, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled as _,
    Subscription, Window, div, img, prelude::FluentBuilder as _, px, rgb, rgba,
    component::{Sizable as _, input::{Input, InputEvent, InputState, Textarea, TextareaState}},
};
use oxicord_media::cdn;
use oxicord_model::{Activity, ActivityType, RelationshipKind, ServerTag, Snowflake, Status};
use oxicord_store::Changes;

use crate::{
    backend::{Backend, Command},
    dm_picker::Purpose,
    icons::icon,
    media::{Media, avatar},
    member_list::{presence_dot, server_tag_chip},
    session::{DmOpened, OpenPicker, OpenPrivacySettings, Session},
    theme,
    tip::Tip,
};

/// How many characters a friend request note may hold.
const NOTE_LIMIT: usize = 120;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tab {
    Online,
    All,
    Pending,
    Blocked,
    /// The form for sending a request by username.
    Add,
}

impl Tab {

    fn label(self) -> &'static str {
        match self {
            Tab::Online => "Online",
            Tab::All => "All",
            Tab::Pending => "Pending",
            Tab::Blocked => "Blocked",
            Tab::Add => "Add Friend",
        }
    }
}

/// Which tabs the strip shows. The official client lists a tab only while it
/// has something behind it, and keeps Add Friend always. It has no Blocked
/// tab: that list lives in its settings. Until the settings here have one,
/// Blocked stays on the strip while someone is blocked.
fn visible_tabs(online: usize, friends: usize, pending: usize, blocked: usize) -> Vec<Tab> {
    [
        (Tab::Online, online > 0),
        (Tab::All, friends > 0),
        (Tab::Pending, pending > 0),
        (Tab::Blocked, blocked > 0),
        (Tab::Add, true),
    ]
    .into_iter()
    .filter_map(|(tab, shown)| shown.then_some(tab))
    .collect()
}

/// The tab to draw: the chosen one while it is on the strip, otherwise the
/// first that is.
fn shown_tab(chosen: Tab, visible: &[Tab]) -> Tab {
    if visible.contains(&chosen) { chosen } else { visible.first().copied().unwrap_or(Tab::Add) }
}

/// Whether a person matches what was typed in the search box.
fn matches_query(person: &Person, query: &str) -> bool {
    let query = query.trim().to_lowercase();
    query.is_empty() || person.name.to_lowercase().contains(&query) || person.username.to_lowercase().contains(&query)
}

struct Person {
    id: Snowflake,
    tag: Option<ServerTag>,
    name: String,
    username: String,
    avatar: String,
    status: Status,
    kind: RelationshipKind,
    custom_status: Option<String>,
}

struct Section {
    title: &'static str,
    people: Vec<Person>,
}

/// What a friend is up to, as drawn on an Active Now card.
enum Doing {
    Activity(Activity),
    /// In a voice channel, with the channel and server names.
    Voice { channel: String, guild: String },
}

/// The Active Now column needs room the friend list can spare.
const ACTIVE_NOW_MIN_WINDOW: f32 = 1100.;
const ACTIVE_NOW_WIDTH: f32 = 420.;

/// What a conversation header says under a person's name: their custom
/// status, or failing that what they are doing, as bots show their help line.
pub(crate) fn status_line(activities: &[Activity]) -> Option<String> {
    custom_status(activities).or_else(|| {
        activities.iter().find(|a| a.verb().is_some()).map(|a| a.name.clone()).filter(|n| !n.is_empty())
    })
}

/// The words a custom status shows, its emoji first when it has a plain one.
pub(crate) fn custom_status(activities: &[Activity]) -> Option<String> {
    let custom = activities.iter().find(|a| a.is_custom_status())?;
    let glyph = custom.emoji.as_ref().filter(|e| e.id.is_none()).and_then(|e| e.name.clone());
    let text = [glyph, custom.state.clone()].into_iter().flatten().collect::<Vec<_>>().join(" ");
    (!text.is_empty()).then_some(text)
}

/// The home screen: everyone you have a tie with, split into tabs.
pub struct FriendsView {
    session: Entity<Session>,
    media: Entity<Media>,
    backend: Backend,
    tab: Tab,
    /// The person whose remove button was pressed once already.
    confirm_remove: Option<Snowflake>,
    add_name: Entity<InputState>,
    /// The optional note sent with a friend request.
    add_note: Entity<TextareaState>,
    /// The Search box above the list.
    search: Entity<InputState>,
    _subscriptions: Vec<Subscription>,
}

impl FriendsView {
    pub fn new(
        session: Entity<Session>,
        media: Entity<Media>,
        backend: Backend,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let add_name = cx.new(|cx| InputState::new(window, cx).placeholder("Enter a username"));
        let add_note = cx.new(|cx| TextareaState::new(window, cx).auto_grow(1, 2).placeholder("Personalize your request (Optional)"));
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search"));
        let subscriptions = vec![
            cx.observe(&add_name, |_, _, cx| cx.notify()),
            cx.observe(&add_note, |_, _, cx| cx.notify()),
            cx.subscribe(&search, |_, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            }),
            cx.observe(&session, |_, _, cx| cx.notify()),
            cx.subscribe(&session, |_, _, changes: &Changes, cx| {
                if changes.full || changes.relationships || changes.presences {
                    cx.notify();
                }
            }),
            cx.observe(&media, |_, _, cx| cx.notify()),
        ];
        Self {
            session,
            media,
            backend,
            tab: Tab::Online,
            confirm_remove: None,
            add_name,
            add_note,
            search,
            _subscriptions: subscriptions,
        }
    }

    fn person(&self, user: &oxicord_model::User, kind: RelationshipKind, cx: &gpui_kit::App) -> Person {
        let store = &self.session.read(cx).store;
        Person {
            id: user.id,
            tag: store.user(user.id).unwrap_or(user).server_tag(),
            name: store
                .relationship_nick(user.id)
                .unwrap_or_else(|| user.display_name().to_owned()),
            username: user.username.clone(),
            avatar: cdn::user_avatar(user.id, user.avatar.as_deref(), 64),
            status: store.presence(user.id),
            kind,
            custom_status: custom_status(store.activities(user.id)),
        }
    }

    /// Friends doing something worth showing: a game, music, a stream, or
    /// sitting in voice. A custom status alone does not count.
    fn active_now(&self, cx: &gpui_kit::App) -> Vec<(Person, Doing)> {
        let store = &self.session.read(cx).store;
        store
            .relationships(RelationshipKind::Friend)
            .into_iter()
            .filter(|user| store.presence(user.id) != Status::Offline)
            .filter_map(|user| {
                let activity = store.activities(user.id).iter().find(|a| a.verb().is_some()).cloned();
                let doing = match activity {
                    Some(activity) => Doing::Activity(activity),
                    None => {
                        let voice = store.voice_of(user.id)?;
                        let channel = store.channel(voice.channel_id?)?;
                        let guild = channel.guild_id.and_then(|g| store.guild(g)).map(|g| g.name.clone());
                        Doing::Voice {
                            channel: channel.name.clone().unwrap_or_default(),
                            guild: guild.unwrap_or_default(),
                        }
                    }
                };
                Some((self.person(user, RelationshipKind::Friend, cx), doing))
            })
            .collect()
    }

    fn render_active_now(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let active = self.active_now(cx);
        let mut list = div().id("active-now").flex_1().min_h_0().overflow_y_scroll().flex().flex_col().gap_2();
        if active.is_empty() {
            list = list.child(
                div()
                    .p(px(16.))
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(4.))
                    .text_center()
                    .child(
                        div()
                            .text_size(px(16.))
                            .line_height(px(20.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(theme::TEXT_BRIGHT))
                            .child("It's quiet for now..."),
                    )
                    .child(
                        div()
                            .text_size(px(14.))
                            .line_height(px(18.))
                            .text_color(rgb(theme::TEXT_MUTED))
                            .child(
                                "When a friend starts an activity\u{2014}like playing a game or hanging out on voice\u{2014}we'll show it here!",
                            ),
                    ),
            );
        }
        for (person, doing) in active {
            list = list.child(self.render_active_card(person, doing, cx));
        }

        div()
            .w(px(ACTIVE_NOW_WIDTH))
            .h_full()
            .flex_none()
            .flex()
            .flex_col()
            .gap(px(9.))
            .pt(px(14.))
            .pl(px(17.))
            .pr(px(16.))
            .child(
                div()
                    .text_size(px(20.))
                    .line_height(px(24.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(theme::TEXT_BRIGHT))
                    .child("Active Now"),
            )
            .child(list)
            .into_any_element()
    }

    /// The notice above the list pointing at the blocked and ignored accounts.
    fn render_banner(&mut self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .h(px(59.))
            .flex_none()
            .ml(px(30.))
            .mr(px(20.))
            .mb(px(20.))
            .px(px(16.))
            .flex()
            .items_center()
            .gap(px(8.))
            .rounded(px(4.))
            .child(icon("circle-info", 24., theme::TEXT_MUTED))
            .child(
                div()
                    .flex_1()
                    .flex()
                    .gap(px(4.))
                    .text_size(px(14.))
                    .line_height(px(18.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(theme::TEXT_BRIGHT))
                    .child("Looking for accounts you've blocked or ignored?")
                    .child(
                        div()
                            .id("restricted-settings")
                            .cursor_pointer()
                            .text_color(rgb(0x2781e7))
                            .hover(|s| s.underline())
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.session.update(cx, |_, cx| cx.emit(OpenPrivacySettings));
                            }))
                            .child("Go to settings"),
                    ),
            )
            .child(
                div()
                    .id("restricted-close")
                    .size(px(24.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| {
                        let backend = this.backend.clone();
                        this.session.update(cx, |session, cx| {
                            session.change_settings(&backend, |s| s.restricted_notice_dismissed = true, cx);
                        });
                    }))
                    .child(icon("x", 14., theme::TEXT_MUTED)),
            )
            .into_any_element()
    }

    /// The box that narrows the list to the people matching what is typed.
    fn render_search(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let typed = !self.search.read(cx).value().is_empty();
        div()
            .h(px(40.))
            .flex_none()
            .ml(px(24.))
            .mr(px(16.))
            .pl(px(4.))
            .flex()
            .items_center()
            .gap(px(4.))
            .rounded(px(8.))
            .border_1()
            .border_color(rgba(0x97979f3d))
            .child(div().size(px(30.)).flex_none().flex().items_center().justify_center().child(icon("search", 16., theme::TEXT_MUTED)))
            .child(div().flex_1().text_size(px(16.)).child(Input::new(&self.search).appearance(false).bordered(false)))
            .children(typed.then(|| {
                div()
                    .id("friends-search-clear")
                    .size(px(30.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.search.update(cx, |input, cx| input.set_value("", window, cx));
                    }))
                    .child(icon("x", 14., theme::TEXT_MUTED))
            }))
            .into_any_element()
    }

    fn render_active_card(&mut self, person: Person, doing: Doing, cx: &mut Context<Self>) -> AnyElement {
        let id = person.id;
        let (summary, glyph, title, lines): (String, &str, String, Vec<String>) = match doing {
            Doing::Activity(a) => {
                let glyph = match a.kind {
                    ActivityType::Listening => "music",
                    ActivityType::Watching => "tv",
                    ActivityType::Streaming => "radio",
                    ActivityType::Competing => "trophy",
                    _ => "gamepad-2",
                };
                let summary = format!("{} {}", a.verb().unwrap_or_default(), a.name);
                (summary, glyph, a.name, [a.details, a.state].into_iter().flatten().collect())
            }
            Doing::Voice { channel, guild } => {
                ("In voice".to_owned(), "volume-2", channel, vec![guild].into_iter().filter(|g| !g.is_empty()).collect())
            }
        };

        div()
            .id(("active-now", id.0))
            .p_3()
            .flex()
            .flex_col()
            .gap_2()
            .rounded(px(8.))
            .border_1()
            .border_color(rgb(theme::BORDER))
            .bg(rgb(theme::BG_SIDEBAR))
            .cursor_pointer()
            .hover(|s| s.bg(rgb(theme::BG_HOVER)))
            .on_click(cx.listener(move |this, _, _, cx| this.message(id, cx)))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .relative()
                            .flex_none()
                            .child(avatar(&self.media, &person.avatar, 32., &person.name, cx))
                            .child(presence_dot(person.status)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .truncate()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(theme::TEXT_BRIGHT))
                                    .child(person.name),
                            )
                            .child(
                                div().truncate().text_size(px(12.)).text_color(rgb(theme::TEXT_MUTED)).child(summary),
                            ),
                    ),
            )
            .child(
                div()
                    .p_2()
                    .flex()
                    .items_center()
                    .gap_3()
                    .rounded(px(theme::RADIUS))
                    .bg(rgb(theme::BG_PANEL))
                    .child(
                        div()
                            .size(px(40.))
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(8.))
                            .bg(rgb(theme::BG_ACTIVE))
                            .child(icon(glyph, 22., theme::TEXT_MUTED)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .truncate()
                                    .text_size(px(13.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(theme::TEXT))
                                    .child(title),
                            )
                            .children(lines.into_iter().map(|line| {
                                div().truncate().text_size(px(12.)).text_color(rgb(theme::TEXT_MUTED)).child(line)
                            })),
                    ),
            )
            .into_any_element()
    }

    fn people(&self, kind: RelationshipKind, cx: &gpui_kit::App) -> Vec<Person> {
        let session = self.session.read(cx);
        session
            .store
            .relationships(kind)
            .into_iter()
            .map(|user| self.person(user, kind, cx))
            .collect()
    }

    fn sections(&self, tab: Tab, query: &str, cx: &gpui_kit::App) -> Vec<Section> {
        let section = |title, people: Vec<Person>| {
            let people: Vec<Person> = people.into_iter().filter(|p| matches_query(p, query)).collect();
            (!people.is_empty()).then_some(Section { title, people })
        };
        let friends = || self.people(RelationshipKind::Friend, cx);

        match tab {
            Tab::Online => section(
                "Online",
                friends().into_iter().filter(|p| p.status != Status::Offline).collect(),
            )
            .into_iter()
            .collect(),
            Tab::All => section("All friends", friends()).into_iter().collect(),
            Tab::Pending => [
                section("Received", self.people(RelationshipKind::IncomingRequest, cx)),
                section("Sent", self.people(RelationshipKind::OutgoingRequest, cx)),
            ]
            .into_iter()
            .flatten()
            .collect(),
            Tab::Blocked => section("Blocked", self.people(RelationshipKind::Blocked, cx))
                .into_iter()
                .collect(),
            Tab::Add => Vec::new(),
        }
    }

    /// The tabs on the strip right now.
    fn visible_tabs(&self, cx: &gpui_kit::App) -> Vec<Tab> {
        let store = &self.session.read(cx).store;
        let friends = store.relationships(RelationshipKind::Friend);
        let online = friends.iter().filter(|user| store.presence(user.id) != Status::Offline).count();
        let pending = store.relationships(RelationshipKind::IncomingRequest).len()
            + store.relationships(RelationshipKind::OutgoingRequest).len();
        visible_tabs(online, friends.len(), pending, store.relationships(RelationshipKind::Blocked).len())
    }

    fn pending_count(&self, cx: &gpui_kit::App) -> usize {
        self.session
            .read(cx)
            .store
            .relationships(RelationshipKind::IncomingRequest)
            .len()
    }

    fn message(&mut self, user: Snowflake, cx: &mut Context<Self>) {
        let existing = self.session.read(cx).store.dm_with(user);
        match existing {
            // Already open, so there is nothing to ask the server.
            Some(channel) => self
                .session
                .update(cx, |_, cx| cx.emit(DmOpened(channel))),
            None => self.backend.send(Command::OpenDm(user)),
        }
    }

    fn remove(&mut self, user: Snowflake, cx: &mut Context<Self>) {
        self.confirm_remove = None;
        self.backend.send(Command::RemoveRelationship(user));
        cx.notify();
    }

    /// The Add Friend page: a username box and what happened last time.
    fn render_add_form(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let outcome = self.session.read(cx).friend_request.clone();

        // The note holds 120 characters at most.
        let note = self.add_note.read(cx).value().to_string();
        if note.chars().count() > NOTE_LIMIT {
            let kept: String = note.chars().take(NOTE_LIMIT).collect();
            self.add_note.update(cx, |input, cx| input.set_value(kept, window, cx));
        }
        let left = NOTE_LIMIT.saturating_sub(note.chars().count());
        let ready = !self.add_name.read(cx).value().trim().is_empty();

        let send = cx.listener(|this, _: &ClickEvent, window, cx| {
            let name = this.add_name.read(cx).value().trim().to_owned();
            if name.is_empty() {
                return;
            }
            let note = this.add_note.read(cx).value().trim().to_owned();
            this.backend.send(Command::AddFriendByName(name, (!note.is_empty()).then_some(note)));
            this.add_name.update(cx, |input, cx| input.set_value("", window, cx));
            this.add_note.update(cx, |input, cx| input.set_value("", window, cx));
        });

        let rule = || div().h(px(1.)).flex_none().bg(rgba(0x97979f3d));
        let heading = |text: &'static str| {
            div()
                .text_size(px(20.))
                .line_height(px(24.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(theme::TEXT_BRIGHT))
                .child(text)
        };
        let body = |text: &'static str| {
            div()
                .text_size(px(16.))
                .line_height(px(20.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(rgb(theme::TEXT))
                .child(text)
        };

        let send_button = div()
            .id("send-request")
            .h(px(32.))
            .px(px(12.))
            .flex_none()
            .flex()
            .items_center()
            .rounded(px(8.))
            .bg(rgb(theme::ACCENT))
            .text_size(px(14.))
            .line_height(px(18.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(rgb(0xffffff))
            .child("Send Friend Request")
            .map(|button| {
                if ready {
                    button.cursor_pointer().hover(|s| s.opacity(0.85)).on_click(send)
                } else {
                    button.opacity(0.5)
                }
            });

        let form = div()
            .mt(px(16.))
            .rounded(px(16.))
            .border_1()
            .border_color(rgba(0x97979f3d))
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(56.))
                    .flex_none()
                    .px(px(12.))
                    .flex()
                    .items_center()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(16.))
                            .child(Input::new(&self.add_name).large().appearance(false).bordered(false)),
                    )
                    .child(send_button),
            )
            .child(rule())
            .child(
                div()
                    .relative()
                    .h(px(90.))
                    .flex_none()
                    .px(px(12.))
                    .pt(px(12.))
                    .child(Textarea::new(&self.add_note).large().appearance(false).bordered(false))
                    .child(
                        div()
                            .absolute()
                            .right(px(12.))
                            .bottom(px(12.))
                            .text_size(px(14.))
                            .line_height(px(18.))
                            .text_color(rgb(theme::TEXT_MUTED))
                            .child(left.to_string()),
                    ),
            );

        let top = div()
            .relative()
            .flex_none()
            .px(px(30.))
            .py(px(20.))
            .child(heading("Add Friend"))
            .child(div().mt(px(8.)).child(body("You can add friends with their Discord username.")))
            .child(form)
            .child(
                div()
                    .mt(px(8.))
                    .text_size(px(14.))
                    .line_height(px(18.))
                    .text_color(rgb(theme::TEXT_MUTED))
                    .child("What you write here will also show up in your DM if you become friends."),
            )
            .children(outcome.map(|(name, result)| {
                let (text, color) = match result {
                    Ok(()) => (format!("Your friend request to {name} was sent."), theme::ONLINE),
                    Err(error) => (error, theme::DANGER),
                };
                div().mt(px(8.)).text_size(px(14.)).text_color(rgb(color)).child(text)
            }))
            .child(
                img(gpui_kit::SharedString::from("friends/add-friend-wumpus.svg"))
                    .absolute()
                    .top(px(16.))
                    .right(px(46.))
                    .w(px(84.))
                    .h(px(72.)),
            );

        let explore = div()
            .id("explore-servers")
            .ml(px(20.))
            .w(px(526.))
            .h(px(56.))
            .px(px(15.))
            .flex()
            .items_center()
            .gap(px(8.))
            .rounded(px(8.))
            .border_1()
            .border_color(rgba(0x97979f33))
            .cursor_pointer()
            .hover(|s| s.bg(rgb(theme::BG_HOVER)))
            .on_click(|_, _, cx| cx.open_url("https://discord.com/discovery/servers"))
            .child(
                div()
                    .size(px(36.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(8.))
                    .bg(rgb(0x3d9e60))
                    .child(icon("discovery", 24., 0x000000)),
            )
            .child(div().flex_1().child(body("Explore Discoverable Servers")))
            .child(icon("chevron-right", 20., theme::TEXT_MUTED));

        div()
            .id("friends-add")
            .size_full()
            .overflow_y_scroll()
            .child(top)
            .child(rule())
            .child(
                div()
                    .px(px(30.))
                    .py(px(20.))
                    .flex()
                    .flex_col()
                    .child(heading("Other Places to Make Friends"))
                    .child(div().mt(px(8.)).child(body(
                        "Don't have a username on hand? Check out our list of public servers that includes everything from gaming to cooking, music, anime and more.",
                    )))
                    .child(div().mt(px(20.)).ml(px(-30.)).child(explore)),
            )
            .into_any_element()
    }

    fn render_tabs(&mut self, visible: &[Tab], cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let pending = self.pending_count(cx);
        let mut tabs = div().flex().items_center().gap(px(16.)).px(px(8.));

        for &tab in visible {
            let selected = self.tab == tab;
            let add = tab == Tab::Add;
            let badge = (tab == Tab::Pending && pending > 0).then(|| {
                div()
                    .min_w(px(16.))
                    .h(px(16.))
                    .px(px(4.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_full()
                    .bg(rgb(theme::BADGE))
                    .text_size(px(12.))
                    .line_height(px(16.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(0xffffff))
                    .child(pending.to_string())
            });
            // Add Friend is the one filled button; the rest are plain until
            // they are the open tab.
            let (fill, text) = match (add, selected) {
                (true, true) => (rgba(0x5865f229), 0x6374f4),
                (true, false) => (rgba(0x5865f2ff), 0xffffff),
                (false, true) => (rgba(0x97979f3d), theme::TEXT_BRIGHT),
                (false, false) => (rgba(0x00000000), theme::TEXT_MUTED),
            };
            tabs = tabs.child(
                div()
                    .id(tab.label())
                    .h(px(32.))
                    .px(px(12.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .rounded(px(8.))
                    .cursor_pointer()
                    .text_size(px(16.))
                    .line_height(px(20.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(text))
                    .bg(fill)
                    .when(!selected && !add, |s| s.hover(|s| s.bg(rgba(0x97979f1f)).text_color(rgb(theme::TEXT_BRIGHT))))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.tab = tab;
                        this.confirm_remove = None;
                        this.search.update(cx, |input, cx| input.set_value("", window, cx));
                        cx.notify();
                    }))
                    .child(tab.label())
                    .children(badge),
            );
        }

        div()
            .h(px(theme::CHANNEL_HEADER_HEIGHT))
            .flex_none()
            .flex()
            .items_center()
            .px(px(16.))
            .border_b_1()
            .border_color(rgb(theme::BORDER))
            .child(
                div()
                    .flex()
                    .items_center()
                    .text_size(px(16.))
                    .line_height(px(20.))
                    .text_color(rgb(theme::TEXT_BRIGHT))
                    .font_weight(FontWeight::MEDIUM)
                    .child(div().size(px(32.)).flex().items_center().justify_center().child(icon("friends", 24., theme::TEXT_BRIGHT)))
                    .child("Friends"),
            )
            .child(div().ml(px(12.)).mr(px(4.)).size(px(4.)).flex_none().rounded_full().bg(rgba(0x97979f3d)))
            .child(tabs)
            .child(div().flex_1())
            .child(
                div()
                    .id("new-group-dm")
                    .group("new-group-dm")
                    .tooltip(|_, cx| cx.new(|_| Tip("New Group DM".into())).into())
                    .size(px(32.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .on_click(cx.listener(|this, event: &ClickEvent, _, cx| {
                        // Live opens a centered New Message modal; until that is
                        // built the picker drops below the button, inside the window.
                        let at = gpui_kit::point(event.position().x - px(384.), px(82.));
                        this.session.update(cx, |_, cx| cx.emit(OpenPicker { purpose: Purpose::Create, at }));
                    }))
                    .child(
                        icon("new-group-dm", 20., theme::TEXT_MUTED)
                            .group_hover("new-group-dm", |s| s.text_color(rgb(theme::TEXT))),
                    ),
            )
    }

    fn render_person(&mut self, person: Person, cx: &mut Context<Self>) -> AnyElement {
        let id = person.id;
        let subtitle: SharedString = match person.kind {
            RelationshipKind::IncomingRequest | RelationshipKind::OutgoingRequest | RelationshipKind::Blocked => {
                person.username.clone().into()
            }
            _ if person.custom_status.is_some() && person.status != Status::Offline => {
                person.custom_status.clone().unwrap_or_default().into()
            }
            _ => match person.status {
                Status::Online => "Online",
                Status::Idle => "Idle",
                Status::Dnd => "Do not disturb",
                Status::Offline => "Offline",
            }
            .into(),
        };
        let arming = self.confirm_remove == Some(id);

        let mut actions = div().flex().items_center().gap(px(10.));
        match person.kind {
            RelationshipKind::Friend => {
                actions = actions
                    .child(
                        round_button(("message", id.0), "message-circle", theme::TEXT_MUTED)
                            .on_click(cx.listener(move |this, _, _, cx| this.message(id, cx))),
                    )
                    .child(
                        round_button(
                            ("remove", id.0),
                            "user-x",
                            if arming { theme::DANGER } else { theme::TEXT_MUTED },
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if this.confirm_remove == Some(id) {
                                this.remove(id, cx);
                            } else {
                                this.confirm_remove = Some(id);
                                cx.notify();
                            }
                        })),
                    );
            }
            RelationshipKind::IncomingRequest => {
                actions = actions
                    .child(
                        round_button(("accept", id.0), "check", theme::TEXT_MUTED).on_click(
                            cx.listener(move |this, _, _, _| {
                                this.backend.send(Command::AcceptFriend(id));
                            }),
                        ),
                    )
                    .child(
                        round_button(("decline", id.0), "x", theme::TEXT_MUTED)
                            .on_click(cx.listener(move |this, _, _, cx| this.remove(id, cx))),
                    );
            }
            RelationshipKind::OutgoingRequest | RelationshipKind::Blocked => {
                actions = actions.child(
                    round_button(("cancel", id.0), "x", theme::TEXT_MUTED)
                        .on_click(cx.listener(move |this, _, _, cx| this.remove(id, cx))),
                );
            }
            _ => {}
        }

        let on_open = person.kind == RelationshipKind::Friend;
        let mut row = div()
            .id(("friend", id.0))
            .h(px(61.))
            .ml(px(14.))
            .mr(px(26.))
            .px(px(10.))
            .flex()
            .items_center()
            .gap(px(12.))
            .rounded(px(8.))
            .hover(|s| s.bg(rgb(theme::BG_HOVER)));
        if on_open {
            row = row
                .cursor_pointer()
                .on_click(cx.listener(move |this, _, _, cx| this.message(id, cx)));
        }

        let tag = person.tag.as_ref().map(|t| server_tag_chip(t, 14., &self.media, cx));
        let row = row
            .child(
                div()
                    .relative()
                    .flex_none()
                    .child(avatar(&self.media, &person.avatar, 32., &person.name, cx))
                    .children(
                        (person.kind == RelationshipKind::Friend).then(|| presence_dot(person.status)),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(4.))
                            .child(
                                div()
                                    .truncate()
                                    .text_size(px(16.))
                                    .line_height(px(20.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(theme::TEXT_BRIGHT))
                                    .child(person.name),
                            )
                            .children(tag),
                    )
                    .child(
                        div()
                            .truncate()
                            .text_size(px(14.))
                            .line_height(px(18.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(theme::TEXT_MUTED))
                            .child(subtitle),
                    ),
            )
            .child(actions);

        // A hairline sits above every row, a little inside the row edges.
        div()
            .flex()
            .flex_col()
            .child(div().ml(px(30.)).mr(px(36.)).h(px(1.)).bg(rgba(0x97979f33)))
            .child(row)
            .into_any_element()
    }
}

impl Render for FriendsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let visible = self.visible_tabs(cx);
        self.tab = shown_tab(self.tab, &visible);
        let header = self.render_tabs(&visible, cx);
        let active_now = (window.viewport_size().width >= px(ACTIVE_NOW_MIN_WINDOW))
            .then(|| self.render_active_now(cx));
        let page = |main: AnyElement| {
            div()
                .flex_1()
                .min_w_0()
                .h_full()
                .flex()
                .flex_col()
                .bg(rgb(theme::BG_CHAT))
                .child(header)
                .child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .flex()
                        .child(div().flex_1().min_w_0().h_full().flex().flex_col().child(main))
                        .children(active_now),
                )
                .into_any_element()
        };
        if self.tab == Tab::Add {
            return page(self.render_add_form(window, cx));
        }

        let query = self.search.read(cx).value().to_string();
        let sections = self.sections(self.tab, &query, cx);
        let mut body = div().id("friends-body").flex_1().min_h_0().overflow_y_scroll().pb_4();
        if sections.is_empty() {
            body = body.child(
                div()
                    .size_full()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap_3()
                    .text_color(rgb(theme::TEXT_MUTED))
                    .child(icon("users", 48., theme::TEXT_FAINT))
                    .child(match (self.tab, query.trim().is_empty()) {
                        (_, false) => "No one matches that search.",
                        (Tab::Online, _) => "No one is around right now.",
                        (Tab::All, _) => "You have no friends here yet.",
                        (Tab::Pending, _) => "There are no pending requests.",
                        (Tab::Blocked, _) => "You haven't blocked anyone.",
                        (Tab::Add, _) => "",
                    }),
            );
        }
        for section in sections {
            body = body.child(
                div()
                    .ml(px(24.))
                    .pt(px(16.))
                    .pb(px(16.))
                    .text_size(px(14.))
                    .line_height(px(18.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(theme::TEXT_BRIGHT))
                    .child(format!("{} \u{2014} {}", section.title, section.people.len())),
            );
            for person in section.people {
                body = body.child(self.render_person(person, cx));
            }
        }

        let banner = (!self.session.read(cx).settings.restricted_notice_dismissed).then(|| self.render_banner(cx));
        page(
            div()
                .size_full()
                .flex()
                .flex_col()
                .pt(px(16.))
                .children(banner)
                .child(self.render_search(cx))
                .child(div().h(px(8.)).flex_none())
                .child(body)
                .into_any_element(),
        )
    }
}

/// A small circular button with an icon, the way Discord draws row actions.
fn round_button(
    id: impl Into<gpui_kit::ElementId>,
    glyph: &str,
    color: u32,
) -> gpui_kit::Stateful<gpui_kit::Div> {
    div()
        .id(id)
        .size(px(36.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .bg(rgb(theme::BG_SIDEBAR))
        .cursor_pointer()
        .hover(|s| s.bg(rgb(theme::BG_ACTIVE)))
        .child(icon(glyph, 20., color))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn person(name: &str, username: &str) -> Person {
        Person {
            id: Snowflake(1),
            tag: None,
            name: name.into(),
            username: username.into(),
            avatar: String::new(),
            status: Status::Offline,
            kind: RelationshipKind::Friend,
            custom_status: None,
        }
    }

    #[test]
    fn the_strip_lists_only_tabs_with_something_behind_them() {
        assert_eq!(visible_tabs(0, 0, 0, 0), [Tab::Add]);
        assert_eq!(visible_tabs(0, 0, 1, 0), [Tab::Pending, Tab::Add]);
        assert_eq!(visible_tabs(0, 3, 0, 0), [Tab::All, Tab::Add]);
        assert_eq!(visible_tabs(2, 3, 1, 1), [Tab::Online, Tab::All, Tab::Pending, Tab::Blocked, Tab::Add]);
    }

    #[test]
    fn a_tab_that_left_the_strip_falls_back_to_the_first_one_left() {
        let visible = visible_tabs(0, 0, 1, 0);
        assert_eq!(shown_tab(Tab::Online, &visible), Tab::Pending);
        assert_eq!(shown_tab(Tab::Add, &visible), Tab::Add);
        assert_eq!(shown_tab(Tab::Online, &visible_tabs(0, 0, 0, 0)), Tab::Add);
    }

    #[test]
    fn the_search_matches_the_name_or_the_username_in_any_case() {
        let ann = person("Ann Lee", "annl0");
        assert!(matches_query(&ann, ""));
        assert!(matches_query(&ann, "  "));
        assert!(matches_query(&ann, "ann"));
        assert!(matches_query(&ann, "LEE"));
        assert!(matches_query(&ann, "annl0"));
        assert!(!matches_query(&ann, "bob"));
    }
}
