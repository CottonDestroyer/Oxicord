use gpui_kit::{
    AnyElement, AppContext as _, ClickEvent, Context, Entity, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled as _,
    Subscription, Window, div, px, rgb,
    component::input::{Input, InputState},
};
use oxicord_media::cdn;
use oxicord_model::{Activity, ActivityType, RelationshipKind, Snowflake, Status};
use oxicord_store::Changes;

use crate::{
    backend::{Backend, Command},
    icons::icon,
    media::{Media, avatar},
    member_list::presence_dot,
    session::{DmOpened, Session},
    theme,
};

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
    const ALL: [Tab; 5] = [Tab::Online, Tab::All, Tab::Pending, Tab::Blocked, Tab::Add];

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

struct Person {
    id: Snowflake,
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
const ACTIVE_NOW_WIDTH: f32 = 360.;

/// The words a custom status shows, its emoji first when it has a plain one.
fn custom_status(activities: &[Activity]) -> Option<String> {
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
        let subscriptions = vec![
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
            _subscriptions: subscriptions,
        }
    }

    fn person(&self, user: &oxicord_model::User, kind: RelationshipKind, cx: &gpui_kit::App) -> Person {
        let store = &self.session.read(cx).store;
        Person {
            id: user.id,
            name: user.display_name().to_owned(),
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
                    .pt_4()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_1()
                    .text_center()
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(theme::TEXT_BRIGHT))
                            .child("It's quiet for now..."),
                    )
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(rgb(theme::TEXT_MUTED))
                            .child("When a friend starts a game, plays music or joins voice, it shows up here."),
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
            .gap_4()
            .p_4()
            .border_l_1()
            .border_color(rgb(theme::BORDER))
            .child(
                div()
                    .text_size(px(20.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(theme::TEXT_BRIGHT))
                    .child("Active Now"),
            )
            .child(list)
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
                    .rounded(px(6.))
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

    fn sections(&self, cx: &gpui_kit::App) -> Vec<Section> {
        let section = |title, people: Vec<Person>| {
            (!people.is_empty()).then_some(Section { title, people })
        };
        let friends = || self.people(RelationshipKind::Friend, cx);

        match self.tab {
            Tab::Online => section(
                "Online",
                friends().into_iter().filter(|p| p.status != Status::Offline).collect(),
            )
            .into_iter()
            .collect(),
            Tab::All => section("All friends", friends()).into_iter().collect(),
            Tab::Pending => [
                section("Incoming", self.people(RelationshipKind::IncomingRequest, cx)),
                section("Outgoing", self.people(RelationshipKind::OutgoingRequest, cx)),
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
    fn render_add_form(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let outcome = self.session.read(cx).friend_request.clone();
        let send = cx.listener(|this, _: &ClickEvent, window, cx| {
            let name = this.add_name.read(cx).value().trim().to_owned();
            if name.is_empty() {
                return;
            }
            this.backend.send(Command::AddFriendByName(name));
            this.add_name.update(cx, |input, cx| input.set_value("", window, cx));
        });

        div()
            .p_6()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .text_size(px(18.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(theme::TEXT_BRIGHT))
                    .child("Add Friend"),
            )
            .child(
                div()
                    .text_color(rgb(theme::TEXT_MUTED))
                    .child("You can add friends with their Discord username."),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .max_w(px(560.))
                    .child(div().flex_1().child(Input::new(&self.add_name)))
                    .child(
                        div()
                            .id("send-request")
                            .h(px(36.))
                            .px_4()
                            .flex()
                            .items_center()
                            .rounded(px(6.))
                            .cursor_pointer()
                            .bg(rgb(theme::ACCENT))
                            .text_color(rgb(theme::TEXT_BRIGHT))
                            .hover(|s| s.opacity(0.85))
                            .on_click(send)
                            .child("Send Friend Request"),
                    ),
            )
            .children(outcome.map(|(name, result)| {
                let (text, color) = match result {
                    Ok(()) => (format!("Your friend request to {name} was sent."), theme::ONLINE),
                    Err(error) => (error, theme::DANGER),
                };
                div().text_color(rgb(color)).child(text)
            }))
            .into_any_element()
    }

    fn render_tabs(&mut self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let pending = self.pending_count(cx);
        let mut tabs = div().flex().items_center().gap_1();

        for tab in Tab::ALL {
            let selected = self.tab == tab;
            let badge = (tab == Tab::Pending && pending > 0).then(|| {
                div()
                    .min_w(px(16.))
                    .h(px(16.))
                    .px(px(5.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_full()
                    .bg(rgb(theme::BADGE))
                    .text_size(px(11.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(theme::TEXT_BRIGHT))
                    .child(pending.to_string())
            });
            tabs = tabs.child(
                div()
                    .id(tab.label())
                    .h(px(28.))
                    .px_3()
                    .flex()
                    .items_center()
                    .gap_2()
                    .rounded(px(6.))
                    .cursor_pointer()
                    .text_color(rgb(if selected { theme::TEXT_BRIGHT } else { theme::TEXT_MUTED }))
                    .bg(rgb(if selected { theme::BG_ACTIVE } else { theme::BG_CHAT }))
                    .hover(|s| s.bg(rgb(theme::BG_HOVER)))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.tab = tab;
                        this.confirm_remove = None;
                        cx.notify();
                    }))
                    .child(tab.label())
                    .children(badge),
            );
        }

        div()
            .h(px(48.))
            .flex_none()
            .flex()
            .items_center()
            .gap_4()
            .px_4()
            .border_b_1()
            .border_color(rgb(theme::BORDER))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_color(rgb(theme::TEXT_BRIGHT))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(icon("users", 20., theme::TEXT_MUTED))
                    .child("Friends"),
            )
            .child(div().w(px(1.)).h(px(20.)).bg(rgb(theme::BORDER)))
            .child(tabs)
    }

    fn render_person(&mut self, person: Person, cx: &mut Context<Self>) -> AnyElement {
        let id = person.id;
        let subtitle: SharedString = match person.kind {
            RelationshipKind::IncomingRequest => "Incoming friend request".into(),
            RelationshipKind::OutgoingRequest => "Outgoing friend request".into(),
            RelationshipKind::Blocked => "Blocked".into(),
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

        let mut actions = div().flex().items_center().gap_2();
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
                        round_button(("accept", id.0), "check", theme::ONLINE).on_click(
                            cx.listener(move |this, _, _, _| {
                                this.backend.send(Command::AcceptFriend(id));
                            }),
                        ),
                    )
                    .child(
                        round_button(("decline", id.0), "x", theme::DANGER)
                            .on_click(cx.listener(move |this, _, _, cx| this.remove(id, cx))),
                    );
            }
            RelationshipKind::OutgoingRequest | RelationshipKind::Blocked => {
                actions = actions.child(
                    round_button(("cancel", id.0), "x", theme::DANGER)
                        .on_click(cx.listener(move |this, _, _, cx| this.remove(id, cx))),
                );
            }
            _ => {}
        }

        let on_open = person.kind == RelationshipKind::Friend;
        let mut row = div()
            .id(("friend", id.0))
            .h(px(62.))
            .mx_4()
            .px_2()
            .flex()
            .items_center()
            .gap_3()
            .rounded(px(8.))
            .border_t_1()
            .border_color(rgb(theme::BORDER))
            .hover(|s| s.bg(rgb(theme::BG_HOVER)));
        if on_open {
            row = row
                .cursor_pointer()
                .on_click(cx.listener(move |this, _, _, cx| this.message(id, cx)));
        }

        row.child(
            div()
                .relative()
                .flex_none()
                .child(avatar(&self.media, &person.avatar, 36., &person.name, cx))
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
                        .items_baseline()
                        .gap_2()
                        .child(
                            div()
                                .truncate()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(rgb(theme::TEXT_BRIGHT))
                                .child(person.name),
                        )
                        .child(
                            div()
                                .truncate()
                                .text_size(px(12.))
                                .text_color(rgb(theme::TEXT_FAINT))
                                .child(person.username),
                        ),
                )
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(rgb(theme::TEXT_MUTED))
                        .child(subtitle),
                ),
        )
        .child(actions)
        .into_any_element()
    }
}

impl Render for FriendsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let header = self.render_tabs(cx);
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
            return page(self.render_add_form(cx));
        }

        let sections = self.sections(cx);
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
                    .child(match self.tab {
                        Tab::Online => "No one is around right now.",
                        Tab::All => "You have no friends here yet.",
                        Tab::Pending => "There are no pending requests.",
                        Tab::Blocked => "You haven't blocked anyone.",
                        Tab::Add => "",
                    }),
            );
        }
        for section in sections {
            body = body.child(
                div()
                    .px_4()
                    .pt_4()
                    .pb_2()
                    .text_size(px(12.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(theme::TEXT_MUTED))
                    .child(format!(
                        "{} \u{2013} {}",
                        section.title.to_uppercase(),
                        section.people.len()
                    )),
            );
            for person in section.people {
                body = body.child(self.render_person(person, cx));
            }
        }

        page(body.into_any_element())
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
        .size(px(34.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .bg(rgb(theme::BG_SIDEBAR))
        .cursor_pointer()
        .hover(|s| s.bg(rgb(theme::BG_ACTIVE)))
        .child(icon(glyph, 18., color))
}
