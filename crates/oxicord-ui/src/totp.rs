//! What the authenticator app setup needs: a random key, the address an
//! authenticator scans, and the picture of it. The key and the address follow
//! the web client's module 464477.

use data_encoding::BASE32_NOPAD;
use qrcode::{Color, QrCode};

/// A new key: 20 random bytes as base32, which is 32 letters and digits.
pub fn new_secret() -> String {
    let mut bytes = [0u8; 20];
    // The system generator only fails when the machine has no entropy source,
    // and then there is no safe key to make.
    getrandom::fill(&mut bytes).expect("the system has a random number source");
    BASE32_NOPAD.encode(&bytes)
}

/// The key as the manual entry shows it: lower case in groups of four.
pub fn spaced(secret: &str) -> String {
    let lower = secret.to_lowercase();
    lower
        .as_bytes()
        .chunks(4)
        .map(|group| String::from_utf8_lossy(group).into_owned())
        .collect::<Vec<_>>()
        .join(" ")
}

/// The key with the spaces, dots and dashes taken out and in capitals, which
/// is how an authenticator wants it.
pub fn plain(secret: &str) -> String {
    secret
        .chars()
        .filter(|c| !c.is_whitespace() && !matches!(c, '.' | '_' | '-'))
        .collect::<String>()
        .to_uppercase()
}

/// What the QR code holds: `otpauth://totp/Discord:<account>?secret=..&issuer=Discord`.
pub fn otpauth_uri(account: &str, secret: &str) -> String {
    format!("otpauth://totp/Discord:{}?secret={}&issuer=Discord", escape(account), plain(secret))
}

/// JavaScript's `encodeURI`, which leaves `@` and the usual address
/// punctuation alone.
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        let keep = byte.is_ascii_alphanumeric() || b";,/?:@&=+$-_.!~*'()#".contains(&byte);
        if keep {
            out.push(char::from(byte));
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// A QR code as rows of dark modules, without the quiet zone around it.
pub struct Qr {
    pub size: usize,
    pub dark: Vec<bool>,
}

impl Qr {
    pub fn new(text: &str) -> Option<Self> {
        let code = QrCode::new(text.as_bytes()).ok()?;
        let size = code.width();
        let dark = code.to_colors().into_iter().map(|c| c == Color::Dark).collect();
        Some(Self { size, dark })
    }

    pub fn is_dark(&self, x: usize, y: usize) -> bool {
        self.dark[y * self.size + x]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_secret_is_32_base32_characters_and_never_repeats() {
        let a = new_secret();
        assert_eq!(a.len(), 32);
        assert!(a.chars().all(|c| matches!(c, 'A'..='Z' | '2'..='7')));
        assert_ne!(a, new_secret());
    }

    #[test]
    fn the_manual_entry_is_lower_case_in_groups_of_four() {
        assert_eq!(spaced("NJYGYR6PGR7SDKS6KEWQRPWX6WLASBWG"), "njyg yr6p gr7s dks6 kewq rpwx 6wla sbwg");
    }

    #[test]
    fn the_address_names_the_account_and_the_plain_key() {
        assert_eq!(
            otpauth_uri("me@example.com", "njyg yr6p-gr7s"),
            "otpauth://totp/Discord:me@example.com?secret=NJYGYR6PGR7S&issuer=Discord"
        );
        assert!(otpauth_uri("a b", "AAAA").contains("Discord:a%20b?"));
    }

    #[test]
    fn the_address_makes_a_square_code() {
        let qr = Qr::new(&otpauth_uri("me@example.com", &new_secret())).unwrap();
        assert!(qr.size >= 29 && qr.size % 4 == 1);
        assert_eq!(qr.dark.len(), qr.size * qr.size);
        // The top left finder pattern always has a dark corner.
        assert!(qr.is_dark(0, 0));
    }
}
