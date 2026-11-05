use oxicord_model::Snowflake;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Inline {
    Text(String),
    Bold(Vec<Inline>),
    Italic(Vec<Inline>),
    Underline(Vec<Inline>),
    Strike(Vec<Inline>),
    Spoiler(Vec<Inline>),
    Code(String),
    Link { label: Vec<Inline>, url: String },
    User(Snowflake),
    Role(Snowflake),
    Channel(Snowflake),
    Everyone,
    Here,
    Emoji {
        name: String,
        id: Snowflake,
        animated: bool,
    },
    /// `<t:unix:style>`. The style letter is Discord's: t, T, d, D, f, F, R.
    Timestamp { unix: i64, style: char },
    LineBreak,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Block {
    Paragraph(Vec<Inline>),
    Heading { level: u8, content: Vec<Inline> },
    /// `-# small text`
    Subtext(Vec<Inline>),
    Quote(Vec<Block>),
    Code {
        language: Option<String>,
        code: String,
    },
    /// `start` is `Some` for a numbered list.
    List {
        start: Option<u32>,
        items: Vec<Vec<Inline>>,
    },
}
