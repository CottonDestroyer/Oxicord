use std::{collections::HashMap, sync::Arc};

use gpui_kit::{
    App, Context, Entity, Image, ImageFormat, ImageSource, IntoElement, ParentElement as _,
    Styled as _, div, img, px, rgb,
};
use oxicord_media::{ImageKind, sniff};

use crate::{
    backend::{Backend, Command},
    theme,
};

enum Slot {
    Loading,
    Ready(Arc<Image>),
    Failed,
}

/// Pictures we have downloaded, by URL. Asking for one that isn't here
/// starts the download and returns nothing until it lands.
pub struct Media {
    backend: Backend,
    images: HashMap<String, Slot>,
}

impl Media {
    pub fn new(backend: Backend) -> Self {
        Self {
            backend,
            images: HashMap::new(),
        }
    }

    pub fn get(&mut self, url: &str) -> Option<Arc<Image>> {
        match self.images.get(url) {
            Some(Slot::Ready(image)) => Some(image.clone()),
            Some(Slot::Loading | Slot::Failed) => None,
            None => {
                self.images.insert(url.to_owned(), Slot::Loading);
                self.backend.send(Command::FetchImage(url.to_owned()));
                None
            }
        }
    }

    pub fn on_image(&mut self, url: String, bytes: Option<Vec<u8>>, cx: &mut Context<Self>) {
        let slot = match bytes.and_then(|b| sniff(&b).map(|kind| (kind, b))) {
            Some((kind, bytes)) => Slot::Ready(Arc::new(Image::from_bytes(format_of(kind), bytes))),
            None => Slot::Failed,
        };
        self.images.insert(url, slot);
        cx.notify();
    }
}

fn format_of(kind: ImageKind) -> ImageFormat {
    match kind {
        ImageKind::Png => ImageFormat::Png,
        ImageKind::Jpeg => ImageFormat::Jpeg,
        ImageKind::Gif => ImageFormat::Gif,
        ImageKind::Webp => ImageFormat::Webp,
    }
}

/// A round picture, or a colored circle with the first letter while the
/// picture loads or if it never does.
pub fn avatar(media: &Entity<Media>, url: &str, size: f32, name: &str, cx: &mut App) -> impl IntoElement {
    let image = media.update(cx, |media, _| media.get(url));
    let base = div().size(px(size)).flex_none().rounded_full().overflow_hidden();

    match image {
        Some(image) => base
            .child(img(ImageSource::Image(image)).size_full().rounded_full())
            .into_any_element(),
        None => base
            .flex()
            .items_center()
            .justify_center()
            .bg(rgb(color_for(name)))
            .text_color(rgb(theme::TEXT_BRIGHT))
            .text_size(px(size * 0.45))
            .child(initial(name))
            .into_any_element(),
    }
}

fn initial(name: &str) -> String {
    name.chars()
        .find(|c| c.is_alphanumeric())
        .map(|c| c.to_uppercase().to_string())
        .unwrap_or_else(|| "?".into())
}

/// The same name always gets the same color, so placeholders are stable.
fn color_for(name: &str) -> u32 {
    const PALETTE: [u32; 6] = [0x5865f2, 0x3ba55d, 0xeb459e, 0xf0b232, 0x4f8cff, 0xed6a43];
    let sum: usize = name.bytes().map(usize::from).sum();
    PALETTE[sum % PALETTE.len()]
}

/// Up to two letters for an icon-less guild: the first of each word.
pub fn acronym(name: &str) -> String {
    let letters: String = name
        .split_whitespace()
        .filter_map(|word| word.chars().next())
        .take(2)
        .collect();
    if letters.is_empty() { "?".into() } else { letters.to_uppercase() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acronym_takes_the_first_letter_of_up_to_two_words() {
        assert_eq!(acronym("rust lang community"), "RL");
        assert_eq!(acronym("Solo"), "S");
        assert_eq!(acronym("   "), "?");
    }

    #[test]
    fn initial_skips_leading_symbols() {
        assert_eq!(initial("-_-ada"), "A");
        assert_eq!(initial(""), "?");
    }

    #[test]
    fn placeholder_color_is_stable_per_name() {
        assert_eq!(color_for("ada"), color_for("ada"));
    }
}
