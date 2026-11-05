//! Draws parsed markdown. Inline content is flattened into runs of styled
//! text so it wraps as one paragraph, which a row of separate elements
//! wouldn't.

use std::{
    collections::{HashMap, HashSet},
    ops::Range,
    rc::Rc,
};

use chrono::{DateTime, FixedOffset, Utc};
use gpui_kit::{
    AnyElement, App, Entity, FontStyle, FontWeight, ImageSource, InteractiveText,
    IntoElement, ParentElement as _, SharedString, StrikethroughStyle, Styled as _, StyledText, TextRun, UnderlineStyle, Window, div, img, px, rgb,
};
use oxicord_markdown::{Block, Inline};
use oxicord_media::cdn;
use oxicord_model::Snowflake;

use super::format;
use crate::{media::Media, theme};

pub const LINK_COLOR: u32 = 0x00a8fc;
const MENTION_FG: u32 = 0xc9cdfb;
const MENTION_BG: u32 = 0x3c4270;
const CODE_BG: u32 = 0x1a1b1f;
const SPOILER_HIDDEN: u32 = 0x0f1012;
const SPOILER_SHOWN: u32 = 0x2e3035;
const MONO: &str = "Consolas";
const JUMBO_EMOJI_LIMIT: usize = 27;

/// Called with a spoiler's index when a hidden one is clicked.
pub type SpoilerHandler = Rc<dyn Fn(usize, &mut Window, &mut App)>;

/// Display names for the mentions in one message.
#[derive(Default)]
pub struct Names {
    pub users: HashMap<Snowflake, String>,
    /// Name and color. A color of 0 means the role has none.
    pub roles: HashMap<Snowflake, (String, u32)>,
    pub channels: HashMap<Snowflake, String>,
}

/// Everything a message needs to be drawn.
pub struct Options<'a> {
    pub names: &'a Names,
    pub revealed: &'a HashSet<usize>,
    pub media: &'a Entity<Media>,
    pub now: DateTime<Utc>,
    pub offset: FixedOffset,
    /// Show clock times on a 12 hour clock.
    pub hour12: bool,
    /// Spoilers are drawn open without a click.
    pub reveal_spoilers: bool,
    pub edited: bool,
    /// Keeps element ids unique across messages.
    pub key: u64,
    pub on_spoiler: SpoilerHandler,
}

/// The users, roles and channels a message mentions, so their names can be
/// looked up before drawing.
#[derive(Default)]
pub struct MentionIds {
    pub users: Vec<Snowflake>,
    pub roles: Vec<Snowflake>,
    pub channels: Vec<Snowflake>,
}

pub fn mention_ids(blocks: &[Block]) -> MentionIds {
    fn inlines(list: &[Inline], out: &mut MentionIds) {
        for node in list {
            match node {
                Inline::User(id) => out.users.push(*id),
                Inline::Role(id) => out.roles.push(*id),
                Inline::Channel(id) => out.channels.push(*id),
                Inline::Bold(c)
                | Inline::Italic(c)
                | Inline::Underline(c)
                | Inline::Strike(c)
                | Inline::Spoiler(c) => inlines(c, out),
                Inline::Link { label, .. } => inlines(label, out),
                _ => {}
            }
        }
    }
    fn walk(blocks: &[Block], out: &mut MentionIds) {
        for block in blocks {
            match block {
                Block::Paragraph(c) | Block::Subtext(c) => inlines(c, out),
                Block::Heading { content, .. } => inlines(content, out),
                Block::Quote(inner) => walk(inner, out),
                Block::List { items, .. } => items.iter().for_each(|i| inlines(i, out)),
                Block::Code { .. } => {}
            }
        }
    }

    let mut ids = MentionIds::default();
    walk(blocks, &mut ids);
    ids
}

