//! The keys that can be the push to talk key. A key is named the way the
//! window names a pressed key, and watched by its Windows virtual key code,
//! since the app has to see it while another program has the focus.

/// The virtual key code for a key name, or none when it cannot be used.
pub fn virtual_key(name: &str) -> Option<u16> {
    let mut chars = name.chars();
    if let (Some(c), None) = (chars.next(), chars.next()) {
        return match c {
            'a'..='z' => Some(u16::from(c as u8 - b'a') + 0x41),
            '0'..='9' => Some(u16::from(c as u8 - b'0') + 0x30),
            ';' => Some(0xBA),
            '=' => Some(0xBB),
            ',' => Some(0xBC),
            '-' => Some(0xBD),
            '.' => Some(0xBE),
            '/' => Some(0xBF),
            '`' => Some(0xC0),
            '[' => Some(0xDB),
            '\\' => Some(0xDC),
            ']' => Some(0xDD),
            '\'' => Some(0xDE),
            _ => None,
        };
    }
    if let Some(number) = name.strip_prefix('f').and_then(|n| n.parse::<u16>().ok()) {
        return (1..=24).contains(&number).then(|| 0x6F + number);
    }
    match name {
        "space" => Some(0x20),
        "tab" => Some(0x09),
        "capslock" => Some(0x14),
        "pageup" => Some(0x21),
        "pagedown" => Some(0x22),
        "end" => Some(0x23),
        "home" => Some(0x24),
        "left" => Some(0x25),
        "up" => Some(0x26),
        "right" => Some(0x27),
        "down" => Some(0x28),
        "insert" => Some(0x2D),
        "delete" => Some(0x2E),
        _ => None,
    }
}

/// How the key reads on a button, like `V`, `F8` or `Space`.
pub fn label(name: &str) -> String {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letters_digits_and_function_keys_map_to_their_codes() {
        assert_eq!(virtual_key("a"), Some(0x41));
        assert_eq!(virtual_key("z"), Some(0x5A));
        assert_eq!(virtual_key("0"), Some(0x30));
        assert_eq!(virtual_key("f1"), Some(0x70));
        assert_eq!(virtual_key("f12"), Some(0x7B));
        assert_eq!(virtual_key("space"), Some(0x20));
        assert_eq!(virtual_key("`"), Some(0xC0));
    }

    #[test]
    fn unknown_or_out_of_range_keys_are_refused() {
        assert_eq!(virtual_key(""), None);
        assert_eq!(virtual_key("f0"), None);
        assert_eq!(virtual_key("f25"), None);
        assert_eq!(virtual_key("escape"), None);
        assert_eq!(virtual_key("é"), None);
    }

    #[test]
    fn a_key_reads_with_a_capital() {
        assert_eq!(label("v"), "V");
        assert_eq!(label("f8"), "F8");
        assert_eq!(label("space"), "Space");
        assert_eq!(label(""), "");
    }
}
