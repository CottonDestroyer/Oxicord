//! Attachments and embeds: pictures, file cards and link previews.

use gpui_kit::{
    AnyElement, App, Entity, ImageSource, InteractiveElement as _, IntoElement, ParentElement as _,
    SharedString, StatefulInteractiveElement as _, Styled as _, Window, div, img, px, rgb,
};
use oxicord_model::{Attachment, Embed, Message, embed::EmbedMedia};

use super::markdown::{Names, humanize, rich};
use crate::{
    icons::icon,
    media::{Media, ViewImage},
    theme,
};

/// Largest a picture is drawn.
const MAX_WIDTH: f32 = 400.;
const MAX_HEIGHT: f32 = 300.;
const EMBED_IMAGE_WIDTH: f32 = 400.;
const THUMBNAIL: f32 = 80.;

/// Scales `(width, height)` down to fit the box, keeping the proportions.
/// Never scales up, a small picture stays small.
pub fn fit(width: f32, height: f32, max_width: f32, max_height: f32) -> (f32, f32) {
    if width <= 0. || height <= 0. {
        return (max_width.min(THUMBNAIL), max_height.min(THUMBNAIL));
    }
    let scale = (max_width / width).min(max_height / height).min(1.);
    ((width * scale).round().max(1.), (height * scale).round().max(1.))
}

/// "512 B", "1.5 KB", "3.2 MB".
pub fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024. && unit < UNITS.len() - 1 {
        value /= 1024.;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// Asks Discord's media proxy for a smaller copy, which saves downloading a
/// multi-megabyte original to draw a 400 pixel preview.
pub(super) fn resized(url: &str, width: f32, height: f32) -> String {
    let joiner = if url.contains('?') { '&' } else { '?' };
    format!("{url}{joiner}format=webp&width={}&height={}", width as u32, height as u32)
}

/// A picture of a known size, with a placeholder of the same size while it
/// loads so the message doesn't change height when it arrives.
pub(super) fn picture(
    media: &Entity<Media>,
    url: &str,
    link: &str,
    size: (f32, f32),
    id: SharedString,
    cx: &mut App,
) -> AnyElement {
    let image = media.update(cx, |media, _| media.get(url));
    let (link, viewer) = (link.to_owned(), media.clone());
    let frame = div()
        .id(id)
        .w(px(size.0))
        .h(px(size.1))
        .flex_none()
        .rounded(px(8.))
        .overflow_hidden()
        .cursor_pointer()
        .on_click(move |_, _, cx| {
            viewer.update(cx, |_, cx| cx.emit(ViewImage(link.clone())));
        });

    match image {
        Some(image) => frame
            .child(img(ImageSource::Image(image)).size_full().rounded(px(8.)))
            .into_any_element(),
        None => frame.bg(rgb(theme::BG_SIDEBAR)).into_any_element(),
    }
}

pub fn render_attachments(
    message: &Message,
    media: &Entity<Media>,
    inline_images: bool,
    cx: &mut App,
) -> Option<AnyElement> {
    if message.attachments.is_empty() {
        return None;
    }
    let mut column = div().flex().flex_col().gap_1().mt_1();
    for attachment in &message.attachments {
        column = column.child(if inline_images && attachment.is_image() {
            image_attachment(attachment, message, media, cx)
        } else {
            file_card(attachment, message)
        });
    }
    Some(column.into_any_element())
}

fn image_attachment(
    attachment: &Attachment,
    message: &Message,
    media: &Entity<Media>,
    cx: &mut App,
) -> AnyElement {
    let (w, h) = fit(
        attachment.width.unwrap_or(0) as f32,
        attachment.height.unwrap_or(0) as f32,
        MAX_WIDTH,
        MAX_HEIGHT,
    );
    let source = attachment.proxy_url.as_deref().unwrap_or(&attachment.url);
    let id = SharedString::from(format!("attachment-{}-{}", message.id, attachment.id));
    picture(media, &resized(source, w, h), &attachment.url, (w, h), id, cx)
}

fn file_card(attachment: &Attachment, message: &Message) -> AnyElement {
    let url = attachment.url.clone();
    div()
        .id(SharedString::from(format!("file-{}-{}", message.id, attachment.id)))
        .flex()
        .items_center()
        .gap_3()
        .w(px(360.))
        .px_3()
        .py_2()
        .rounded(px(8.))
        .bg(rgb(theme::BG_SIDEBAR))
        .border_1()
        .border_color(rgb(theme::BORDER))
        .cursor_pointer()
        .hover(|s| s.bg(rgb(theme::BG_HOVER)))
        .on_click(move |_, _, cx| cx.open_url(&url))
        .child(icon("file", 28., theme::TEXT_MUTED))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(
                    div()
                        .truncate()
                        .text_color(rgb(super::markdown::LINK_COLOR))
                        .child(attachment.filename.clone()),
                )
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(rgb(theme::TEXT_MUTED))
                        .child(human_size(attachment.size)),
                ),
        )
        .child(icon("download", 18., theme::TEXT_MUTED))
        .into_any_element()
}

