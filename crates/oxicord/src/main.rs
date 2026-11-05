mod bridge;
mod credentials;
mod logging;

use gpui_kit::{
    AppContext as _, Bounds, TitlebarOptions, WindowBounds, WindowOptions, px, size,
    component::{Theme, ThemeMode},
};
use oxicord_ui::{AppView, theme};

/// The component library paints primary buttons white in dark mode. Bring
/// them in line with the rest of the palette.
fn apply_accent(cx: &mut gpui_kit::App) {
    let accent = |value: u32| gpui_kit::Hsla::from(gpui_kit::rgb(value));
    let colors = &mut Theme::global_mut(cx).colors;
    colors.primary = accent(theme::ACCENT);
    colors.primary_hover = accent(0x4752c4);
    colors.primary_active = accent(0x3c45a5);
    colors.primary_foreground = accent(theme::TEXT_BRIGHT);
    colors.button_primary = colors.primary;
    colors.button_primary_hover = colors.primary_hover;
    colors.button_primary_active = colors.primary_active;
    colors.button_primary_foreground = colors.primary_foreground;
}

fn main() {
    logging::init();

    let backend = bridge::start();
    let saved_token = credentials::load().map(|saved| saved.token);

    gpui_kit::application()
        .with_assets(gpui_kit::assets::AllAssets)
        .run(move |cx| {
            gpui_kit::init(cx);
            Theme::change(ThemeMode::Dark, None, cx);
            apply_accent(cx);

            // Without this the process lingers after the last window closes.
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();

            let bounds = Bounds::centered(None, size(px(1280.), px(800.)), cx);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(px(940.), px(500.))),
                titlebar: Some(TitlebarOptions {
                    title: Some("Oxicord".into()),
                    ..Default::default()
                }),
                ..Default::default()
            };

            gpui_kit::open_window(options, cx, move |window, cx| {
                cx.new(|cx| AppView::new(backend, saved_token, window, cx))
            })
            .expect("failed to open the main window");
            cx.activate(true);
        });
}
