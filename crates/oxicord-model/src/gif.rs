use serde_json::Value;

/// One result of the GIF picker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Gif {
    pub id: String,
    pub title: String,
    /// The page address, which is what gets posted: Discord turns it into an
    /// inline animation.
    pub url: String,
    /// A small still picture for the grid.
    pub preview: String,
    pub width: u32,
    pub height: u32,
}

impl Gif {
    pub fn from_value(value: &Value) -> Option<Self> {
        let text = |key: &str| value.get(key).and_then(Value::as_str).map(str::to_owned);
        let number = |key: &str| value.get(key).and_then(Value::as_u64).and_then(|n| u32::try_from(n).ok());
        Some(Self {
            id: text("id")?,
            title: text("title").unwrap_or_default(),
            url: text("url")?,
            // The animated copy comes first: the still `preview` is in a format
            // we cannot decode (AVIF), while the first frame of a GIF is fine.
            preview: text("gif_src").or_else(|| text("preview"))?,
            width: number("width").unwrap_or(0),
            height: number("height").unwrap_or(0),
        })
    }

    /// Reads a list of results, leaving out any it cannot understand.
    pub fn list(value: &Value) -> Vec<Self> {
        value
            .as_array()
            .map(|items| items.iter().filter_map(Self::from_value).collect())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn results_read_and_junk_is_skipped() {
        let list = Gif::list(&json!([
            { "id": "1", "title": "cat", "url": "https://tenor.com/x", "preview": "https://media.tenor.com/p.png", "width": 200, "height": 100 },
            { "id": "2", "url": "https://tenor.com/y", "gif_src": "https://media.tenor.com/g.gif" },
            { "id": "3", "title": "no urls" },
            "junk"
        ]));
        assert_eq!(list.len(), 2);
        assert_eq!((list[0].width, list[0].height), (200, 100));
        assert_eq!(list[0].preview, "https://media.tenor.com/p.png");
        assert_eq!(list[1].preview, "https://media.tenor.com/g.gif");
        assert!(Gif::list(&json!({})).is_empty());
    }
}