pub fn render_embeds(
    message: &Message,
    media: &Entity<Media>,
    link_images: bool,
    names: &Names,
    window: &mut Window,
    cx: &mut App,
) -> Option<AnyElement> {
    let visible: Vec<_> = message
        .embeds
        .iter()
        .enumerate()
        .filter(|(_, e)| !is_bare_image_link(e))
        .collect();
    // Picture links are the media; with those turned off only cards remain.
    let images: Vec<_> = message
        .embeds
        .iter()
        .filter(|e| link_images && is_bare_image_link(e))
        .collect();
    if visible.is_empty() && images.is_empty() {
        return None;
    }

    let mut column = div().flex().flex_col().gap_1().mt_1();
    for (ix, embed) in visible {
        column = column.child(embed_card(embed, message, ix, media, names, window, cx));
    }
    for (ix, embed) in images.into_iter().enumerate() {
        if let Some(media_ref) = embed.image.as_ref().or(embed.thumbnail.as_ref()) {
            column = column.child(embed_picture(media_ref, embed, message, ix + 100, media, cx));
        }
    }
    Some(column.into_any_element())
}

/// Pasting an image link makes an "image" embed, which is just the picture,
/// without a card around it.
fn is_bare_image_link(embed: &Embed) -> bool {
    embed.kind.as_deref() == Some("image")
}

fn embed_picture(
    picture_ref: &EmbedMedia,
    embed: &Embed,
    message: &Message,
    ix: usize,
    media: &Entity<Media>,
    cx: &mut App,
) -> AnyElement {
    let (w, h) = fit(
        picture_ref.width.unwrap_or(0) as f32,
        picture_ref.height.unwrap_or(0) as f32,
        MAX_WIDTH,
        MAX_HEIGHT,
    );
    let source = picture_ref
        .proxy_url
        .as_deref()
        .or(picture_ref.url.as_deref())
        .unwrap_or_default();
    let link = embed.url.as_deref().or(picture_ref.url.as_deref()).unwrap_or_default();
    let id = SharedString::from(format!("embed-image-{}-{ix}", message.id));
    picture(media, &resized(source, w, h), link, (w, h), id, cx)
}

