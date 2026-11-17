//! A Rune-style headless preview for Oxicord views. No gateway, login or
//! network work: it opens one offscreen window with one scene, then writes a
//! PNG and a JSON element tree for an agent to inspect.
//!
//! ```text
//! cargo run -p oxicord-preview -- \
//!   --scene interaction-modal --size 1920x1080 \
//!   --snap screenshots/port/preview.png --tree screenshots/port/preview.json
//! ```

use std::{borrow::Cow, path::PathBuf, sync::Arc};

use anyhow::{Result, bail};
use gpui_kit::{
    App, AssetSource, Bounds, HeadlessAppContext, SharedString,
    Window, WindowBounds, WindowOptions, px, size,
    assets::Assets as KitAssets,
    base::test_support,
    component::{Theme, ThemeMode},
};
use oxicord_ui::theme;

#[derive(Clone, Debug)]
struct Args {
    scene: String,
    width: u32,
    height: u32,
    snap: Option<PathBuf>,
    tree: Option<PathBuf>,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            scene: "interaction-modal".into(),
            width: 1920,
            height: 1080,
            snap: None,
            tree: None,
        }
    }
}

fn parse() -> Result<Args> {
    let mut args = Args::default();
    let mut it = std::env::args().skip(1);
    while let Some(flag) = it.next() {
        let mut value = || -> Result<String> {
            it.next().ok_or_else(|| anyhow::anyhow!("{flag} needs a value"))
        };
        match flag.as_str() {
            "--scene" => args.scene = value()?,
            "--size" => {
                let v = value()?;
                let (w, h) = v.split_once('x').ok_or_else(|| anyhow::anyhow!("--size is WxH"))?;
                args.width = w.parse()?;
                args.height = h.parse()?;
            }
            "--snap" => args.snap = Some(value()?.into()),
            "--tree" => args.tree = Some(value()?.into()),
            "-h" | "--help" => {
                print_help();
                std::process::exit(0);
            }
            other => bail!("unknown flag {other}; try --help"),
        }
    }
    Ok(args)
}

fn print_help() {
    eprintln!(
        "Oxicord headless preview\n\
         --scene NAME   interaction-modal or tip\n\
         --size WxH     window size, default 1920x1080\n\
         --snap FILE    write a PNG of the frame\n\
         --tree FILE    write the element tree as JSON"
    );
}

/// Discord's own icons and font, then the bundled catalog for everything else.
struct PreviewAssets;

impl AssetSource for PreviewAssets {
    fn load(&self, path: &str) -> gpui_kit::Result<Option<Cow<'static, [u8]>>> {
        let disk = std::path::Path::new("discord-assets").join(path);
        if disk.is_file() {
            return Ok(Some(Cow::Owned(std::fs::read(disk)?)));
        }
        KitAssets.load(path)
    }

    fn list(&self, path: &str) -> gpui_kit::Result<Vec<SharedString>> {
        KitAssets.list(path)
    }
}

fn load_fonts(cx: &mut App) {
    let mut fonts = Vec::new();
    if let Ok(dir) = std::fs::read_dir("discord-assets/fonts") {
        for entry in dir.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "ttf")
                && let Ok(bytes) = std::fs::read(path)
            {
                fonts.push(Cow::Owned(bytes));
            }
        }
    }
    if fonts.is_empty() {
        return;
    }
    if cx.text_system().add_fonts(fonts).is_ok() {
        theme::gg_sans_loaded();
        Theme::global_mut(cx).font_family = theme::font_family().into();
    }
}

fn apply_accent(cx: &mut App) {
    let c = |value: u32| gpui_kit::Hsla::from(gpui_kit::rgb(value));
    let colors = &mut Theme::global_mut(cx).colors;
    colors.primary = c(theme::ACCENT);
    colors.primary_hover = c(0x4452bb);
    colors.primary_active = c(0x3a48a3);
    colors.primary_foreground = c(theme::TEXT_BRIGHT);
    colors.button_primary = colors.primary;
    colors.button_primary_hover = colors.primary_hover;
    colors.button_primary_active = colors.primary_active;
    colors.button_primary_foreground = colors.primary_foreground;
    colors.secondary = c(0x121213);
    colors.secondary_hover = c(0x1e1e20);
    colors.secondary_active = c(0x242426);
    colors.secondary_foreground = c(theme::TEXT_BRIGHT);
    colors.button_secondary = colors.secondary;
    colors.button_secondary_hover = colors.secondary_hover;
    colors.button_secondary_active = colors.secondary_active;
    colors.button_secondary_foreground = colors.secondary_foreground;
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
    colors.scrollbar_thumb = c(0x595a63);
    colors.scrollbar_thumb_hover = c(0x6c6d76);
}

