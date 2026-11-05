mod bridge;
mod credentials;
mod logging;

use gpui_kit::{
    AppContext as _, Bounds, TitlebarOptions, WindowBounds, WindowOptions, px, size,
    component::{Theme, ThemeMode},
};
use oxicord_ui::AppView;

fn main() {
    logging::init();

    let backend = bridge::start();
    let saved_token = credentials::load().map(|saved| saved.token);

    gpui_kit::application()
        .with_assets(gpui_kit::assets::AllAssets)
        .run(move |cx| {
            gpui_kit::init(cx);
            Theme::change(ThemeMode::Dark, None, cx);

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