fn embed_card(
    embed: &Embed,
    message: &Message,
    ix: usize,
    media: &Entity<Media>,
    names: &Names,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let accent = embed.color.unwrap_or(0x424246);

    let mut body = div().flex_1().min_w_0().flex().flex_col().gap(px(4.));

    if let Some(provider) = embed.provider.as_ref().and_then(|p| p.name.clone()) {
        body = body.child(small(provider));
    }
    if let Some(author) = &embed.author {
        body = body.child(
            div()
                .text_size(px(13.))
                .text_color(rgb(theme::TEXT_BRIGHT))
                .child(humanize(&author.name, names)),
        );
    }
    if let Some(title) = embed.title.clone() {
        let link = embed.url.clone();
        let heading = div()
            .id(SharedString::from(format!("embed-title-{}-{ix}", message.id)))
            .font_weight(gpui_kit::FontWeight::SEMIBOLD)
            .text_color(rgb(if link.is_some() { super::markdown::LINK_COLOR } else { theme::TEXT_BRIGHT }))
            .child(humanize(&title, names));
        body = body.child(match link {
            Some(url) => heading.cursor_pointer().on_click(move |_, _, cx| cx.open_url(&url)).into_any_element(),
            None => heading.into_any_element(),
        });
    }
    if let Some(description) = embed.description.clone().filter(|d| !d.is_empty()) {
        body = body.child(
            div()
                .text_size(px(14.))
                .text_color(rgb(theme::TEXT))
                .child(rich(&description, message.id.0 ^ ((ix as u64 + 1) << 48), media, names, window, cx)),
        );
    }
    for (field_ix, field) in embed.fields.iter().enumerate() {
        body = body.child(
            div()
                .flex()
                .flex_col()
                .child(
                    div()
                        .text_size(px(13.))
                        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                        .text_color(rgb(theme::TEXT_BRIGHT))
                        .child(humanize(&field.name, names)),
                )
                .child(div().text_size(px(14.)).child(rich(&field.value, message.id.0 ^ ((ix as u64 + 1) << 48) ^ ((field_ix as u64 + 1) << 40), media, names, window, cx))),
        );
    }
    if let Some(image) = embed.image.as_ref().filter(|i| i.proxy_url.is_some() || i.url.is_some()) {
        let (w, h) = fit(
            image.width.unwrap_or(0) as f32,
            image.height.unwrap_or(0) as f32,
            EMBED_IMAGE_WIDTH,
            MAX_HEIGHT,
        );
        let source = image.proxy_url.as_deref().or(image.url.as_deref()).unwrap_or_default();
        let link = embed.url.as_deref().or(image.url.as_deref()).unwrap_or_default();
        let id = SharedString::from(format!("embed-img-{}-{ix}", message.id));
        body = body.child(div().mt_1().child(picture(media, &resized(source, w, h), link, (w, h), id, cx)));
    }
    if let Some(footer) = &embed.footer {
        body = body.child(small(humanize(&footer.text, names)));
    }

    let mut card = div()
        .flex()
        .max_w(px(520.))
        .rounded(px(4.))
        .bg(rgb(theme::BG_SIDEBAR))
        .border_1()
        .border_color(rgb(theme::BORDER))
        .overflow_hidden()
        .child(div().w(px(4.)).flex_none().bg(rgb(accent)))
        .child(div().flex_1().min_w_0().flex().gap_3().p_3().child(body));

    if let Some(thumbnail) = embed
        .thumbnail
        .as_ref()
        .filter(|_| embed.image.is_none())
        .filter(|t| t.proxy_url.is_some() || t.url.is_some())
    {
        let source = thumbnail.proxy_url.as_deref().or(thumbnail.url.as_deref()).unwrap_or_default();
        let id = SharedString::from(format!("embed-thumb-{}-{ix}", message.id));
        let link = embed.url.as_deref().unwrap_or_default();
        card = card.child(div().p_3().child(picture(
            media,
            &resized(source, THUMBNAIL, THUMBNAIL),
            link,
            (THUMBNAIL, THUMBNAIL),
            id,
            cx,
        )));
    }
    card.into_any_element()
}

fn small(text: String) -> impl IntoElement {
    div().text_size(px(12.)).text_color(rgb(theme::TEXT_MUTED)).child(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn large_pictures_shrink_to_fit_keeping_their_shape() {
        assert_eq!(fit(1600., 1200., 400., 300.), (400., 300.));
        // Limited by height here, not width.
        assert_eq!(fit(1000., 2000., 400., 300.), (150., 300.));
        // Wide and short is limited by width.
        assert_eq!(fit(2000., 500., 400., 300.), (400., 100.));
    }

    #[test]
    fn small_pictures_are_not_blown_up() {
        assert_eq!(fit(120., 80., 400., 300.), (120., 80.));
    }

    #[test]
    fn unknown_dimensions_get_a_small_placeholder_box() {
        assert_eq!(fit(0., 0., 400., 300.), (80., 80.));
    }

    #[test]
    fn sizes_are_human_readable() {
        assert_eq!(human_size(0), "0 B");
        assert_eq!(human_size(512), "512 B");
        assert_eq!(human_size(1536), "1.5 KB");
        assert_eq!(human_size(3 * 1024 * 1024 + 200 * 1024), "3.2 MB");
    }

    #[test]
    fn resize_params_join_to_signed_urls_correctly() {
        assert_eq!(
            resized("https://media.discordapp.net/a.png", 400., 300.),
            "https://media.discordapp.net/a.png?format=webp&width=400&height=300"
        );
        assert_eq!(
            resized("https://media.discordapp.net/a.png?ex=1&hm=2", 400., 300.),
            "https://media.discordapp.net/a.png?ex=1&hm=2&format=webp&width=400&height=300"
        );
    }
}
