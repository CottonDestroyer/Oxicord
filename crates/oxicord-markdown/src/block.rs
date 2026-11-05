use crate::{
    ast::Block,
    inline::parse_inline,
};

/// Parses a whole message into blocks.
pub fn parse(input: &str) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut rest = input;

    // Fenced code goes first, since nothing inside a fence is markup.
    while let Some(open) = rest.find("```") {
        let after_open = &rest[open + 3..];
        let Some(close) = after_open.find("```") else {
            break;
        };

        text_blocks(&rest[..open], &mut blocks);
        blocks.push(code_block(&after_open[..close]));
        rest = &after_open[close + 3..];
    }
    text_blocks(rest, &mut blocks);
    blocks
}

/// The text between a fence's backticks: an optional language, a newline,
/// then the code. A fence on one line has no language.
fn code_block(body: &str) -> Block {
    if let Some((first, code)) = body.split_once('\n') {
        let language = first.trim();
        let looks_like_language = !language.is_empty()
            && language.len() <= 20
            && language
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '_' | '#'));
        if looks_like_language {
            return Block::Code {
                language: Some(language.to_owned()),
                code: code.trim_end_matches('\n').to_owned(),
            };
        }
    }
    Block::Code {
        language: None,
        code: body.trim_matches('\n').to_owned(),
    }
}

fn text_blocks(text: &str, blocks: &mut Vec<Block>) {
    let lines: Vec<&str> = text.split('\n').collect();
    let mut paragraph: Vec<&str> = Vec::new();
    let mut i = 0;

    fn flush(paragraph: &mut Vec<&str>, blocks: &mut Vec<Block>) {
        if !paragraph.is_empty() {
            blocks.push(Block::Paragraph(parse_inline(&paragraph.join("\n"))));
            paragraph.clear();
        }
    }

    while i < lines.len() {
        let line = lines[i];

        if line.trim().is_empty() {
            flush(&mut paragraph, blocks);
            i += 1;
        } else if let Some(first) = line.strip_prefix(">>> ") {
            flush(&mut paragraph, blocks);
            let mut quoted = vec![first];
            quoted.extend(&lines[i + 1..]);
            blocks.push(Block::Quote(parse(&quoted.join("\n"))));
            return;
        } else if quote_line(line).is_some() {
            flush(&mut paragraph, blocks);
            let mut quoted = Vec::new();
            while let Some(content) = lines.get(i).and_then(|l| quote_line(l)) {
                quoted.push(content);
                i += 1;
            }
            blocks.push(Block::Quote(parse(&quoted.join("\n"))));
        } else if let Some((level, content)) = heading(line) {
            flush(&mut paragraph, blocks);
            blocks.push(Block::Heading {
                level,
                content: parse_inline(content),
            });
            i += 1;
        } else if let Some(content) = line.strip_prefix("-# ") {
            flush(&mut paragraph, blocks);
            blocks.push(Block::Subtext(parse_inline(content)));
            i += 1;
        } else if list_marker(line).is_some() {
            flush(&mut paragraph, blocks);
            i = list(&lines, i, blocks);
        } else {
            paragraph.push(line);
            i += 1;
        }
    }
    flush(&mut paragraph, blocks);
}

/// `> text` or a bare `>`.
fn quote_line(line: &str) -> Option<&str> {
    line.strip_prefix("> ").or_else(|| (line == ">").then_some(""))
}

fn heading(line: &str) -> Option<(u8, &str)> {
    for (marker, level) in [("### ", 3), ("## ", 2), ("# ", 1)] {
        if let Some(content) = line.strip_prefix(marker) {
            return Some((level, content));
        }
    }
    None
}

#[derive(PartialEq)]
enum Marker {
    Bullet,
    Number(u32),
}

/// What kind of list item a line starts, and where its text begins.
fn list_marker(line: &str) -> Option<(Marker, &str)> {
    if let Some(content) = line.strip_prefix("- ").or_else(|| line.strip_prefix("* ")) {
        return Some((Marker::Bullet, content));
    }
    let digits = line.bytes().take_while(u8::is_ascii_digit).count();
    if (1..=9).contains(&digits)
        && let Some(content) = line[digits..].strip_prefix(". ")
    {
        return Some((Marker::Number(line[..digits].parse().ok()?), content));
    }
    None
}

/// Collects consecutive items of one kind starting at `from`. Returns the
/// index of the first line after the list.
fn list(lines: &[&str], from: usize, blocks: &mut Vec<Block>) -> usize {
    let (first, _) = list_marker(lines[from]).expect("caller checked");
    let start = match first {
        Marker::Number(n) => Some(n),
        Marker::Bullet => None,
    };

    let mut items = Vec::new();
    let mut i = from;
    while let Some((marker, content)) = lines.get(i).and_then(|l| list_marker(l)) {
        if matches!(marker, Marker::Bullet) != start.is_none() {
            break;
        }
        items.push(parse_inline(content));
        i += 1;
    }
    blocks.push(Block::List { start, items });
    i
}

