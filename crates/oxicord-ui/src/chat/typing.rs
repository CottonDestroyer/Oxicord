/// The line under the message list while people are typing.
pub fn typing_text(names: &[String]) -> Option<String> {
    match names {
        [] => None,
        [one] => Some(format!("{one} is typing...")),
        [a, b] => Some(format!("{a} and {b} are typing...")),
        [a, b, c] => Some(format!("{a}, {b} and {c} are typing...")),
        _ => Some("Several people are typing...".to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn nobody_typing_shows_nothing() {
        assert_eq!(typing_text(&[]), None);
    }

    #[test]
    fn names_are_listed_up_to_three() {
        assert_eq!(typing_text(&names(&["Ada"])).unwrap(), "Ada is typing...");
        assert_eq!(typing_text(&names(&["Ada", "Bob"])).unwrap(), "Ada and Bob are typing...");
        assert_eq!(
            typing_text(&names(&["Ada", "Bob", "Cy"])).unwrap(),
            "Ada, Bob and Cy are typing..."
        );
    }

    #[test]
    fn a_crowd_is_summarized() {
        let crowd = names(&["a", "b", "c", "d"]);
        assert_eq!(typing_text(&crowd).unwrap(), "Several people are typing...");
    }
}
