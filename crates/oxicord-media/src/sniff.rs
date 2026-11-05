/// The image formats we expect Discord's CDN to hand back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageKind {
    Png,
    Jpeg,
    Gif,
    Webp,
}

/// Identifies an image by its first bytes. The Content-Type header isn't
/// trusted, a proxy in the middle can get it wrong.
pub fn sniff(bytes: &[u8]) -> Option<ImageKind> {
    match bytes {
        [0x89, b'P', b'N', b'G', ..] => Some(ImageKind::Png),
        [0xff, 0xd8, 0xff, ..] => Some(ImageKind::Jpeg),
        [b'G', b'I', b'F', b'8', ..] => Some(ImageKind::Gif),
        [b'R', b'I', b'F', b'F', _, _, _, _, b'W', b'E', b'B', b'P', ..] => Some(ImageKind::Webp),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_the_common_formats() {
        assert_eq!(sniff(b"\x89PNG\r\n\x1a\n...."), Some(ImageKind::Png));
        assert_eq!(sniff(&[0xff, 0xd8, 0xff, 0xe0, 0]), Some(ImageKind::Jpeg));
        assert_eq!(sniff(b"GIF89a...."), Some(ImageKind::Gif));
        assert_eq!(sniff(b"RIFF\x10\0\0\0WEBPVP8 "), Some(ImageKind::Webp));
    }

    #[test]
    fn rejects_everything_else() {
        assert_eq!(sniff(b"<html>nope</html>"), None);
        assert_eq!(sniff(b"RIFF\x10\0\0\0WAVEfmt "), None);
        assert_eq!(sniff(b""), None);
    }
}
