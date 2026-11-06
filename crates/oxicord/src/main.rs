mod bridge;
mod credentials;
mod logging;
mod settings;

use gpui_kit::{
    AppContext as _, Bounds, TitlebarOptions, WindowBounds, WindowOptions, point, px, size,
    component::{Theme, ThemeMode},
};
use oxicord_ui::{AppView, theme};

/// The component library paints primary buttons white in dark mode. Bring
/// them in line with the rest of the palette.
fn apply_accent(cx: &mut gpui_kit::App) {
    let accent = |value: u32| gpui_kit::Hsla::from(gpui_kit::rgb(value));
    let colors = &mut Theme::global_mut(cx).colors;
    colors.primary = accent(theme::ACCENT);
    colors.primary_hover = accent(0x4452bb);
    colors.primary_active = accent(0x3a48a3);
    colors.primary_foreground = accent(theme::TEXT_BRIGHT);
    colors.button_primary = colors.primary;
    colors.button_primary_hover = colors.primary_hover;
    colors.button_primary_active = colors.primary_active;
    colors.button_primary_foreground = colors.primary_foreground;
}

/// Registers the gg sans files from a `fonts` folder next to the executable
/// or in the working directory. They are Discord's and not part of the
/// repository, so a missing folder just leaves the system font in place.
fn load_fonts(cx: &mut gpui_kit::App) {
    let beside_exe = std::env::current_exe().ok().and_then(|exe| Some(exe.parent()?.join("fonts")));
    let dirs = beside_exe.into_iter().chain([std::path::PathBuf::from("fonts")]);

    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        let fonts: Vec<_> = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "ttf"))
            .filter_map(|path| std::fs::read(path).ok())
            .map(std::borrow::Cow::Owned)
            .collect();
        if fonts.is_empty() {
            continue;
        }
        match cx.text_system().add_fonts(fonts) {
            Ok(()) => {
                theme::gg_sans_loaded();
                Theme::global_mut(cx).font_family = theme::font_family().into();
            }
            Err(err) => tracing::warn!(?err, "could not register the fonts in {}", dir.display()),
        }
        return;
    }
}

fn main() {
    logging::init();

    let backend = bridge::start();
    let saved_token = credentials::load().map(|saved| saved.token);
    let settings = settings::load();

    gpui_kit::application()
        .with_assets(gpui_kit::assets::AllAssets)
        .run(move |cx| {
            gpui_kit::init(cx);
            Theme::change(ThemeMode::Dark, None, cx);
            apply_accent(cx);
            load_fonts(cx);
            oxicord_ui::bind_keys(cx);

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
                // The app draws its own title bar, see oxicord_ui::title_bar. The
                // title still names the window in the taskbar.
                titlebar: Some(TitlebarOptions {
                    title: Some("Oxicord".into()),
                    appears_transparent: true,
                    traffic_light_position: Some(point(px(12.), px(12.))),
                }),
                ..Default::default()
            };

            gpui_kit::open_window(options, cx, move |window, cx| {
                cx.new(|cx| AppView::new(backend, saved_token, settings, window, cx))
            })
            .expect("failed to open the main window");
            cx.activate(true);
        });
}
