//! The quick switcher: one box that finds people, channels and servers.
//! Searching is kept apart from the view so the ranking can be tested.

use gpui_kit::{
    AnyElement, AppContext as _, ClickEvent, Context, Entity, EventEmitter, FontWeight, InteractiveElement as _,
    IntoElement, KeyDownEvent, ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _,
    Subscription, Window, div, px, rgb,
    component::input::{Input, InputEvent, InputState},
};
use oxicord_media::cdn;
use oxicord_model::{ChannelType, Snowflake};
use oxicord_store::Store;

use crate::{
    icons::icon,
    media::{Media, avatar},
    profile::modal,
    session::Session,
    theme,
};

/// Where a result leads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Goal {
    /// An open conversation.
    Dm(Snowflake),
    /// A person we have no conversation with yet.
    User(Snowflake),
    Channel(Snowflake),
    Guild(Snowflake),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Hit {
    pub goal: Goal,
    pub title: String,
    /// The server or category a result lives in.
    pub note: String,
    pub glyph: &'static str,
    /// An avatar or icon address, for results that have a picture.
    pub picture: Option<String>,
}

const MAX_HITS: usize = 25;

/// What a leading character narrows the search to.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Scope {
    Everything,
    People,
    Text,
    Voice,
    Servers,
}

fn split_scope(query: &str) -> (Scope, &str) {
    let query = query.trim_start();
    match query.chars().next() {
        Some('@') => (Scope::People, &query[1..]),
        Some('#') => (Scope::Text, &query[1..]),
        Some('!') => (Scope::Voice, &query[1..]),
        Some('*') => (Scope::Servers, &query[1..]),
        _ => (Scope::Everything, query),
    }
}

/// How well `name` matches `needle`, lower is better. Starting a name beats
/// starting a word, which beats appearing somewhere inside.
fn rank(name: &str, needle: &str) -> Option<u8> {
    let name = name.to_lowercase();
    if name == needle {
        Some(0)
    } else if name.starts_with(needle) {
        Some(1)
    } else if name.split(|c: char| !c.is_alphanumeric()).any(|w| w.starts_with(needle)) {
        Some(2)
    } else if name.contains(needle) {
        Some(3)
    } else {
        None
    }
}

fn is_voice(kind: ChannelType) -> bool {
    matches!(kind, ChannelType::GuildVoice | ChannelType::GuildStageVoice)
}

fn dm_picture(channel: &oxicord_model::Channel) -> String {
    match (channel.kind, channel.recipients.first()) {
        (ChannelType::Dm, Some(user)) => cdn::user_avatar(user.id, user.avatar.as_deref(), 64),
        _ => match channel.icon.as_deref() {
            Some(hash) => cdn::group_icon(channel.id, hash, 64),
            None => cdn::default_avatar(channel.id),
        },
    }
}

/// Everything matching `query`, best first. An empty query lists the latest
/// conversations.
pub fn search(store: &Store, query: &str) -> Vec<Hit> {
    let (scope, text) = split_scope(query);
    let needle = text.trim().to_lowercase();

    if needle.is_empty() {
        if scope != Scope::Everything && scope != Scope::People {
            return Vec::new();
        }
        return store
            .dm_channels()
            .into_iter()
            .take(10)
            .map(|c| Hit {
                goal: Goal::Dm(c.id),
                title: store.dm_name(c),
                note: String::new(),
                glyph: if c.kind == ChannelType::Dm { "at-sign" } else { "users" },
                picture: Some(dm_picture(c)),
            })
            .collect();
    }

    let mut scored: Vec<(u8, Hit)> = Vec::new();
    let mut push = |score: Option<u8>, hit: Hit| {
        if let Some(score) = score {
            scored.push((score, hit));
        }
    };

    let people = matches!(scope, Scope::Everything | Scope::People);
    if people {
        for c in store.dm_channels() {
            let name = store.dm_name(c);
            let score = rank(&name, &needle);
            push(
                score,
                Hit {
                    goal: Goal::Dm(c.id),
                    title: name,
                    note: if c.kind == ChannelType::Dm { String::new() } else { "Group".into() },
                    glyph: if c.kind == ChannelType::Dm { "at-sign" } else { "users" },
                    picture: Some(dm_picture(c)),
                },
            );
        }
        // Friends we never wrote to still deserve a result.
        for user in store.relationships(oxicord_model::RelationshipKind::Friend) {
            if store.dm_with(user.id).is_some() {
                continue;
            }
            let score = rank(user.display_name(), &needle).or_else(|| rank(&user.username, &needle));
            push(
                score,
                Hit {
                    goal: Goal::User(user.id),
                    title: user.display_name().to_owned(),
                    note: user.username.clone(),
                    glyph: "at-sign",
                    picture: Some(cdn::user_avatar(user.id, user.avatar.as_deref(), 64)),
                },
            );
        }
    }

    if matches!(scope, Scope::Everything | Scope::Text | Scope::Voice) {
        for guild in store.guilds() {
            for group in store.channel_groups(guild.id) {
                for c in group.channels {
                    let voice = is_voice(c.kind);
                    let wanted = match scope {
                        Scope::Text => !voice,
                        Scope::Voice => voice,
                        _ => true,
                    };
                    if !wanted || !(c.kind.has_messages() || voice) {
                        continue;
                    }
                    let name = c.name.clone().unwrap_or_default();
                    push(
                        rank(&name, &needle),
                        Hit {
                            goal: Goal::Channel(c.id),
                            title: name,
                            note: guild.name.clone(),
                            glyph: if voice { "volume-2" } else { "hash" },
                            picture: None,
                        },
                    );
                }
            }
        }
    }

    if matches!(scope, Scope::Everything | Scope::Servers) {
        for guild in store.guilds() {
            push(
                rank(&guild.name, &needle),
                Hit {
                    goal: Goal::Guild(guild.id),
                    title: guild.name.clone(),
                    note: String::new(),
                    glyph: "server",
                    picture: guild.icon.as_deref().map(|h| cdn::guild_icon(guild.id, h, 64)),
                },
            );
        }
    }

    // Stable, so equal scores keep the order they were found in: people
    // first, then channels, then servers.
    scored.sort_by_key(|(score, _)| *score);
    scored.into_iter().map(|(_, hit)| hit).take(MAX_HITS).collect()
}