pub fn render(blocks: &[Block], opts: &Options, window: &mut Window, cx: &mut App) -> AnyElement {
    let mut column = div().w_full().min_w_0().flex().flex_col().gap(px(2.));
    let mut spoilers = 0;
    let last = blocks.len().saturating_sub(1);

    for (ix, block) in blocks.iter().enumerate() {
        let tag_edited = opts.edited && ix == last;
        column = column.child(render_block(block, ix, tag_edited, opts, &mut spoilers, window, cx));
    }
    // A message that is only attachments has no text block to carry the tag.
    if opts.edited && !matches!(blocks.last(), Some(Block::Paragraph(_))) {
        column = column.child(
            div()
                .text_size(px(11.))
                .text_color(rgb(theme::TEXT_FAINT))
                .child("(edited)"),
        );
    }
    column.into_any_element()
}

fn render_block(
    block: &Block,
    ix: usize,
    edited: bool,
    opts: &Options,
    spoilers: &mut usize,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let id = |kind: &str| SharedString::from(format!("{kind}-{}-{ix}", opts.key));

    match block {
        Block::Paragraph(content) => {
            if let Some(emoji) = jumbo_emoji(content) {
                return emoji_row(&emoji, opts, cx);
            }
            text_element(id("p"), content, Base::default(), edited, opts, spoilers, window)
        }
        Block::Heading { level, content } => {
            let size = match level {
                1 => 24.,
                2 => 20.,
                _ => 16.,
            };
            let base = Base {
                bold: true,
                ..Base::default()
            };
            div()
                .mt(px(4.))
                .text_size(px(size))
                .child(text_element(id("h"), content, base, edited, opts, spoilers, window))
                .into_any_element()
        }
        Block::Subtext(content) => {
            let base = Base {
                color: theme::TEXT_MUTED,
                ..Base::default()
            };
            div()
                .text_size(px(12.))
                .child(text_element(id("s"), content, base, edited, opts, spoilers, window))
                .into_any_element()
        }
        Block::Quote(inner) => {
            let mut column = div().flex().flex_col().gap(px(2.)).min_w_0();
            for (i, block) in inner.iter().enumerate() {
                column = column.child(render_block(block, i, false, opts, spoilers, window, cx));
            }
            div()
                .flex()
                .my(px(2.))
                .child(div().w(px(4.)).flex_none().rounded_full().bg(rgb(0x4e5058)))
                .child(div().flex_1().min_w_0().pl(px(12.)).child(column))
                .into_any_element()
        }
        Block::Code { code, .. } => div()
            .w_full()
            .my(px(2.))
            .px_3()
            .py_2()
            .rounded(px(4.))
            .bg(rgb(CODE_BG))
            .border_1()
            .border_color(rgb(theme::BORDER))
            .font_family(MONO)
            .text_size(px(13.))
            .text_color(rgb(theme::TEXT))
            .child(code.clone())
            .into_any_element(),
        Block::List { start, items } => {
            let mut list = div().flex().flex_col().gap(px(1.)).pl(px(8.));
            for (n, item) in items.iter().enumerate() {
                let marker = match start {
                    Some(first) => format!("{}.", first + n as u32),
                    None => "\u{2022}".to_owned(),
                };
                let line = text_element(
                    id(&format!("li{n}")),
                    item,
                    Base::default(),
                    edited && n + 1 == items.len(),
                    opts,
                    spoilers,
                    window,
                );
                list = list.child(
                    div()
                        .flex()
                        .gap_2()
                        .child(div().flex_none().text_color(rgb(theme::TEXT_MUTED)).child(marker))
                        .child(div().flex_1().min_w_0().child(line)),
                );
            }
            list.into_any_element()
        }
    }
}

/// A paragraph made only of custom emoji is shown large, like the official
/// client does.
fn jumbo_emoji(content: &[Inline]) -> Option<Vec<(Snowflake, String)>> {
    let mut emoji = Vec::new();
    for node in content {
        match node {
            Inline::Emoji { id, name, .. } => emoji.push((*id, name.clone())),
            Inline::Text(t) if t.trim().is_empty() => {}
            _ => return None,
        }
    }
    (!emoji.is_empty() && emoji.len() <= JUMBO_EMOJI_LIMIT).then_some(emoji)
}

