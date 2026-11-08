//! The effect over a profile card: layers of animation that fall across it
//! when it opens. It is a view of its own, because while a layer plays the
//! picture asks for a repaint every frame and that should cost the little
//! view, not the whole window.

use std::time::{Duration, Instant};

use gpui_kit::{
    Context, Entity, ImageSource, InteractiveElement as _, IntoElement, ObjectFit, ParentElement as _, Render, Styled as _,
    StyledImage as _, Subscription, Task, Window, div, img, px,
};
use oxicord_model::ProfileEffect;
use oxicord_model::effect::INTRO_MS;

use crate::media::{Media, Playback};

/// A layer is 450 pixels across, as the effect is drawn for a card that wide.
const CANVAS_WIDTH: f32 = 450.;

/// Pixels across the frames are decoded at. The popout is 300 wide and the
/// full profile's left side 400, so this is a little sharper than the first
/// and not much blurrier than the second, for half the memory of full size.
const FRAME_WIDTH: u32 = 320;

pub struct EffectView {
    media: Entity<Media>,
    effect: ProfileEffect,
    /// How wide the card is, which the layers are scaled to.
    card_width: f32,
    /// When the effect began. It waits until every layer is ready, so a
    /// layer that plays once is not half over when its pictures arrive.
    started: Option<Instant>,
    /// When the next layer is due to appear, and the timer that wakes us.
    wake: Option<(i64, Task<()>)>,
    _media: Subscription,
}

impl EffectView {
    pub fn new(media: Entity<Media>, effect: ProfileEffect, card_width: f32, cx: &mut Context<Self>) -> Self {
        let watch = cx.observe(&media, |_, _, cx| cx.notify());
        Self { media, effect, card_width, started: None, wake: None, _media: watch }
    }

    /// Asks for every layer, and says whether all are settled. A layer that
    /// cannot be shown does not hold the rest back.
    fn ready(&mut self, cx: &mut Context<Self>) -> bool {
        let mut ready = true;
        for layer in &self.effect.layers {
            let hold = !layer.looping;
            let state = self.media.update(cx, |media, cx| media.animation(&layer.src, FRAME_WIDTH, hold, cx));
            ready &= !matches!(state, Playback::Loading);
        }
        ready
    }

    /// Calls for a repaint when the next layer is due. While a layer plays
    /// the repaints come by themselves, but a gap between two would not.
    fn wake_at(&mut self, next: Option<i64>, now: i64, cx: &mut Context<Self>) {
        let Some(next) = next else {
            self.wake = None;
            return;
        };
        if self.wake.as_ref().is_some_and(|(at, _)| *at == next) {
            return;
        }
        let wait = Duration::from_millis((next - now).max(1) as u64);
        let task = cx.spawn(async move |this, cx| {
            cx.background_executor().timer(wait).await;
            this.update(cx, |_, cx| cx.notify()).ok();
        });
        self.wake = Some((next, task));
    }
}

impl Render for EffectView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let scale = self.card_width / CANVAS_WIDTH;
        let root = div().absolute().top_0().left_0().size_full().overflow_hidden();

        // Anyone who turned motion off gets the one still picture.
        if cx.reduce_motion() {
            let still = self
                .effect
                .still
                .as_deref()
                .and_then(|url| self.media.update(cx, |media, _| media.get(url)));
            return root.children(still.map(|image| {
                img(ImageSource::Image(image)).w(px(self.card_width)).object_fit(ObjectFit::Contain)            }));
        }

        if self.started.is_none() {
            if !self.ready(cx) {
                return root;
            }
            self.started = Some(Instant::now());
        }
        let now = self.started.map_or(0, |at| at.elapsed().as_millis() as i64) - INTRO_MS;

        let mut shown = 0;
        let mut layers = Vec::new();
        // Each layer keeps its place in the effect as its id, so one that
        // goes away does not hand its frame over to the next.
        for (index, layer) in self.effect.layers.iter().enumerate().filter(|(_, l)| l.visible_at(now)) {
            shown += 1;
            // A layer that plays once keeps its last frame until it is taken down.
            let hold = !layer.looping;
            let Playback::Ready(frames) =
                self.media.update(cx, |media, cx| media.animation(&layer.src, FRAME_WIDTH, hold, cx))
            else {
                continue;
            };
            let size = frames.size(0);
            let width = self.card_width;
            let height = width * size.height.0 as f32 / size.width.0.max(1) as f32;
            layers.push(
                img(ImageSource::Render(frames))
                    .id(("effect-layer", index as u64))
                    .absolute()
                    .left(px(layer.x as f32 * scale))
                    .top(px(layer.y as f32 * scale))
                    .w(px(width))
                    .h(px(height))
                    .object_fit(ObjectFit::Fill),
            );
        }

        // With nothing on screen no picture asks for the next frame, so a
        // timer has to bring the next layer in.
        let next = if shown == 0 { self.effect.next_change(now) } else { None };
        self.wake_at(next, now, cx);

        root.children(layers)
    }
}
