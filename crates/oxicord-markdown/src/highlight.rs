//! Syntax coloring for fenced code blocks. It is a small lexer, not a
//! parser: comments, strings, numbers, keywords, built in types and the
//! name that follows a definition keyword. That covers what highlight.js
//! colors in most languages, which is what Discord shows.

use std::ops::Range;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Token {
    Keyword,
    Type,
    String,
    Number,
    Comment,
    /// The name being defined: a function, class or similar.
    Title,
    /// A key, as in a JSON object or an HTML attribute.
    Attribute,
    /// Decorators, annotations and preprocessor lines.
    Meta,
}

struct Lang {
    line_comments: &'static [&'static str],
    block_comment: Option<(&'static str, &'static str)>,
    quotes: &'static [char],
    keywords: &'static [&'static str],
    types: &'static [&'static str],
    /// Words after which the next identifier is a definition's name.
    definers: &'static [&'static str],
    /// Characters that start a decorator or directive.
    meta: &'static [char],
    /// Strings followed by a colon are keys.
    keys: bool,
}

const NONE: &[&str] = &[];

const C_STYLE: &[&str] = &["//"];
const HASH: &[&str] = &["#"];

fn lang(name: &str) -> Option<Lang> {
    let name = name.to_ascii_lowercase();
    Some(match name.as_str() {
        "rust" | "rs" => Lang {
            line_comments: C_STYLE,
            block_comment: Some(("/*", "*/")),
            quotes: &['"'],
            keywords: &[
                "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern",
                "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub",
                "ref", "return", "self", "static", "struct", "super", "trait", "true", "type", "unsafe", "use",
                "where", "while",
            ],
            types: &[
                "i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16", "u32", "u64", "u128", "usize", "f32",
                "f64", "bool", "char", "str", "String", "Vec", "Option", "Result", "Box", "Self", "Some", "None",
                "Ok", "Err",
            ],
            definers: &["fn", "struct", "enum", "trait", "type", "mod", "impl"],
            meta: &['#'],
            keys: false,
        },
        "js" | "javascript" | "jsx" | "ts" | "typescript" | "tsx" | "node" => Lang {
            line_comments: C_STYLE,
            block_comment: Some(("/*", "*/")),
            quotes: &['"', '\'', '`'],
            keywords: &[
                "async", "await", "break", "case", "catch", "class", "const", "continue", "default", "delete", "do",
                "else", "export", "extends", "false", "finally", "for", "from", "function", "if", "import", "in",
                "instanceof", "let", "new", "null", "of", "return", "static", "super", "switch", "this", "throw",
                "true", "try", "typeof", "undefined", "var", "void", "while", "yield", "interface", "type", "enum",
                "implements", "public", "private", "protected", "readonly",
            ],
            types: &["string", "number", "boolean", "any", "unknown", "never", "Promise", "Array", "Map", "Set", "Object"],
            definers: &["function", "class", "interface", "type", "enum"],
            meta: &['@'],
            keys: false,
        },
        "py" | "python" => Lang {
            line_comments: HASH,
            block_comment: None,
            quotes: &['"', '\''],
            keywords: &[
                "and", "as", "assert", "async", "await", "break", "class", "continue", "def", "del", "elif", "else",
                "except", "False", "finally", "for", "from", "global", "if", "import", "in", "is", "lambda", "None",
                "nonlocal", "not", "or", "pass", "raise", "return", "True", "try", "while", "with", "yield",
            ],
            types: &["int", "str", "float", "bool", "list", "dict", "set", "tuple", "bytes", "self", "print", "len", "range"],
            definers: &["def", "class"],
            meta: &['@'],
            keys: false,
        },
        "java" | "kotlin" | "kt" | "csharp" | "cs" | "c#" | "scala" | "swift" | "dart" => Lang {
            line_comments: C_STYLE,
            block_comment: Some(("/*", "*/")),
            quotes: &['"', '\''],
            keywords: &[
                "abstract", "as", "break", "case", "catch", "class", "const", "continue", "default", "do", "else",
                "enum", "extends", "false", "final", "finally", "for", "fun", "func", "if", "implements", "import",
                "in", "interface", "internal", "is", "namespace", "new", "null", "object", "override", "package",
                "private", "protected", "public", "return", "static", "struct", "super", "switch", "this", "throw",
                "true", "try", "using", "val", "var", "void", "when", "while",
            ],
            types: &["int", "long", "double", "float", "boolean", "bool", "char", "byte", "string", "String", "Int", "Boolean", "List", "Map"],
            definers: &["class", "interface", "enum", "struct", "object", "fun", "func"],
            meta: &['@'],
            keys: false,
        },
        "c" | "cpp" | "c++" | "cc" | "h" | "hpp" => Lang {
            line_comments: C_STYLE,
            block_comment: Some(("/*", "*/")),
            quotes: &['"', '\''],
            keywords: &[
                "auto", "break", "case", "catch", "class", "const", "continue", "default", "delete", "do", "else",
                "enum", "extern", "false", "for", "goto", "if", "inline", "namespace", "new", "nullptr", "operator",
                "private", "protected", "public", "return", "sizeof", "static", "struct", "switch", "template",
                "this", "throw", "true", "try", "typedef", "union", "using", "virtual", "volatile", "while",
            ],
            types: &[
                "int", "long", "short", "double", "float", "char", "void", "bool", "unsigned", "signed", "size_t",
                "uint8_t", "uint16_t", "uint32_t", "uint64_t", "int8_t", "int16_t", "int32_t", "int64_t", "string",
            ],
            definers: &["class", "struct", "enum", "union", "namespace"],
            meta: &['#'],
            keys: false,
        },
        "go" | "golang" => Lang {
            line_comments: C_STYLE,
            block_comment: Some(("/*", "*/")),
            quotes: &['"', '\'', '`'],
            keywords: &[
                "break", "case", "chan", "const", "continue", "default", "defer", "else", "false", "for", "func",
                "go", "if", "import", "interface", "map", "nil", "package", "range", "return", "select", "struct",
                "switch", "true", "type", "var",
            ],
            types: &["int", "int8", "int16", "int32", "int64", "uint", "uint8", "uint32", "uint64", "float32", "float64", "string", "bool", "byte", "rune", "error"],
            definers: &["func", "type"],
            meta: NO_CHARS,
            keys: false,
        },
        "json" | "jsonc" | "json5" => Lang {
            line_comments: C_STYLE,
            block_comment: None,
            quotes: &['"'],
            keywords: &["true", "false", "null"],
            types: NONE,
            definers: NONE,
            meta: NO_CHARS,
            keys: true,
        },
        "sh" | "bash" | "shell" | "zsh" | "console" => Lang {
            line_comments: HASH,
            block_comment: None,
            quotes: &['"', '\''],
            keywords: &["if", "then", "else", "elif", "fi", "for", "while", "do", "done", "case", "esac", "function", "in", "return", "export", "local", "echo", "cd", "sudo"],
            types: NONE,
            definers: &["function"],
            meta: NO_CHARS,
            keys: false,
        },
        "sql" | "mysql" | "postgres" | "postgresql" => Lang {
            line_comments: &["--"],
            block_comment: Some(("/*", "*/")),
            quotes: &['\'', '"'],
            keywords: &[
                "select", "from", "where", "insert", "into", "values", "update", "set", "delete", "create", "table",
                "drop", "alter", "join", "left", "right", "inner", "outer", "on", "group", "by", "order", "having",
                "limit", "and", "or", "not", "null", "as", "distinct", "union", "index", "primary", "key", "foreign",
                "references", "default", "unique", "like", "in", "is", "between", "exists", "case", "when", "then", "else", "end",
                "SELECT", "FROM", "WHERE", "INSERT", "INTO", "VALUES", "UPDATE", "SET", "DELETE", "CREATE", "TABLE",
                "DROP", "ALTER", "JOIN", "LEFT", "RIGHT", "INNER", "OUTER", "ON", "GROUP", "BY", "ORDER", "HAVING",
                "LIMIT", "AND", "OR", "NOT", "NULL", "AS", "DISTINCT", "UNION", "INDEX", "PRIMARY", "KEY", "FOREIGN",
                "REFERENCES", "DEFAULT", "UNIQUE", "LIKE", "IN", "IS", "BETWEEN", "EXISTS", "CASE", "WHEN", "THEN", "ELSE", "END",
            ],
            types: &["int", "integer", "varchar", "text", "boolean", "date", "timestamp", "INT", "INTEGER", "VARCHAR", "TEXT", "BOOLEAN", "DATE", "TIMESTAMP"],
            definers: NONE,
            meta: NO_CHARS,
            keys: false,
        },
        "lua" => Lang {
            line_comments: &["--"],
            block_comment: Some(("--[[", "]]")),
            quotes: &['"', '\''],
            keywords: &[
                "and", "break", "do", "else", "elseif", "end", "false", "for", "function", "if", "in", "local", "nil",
                "not", "or", "repeat", "return", "then", "true", "until", "while",
            ],
            types: NONE,
            definers: &["function"],
            meta: NO_CHARS,
            keys: false,
        },
        "css" | "scss" | "less" => Lang {
            line_comments: NONE,
            block_comment: Some(("/*", "*/")),
            quotes: &['"', '\''],
            keywords: &["important"],
            types: NONE,
            definers: NONE,
            meta: &['@'],
            keys: false,
        },
        "yaml" | "yml" | "toml" | "ini" => Lang {
            line_comments: HASH,
            block_comment: None,
            quotes: &['"', '\''],
            keywords: &["true", "false", "null", "yes", "no"],
            types: NONE,
            definers: NONE,
            meta: NO_CHARS,
            keys: false,
        },
        _ => return None,
    })
}