fn emoji_row(emoji: &[(Snowflake, String)], opts: &Options, cx: &mut App) -> AnyElement {
    let mut row = div().flex().flex_wrap().gap_1();
    for (id, name) in emoji {
        let url = cdn::emoji(*id, 96);
        let image = opts.media.update(cx, |media, _| media.get(&url));
        row = row.child(match image {
            Some(image) => img(ImageSource::Image(image))
                .size(px(44.))
                .flex_none()
                .into_any_element(),
            None => div()
                .size(px(44.))
                .flex_none()
                .text_size(px(11.))
                .text_color(rgb(theme::TEXT_FAINT))
                .child(format!(":{name}:"))
                .into_any_element(),
        });
    }
    row.into_any_element()
}

/// Defaults for a block of text: headings are bold, subtext is dimmer.
#[derive(Clone, Copy)]
struct Base {
    bold: bool,
    color: u32,
}

impl Default for Base {
    fn default() -> Self {
        Self {
            bold: false,
            color: theme::TEXT,
        }
    }
}

#[derive(Clone, Default)]
struct Style {
    bold: bool,
    italic: bool,
    underline: bool,
    strike: bool,
    code: bool,
    /// The index of the spoiler this text hides behind.
    spoiler: Option<usize>,
    link: Option<String>,
    mention: Option<u32>,
    faint: bool,
}

struct Segment {
    text: String,
    style: Style,
}

fn push(out: &mut Vec<Segment>, text: String, style: Style) {
    out.push(Segment { text, style });
}

fn flatten(
    inlines: &[Inline],
    style: &Style,
    opts: &Options,
    spoilers: &mut usize,
    out: &mut Vec<Segment>,
) {
    let mention = |s: &Style| Style {
        mention: Some(MENTION_FG),
        ..s.clone()
    };

    for node in inlines {
        match node {
            Inline::Text(t) => push(out, t.clone(), style.clone()),
            Inline::LineBreak => push(out, "\n".into(), style.clone()),
            Inline::Code(code) => push(out, 
                code.clone(),
                Style {
                    code: true,
                    ..style.clone()
                },
            ),
            Inline::Bold(c) => {
                let s = Style { bold: true, ..style.clone() };
                flatten(c, &s, opts, spoilers, out);
            }
            Inline::Italic(c) => {
                let s = Style { italic: true, ..style.clone() };
                flatten(c, &s, opts, spoilers, out);
            }
            Inline::Underline(c) => {
                let s = Style { underline: true, ..style.clone() };
                flatten(c, &s, opts, spoilers, out);
            }
            Inline::Strike(c) => {
                let s = Style { strike: true, ..style.clone() };
                flatten(c, &s, opts, spoilers, out);
            }
            Inline::Spoiler(c) => {
                let index = *spoilers;
                *spoilers += 1;
                let s = Style { spoiler: Some(index), ..style.clone() };
                flatten(c, &s, opts, spoilers, out);
            }
            Inline::Link { label, url } => {
                let s = Style { link: Some(url.clone()), ..style.clone() };
                flatten(label, &s, opts, spoilers, out);
            }
            Inline::User(id) => {
                let name = opts.names.users.get(id).map_or("unknown user", String::as_str);
                push(out, format!("@{name}"), mention(style));
            }
            Inline::Role(id) => {
                let (name, color) = opts
                    .names
                    .roles
                    .get(id)
                    .map_or(("deleted-role", 0), |(n, c)| (n.as_str(), *c));
                let mut s = mention(style);
                if color != 0 {
                    s.mention = Some(color);
                }
                push(out, format!("@{name}"), s);
            }
            Inline::Channel(id) => {
                let name = opts.names.channels.get(id).map_or("unknown", String::as_str);
                push(out, format!("#{name}"), mention(style));
            }
            Inline::Everyone => push(out, "@everyone".into(), mention(style)),
            Inline::Here => push(out, "@here".into(), mention(style)),
            Inline::Emoji { name, .. } => push(out, format!(":{name}:"), mention(style)),
            Inline::Timestamp { unix, style: kind } => push(out, 
                format::timestamp(*unix, *kind, opts.now, opts.offset, opts.hour12),
                mention(style),
            ),
        }
    }
}

