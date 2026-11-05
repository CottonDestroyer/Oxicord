use oxicord_model::Snowflake;

use crate::ast::Inline;

/// Longest `<...>` form we will look for. Mentions and emoji are short, and
/// a bound keeps a stray `<` from scanning the whole message.
const MAX_ANGLE_LEN: usize = 120;

pub fn parse_inline(input: &str) -> Vec<Inline> {
    let mut out = Vec::new();
    let mut text = String::new();
    let mut i = 0;
    // The character just before `i`, needed to tell `snake_case` from `_emphasis_`.
    let mut prev: Option<char> = None;

    fn flush(text: &mut String, out: &mut Vec<Inline>) {
        if !text.is_empty() {
            out.push(Inline::Text(std::mem::take(text)));
        }
    }

    while i < input.len() {
        let rest = &input[i..];
        let c = rest.chars().next().expect("not at the end");

        let parsed: Option<(Inline, usize)> = match c {
            '\\' => {
                let escaped = rest[1..].chars().next().filter(char::is_ascii_punctuation);
                if let Some(next) = escaped {
                    text.push(next);
                    i += 1 + next.len_utf8();
                    prev = Some(next);
                    continue;
                }
                None
            }
            '\n' => Some((Inline::LineBreak, 1)),
            '`' => code_span(rest),
            '<' => angle(rest),
            '[' => masked_link(rest),
            'h' if rest.starts_with("http://") || rest.starts_with("https://") => autolink(rest),
            '@' => mention_word(rest),
            '*' | '_' | '~' | '|' => emphasis(rest, c, prev),
            _ => None,
        };

        match parsed {
            Some((node, used)) => {
                flush(&mut text, &mut out);
                out.push(node);
                prev = input[..i + used].chars().next_back();
                i += used;
            }
            None => {
                text.push(c);
                prev = Some(c);
                i += c.len_utf8();
            }
        }
    }
    flush(&mut text, &mut out);
    out
}

/// `` `code` `` or ``` ``code with ` inside`` ```.
fn code_span(rest: &str) -> Option<(Inline, usize)> {
    let ticks = rest.bytes().take_while(|b| *b == b'`').count();
    // A run of three or more belongs to a fenced block, not an inline span.
    if ticks >= 3 {
        return None;
    }

    let body = &rest[ticks..];
    let mut search = 0;
    while let Some(found) = body[search..].find('`') {
        let start = search + found;
        let run = body[start..].bytes().take_while(|b| *b == b'`').count();
        if run == ticks && start > 0 {
            return Some((Inline::Code(body[..start].to_owned()), ticks + start + run));
        }
        search = start + run;
    }
    None
}

/// Mentions, emoji, timestamps and `<https://...>`.
fn angle(rest: &str) -> Option<(Inline, usize)> {
    let end = rest.char_indices().take(MAX_ANGLE_LEN).find(|(_, c)| *c == '>')?.0;
    let inner = &rest[1..end];
    let used = end + 1;

    let id = |s: &str| s.parse::<u64>().ok().map(Snowflake);

    if let Some(role) = inner.strip_prefix("@&") {
        return Some((Inline::Role(id(role)?), used));
    }
    if let Some(user) = inner.strip_prefix("@!").or_else(|| inner.strip_prefix('@')) {
        return Some((Inline::User(id(user)?), used));
    }
    if let Some(channel) = inner.strip_prefix('#') {
        return Some((Inline::Channel(id(channel)?), used));
    }
    if let Some(tail) = inner.strip_prefix("t:") {
        let mut parts = tail.splitn(2, ':');
        let unix = parts.next()?.parse::<i64>().ok()?;
        let style = parts
            .next()
            .and_then(|s| s.chars().next().filter(|_| s.len() == 1))
            .unwrap_or('f');
        if !"tTdDfFR".contains(style) {
            return None;
        }
        return Some((Inline::Timestamp { unix, style }, used));
    }
    if let Some(emoji) = custom_emoji(inner) {
        return Some((emoji, used));
    }
    if inner.starts_with("http://") || inner.starts_with("https://") {
        let label = vec![Inline::Text(inner.to_owned())];
        return Some((Inline::Link { label, url: inner.to_owned() }, used));
    }
    None
}

