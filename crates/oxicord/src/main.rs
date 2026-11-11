// All builds are GUI apps, so Windows shouldn't open a console for any of them.
#![windows_subsystem = "windows"]

mod assets;
mod bridge;
mod credentials;
#[cfg(debug_assertions)]
mod logging;
// Only the debug build installs a subscriber.
#[cfg(not(debug_assertions))]
use tracing_subscriber as _;
mod hotkey;
mod settings;
mod voice;

use gpui_kit::{
    AppContext as _, Bounds, TitlebarOptions, WindowBounds, WindowOptions, point, px, size,
    component::{Theme, ThemeMode},
};
use oxicord_ui::{AppView, theme};

/// The component library ships its own dark palette. Bring the pieces we
/// use (buttons, inputs, scrollbars, popovers) in line with Midnight, using
/// the control tokens of the official client.
fn apply_accent(cx: &mut gpui_kit::App) {
    let c = |value: u32| gpui_kit::Hsla::from(gpui_kit::rgb(value));
    let colors = &mut Theme::global_mut(cx).colors;

    // control-primary
    colors.primary = c(theme::ACCENT);
    colors.primary_hover = c(0x4452bb);
    colors.primary_active = c(0x3a48a3);
    colors.primary_foreground = c(theme::TEXT_BRIGHT);
    colors.button_primary = colors.primary;
    colors.button_primary_hover = colors.primary_hover;
    colors.button_primary_active = colors.primary_active;
    colors.button_primary_foreground = colors.primary_foreground;

    // control-secondary, the translucent grey flattened onto black
    colors.secondary = c(0x121213);
    colors.secondary_hover = c(0x1e1e20);
    colors.secondary_active = c(0x242426);
    colors.secondary_foreground = c(theme::TEXT_BRIGHT);
    colors.button_secondary = colors.secondary;
    colors.button_secondary_hover = colors.secondary_hover;
    colors.button_secondary_active = colors.secondary_active;
    colors.button_secondary_foreground = colors.secondary_foreground;

    // control-critical-primary
    colors.danger = c(0xd22d39);
    colors.danger_hover = c(0xa9232e);
    colors.danger_active = c(0x971d28);
    colors.danger_foreground = c(theme::TEXT_BRIGHT);
    colors.button_danger = colors.danger;
    colors.button_danger_hover = colors.danger_hover;
    colors.button_danger_active = colors.danger_active;
    colors.button_danger_foreground = colors.danger_foreground;

    colors.muted = c(theme::BG_HOVER);
    colors.muted_foreground = c(theme::TEXT_FAINT);
    colors.popover = c(theme::BG_MENU);
    colors.popover_foreground = c(theme::TEXT);
    colors.link = c(0x2781e7);
    // scrollbar-thin-thumb
    colors.scrollbar_thumb = c(0x595a63);
    colors.scrollbar_thumb_hover = c(0x6c6d76);
}

/// Registers the gg sans files compiled in from `discord-assets/fonts`. A
/// build without that folder just leaves the system font in place.
fn load_fonts(cx: &mut gpui_kit::App) {
    let fonts = assets::fonts();
    if fonts.is_empty() {
        return;
    }
    match cx.text_system().add_fonts(fonts) {
        Ok(()) => {
            theme::gg_sans_loaded();
            Theme::global_mut(cx).font_family = theme::font_family().into();
        }
        Err(err) => tracing::warn!(?err, "could not register the embedded fonts"),
    }
}

fn main() {
    #[cfg(debug_assertions)]
    logging::init();

    let backend = bridge::start();
    let saved_token = credentials::load().map(|saved| saved.token);
    let settings = settings::load();

    gpui_kit::application()
        .with_assets(assets::Assets::new(gpui_kit::assets::AllAssets))
        .run(move |cx| {
            gpui_kit::init(cx);
            Theme::change(ThemeMode::Dark, None, cx);
            apply_accent(cx);
            load_fonts(cx);
            oxicord_ui::bind_keys(cx);

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

            let (main, _) = gpui_kit::open_window(options, cx, move |window, cx| {
                cx.new(|cx| AppView::new(backend, saved_token, settings, window, cx))
            })
            .expect("failed to open the main window");

            // Without this the process lingers after the main window closes,
            // as a popped out call would keep it alive.
            cx.on_window_closed(move |cx, _| {
                if !cx.windows().iter().any(|w| w.window_id() == main.window_id()) {
                    cx.quit();
                }
            })
            .detach();
            cx.activate(true);
        });
}