enum Action {
    Open(String),
    Reveal(usize),
}

fn text_element(
    id: SharedString,
    content: &[Inline],
    base: Base,
    edited: bool,
    opts: &Options,
    spoilers: &mut usize,
    window: &mut Window,
) -> AnyElement {
    let mut segments = Vec::new();
    flatten(content, &Style { bold: base.bold, ..Style::default() }, opts, spoilers, &mut segments);
    if edited {
        let faint = Style { faint: true, ..Style::default() };
        segments.push(Segment { text: " (edited)".into(), style: faint });
    }

    let font = window.text_style().font();
    let mut text = String::new();
    let mut runs = Vec::new();
    let mut ranges: Vec<Range<usize>> = Vec::new();
    let mut actions: Vec<Action> = Vec::new();

    for segment in segments.into_iter().filter(|s| !s.text.is_empty()) {
        let Segment { text: piece, style } = segment;
        let start = text.len();
        text.push_str(&piece);

        let mut run = TextRun {
            len: piece.len(),
            font: font.clone(),
            color: rgb(base.color).into(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        if style.bold {
            run.font.weight = FontWeight::BOLD;
        }
        if style.italic {
            run.font.style = FontStyle::Italic;
        }
        if style.underline {
            run.underline = Some(UnderlineStyle {
                thickness: px(1.),
                color: None,
                wavy: false,
            });
        }
        if style.strike {
            run.strikethrough = Some(StrikethroughStyle {
                thickness: px(1.),
                color: None,
            });
        }
        if style.code {
            run.font.family = MONO.into();
            run.background_color = Some(rgb(CODE_BG).into());
        }
        if style.faint {
            run.color = rgb(theme::TEXT_FAINT).into();
        }
        if let Some(color) = style.mention {
            run.color = rgb(color).into();
            run.background_color = Some(rgb(MENTION_BG).into());
        }
        if let Some(url) = style.link {
            run.color = rgb(LINK_COLOR).into();
            ranges.push(start..start + piece.len());
            actions.push(Action::Open(url));
        }
        if let Some(index) = style.spoiler {
            if opts.reveal_spoilers || opts.revealed.contains(&index) {
                run.background_color = Some(rgb(SPOILER_SHOWN).into());
            } else {
                // Hidden: the text is painted the same color as its box.
                run.color = rgb(SPOILER_HIDDEN).into();
                run.background_color = Some(rgb(SPOILER_HIDDEN).into());
                ranges.push(start..start + piece.len());
                actions.push(Action::Reveal(index));
            }
        }
        runs.push(run);
    }

    if text.is_empty() {
        return div().into_any_element();
    }

    let styled = StyledText::new(text).with_runs(runs);
    if ranges.is_empty() {
        return styled.into_any_element();
    }

    let on_spoiler = opts.on_spoiler.clone();
    InteractiveText::new(id, styled)
        .on_click(ranges, move |ix, window, cx| match &actions[ix] {
            Action::Open(url) => cx.open_url(url),
            Action::Reveal(index) => on_spoiler(*index, window, cx),
        })
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use oxicord_markdown::parse;

    use super::*;

    #[test]
    fn mention_ids_are_found_through_nesting_and_blocks() {
        let blocks = parse("> hi <@1> **<@&2>** [<#3>](https://a.dev)\n- <@4>\n```<@5>```");
        let ids = mention_ids(&blocks);

        assert_eq!(ids.users, [Snowflake(1), Snowflake(4)]);
        assert_eq!(ids.roles, [Snowflake(2)]);
        assert_eq!(ids.channels, [Snowflake(3)]);
    }

    #[test]
    fn only_all_emoji_paragraphs_are_jumbo() {
        let only = parse("<:a1:1> <:b2:2>");
        let Block::Paragraph(content) = &only[0] else { panic!("expected a paragraph") };
        assert_eq!(jumbo_emoji(content).unwrap().len(), 2);

        let mixed = parse("hi <:a1:1>");
        let Block::Paragraph(content) = &mixed[0] else { panic!("expected a paragraph") };
        assert!(jumbo_emoji(content).is_none());
    }
}
