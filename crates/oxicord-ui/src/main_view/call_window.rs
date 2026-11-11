//! The call in a window of its own, opened by the pop out button on the call
//! bar. It shows the same tiles and the main controls, and closing it puts
//! the call back in the main window.

use gpui_kit::{
    AnyWindowHandle, App, AppContext as _, Bounds, Context, Entity, InteractiveElement as _, IntoElement, ParentElement as _,
    Render, StatefulInteractiveElement as _, Styled as _, Subscription, TitlebarOptions, Window, WindowBounds,
    WindowOptions, div, px, rgb, size,
};

use super::{VoiceItem, call_tile, call_tile_size, channel_item};
use crate::{backend::Backend, icons::icon, media::Media, session::Session, theme};

const GAP: f32 = 8.;
const BAR_HEIGHT: f32 = 66.;

pub(super) struct CallWindow {
    session: Entity<Session>,
    media: Entity<Media>,
    backend: Backend,
    _session: Subscription,
}

/// Opens the window. The view it holds is returned so its release, which is
/// the window closing, can be watched.
pub(super) fn open(
    session: Entity<Session>,
    media: Entity<Media>,
    backend: Backend,
    cx: &mut App,
) -> Option<(AnyWindowHandle, Entity<CallWindow>)> {
    let bounds = Bounds::centered(None, size(px(960.), px(600.)), cx);
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        window_min_size: Some(size(px(480.), px(320.))),
        titlebar: Some(TitlebarOptions { title: Some("Oxicord".into()), ..Default::default() }),
        ..Default::default()
    };
    gpui_kit::open_window(options, cx, move |window, cx| {
        cx.new(|cx| CallWindow::new(session, media, backend, window, cx))
    })
    .ok()
}

impl CallWindow {
    fn new(
        session: Entity<Session>,
        media: Entity<Media>,
        backend: Backend,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        // Redraws with the call, and goes away when the call ends.
        let watch = cx.observe_in(&session, window, |_, session, window, cx| {
            if session.read(cx).voice.target.is_none() {
                window.remove_window();
            } else {
                cx.notify();
            }
        });
        Self { session, media, backend, _session: watch }
    }
}

impl Render for CallWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (people, call): (Vec<VoiceItem>, _) = {
            let session = self.session.read(cx);
            let store = &session.store;
            let people = session
                .voice
                .target
                .and_then(|(_, channel)| store.channel(channel))
                .map(|c| channel_item(store, c).voice)
                .unwrap_or_default();
            (people, session.voice.clone())
        };

        let viewport = window.viewport_size();
        let area_w = f32::from(viewport.width) - 2. * GAP;
        let area_h = f32::from(viewport.height) - BAR_HEIGHT - 2. * GAP;
        let tile_size = call_tile_size(people.len(), area_w, area_h, GAP);
        let tiles: Vec<_> = people
            .iter()
            .map(|person| call_tile(&self.media, person, call.speaking.contains(&person.user), tile_size, cx))
            .collect();

        let button = |id: &'static str, glyph: &'static str, color: u32| {
            div()
                .id(id)
                .size(px(40.))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(theme::RADIUS))
                .cursor_pointer()
                .child(icon(glyph, 20., color))
        };
        let mic_off = call.muted || call.deafened;
        let (mute_session, mute_backend) = (self.session.clone(), self.backend.clone());
        let (leave_session, leave_backend) = (self.session.clone(), self.backend.clone());

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(theme::BG_CHAT))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .justify_center()
                    .content_center()
                    .gap(px(GAP))
                    .p(px(GAP))
                    .children(tiles),
            )
            .child(
                div()
                    .relative()
                    .flex_none()
                    .h(px(BAR_HEIGHT))
                    .flex()
                    .items_start()
                    .justify_center()
                    .gap(px(12.))
                    .child(
                        div().flex().p(px(4.)).rounded(px(12.)).bg(rgb(theme::BG_PANEL)).child(
                            button("popout-mute", if mic_off { "mic-off" } else { "mic" }, if mic_off { theme::DANGER } else { theme::TEXT_BRIGHT })
                                .hover(|s| s.bg(rgb(theme::BG_HOVER)))
                                .on_click(move |_, _, cx| {
                                    mute_session.update(cx, |s, cx| s.toggle_voice_mute(&mute_backend, cx));
                                }),
                        ),
                    )
                    .child(
                        div()
                            .id("popout-hangup")
                            .w(px(66.))
                            .h(px(50.))
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(12.))
                            .bg(rgb(theme::BADGE))
                            .cursor_pointer()
                            .on_click(move |_, _, cx| {
                                leave_session.update(cx, |s, cx| s.leave_voice(&leave_backend, cx));
                            })
                            .child(icon("voice-hangup", 20., 0xffffff)),
                    )
                    // Closing the window brings the call back to the main one.
                    .child(
                        div()
                            .id("popout-return")
                            .absolute()
                            .right(px(16.))
                            .top(px(13.))
                            .size(px(24.))
                            .cursor_pointer()
                            .on_click(|_, window, _| window.remove_window())
                            .child(icon("minimize", 24., theme::TEXT_BRIGHT)),
                    ),
            )
    }
}