pub struct CloseSwitcher;

impl EventEmitter<CloseSwitcher> for QuickSwitcher {}

pub struct Chosen(pub Goal);

impl EventEmitter<Chosen> for QuickSwitcher {}

pub struct QuickSwitcher {
    session: Entity<Session>,
    media: Entity<Media>,
    input: Entity<InputState>,
    hits: Vec<Hit>,
    selected: usize,
    _subscriptions: Vec<Subscription>,
}

impl QuickSwitcher {
    pub fn new(session: Entity<Session>, media: Entity<Media>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Where would you like to go?"));
        let subscriptions = vec![
            cx.subscribe(&input, |this, _, event: &InputEvent, cx| match event {
                InputEvent::Change => this.refresh(cx),
                InputEvent::PressEnter { .. } => this.choose(cx),
                _ => {}
            }),
            cx.observe(&media, |_, _, cx| cx.notify()),
        ];
        Self { session, media, input, hits: Vec::new(), selected: 0, _subscriptions: subscriptions }
    }

    pub fn opened(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.input.update(cx, |input, cx| {
            input.set_value("", window, cx);
            input.focus(window, cx);
        });
        self.refresh(cx);
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        let query = self.input.read(cx).value().to_string();
        self.hits = search(&self.session.read(cx).store, &query);
        self.selected = 0;
        cx.notify();
    }

    fn choose(&mut self, cx: &mut Context<Self>) {
        if let Some(hit) = self.hits.get(self.selected) {
            cx.emit(Chosen(hit.goal));
        }
    }

    fn step(&mut self, forward: bool, cx: &mut Context<Self>) {
        if self.hits.is_empty() {
            return;
        }
        let last = self.hits.len() - 1;
        self.selected = if forward {
            if self.selected >= last { 0 } else { self.selected + 1 }
        } else if self.selected == 0 {
            last
        } else {
            self.selected - 1
        };
        cx.notify();
    }
}

impl Render for QuickSwitcher {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut results = div().id("switcher-results").max_h(px(360.)).overflow_y_scroll().flex().flex_col();
        for (index, hit) in self.hits.clone().into_iter().enumerate() {
            let picked = index == self.selected;
            let leading: AnyElement = match &hit.picture {
                Some(url) => avatar(&self.media, url, 24., &hit.title, cx).into_any_element(),
                None => icon(hit.glyph, 18., theme::TEXT_MUTED).into_any_element(),
            };
            results = results.child(
                div()
                    .id(("hit", index as u64))
                    .h(px(36.))
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .rounded(px(4.))
                    .cursor_pointer()
                    .bg(rgb(if picked { theme::BG_ACTIVE } else { theme::BG_SIDEBAR }))
                    .hover(|s| s.bg(rgb(theme::BG_HOVER)))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.selected = index;
                        this.choose(cx);
                    }))
                    .child(div().w(px(24.)).flex().justify_center().child(leading))
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(theme::TEXT_BRIGHT))
                            .child(hit.title.clone()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(px(12.))
                            .text_color(rgb(theme::TEXT_MUTED))
                            .child(hit.note.clone()),
                    ),
            );
        }

        let card = div()
            .id("switcher-card")
            .occlude()
            .w(px(520.))
            .mt(px(-120.))
            .p_4()
            .flex()
            .flex_col()
            .gap_3()
            .rounded(px(10.))
            .border_1()
            .border_color(rgb(theme::BORDER))
            .bg(rgb(theme::BG_SIDEBAR))
            .capture_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                match event.keystroke.key.as_str() {
                    "down" => this.step(true, cx),
                    "up" => this.step(false, cx),
                    "escape" => cx.emit(CloseSwitcher),
                    _ => return,
                }
                cx.stop_propagation();
            }))
            .child(Input::new(&self.input))
            .child(results)
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(rgb(theme::TEXT_MUTED))
                    .child("Start with @ for people, # for channels, ! for voice channels or * for servers."),
            );

        modal(card.into_any_element(), Box::new(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(CloseSwitcher))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_that_starts_with_the_text_beats_one_that_only_contains_it() {
        assert_eq!(rank("General", "gen"), Some(1));
        assert_eq!(rank("off-topic general", "gen"), Some(2));
        assert_eq!(rank("regen", "gen"), Some(3));
        assert_eq!(rank("general", "general"), Some(0));
        assert_eq!(rank("random", "gen"), None);
    }

    #[test]
    fn a_leading_symbol_narrows_the_search() {
        assert!(matches!(split_scope("@ada"), (Scope::People, "ada")));
        assert!(matches!(split_scope("  #chat"), (Scope::Text, "chat")));
        assert!(matches!(split_scope("!lounge"), (Scope::Voice, "lounge")));
        assert!(matches!(split_scope("*my server"), (Scope::Servers, "my server")));
        assert!(matches!(split_scope("plain"), (Scope::Everything, "plain")));
    }
}