#[cfg(test)]
mod tests {
    use oxicord_model::Snowflake;

    use super::*;
    use crate::ast::Inline;

    fn text(s: &str) -> Inline {
        Inline::Text(s.to_owned())
    }

    #[test]
    fn a_plain_message_is_one_paragraph() {
        assert_eq!(parse("hello"), [Block::Paragraph(vec![text("hello")])]);
        assert_eq!(parse(""), []);
        assert_eq!(parse("\n\n"), []);
    }

    #[test]
    fn single_newlines_stay_in_the_paragraph_blank_lines_split_it() {
        assert_eq!(
            parse("a\nb\n\nc"),
            [
                Block::Paragraph(vec![text("a"), Inline::LineBreak, text("b")]),
                Block::Paragraph(vec![text("c")]),
            ]
        );
    }

    #[test]
    fn fenced_code_with_and_without_a_language() {
        assert_eq!(
            parse("```rust\nfn main() {}\n```"),
            [Block::Code { language: Some("rust".into()), code: "fn main() {}".into() }]
        );
        assert_eq!(
            parse("```\nplain\n```"),
            [Block::Code { language: None, code: "plain".into() }]
        );
        assert_eq!(
            parse("```one line```"),
            [Block::Code { language: None, code: "one line".into() }]
        );
    }

    #[test]
    fn markup_inside_a_fence_is_left_alone() {
        assert_eq!(
            parse("before\n```\n**not bold**\n```\nafter"),
            [
                Block::Paragraph(vec![text("before")]),
                Block::Code { language: None, code: "**not bold**".into() },
                Block::Paragraph(vec![text("after")]),
            ]
        );
    }

    #[test]
    fn an_unclosed_fence_is_plain_text() {
        assert_eq!(parse("```oops"), [Block::Paragraph(vec![text("```oops")])]);
    }

    #[test]
    fn a_first_line_that_is_not_a_language_is_code() {
        let blocks = parse("```hello world\nmore```");
        assert_eq!(
            blocks,
            [Block::Code { language: None, code: "hello world\nmore".into() }]
        );
    }

    #[test]
    fn quotes_single_and_to_the_end() {
        assert_eq!(
            parse("> a\n> b\nplain"),
            [
                Block::Quote(vec![Block::Paragraph(vec![text("a"), Inline::LineBreak, text("b")])]),
                Block::Paragraph(vec![text("plain")]),
            ]
        );
        assert_eq!(
            parse(">>> a\nb"),
            [Block::Quote(vec![Block::Paragraph(vec![text("a"), Inline::LineBreak, text("b")])])]
        );
    }

    #[test]
    fn headings_and_subtext() {
        assert_eq!(
            parse("# Big\n## Medium\n### Small\n-# tiny"),
            [
                Block::Heading { level: 1, content: vec![text("Big")] },
                Block::Heading { level: 2, content: vec![text("Medium")] },
                Block::Heading { level: 3, content: vec![text("Small")] },
                Block::Subtext(vec![text("tiny")]),
            ]
        );
        assert_eq!(parse("#nospace"), [Block::Paragraph(vec![text("#nospace")])]);
    }

    #[test]
    fn bullet_and_numbered_lists() {
        assert_eq!(
            parse("- a\n- b\n* c"),
            [Block::List { start: None, items: vec![vec![text("a")], vec![text("b")], vec![text("c")]] }]
        );
        assert_eq!(
            parse("3. x\n4. y"),
            [Block::List { start: Some(3), items: vec![vec![text("x")], vec![text("y")]] }]
        );
    }

    #[test]
    fn a_star_without_a_space_is_emphasis_not_a_list() {
        assert_eq!(
            parse("*italic* text"),
            [Block::Paragraph(vec![Inline::Italic(vec![text("italic")]), text(" text")])]
        );
    }

    #[test]
    fn inline_markup_survives_inside_blocks() {
        assert_eq!(
            parse("> <@5> said **hi**"),
            [Block::Quote(vec![Block::Paragraph(vec![
                Inline::User(Snowflake(5)),
                text(" said "),
                Inline::Bold(vec![text("hi")]),
            ])])]
        );
    }

    #[test]
    fn hostile_input_never_panics() {
        for input in ["```", "``````", "> ", ">>>", "<", "<<<>>>", "[", "[](", "***", "||||", "\\", "- ", "1. "] {
            let _ = parse(input);
        }
    }
}
