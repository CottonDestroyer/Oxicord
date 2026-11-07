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

/// A picture was clicked and should be shown full size.
pub struct ViewImage(pub String);

impl gpui_kit::EventEmitter<ViewImage> for Media {}

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
    /// The main color of each picture we have looked at, found once.
    dominant: HashMap<String, Option<u32>>,
}

impl Media {
    pub fn new(backend: Backend) -> Self {
        Self {
            backend,
            images: HashMap::new(),
            dominant: HashMap::new(),
        }
    }

    /// The main color of a picture, for backgrounds that take after it. None
    /// until the picture has loaded.
    pub fn dominant_color(&mut self, url: &str) -> Option<u32> {
        if let Some(found) = self.dominant.get(url) {
            return *found;
        }
        let image = self.get(url)?;
        let found = dominant_color_of(&image.bytes);
        self.dominant.insert(url.to_owned(), found);
        found
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

/// The color most of a picture is made of, as 0xrrggbb. Near white and near
/// black are skipped, so a flag on a white ground still gives its blue.
fn dominant_color_of(bytes: &[u8]) -> Option<u32> {
    let picture = image::load_from_memory(bytes).ok()?.thumbnail(48, 48).to_rgba8();
    // Sixteen steps per channel, each bucket remembering its own average.
    let mut buckets: HashMap<(u8, u8, u8), (u32, [u32; 3])> = HashMap::new();
    for pixel in picture.pixels() {
        let [r, g, b, a] = pixel.0;
        let bright = r > 235 && g > 235 && b > 235;
        let dark = r < 20 && g < 20 && b < 20;
        if a < 128 || bright || dark {
            continue;
        }
        let bucket = buckets.entry((r >> 4, g >> 4, b >> 4)).or_default();
        bucket.0 += 1;
        for (sum, value) in bucket.1.iter_mut().zip([r, g, b]) {
            *sum += u32::from(value);
        }
    }
    let (count, sums) = buckets.into_values().max_by_key(|(count, _)| *count)?;
    let [r, g, b] = sums.map(|sum| sum / count);
    Some((r << 16) | (g << 8) | b)
}

#[cfg(test)]
mod color_tests {
    use super::*;

    fn png(pixels: &[[u8; 4]]) -> Vec<u8> {
        let side = (pixels.len() as f64).sqrt() as u32;
        let picture = image::RgbaImage::from_fn(side, side, |x, y| image::Rgba(pixels[(y * side + x) as usize]));
        let mut bytes = Vec::new();
        picture
            .write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png)
            .unwrap();
        bytes
    }

    /// Resizing may be off by a step in a channel.
    fn close(found: Option<u32>, wanted: u32) -> bool {
        found.is_some_and(|found| {
            (0..3).all(|shift| {
                let (a, b) = ((found >> (shift * 8)) & 0xff, (wanted >> (shift * 8)) & 0xff);
                a.abs_diff(b) <= 2
            })
        })
    }

    #[test]
    fn a_flat_picture_gives_its_own_color() {
        assert!(close(dominant_color_of(&png(&[[200, 100, 50, 255]; 16])), 0xc86432));
    }

    #[test]
    fn white_ground_does_not_hide_the_color_on_it() {
        // Ten white pixels and six blue: the blue is what we want.
        let mut pixels = vec![[255, 255, 255, 255]; 10];
        pixels.extend([[13, 62, 177, 255]; 6]);
        assert!(close(dominant_color_of(&png(&pixels)), 0x0d3eb1));
    }

    #[test]
    fn a_blank_picture_has_no_color() {
        assert_eq!(dominant_color_of(&png(&[[255, 255, 255, 255]; 4])), None);
        assert_eq!(dominant_color_of(b"not a picture"), None);
    }
}