/// `:name:id` or `a:name:id`, the part between the angle brackets.
fn custom_emoji(inner: &str) -> Option<Inline> {
    let (animated, tail) = match inner.strip_prefix("a:") {
        Some(tail) => (true, tail),
        None => (false, inner.strip_prefix(':')?),
    };
    let (name, id) = tail.split_once(':')?;
    let valid_name = (2..=32).contains(&name.len())
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    if !valid_name {
        return None;
    }
    Some(Inline::Emoji {
        name: name.to_owned(),
        id: Snowflake(id.parse().ok()?),
        animated,
    })
}

/// `[label](https://example.com)`
fn masked_link(rest: &str) -> Option<(Inline, usize)> {
    let close = rest.find(']')?;
    let label = &rest[1..close];
    let after = rest[close + 1..].strip_prefix('(')?;
    let paren = after.find(')')?;

    let raw = after[..paren].trim();
    // Discord allows `(<url>)` to suppress the embed.
    let url = raw
        .strip_prefix('<')
        .and_then(|u| u.strip_suffix('>'))
        .unwrap_or(raw);
    if label.is_empty() || !(url.starts_with("http://") || url.starts_with("https://")) {
        return None;
    }
    if url.chars().any(char::is_whitespace) {
        return None;
    }

    let used = close + 1 + 1 + paren + 1;
    Some((
        Inline::Link {
            label: parse_inline(label),
            url: url.to_owned(),
        },
        used,
    ))
}

/// A bare URL in running text. Trailing punctuation belongs to the sentence,
/// not the link.
fn autolink(rest: &str) -> Option<(Inline, usize)> {
    let end = rest
        .find(|c: char| c.is_whitespace() || c == '<' || c == '>')
        .unwrap_or(rest.len());
    let mut url = &rest[..end];

    while let Some(last) = url.chars().next_back() {
        let trailing = matches!(last, '.' | ',' | ';' | ':' | '!' | '?' | '\'' | '"' | '*' | '_')
            // A closing bracket only counts as ours if it has no opener.
            || (last == ')' && url.matches('(').count() < url.matches(')').count());
        if !trailing {
            break;
        }
        url = &url[..url.len() - last.len_utf8()];
    }

    let scheme_len = if url.starts_with("https://") { 8 } else { 7 };
    if url.len() <= scheme_len {
        return None;
    }
    Some((
        Inline::Link {
            label: vec![Inline::Text(url.to_owned())],
            url: url.to_owned(),
        },
        url.len(),
    ))
}

fn mention_word(rest: &str) -> Option<(Inline, usize)> {
    for (word, node) in [("@everyone", Inline::Everyone), ("@here", Inline::Here)] {
        if let Some(after) = rest.strip_prefix(word) {
            let ends_word = after.chars().next().is_none_or(|c| !c.is_alphanumeric() && c != '_');
            if ends_word {
                return Some((node, word.len()));
            }
        }
    }
    None
}

/// `**bold**`, `*italic*`, `__underline__`, `_italic_`, `~~strike~~`,
/// `||spoiler||` and `***both***`.
fn emphasis(rest: &str, c: char, prev: Option<char>) -> Option<(Inline, usize)> {
    let run = rest.chars().take_while(|x| *x == c).count();

    match c {
        '*' if run >= 3 => wrap(rest, "***", |inner| {
            Inline::Bold(vec![Inline::Italic(inner)])
        })
        .or_else(|| wrap(rest, "**", Inline::Bold)),
        '*' if run == 2 => wrap(rest, "**", Inline::Bold),
        '*' => single(rest, '*', None),
        '_' if run >= 2 => wrap(rest, "__", Inline::Underline),
        // `snake_case_name` stays plain, an opener must not sit inside a word.
        '_' if prev.is_some_and(|p| p.is_alphanumeric()) => None,
        '_' => single(rest, '_', Some(())),
        '~' if run >= 2 => wrap(rest, "~~", Inline::Strike),
        '|' if run >= 2 => wrap(rest, "||", Inline::Spoiler),
        _ => None,
    }
}

