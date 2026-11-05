//! Turning what a person typed into a slash command's options. This is the
//! pure part of running a command: parsing numbers, resolving names to ids
//! and checking that required options are present.

use oxicord_model::{CommandOption, command::kind};
use serde_json::{Value, json};

/// What was entered for one option.
#[derive(Clone, Debug, PartialEq)]
pub enum Entry {
    Text(String),
    Bool(bool),
    /// Index into the option's choices.
    Choice(usize),
}

/// Looks a typed name up as an id, for users, roles and channels. Gets the
/// option kind and the text, returns the id as a string.
pub type Resolver<'a> = dyn Fn(u8, &str) -> Option<String> + 'a;

/// The answers to send, as (name, kind, value). Fails with a sentence for
/// the person to read.
pub fn answers(
    options: &[CommandOption],
    entries: &[Entry],
    resolve: &Resolver<'_>,
) -> Result<Vec<(String, u8, Value)>, String> {
    let mut out = Vec::new();
    for (option, entry) in options.iter().zip(entries) {
        let blank = matches!(entry, Entry::Text(t) if t.trim().is_empty());
        if blank {
            if option.required {
                return Err(format!("{} is required.", option.name));
            }
            continue;
        }
        let value = match (option.kind, entry) {
            (_, Entry::Choice(i)) => option
                .choices
                .get(*i)
                .map(|c| c.value.clone())
                .ok_or_else(|| format!("Pick a valid choice for {}.", option.name))?,
            (kind::BOOLEAN, Entry::Bool(b)) => json!(b),
            (kind::STRING, Entry::Text(t)) => json!(t),
            (kind::INTEGER, Entry::Text(t)) => json!(
                t.trim()
                    .parse::<i64>()
                    .map_err(|_| format!("{} must be a whole number.", option.name))?
            ),
            (kind::NUMBER, Entry::Text(t)) => json!(
                t.trim()
                    .parse::<f64>()
                    .map_err(|_| format!("{} must be a number.", option.name))?
            ),
            (kind::USER | kind::CHANNEL | kind::ROLE | kind::MENTIONABLE, Entry::Text(t)) => {
                let id = resolve(option.kind, t.trim())
                    .ok_or_else(|| format!("Couldn't find \"{}\" for {}.", t.trim(), option.name))?;
                json!(id)
            }
            (kind::ATTACHMENT, _) => {
                return Err(format!("{} needs a file, which isn't supported here yet.", option.name));
            }
            _ => return Err(format!("Can't read {}.", option.name)),
        };
        out.push((option.name.clone(), option.kind, value));
    }
    Ok(out)
}

/// The starting entry for an option, so a boolean begins as off and a text
/// box empty.
pub fn default_entry(option: &CommandOption) -> Entry {
    if option.kind == kind::BOOLEAN { Entry::Bool(false) } else { Entry::Text(String::new()) }
}

#[cfg(test)]
mod tests {
    use oxicord_model::Choice;

    use super::*;

    fn option(kind: u8, name: &str, required: bool) -> CommandOption {
        CommandOption {
            kind,
            name: name.into(),
            description: String::new(),
            required,
            choices: Vec::new(),
            options: Vec::new(),
            autocomplete: false,
        }
    }

    fn none(_: u8, _: &str) -> Option<String> {
        None
    }

    #[test]
    fn values_take_the_type_the_option_asks_for() {
        let options = vec![
            option(kind::STRING, "reason", true),
            option(kind::INTEGER, "days", false),
            option(kind::NUMBER, "ratio", false),
            option(kind::BOOLEAN, "silent", false),
        ];
        let entries = vec![
            Entry::Text("spam".into()),
            Entry::Text(" 7 ".into()),
            Entry::Text("0.5".into()),
            Entry::Bool(true),
        ];
        let got = answers(&options, &entries, &none).unwrap();
        assert_eq!(got[0].2, json!("spam"));
        assert_eq!(got[1].2, json!(7));
        assert_eq!(got[2].2, json!(0.5));
        assert_eq!(got[3].2, json!(true));
    }

    #[test]
    fn blank_optional_options_are_left_out_and_required_ones_refuse() {
        let options = vec![option(kind::STRING, "a", true), option(kind::STRING, "b", false)];
        assert_eq!(answers(&options, &[Entry::Text("x".into()), Entry::Text(String::new())], &none).unwrap().len(), 1);
        let err = answers(&options, &[Entry::Text("  ".into()), Entry::Text(String::new())], &none).unwrap_err();
        assert_eq!(err, "a is required.");
    }

    #[test]
    fn bad_numbers_say_which_option() {
        let options = vec![option(kind::INTEGER, "days", true)];
        let err = answers(&options, &[Entry::Text("seven".into())], &none).unwrap_err();
        assert_eq!(err, "days must be a whole number.");
    }

    #[test]
    fn names_are_resolved_to_ids() {
        let options = vec![option(kind::USER, "target", true)];
        let resolve = |k: u8, text: &str| (k == kind::USER && text == "ada").then(|| "42".to_owned());
        let got = answers(&options, &[Entry::Text("ada".into())], &resolve).unwrap();
        assert_eq!(got[0].2, json!("42"));
        let err = answers(&options, &[Entry::Text("zed".into())], &resolve).unwrap_err();
        assert!(err.contains("zed"), "{err}");
    }

    #[test]
    fn a_choice_sends_its_value_not_its_label() {
        let mut o = option(kind::STRING, "mode", true);
        o.choices = vec![Choice { name: "Fast".into(), value: json!("f") }, Choice { name: "Slow".into(), value: json!("s") }];
        let got = answers(&[o], &[Entry::Choice(1)], &none).unwrap();
        assert_eq!(got[0].2, json!("s"));
    }

    #[test]
    fn attachments_are_refused_clearly() {
        let options = vec![option(kind::ATTACHMENT, "file", true)];
        let err = answers(&options, &[Entry::Text("x".into())], &none).unwrap_err();
        assert!(err.contains("file"), "{err}");
    }
}