fn element_tree(window: &Window) -> serde_json::Value {
    let elements = test_support::snapshots(window)
        .into_iter()
        .map(|element| {
            let bounds = element.bounds();
            serde_json::json!({
                "path": format!("{:?}", element.path()),
                "x": bounds.origin.x.as_f32(),
                "y": bounds.origin.y.as_f32(),
                "w": bounds.size.width.as_f32(),
                "h": bounds.size.height.as_f32(),
                "role": element.role().map(|role| format!("{role:?}")),
                "label": element.label(),
                "value": element.value(),
                "placeholder": element.placeholder(),
                "checked": element.checked(),
                "disabled": element.disabled(),
                "visible": element.visible(),
            })
        })
        .collect::<Vec<_>>();
    let texts = test_support::texts(window)
        .into_iter()
        .map(|text| {
            let bounds = text.bounds();
            serde_json::json!({
                "path": format!("{:?}", text.path()),
                "text": text.text(),
                "x": bounds.origin.x.as_f32(),
                "y": bounds.origin.y.as_f32(),
                "w": bounds.size.width.as_f32(),
                "h": bounds.size.height.as_f32(),
                "visible": text.visible(),
            })
        })
        .collect::<Vec<_>>();
    serde_json::json!({ "elements": elements, "texts": texts })
}

fn main() -> Result<()> {
    let args = parse()?;
    // GPUI has no built-in headless renderer on Windows. The WGPU one works
    // there, so always give the context a renderer that can be created here.
    // Windows' headless platform has no real text system. The WGPU cosmic
    // text system shapes the gg sans files and works with the WGPU renderer.
    let mut cx = HeadlessAppContext::with_platform(
        Arc::new(gpui_wgpu::CosmicTextSystem::new("sans-serif")),
        Arc::new(PreviewAssets),
        || {
            gpui_wgpu::WgpuHeadlessRenderer::new()
                .map(|renderer| Some(Box::new(renderer) as Box<dyn gpui_kit::PlatformHeadlessRenderer>))
        },
    );
    cx.update(|cx| {
        gpui_kit::init(cx);
        Theme::change(ThemeMode::Dark, None, cx);
        apply_accent(cx);
        load_fonts(cx);
    });

    let (width, height) = (args.width as f32, args.height as f32);
    let (handle, _) = cx
        .update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: Default::default(),
                        size: size(px(width), px(height)),
                    })),
                    focus: false,
                    show: false,
                    ..Default::default()
                },
                cx,
                |window, cx| oxicord_ui::preview::open_scene(&args.scene, window, cx),
            )
        })
        .map_err(|err| anyhow::anyhow!("could not open a preview window: {err}"))?;

    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
    })
    .map_err(|err| anyhow::anyhow!("could not build the preview frame: {err}"))?;
    // Text shaping is asynchronous: let the executor load the glyphs, then
    // build the frame again so their quads are in the scene we capture.
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
    })
    .map_err(|err| anyhow::anyhow!("could not render the preview frame: {err}"))?;

    if let Some(path) = &args.tree {
        let tree = cx
            .update_window(handle, |_, window, _| element_tree(window))
            .map_err(|err| anyhow::anyhow!("could not read the element tree: {err}"))?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, serde_json::to_string_pretty(&tree)?)?;
        eprintln!("wrote {} elements and {} texts to {}", tree["elements"].as_array().unwrap().len(), tree["texts"].as_array().unwrap().len(), path.display());
    }

    if let Some(path) = &args.snap {
        let image = cx
            .capture_screenshot(handle)
            .map_err(|err| anyhow::anyhow!("offscreen capture failed: {err}"))?
            .into_raw();
        let buffer = image::RgbaImage::from_raw(args.width, args.height, image)
            .ok_or_else(|| anyhow::anyhow!("the renderer returned a frame of the wrong size"))?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        buffer.save(path)?;
        eprintln!("wrote {}", path.display());
    }
    Ok(())
}