const NO_CHARS: &[char] = &[];

/// Colored spans of `code` in byte ranges, in order. Anything not listed is
/// plain text. A language this file doesn't know yields nothing.
pub fn highlight(language: &str, code: &str) -> Vec<(Range<usize>, Token)> {
    let Some(lang) = lang(language) else { return Vec::new() };
    let bytes = code.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    // Set right after a definition keyword: the next word is the name.
    let mut expect_name = false;

    while i < bytes.len() {
        let rest = &code[i..];
        let c = rest.chars().next().unwrap_or(' ');

        if let Some(end) = comment_end(&lang, rest) {
            out.push((i..i + end, Token::Comment));
            i += end;
            continue;
        }
        if lang.quotes.contains(&c) {
            let end = string_end(rest, c);
            let key = lang.keys && rest[end..].trim_start_matches([' ', '\t']).starts_with(':');
            out.push((i..i + end, if key { Token::Attribute } else { Token::String }));
            i += end;
            expect_name = false;
            continue;
        }
        if lang.meta.contains(&c) && (i == 0 || bytes[i - 1] == b'\n' || c == '@' || c == '#') {
            let end = meta_end(rest);
            if end > 1 {
                out.push((i..i + end, Token::Meta));
                i += end;
                continue;
            }
        }
        if c.is_ascii_digit() {
            let end = rest
                .find(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_' || ch == '.'))
                .unwrap_or(rest.len());
            out.push((i..i + end, Token::Number));
            i += end;
            continue;
        }
        if c.is_alphabetic() || c == '_' || c == '$' {
            let end = rest
                .find(|ch: char| !(ch.is_alphanumeric() || ch == '_' || ch == '$'))
                .unwrap_or(rest.len());
            let word = &rest[..end];
            if expect_name {
                out.push((i..i + end, Token::Title));
                expect_name = false;
            } else if lang.keywords.contains(&word) {
                out.push((i..i + end, Token::Keyword));
                expect_name = lang.definers.contains(&word);
            } else if lang.types.contains(&word) {
                out.push((i..i + end, Token::Type));
            }
            i += end;
            continue;
        }
        if !c.is_whitespace() {
            expect_name = false;
        }
        i += c.len_utf8();
    }
    out
}

fn comment_end(lang: &Lang, rest: &str) -> Option<usize> {
    if let Some((open, close)) = lang.block_comment
        && rest.starts_with(open)
    {
        return Some(rest[open.len()..].find(close).map_or(rest.len(), |at| open.len() + at + close.len()));
    }
    lang.line_comments
        .iter()
        .any(|p| rest.starts_with(p))
        .then(|| rest.find('\n').unwrap_or(rest.len()))
}

/// Length of the string starting at `rest`, which begins with `quote`.
fn string_end(rest: &str, quote: char) -> usize {
    let mut escaped = false;
    for (at, ch) in rest.char_indices().skip(1) {
        if escaped {
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == quote {
            return at + ch.len_utf8();
        } else if ch == '\n' && quote != '`' {
            return at;
        }
    }
    rest.len()
}

/// A decorator runs to the end of its word, or to the closing bracket of a
/// `#[...]` attribute; a directive runs to the end of the line.
fn meta_end(rest: &str) -> usize {
    if let Some(inner) = rest.strip_prefix("#[") {
        return inner.find(']').map_or(rest.len(), |at| at + 3);
    }
    if rest.starts_with('#') {
        return rest.find('\n').unwrap_or(rest.len());
    }
    1 + rest[1..]
        .find(|ch: char| !(ch.is_alphanumeric() || ch == '_' || ch == '.'))
        .unwrap_or(rest.len() - 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spans<'a>(language: &str, code: &'a str) -> Vec<(&'a str, Token)> {
        highlight(language, code).into_iter().map(|(r, t)| (&code[r], t)).collect()
    }

    #[test]
    fn rust_comments_strings_numbers_and_definitions() {
        let found = spans("rust", "fn main() { let n = 42; // hi\n  \"s\" }");
        assert_eq!(
            found,
            [
                ("fn", Token::Keyword),
                ("main", Token::Title),
                ("let", Token::Keyword),
                ("42", Token::Number),
                ("// hi", Token::Comment),
                ("\"s\"", Token::String),
            ]
        );
    }

    #[test]
    fn a_keyword_inside_a_string_or_comment_stays_plain() {
        let found = spans("js", "const a = \"if\"; /* for */");
        assert_eq!(
            found,
            [("const", Token::Keyword), ("\"if\"", Token::String), ("/* for */", Token::Comment)]
        );
    }

    #[test]
    fn json_keys_differ_from_values() {
        let found = spans("json", "{\"a\": \"b\", \"n\": 1, \"t\": true}");
        assert_eq!(found[0], ("\"a\"", Token::Attribute));
        assert_eq!(found[1], ("\"b\"", Token::String));
        assert!(found.contains(&("1", Token::Number)));
        assert!(found.contains(&("true", Token::Keyword)));
    }

    #[test]
    fn python_decorators_and_hash_comments() {
        let found = spans("py", "@app.route\ndef f(): # note\n    return None");
        assert_eq!(found[0], ("@app.route", Token::Meta));
        assert!(found.contains(&("f", Token::Title)));
        assert!(found.contains(&("# note", Token::Comment)));
    }

    #[test]
    fn unknown_languages_and_unterminated_input_are_safe() {
        assert!(highlight("brainfuck", "+++").is_empty());
        let found = spans("rust", "let s = \"open");
        assert_eq!(found.last(), Some(&("\"open", Token::String)));
        let found = spans("c", "/* never closed");
        assert_eq!(found, [("/* never closed", Token::Comment)]);
    }

    #[test]
    fn ranges_land_on_char_boundaries_with_multibyte_text() {
        let code = "let \u{e9}t\u{e9} = \"\u{65e5}\u{672c}\"; // \u{1f600}";
        for (range, _) in highlight("rust", code) {
            assert!(code.is_char_boundary(range.start) && code.is_char_boundary(range.end));
        }
    }
}