/// A delimiter pair around non-empty content.
fn wrap(rest: &str, delim: &str, make: impl Fn(Vec<Inline>) -> Inline) -> Option<(Inline, usize)> {
    let body = &rest[delim.len()..];
    let close = find_close(body, delim)?;
    let inner = &body[..close];
    if inner.is_empty() {
        return None;
    }
    Some((make(parse_inline(inner)), delim.len() + close + delim.len()))
}

/// The first `delim` in `body` that isn't escaped.
fn find_close(body: &str, delim: &str) -> Option<usize> {
    let mut from = 0;
    while let Some(found) = body[from..].find(delim) {
        let at = from + found;
        let escaped = body[..at].ends_with('\\');
        // Content must be non-empty, and the closer shouldn't be the start
        // of a longer run (`**a***` closes at the first two stars).
        if at > 0 && !escaped {
            return Some(at);
        }
        from = at + delim.len();
    }
    None
}

/// Single character emphasis, `*x*` or `_x_`.
fn single(rest: &str, delim: char, word_boundary: Option<()>) -> Option<(Inline, usize)> {
    let body = &rest[1..];
    // Opening next to whitespace is just a lone character.
    if body.chars().next().is_none_or(char::is_whitespace) {
        return None;
    }

    let mut from = 0;
    while let Some(found) = body[from..].find(delim) {
        let at = from + found;
        let before = body[..at].chars().next_back();
        let after = body[at + 1..].chars().next();
        let part_of_run = after == Some(delim) || before == Some(delim);
        let escaped = before == Some('\\');
        let space_before = before.is_none_or(char::is_whitespace);
        // For `_`, the closer must also end a word.
        let inside_word = word_boundary.is_some() && after.is_some_and(char::is_alphanumeric);

        if at > 0 && !part_of_run && !escaped && !space_before && !inside_word {
            return Some((Inline::Italic(parse_inline(&body[..at])), at + 2));
        }
        from = at + 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(s: &str) -> Inline {
        Inline::Text(s.to_owned())
    }

    #[test]
    fn plain_text_stays_text_and_keeps_unicode() {
        assert_eq!(parse_inline("hello wörld 🎉"), [text("hello wörld 🎉")]);
        assert_eq!(parse_inline(""), []);
    }

    #[test]
    fn basic_emphasis() {
        assert_eq!(parse_inline("**b**"), [Inline::Bold(vec![text("b")])]);
        assert_eq!(parse_inline("*i*"), [Inline::Italic(vec![text("i")])]);
        assert_eq!(parse_inline("_i_"), [Inline::Italic(vec![text("i")])]);
        assert_eq!(parse_inline("__u__"), [Inline::Underline(vec![text("u")])]);
        assert_eq!(parse_inline("~~s~~"), [Inline::Strike(vec![text("s")])]);
        assert_eq!(parse_inline("||x||"), [Inline::Spoiler(vec![text("x")])]);
    }

    #[test]
    fn emphasis_works_inside_a_sentence_and_nests() {
        assert_eq!(
            parse_inline("a **b *c* d** e"),
            [
                text("a "),
                Inline::Bold(vec![text("b "), Inline::Italic(vec![text("c")]), text(" d")]),
                text(" e"),
            ]
        );
        assert_eq!(
            parse_inline("***both***"),
            [Inline::Bold(vec![Inline::Italic(vec![text("both")])])]
        );
    }

    #[test]
    fn unmatched_delimiters_stay_as_text() {
        assert_eq!(parse_inline("2 * 3 = 6"), [text("2 * 3 = 6")]);
        assert_eq!(parse_inline("**open"), [text("**open")]);
        assert_eq!(parse_inline("a ~~ b"), [text("a ~~ b")]);
        assert_eq!(parse_inline("* spaced *"), [text("* spaced *")]);
    }

    #[test]
    fn underscores_inside_words_do_not_italicize() {
        assert_eq!(parse_inline("snake_case_name"), [text("snake_case_name")]);
        assert_eq!(
            parse_inline("a _real_ one"),
            [text("a "), Inline::Italic(vec![text("real")]), text(" one")]
        );
    }

    #[test]
    fn escapes_remove_the_markup() {
        assert_eq!(parse_inline(r"\*not italic\*"), [text("*not italic*")]);
        assert_eq!(parse_inline(r"a\b"), [text(r"a\b")]);
    }

    #[test]
    fn code_spans_are_literal() {
        assert_eq!(
            parse_inline("use `**x**` here"),
            [text("use "), Inline::Code("**x**".into()), text(" here")]
        );
        assert_eq!(parse_inline("``a ` b``"), [Inline::Code("a ` b".into())]);
        assert_eq!(parse_inline("`unclosed"), [text("`unclosed")]);
    }

    #[test]
    fn links_masked_bare_and_bracketed() {
        assert_eq!(
            parse_inline("[docs](https://a.dev/x)"),
            [Inline::Link { label: vec![text("docs")], url: "https://a.dev/x".into() }]
        );
        assert_eq!(
            parse_inline("see https://a.dev/x. ok"),
            [
                text("see "),
                Inline::Link { label: vec![text("https://a.dev/x")], url: "https://a.dev/x".into() },
                text(". ok"),
            ]
        );
        assert_eq!(
            parse_inline("<https://a.dev>"),
            [Inline::Link { label: vec![text("https://a.dev")], url: "https://a.dev".into() }]
        );
        // Not a link: no scheme, so it stays literal.
        assert_eq!(parse_inline("[x](nope)"), [text("[x](nope)")]);
    }

    #[test]
    fn a_closing_paren_in_a_url_is_kept_only_when_balanced() {
        assert_eq!(
            parse_inline("(see https://a.dev/x)"),
            [
                text("(see "),
                Inline::Link { label: vec![text("https://a.dev/x")], url: "https://a.dev/x".into() },
                text(")"),
            ]
        );
        let wiki = parse_inline("https://en.wikipedia.org/wiki/Rust_(language)");
        assert!(matches!(&wiki[0], Inline::Link { url, .. } if url.ends_with("(language)")));
    }

    #[test]
    fn mentions_and_everyone() {
        assert_eq!(parse_inline("<@123>"), [Inline::User(Snowflake(123))]);
        assert_eq!(parse_inline("<@!123>"), [Inline::User(Snowflake(123))]);
        assert_eq!(parse_inline("<@&9>"), [Inline::Role(Snowflake(9))]);
        assert_eq!(parse_inline("<#77>"), [Inline::Channel(Snowflake(77))]);
        assert_eq!(parse_inline("@everyone"), [Inline::Everyone]);
        assert_eq!(parse_inline("hi @here!"), [text("hi "), Inline::Here, text("!")]);
        assert_eq!(parse_inline("@everyoneelse"), [text("@everyoneelse")]);
        assert_eq!(parse_inline("<@notanid>"), [text("<@notanid>")]);
    }

    #[test]
    fn custom_emoji_static_and_animated() {
        assert_eq!(
            parse_inline("<:party:42>"),
            [Inline::Emoji { name: "party".into(), id: Snowflake(42), animated: false }]
        );
        assert_eq!(
            parse_inline("<a:spin:43>"),
            [Inline::Emoji { name: "spin".into(), id: Snowflake(43), animated: true }]
        );
        assert_eq!(parse_inline("<:x:1>"), [text("<:x:1>")], "name too short");
    }

    #[test]
    fn timestamps_with_and_without_a_style() {
        assert_eq!(
            parse_inline("<t:1700000000:R>"),
            [Inline::Timestamp { unix: 1_700_000_000, style: 'R' }]
        );
        assert_eq!(
            parse_inline("<t:1700000000>"),
            [Inline::Timestamp { unix: 1_700_000_000, style: 'f' }]
        );
        assert_eq!(parse_inline("<t:1:Z>"), [text("<t:1:Z>")]);
    }

    #[test]
    fn newlines_become_line_breaks() {
        assert_eq!(parse_inline("a\nb"), [text("a"), Inline::LineBreak, text("b")]);
    }

    #[test]
    fn multibyte_text_around_markup_does_not_panic() {
        assert_eq!(
            parse_inline("日本**語**です"),
            [text("日本"), Inline::Bold(vec![text("語")]), text("です")]
        );
        let _ = parse_inline("é_é_é");
        let _ = parse_inline("🎉*🎉*🎉");
        let _ = parse_inline("<@𝔘>");
    }
}
