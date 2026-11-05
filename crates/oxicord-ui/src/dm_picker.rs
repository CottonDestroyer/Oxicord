//! The popover for starting a conversation: pick friends, then create a DM
//! or a group, or add them to the group that is already open.

use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AppContext as _, ClickEvent, Context, Entity, EventEmitter, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement as _, Pixels, Point, Render, StatefulInteractiveElement as _, Styled as _, Subscription, Window,
    div, px, rgb,
    component::input::{Input, InputEvent, InputState},
};
use oxicord_media::cdn;
use oxicord_model::{ChannelType, RelationshipKind, Snowflake};

use crate::{
    backend::{Backend, Command},
    icons::icon,
    media::{Media, avatar},
    session::Session,
    theme,
};

/// A group conversation holds ten people, counting us.
const MAX_PICKED: usize = 9;

pub struct ClosePicker;

impl EventEmitter<ClosePicker> for DmPicker {}

/// What the chosen people are for.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    /// Start a conversation from nothing.
    Create,
    /// Bring people into the conversation that is open.
    AddTo(Snowflake),
}

pub struct DmPicker {
    session: Entity<Session>,
    media: Entity<Media>,
    backend: Backend,
    purpose: Purpose,
    at: Point<Pixels>,
    search: Entity<InputState>,
    picked: Vec<Snowflake>,
    _subscriptions: Vec<Subscription>,
}

impl DmPicker {
    pub fn new(
        session: Entity<Session>,
        media: Entity<Media>,
        backend: Backend,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Type the username of a friend"));
        let subscriptions = vec![
            cx.subscribe(&search, |_, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            }),
            cx.observe(&media, |_, _, cx| cx.notify()),
        ];
        Self {
            session,
            media,
            backend,
            purpose: Purpose::Create,
            at: Point::default(),
            search,
            picked: Vec::new(),
            _subscriptions: subscriptions,
        }
    }

    pub fn show(&mut self, purpose: Purpose, at: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        self.purpose = purpose;
        self.at = at;
        self.picked.clear();
        self.search.update(cx, |input, cx| {
            input.set_value("", window, cx);
            input.focus(window, cx);
        });
        cx.notify();
    }

    fn toggle(&mut self, user: Snowflake, cx: &mut Context<Self>) {
        if let Some(at) = self.picked.iter().position(|&u| u == user) {
            self.picked.remove(at);
        } else if self.picked.len() < self.room(cx) {
            self.picked.push(user);
        }
        cx.notify();
    }

    /// The conversation being added to, with the people already in it.
    fn existing(&self, cx: &gpui_kit::App) -> Option<(Snowflake, ChannelType, Vec<Snowflake>)> {
        let Purpose::AddTo(channel) = self.purpose else { return None };
        let c = self.session.read(cx).store.channel(channel)?;
        Some((channel, c.kind, c.recipients.iter().map(|r| r.id).collect()))
    }

    /// How many more people fit.
    fn room(&self, cx: &gpui_kit::App) -> usize {
        let present = self.existing(cx).map_or(0, |(_, _, people)| people.len());
        MAX_PICKED.saturating_sub(present)
    }

    fn confirm(&mut self, cx: &mut Context<Self>) {
        if self.picked.is_empty() {
            return;
        }
        let picked = std::mem::take(&mut self.picked);
        match self.existing(cx) {
            Some((_, ChannelType::Dm, mut people)) => {
                // Adding to a one-to-one chat makes a new group, the chat
                // itself stays as it is.
                people.extend(picked);
                self.backend.send(Command::OpenGroupDm(people));
            }
            Some((channel, _, _)) => self.backend.send(Command::AddRecipients { channel, users: picked }),
            None if picked.len() == 1 => self.backend.send(Command::OpenDm(picked[0])),
            None => self.backend.send(Command::OpenGroupDm(picked)),
        }
        cx.emit(ClosePicker);
    }
}

impl Render for DmPicker {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let needle = self.search.read(cx).value().trim().to_lowercase();
        let present: Vec<Snowflake> = self.existing(cx).map(|(_, _, p)| p).unwrap_or_default();
        let friends: Vec<_> = self
            .session
            .read(cx)
            .store
            .relationships(RelationshipKind::Friend)
            .into_iter()
            .filter(|u| !present.contains(&u.id))
            .filter(|u| {
                needle.is_empty()
                    || u.display_name().to_lowercase().contains(&needle)
                    || u.username.to_lowercase().contains(&needle)
            })
            .cloned()
            .collect();

        let room = self.room(cx);
        let title = if matches!(self.purpose, Purpose::Create) { "Select Friends" } else { "Add Friends to DM" };
        let note = format!("You can add {} more friend{}.", room.saturating_sub(self.picked.len()), if room.saturating_sub(self.picked.len()) == 1 { "" } else { "s" });

        let mut rows = div().id("picker-rows").max_h(px(300.)).overflow_y_scroll().flex().flex_col();
        for user in friends {
            let id = user.id;
            let on = self.picked.contains(&id);
            let url = cdn::user_avatar(id, user.avatar.as_deref(), 64);
            rows = rows.child(
                div()
                    .id(("friend", id.0))
                    .h(px(42.))
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_3()
                    .rounded(px(4.))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(theme::BG_HOVER)))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.toggle(id, cx)))
                    .child(avatar(&self.media, &url, 28., user.display_name(), cx))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .child(
                                div()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(rgb(theme::TEXT_BRIGHT))
                                    .child(user.display_name().to_owned()),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(rgb(theme::TEXT_MUTED))
                            .child(user.username.clone()),
                    )
                    .child(
                        div()
                            .size(px(20.))
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(4.))
                            .border_1()
                            .border_color(rgb(if on { theme::ACCENT } else { theme::TEXT_FAINT }))
                            .bg(rgb(if on { theme::ACCENT } else { theme::BG_SIDEBAR }))
                            .children(on.then(|| icon("check", 14., theme::TEXT_BRIGHT))),
                    ),
            );
        }

        let ready = !self.picked.is_empty();
        let label = match (self.purpose, self.picked.len()) {
            (Purpose::AddTo(_), _) => "Add",
            (Purpose::Create, 0 | 1) => "Create DM",
            (Purpose::Create, _) => "Create Group DM",
        };

        let card = div()
            .id("picker-card")
            .occlude()
            .absolute()
            .left(self.at.x)
            .top(self.at.y)
            .w(px(400.))
            .p_4()
            .flex()
            .flex_col()
            .gap_3()
            .rounded(px(8.))
            .border_1()
            .border_color(rgb(theme::BORDER))
            .bg(rgb(theme::BG_SIDEBAR))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .text_size(px(18.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(theme::TEXT_BRIGHT))
                            .child(title),
                    )
                    .child(div().text_size(px(12.)).text_color(rgb(theme::TEXT_MUTED)).child(note)),
            )
            .child(Input::new(&self.search))
            .child(rows)
            .child(
                div()
                    .id("picker-confirm")
                    .h(px(34.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(4.))
                    .text_color(rgb(theme::TEXT_BRIGHT))
                    .bg(rgb(if ready { theme::ACCENT } else { theme::BG_ACTIVE }))
                    .when(ready, |b| b.cursor_pointer().hover(|s| s.opacity(0.85)))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.confirm(cx)))
                    .child(label),
            );

        div()
            .id("picker-overlay")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .child(
                div()
                    .id("picker-scrim")
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .on_click(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(ClosePicker))),
            )
            .child(card)
    }
}
