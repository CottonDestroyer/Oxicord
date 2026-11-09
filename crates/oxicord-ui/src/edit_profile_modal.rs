//! The Edit Profile modal, as the live client draws it: the editor sections
//! down the left (nameplate, avatar and decoration, banner colour, profile
//! effect and frame, display name style, bio) and the profile preview with
//! the widgets board on the right. Opened from the account page's Edit
//! Profile button and from our own profile card.

use gpui_kit::{
    AppContext as _, ClickEvent, Context, Entity, EventEmitter, FontWeight, InteractiveElement as _, IntoElement,
    prelude::FluentBuilder as _, ParentElement as _, PathPromptOptions, Render, StatefulInteractiveElement as _,
    Styled as _, StyledImage as _, Subscription, Window, div, img, px, rgb, rgba,
    component::input::{Input, InputState, Textarea, TextareaState},
};
use oxicord_model::{
    collectible::{EffectCatalog, FrameCatalog, Nameplate, NameplateCatalog},
    Snowflake, User,
};

use crate::{
    backend::{Backend, Command},
    icons::icon,
    media::{Media, avatar, decoration},
    profile::modal,
    session::Session,
    theme, AccountRequest,
};

/// Emitted when the window asks to go away.
pub struct CloseEditProfileModal;

/// Emitted when the nameplate tile is clicked; the window opens the Change
/// Nameplate dialog.
pub struct OpenNameplate;

/// Emitted when the decoration tile is clicked; the window opens the Change
/// Avatar Decoration dialog.
pub struct OpenDecoration;

/// Emitted when the profile effect tile is clicked; the window opens the
/// Change Profile Effect dialog.
pub struct OpenEffect;

/// Emitted when the profile frame tile is clicked; the window opens the
/// Change Profile Frame dialog.
pub struct OpenFrame;

impl EventEmitter<OpenNameplate> for EditProfileModal {}
impl EventEmitter<OpenDecoration> for EditProfileModal {}
impl EventEmitter<OpenEffect> for EditProfileModal {}
impl EventEmitter<OpenFrame> for EditProfileModal {}

impl EventEmitter<CloseEditProfileModal> for EditProfileModal {}

pub struct EditProfileModal {
    session: Entity<Session>,
    media: Entity<Media>,
    backend: Backend,
    bio: Entity<TextareaState>,
    pronouns: Entity<InputState>,
    /// The bio and pronouns are copied in once per open, so a refresh never
    /// overwrites what is being typed.
    filled: bool,
    applied: Option<String>,
    applied_decoration: Option<String>,
    applied_effect: Option<String>,
    applied_frame: Option<String>,
    catalog: NameplateCatalog,
    effect_catalog: EffectCatalog,
    frame_catalog: FrameCatalog,
    /// The board shows the widget library instead of the widgets themselves.
    widget_picker: bool,
    /// The board shows the Favorite Game widget editor.
    favorite_editor: bool,
    _subscriptions: Vec<Subscription>,
}

impl EditProfileModal {
    pub fn new(
        session: Entity<Session>,
        media: Entity<Media>,
        backend: Backend,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let bio = cx.new(|cx| TextareaState::new(window, cx).placeholder("Who are you?"));
        let pronouns = cx.new(|cx| InputState::new(window, cx).placeholder("Add your pronouns"));
        let _subscriptions = vec![cx.observe(&session, |_, _, cx| cx.notify())];
        Self {
            session,
            media,
            backend,
            bio,
            pronouns,
            filled: false,
            applied: None,
            applied_decoration: None,
            applied_effect: None,
            applied_frame: None,
            catalog: NameplateCatalog::bundled(),
            effect_catalog: EffectCatalog::bundled(),
            frame_catalog: FrameCatalog::bundled(),
            widget_picker: false,
            favorite_editor: false,
            _subscriptions,
        }
    }

    /// Fills the editors from the own profile the first time the modal is
    /// opened, and makes sure that profile is on its way.
    pub fn opened(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.filled = false;
        self.widget_picker = false;
        self.favorite_editor = false;
        let me = self.session.read(cx).store.me().cloned();
        if let Some(me) = &me
            && !self.session.read(cx).profiles.contains_key(&me.id)
        {
            self.backend.send(Command::LoadProfile { user: me.id, guild: None });
        }
        self.applied_decoration = me.as_ref().and_then(|m| m.decoration()).map(|sku| sku.to_string());
        let _ = window;
    }

    pub fn applied_nameplate(&self) -> Option<String> {
        self.applied.clone()
    }

    /// The nameplate the editor shows and the dialog starts from.
    pub fn set_applied(&mut self, applied: Option<String>, cx: &mut Context<Self>) {
        self.applied = applied;
        cx.notify();
    }

    pub fn applied_decoration(&self) -> Option<String> {
        self.applied_decoration.clone()
    }

    /// The decoration the editor shows and the dialog starts from.
    pub fn set_applied_decoration(&mut self, applied: Option<String>, cx: &mut Context<Self>) {
        self.applied_decoration = applied;
        cx.notify();
    }

    pub fn applied_effect(&self) -> Option<String> {
        self.applied_effect.clone()
    }

    /// The effect the editor shows and the dialog starts from.
    pub fn set_applied_effect(&mut self, applied: Option<String>, cx: &mut Context<Self>) {
        self.applied_effect = applied;
        cx.notify();
    }

    pub fn applied_frame(&self) -> Option<String> {
        self.applied_frame.clone()
    }

    /// The frame the editor shows and the dialog starts from.
    pub fn set_applied_frame(&mut self, applied: Option<String>, cx: &mut Context<Self>) {
        self.applied_frame = applied;
        cx.notify();
    }

    fn fill_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.filled {
            return;
        }
        let Some(me) = self.session.read(cx).store.me().map(|m| (m.id, m.id)) else {
            return;
        };
        let Some(profile) = self.session.read(cx).profiles.get(&me.0) else {
            return;
        };
        let (bio, pronouns) = (profile.bio.clone(), profile.pronouns.clone());
        self.applied_effect = profile.effect.map(|sku| sku.to_string());
        self.applied_frame = profile.frame.map(|sku| sku.to_string());
        self.bio.update(cx, |i, cx| i.set_value(bio, window, cx));
        self.pronouns.update(cx, |i, cx| i.set_value(pronouns, window, cx));
        self.filled = true;
    }

    /// Own profile details, falling back to the store while details load.
    fn me(&self, cx: &gpui_kit::App) -> Option<(User, Option<u32>, Option<String>, String)> {
        let session = self.session.read(cx);
        let me = session.store.me()?;
        let details = session.account.details.ready().cloned();
        let profile = session.profiles.get(&me.id);
        let accent = details.as_ref().and_then(|d| d.accent_color).or(profile.and_then(|p| p.accent_color));
        let banner = details
            .as_ref()
            .and_then(|d| d.banner.clone())
            .map(|h| oxicord_media::cdn::user_banner(me.id, &h, 600))
            .or_else(|| profile.and_then(|p| p.banner.clone()).map(|h| oxicord_media::cdn::user_banner(me.id, &h, 600)));
        Some((me.clone(), accent, banner, profile.map(|p| p.bio.clone()).unwrap_or_default()))
    }

    fn upload_banner(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        let chosen = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: None,
        });
        let backend = self.backend.clone();
        let session = self.session.clone();
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = chosen.await
                && let Some(path) = paths.first()
                && let Ok(data) = std::fs::read(path)
            {
                let mime = mime_for(path);
                let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &data);
                let uri = format!("data:{mime};base64,{b64}");
                this.update(cx, |_, cx| {
                    session.update(cx, |s, cx| {
                        s.request_account(&backend, AccountRequest::SetBanner(Some(uri)), cx);
                    });
                })
                .ok();
            }
        })
        .detach();
    }

    fn upload_avatar(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        let chosen = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: None,
        });
        let backend = self.backend.clone();
        let session = self.session.clone();
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = chosen.await
                && let Some(path) = paths.first()
                && let Ok(data) = std::fs::read(path)
            {
                let mime = mime_for(path);
                let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &data);
                let uri = format!("data:{mime};base64,{b64}");
                this.update(cx, |_, cx| {
                    session.update(cx, |s, cx| {
                        s.request_account(&backend, AccountRequest::SetAvatar(Some(uri)), cx);
                    });
                })
                .ok();
            }
        })
        .detach();
    }

    /// A labelled tile, the editor's unit: 80 by 80, rounded, brighter on
    /// hover, with a round plus badge in its corner or centre.
    fn tile(
        &self,
        id: &'static str,
        content: impl IntoElement,
        plus_centered: bool,
        on_click: Option<ClickRun>,
    ) -> gpui_kit::AnyElement {
        let mut tile = div()
            .id(id)
            .w(px(99.))
            .h(px(99.))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(theme::RADIUS))
            .bg(rgb(theme::BG_ACTIVE))
            .cursor_pointer()
            .hover(|s| s.bg(rgb(theme::BG_HOVER)))
            .relative()
            .child(content.into_any_element());
        if let Some(run) = on_click {
            tile = tile.on_click(run);
        }
        let plus = div()
            .absolute()
            .when(!plus_centered, |d| d.right(px(4.)).bottom(px(4.)))
            .size(px(32.))
            .flex()
            .items_center()
            .justify_center()
            .rounded_full()
            .bg(rgb(theme::BG_MENU))
            .border_1()
            .border_color(rgb(theme::BORDER))
            .child(icon("plus", 14., theme::TEXT_BRIGHT));
        tile.child(plus).into_any_element()
    }

    /// A section of the editor column: its label over the content.
    fn section(label: &'static str, content: gpui_kit::AnyElement) -> gpui_kit::AnyElement {
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .text_size(px(14.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(theme::TEXT_BRIGHT))
                    .child(label),
            )
            .child(content)
            .into_any_element()
    }
}

pub(crate) fn palette_color(palette: &str) -> u32 {
    match palette {
        "purple" => 0x6a4bd6,
        "violet" => 0x7d3fb0,
        "blue" => 0x3b6fd4,
        "silver" => 0x8e94a3,
        "ember" => 0x8a4a2a,
        "abyss" => 0x2b3350,
        _ => theme::BG_ACTIVE,
    }
}

/// The row the Change Nameplate dialog and the profile card draw for a
/// nameplate: the palette behind a small avatar and the name bar.
pub(crate) fn nameplate_row(
    item: &Nameplate,
    avatar_url: &str,
    media: &Entity<Media>,
    height: f32,
    color: &mut gpui_kit::App,
) -> gpui_kit::AnyElement {
    let avatar_el = avatar(media, avatar_url, height * 0.4, "", color);
    div()
        .w_full()
        .h(px(height))
        .flex_none()
        .flex()
        .items_center()
        .gap(px(8.))
        .px(px(8.))
        .rounded(px(4.))
        .bg(rgb(palette_color(&item.palette)))
        .child(avatar_el)
        .child(div().flex_1().h(px(10.)).rounded_full().bg(rgba(0xffff_ff40)))
        .when(item.locked, |d| d.child(icon("lock", 10., theme::TEXT_BRIGHT)))
        .into_any_element()
}

type ClickRun = Box<dyn Fn(&ClickEvent, &mut Window, &mut gpui_kit::App) + 'static>;

fn mime_for(path: &std::path::Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).map(|e| e.to_lowercase()).as_deref() {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        _ => "application/octet-stream",
    }
}

impl Render for EditProfileModal {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.fill_inputs(window, cx);
        let Some((me, accent, banner, profile_bio)) = self.me(cx) else {
            return div().into_any_element();
        };
        let name = me.display_name().to_owned();
        let avatar_url = oxicord_media::cdn::user_avatar(me.id, me.avatar.as_deref(), 128);

        // Editor column
        let applied_nameplate = self.applied.as_deref().and_then(|sku| self.catalog.find(sku));
        let nameplate_body = match applied_nameplate {
            Some(item) => nameplate_row(item, &avatar_url, &self.media, 48., cx),
            None => div()
                .h(px(48.))
                .w_full()
                .flex()
                .items_center()
                .gap_2()
                .rounded(px(theme::RADIUS))
                .bg(rgb(theme::BG_ACTIVE))
                .px(px(12.))
                .child(icon("tag-wave", 20., theme::TEXT_MUTED))
                .child(
                    div()
                        .flex_1()
                        .h(px(16.))
                        .rounded_full()
                        .bg(rgb(theme::BG_HOVER)),
                )
                .child(icon("circle-plus", 20., theme::TEXT_MUTED))
                .into_any_element(),
        };
        let nameplate = Self::section(
            "Nameplate",
            div()
                .id("edit-nameplate")
                .w_full()
                .cursor_pointer()
                .on_click(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(OpenNameplate)))
                .child(nameplate_body)
                .into_any_element(),
        );

        let avatar_click = cx.listener(Self::upload_avatar);
        let avatar_el = avatar(&self.media, &avatar_url, 88., &name, cx);
        let avatar_tile = self.tile("edit-avatar", avatar_el, false, Some(Box::new(avatar_click)));
        let decoration_art = self
            .applied_decoration
            .as_deref()
            .and_then(|sku| sku.parse::<Snowflake>().ok())
            .and_then(|sku| self.media.update(cx, |media, _| media.get(&oxicord_media::cdn::collectible(sku))))
            .map(|image| {
                div()
                    .size(px(64.))
                    .child(img(gpui_kit::ImageSource::Image(image)).size_full())
                    .into_any_element()
            })
            .unwrap_or_else(|| div().into_any_element());
        let decoration_tile = self.tile(
            "edit-decoration",
            decoration_art,
            true,
            Some(Box::new(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(OpenDecoration)))),
        );
        let avatar_section = Self::section(
            "Avatar & Decoration",
            div().flex().gap(px(10.)).child(avatar_tile).child(decoration_tile).into_any_element(),
        );

        let banner_tile = match banner.as_deref().and_then(|url| self.media.update(cx, |m, _| m.get(url))) {
            Some(image) => div()
                .size_full()
                .rounded(px(theme::RADIUS))
                .overflow_hidden()
                .child(img(gpui_kit::ImageSource::Image(image)).size_full().object_fit(gpui_kit::ObjectFit::Cover))
                .into_any_element(),
            None => div()
                .size_full()
                .rounded(px(theme::RADIUS))
                .bg(rgb(accent.unwrap_or(theme::BG_HOVER)))
                .into_any_element(),
        };
        let banner_section = Self::section(
            "Banner colour",
            div()
                .id("edit-banner")
                .w(px(208.))
                .h(px(42.))
                .flex_none()
                .rounded(px(4.))
                .cursor_pointer()
                .overflow_hidden()
                .hover(|s| s.opacity(0.9))
                .on_click(cx.listener(Self::upload_banner))
                .child(banner_tile)
                .into_any_element(),
        );

        let effect_art = match self
            .applied_effect
            .as_deref()
            .and_then(|sku| self.effect_catalog.find(sku))
        {
            Some(item) => match self.media.update(cx, |media, _| media.get(&item.preview)) {
                Some(image) => div()
                    .size(px(64.))
                    .rounded(px(4.))
                    .overflow_hidden()
                    .child(
                        img(gpui_kit::ImageSource::Image(image))
                            .size_full()
                            .object_fit(gpui_kit::ObjectFit::Cover),
                    )
                    .into_any_element(),
                None => icon("palette", 32., theme::TEXT_MUTED).into_any_element(),
            },
            None => icon("palette", 32., theme::TEXT_MUTED).into_any_element(),
        };
        let effect_tile = self.tile(
            "edit-effect",
            effect_art,
            true,
            Some(Box::new(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(OpenEffect)))),
        );
        let frame_tile = self.tile(
            "edit-frame",
            match self.applied_frame.as_deref().and_then(|sku| self.frame_catalog.find(sku)) {
                Some(item) => match item
                    .top
                    .as_ref()
                    .and_then(|art| self.media.update(cx, |media, _| media.get(&art.url)))
                {
                    Some(image) => div()
                        .size(px(64.))
                        .rounded(px(4.))
                        .overflow_hidden()
                        .child(
                            img(gpui_kit::ImageSource::Image(image))
                                .size_full()
                                .object_fit(gpui_kit::ObjectFit::Contain),
                        )
                        .into_any_element(),
                    None => icon("circle-user-round", 32., theme::TEXT_MUTED).into_any_element(),
                },
                None => icon("circle-user-round", 32., theme::TEXT_MUTED).into_any_element(),
            },
            true,
            Some(Box::new(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(OpenFrame)))),
        );
        let effect_section = Self::section(
            "Profile Effect & Frame",
            div().flex().gap(px(10.)).child(effect_tile).child(frame_tile).into_any_element(),
        );

        let upsell = div()
            .relative()
            .w(px(240.))
            .ml(px(-16.))
            .flex()
            .flex_col()
            .gap_2()
            .bg(rgb(0x2b_2250))
            .pt(px(20.))
            .px(px(16.))
            .pb(px(12.))
            .child(
                div()
                    .absolute()
                    .top(px(-14.))
                    .left(px(106.))
                    .size(px(28.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_full()
                    .bg(rgb(0x3b_2f63))
                    .child(icon("shield", 14., theme::TEXT_MUTED)),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .text_size(px(14.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(theme::TEXT_BRIGHT))
                    .child("Level up your look, only with Nitro"),
            )
            .child(
                div()
                    .id("preview-nitro")
                    .h(px(32.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap_2()
                    .rounded_full()
                    .bg(rgb(theme::ACCENT))
                    .cursor_pointer()
                    .text_size(px(14.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(theme::TEXT_BRIGHT))
                    .child(icon("discord-mark", 16., theme::TEXT_BRIGHT))
                    .child("Preview Nitro"),
            )
            .into_any_element();

        let name_style = Self::section(
            "Display Name Style",
            div()
                .w(px(208.))
                .h(px(44.))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(theme::RADIUS))
                .border_1()
                .border_color(rgb(theme::BORDER))
                .bg(rgb(theme::BG_INPUT))
                .text_size(px(20.))
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(theme::ONLINE))
                .child(name.clone())
                .into_any_element(),
        );

        let bio_section = Self::section(
            "Bio",
            div().w_full().child(Textarea::new(&self.bio)).into_any_element(),
        );

        let pronouns_section = Self::section(
            "Pronouns",
            div().w_full().child(Input::new(&self.pronouns)).into_any_element(),
        );

        let save = div()
            .id("save-edit-profile")
            .h(px(32.))
            .px_4()
            .flex()
            .items_center()
            .rounded(px(theme::RADIUS))
            .bg(rgb(theme::ACCENT))
            .cursor_pointer()
            .text_size(px(14.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(rgb(theme::TEXT_BRIGHT))
            .hover(|s| s.bg(rgb(theme::SUCCESS_HOVER)))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                let bio = this.bio.read(cx).value().trim().to_owned();
                let pronouns = this.pronouns.read(cx).value().trim().to_owned();
                let decoration = this.applied_decoration.clone();
                let effect = this.applied_effect.clone();
                let frame = this.applied_frame.clone();
                let current = this.session.read(cx).store.me().and_then(|m| m.decoration()).map(|sku| sku.to_string());
                let changed = current != decoration;
                let current_effect = this
                    .session
                    .read(cx)
                    .profiles
                    .get(&me.id)
                    .and_then(|p| p.effect)
                    .map(|sku| sku.to_string());
                let effect_changed = current_effect != effect;
                let current_frame = this
                    .session
                    .read(cx)
                    .profiles
                    .get(&me.id)
                    .and_then(|p| p.frame)
                    .map(|sku| sku.to_string());
                let frame_changed = current_frame != frame;
                let backend = this.backend.clone();
                this.session.update(cx, |s, cx| {
                    s.request_account(&backend, AccountRequest::EditProfile { bio, pronouns }, cx);
                    if changed {
                        s.request_account(&backend, AccountRequest::SetDecoration(decoration), cx);
                    }
                    if effect_changed {
                        s.request_account(&backend, AccountRequest::SetEffect(effect), cx);
                    }
                    if frame_changed {
                        s.request_account(&backend, AccountRequest::SetFrame(frame), cx);
                    }
                });
                cx.emit(CloseEditProfileModal);
            }))
            .child("Save changes");

        let editor = div()
            .id("edit-profile-editor")
            .w(px(240.))
            .flex_none()
            .h_full()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap(px(24.))
            .px(px(16.))
            .py(px(16.))
            .border_r_1()
            .border_color(rgb(theme::BORDER))
            .child(nameplate)
            .child(avatar_section)
            .child(banner_section)
            .child(effect_section)
            .child(upsell)
            .child(name_style)
            .child(bio_section)
            .child(pronouns_section)
            .child(save);

        // Preview pane
        let preview_banner = match banner.as_deref().and_then(|url| self.media.update(cx, |m, _| m.get(url))) {
            Some(image) => div()
                .h(px(140.))
                .rounded_t(px(theme::RADIUS))
                .overflow_hidden()
                .child(img(gpui_kit::ImageSource::Image(image)).size_full().object_fit(gpui_kit::ObjectFit::Cover))
                .into_any_element(),
            None => div()
                .h(px(140.))
                .rounded_t(px(theme::RADIUS))
                .bg(rgb(accent.unwrap_or(theme::ACCENT)))
                .into_any_element(),
        };
        let mut preview_avatar = div()
            .relative()
            .size(px(120.))
            .flex_none()
            .child(avatar(&self.media, &avatar_url, 120., &name, cx));
        if let Some(frame) = self
            .applied_decoration
            .as_deref()
            .and_then(|sku| sku.parse::<Snowflake>().ok())
            .and_then(|sku| decoration(&self.media, sku, 120., cx))
        {
            preview_avatar = preview_avatar.child(frame);
        }
        let joined = me.id.datetime().format("%b %-d, %Y").to_string();
        let preview = div()
            .w(px(400.))
            .flex_none()
            .self_start()
            .flex()
            .flex_col()
            .rounded(px(theme::RADIUS))
            .border_1()
            .border_color(rgb(theme::BORDER_MUTED))
            .bg(rgb(theme::BG_MENU))
            .overflow_hidden()
            .child(preview_banner)
            .child(
                div()
                    .px(px(32.))
                    .pb(px(32.))
                    .child(
                        div()
                            .mt(px(-70.))
                            .ml(px(-16.))
                            .child(preview_avatar),
                    )
                    .child(
                        div()
                            .mt(px(44.))
                            .text_size(px(20.))
                            .line_height(px(24.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(theme::TEXT))
                            .child(name),
                    )
                    .child(
                        div()
                            .mt(px(8.))
                            .text_size(px(14.))
                            .line_height(px(20.))
                            .text_color(rgb(theme::TEXT_BRIGHT))
                            .child(me.username.clone()),
                    )
                    .child(
                        div()
                            .mt(px(16.))
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(
                                div()
                                    .id("preview-message")
                                    .h(px(32.))
                                    .px(px(12.))
                                    .flex()
                                    .items_center()
                                    .gap_1()
                                    .rounded(px(theme::RADIUS))
                                    .bg(rgb(theme::ACCENT))
                                    .cursor_pointer()
                                    .text_size(px(14.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(rgb(theme::TEXT_BRIGHT))
                                    .hover(|s| s.bg(rgb(theme::SUCCESS_HOVER)))
                                    .child(icon("message-circle", 16., theme::TEXT_BRIGHT))
                                    .child("Message"),
                            )
                            .child(
                                div()
                                    .id("preview-add-friend")
                                    .size(px(32.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded(px(theme::RADIUS))
                                    .bg(rgb(theme::BG_ACTIVE))
                                    .cursor_pointer()
                                    .hover(|s| s.bg(rgb(theme::BG_HOVER)))
                                    .child(icon("user-plus", 16., theme::TEXT_BRIGHT)),
                            )
                            .child(
                                div()
                                    .id("preview-more")
                                    .size(px(32.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded(px(theme::RADIUS))
                                    .bg(rgb(theme::BG_ACTIVE))
                                    .cursor_pointer()
                                    .hover(|s| s.bg(rgb(theme::BG_HOVER)))
                                    .child(icon("ellipsis", 16., theme::TEXT_BRIGHT)),
                            ),
                    )
                    .children((!profile_bio.is_empty()).then(|| {
                        div()
                            .mt(px(16.))
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(theme::TEXT_BRIGHT))
                                    .child("Bio"),
                            )
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .text_color(rgb(theme::TEXT))
                                    .child(profile_bio.clone()),
                            )
                    }))
                    .child(
                        div()
                            .mt(px(12.))
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(theme::TEXT_BRIGHT))
                                    .child("Member Since"),
                            )
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .text_color(rgb(theme::TEXT))
                                    .child(joined),
                            ),
                    )
                    .child(
                        div()
                            .mt(px(24.))
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .text_color(rgb(theme::TEXT_MUTED))
                                    .child("Connections"),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .text_size(px(14.))
                                    .text_color(rgb(theme::TEXT_MUTED))
                                    .child(icon("plus", 14., theme::TEXT_MUTED))
                                    .child("Add Connection"),
                            ),
                    )
                    .child(
                        div()
                            .mt(px(24.))
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(rgb(theme::TEXT_MUTED))
                                    .child("Note (only visible to you)"),
                            )
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .italic()
                                    .text_color(rgb(theme::TEXT_MUTED))
                                    .child("Click to add a note"),
                            ),
                    ),
            );

        // Widgets board
        let entries: Vec<(String, Option<String>)> = self
            .session
            .read(cx)
            .profiles
            .get(&me.id)
            .map(|p| {
                p.favorite_games
                    .iter()
                    .chain(&p.played_games)
                    .map(|entry| {
                        let known = p.games.get(&entry.id);
                        (
                            known.map(|g| g.name.clone()).unwrap_or_default(),
                            known.and_then(|g| g.cover_url.clone()),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        let empty = entries.is_empty();
        let mut widgets = div().flex().flex_col().gap_3();
        for (title, cover) in entries {
            let mut card = div()
                .w(px(431.))
                .flex_none()
                .flex()
                .items_center()
                .gap(px(16.))
                .rounded(px(theme::RADIUS))
                .bg(rgb(theme::BG_CARD))
                .p(px(8.));
            match cover.and_then(|url| self.media.update(cx, |m, _| m.get(&url))) {
                Some(image) => card = card.child(
                    div()
                        .w(px(80.))
                        .h(px(114.))
                        .flex_none()
                        .rounded(px(4.))
                        .overflow_hidden()
                        .child(img(gpui_kit::ImageSource::Image(image)).size_full().object_fit(gpui_kit::ObjectFit::Cover)),
                ),
                None => card = card.child(
                    div().w(px(80.)).h(px(114.)).flex_none().rounded(px(4.)).bg(rgb(theme::BG_ACTIVE)),
                ),
            }
            card = card.child(
                div()
                    .flex()
                    .flex_col()
                    .text_size(px(13.))
                    .text_color(rgb(theme::TEXT))
                    .child(div().truncate().child(title)),
            );
            widgets = widgets.child(card);
        }
        let board = div()
            .id("edit-profile-board")
            .flex_1()
            .min_w_0()
            .h_full()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap(px(16.))
            .pl(px(4.))
            .pr(px(52.))
            .py(px(24.))
            .border_l_1()
            .border_color(rgb(theme::BORDER))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_4()
                    .border_b_1()
                    .border_color(rgb(theme::BORDER))
                    .pb(px(8.))
                    .child(["Board", "Activity", "Wishlist"].iter().enumerate().fold(
                        div().flex().gap_4(),
                        |row, (i, tab)| {
                            let selected = i == 0;
                            row.child(
                                div()
                                    .id(if i == 0 { "board-tab" } else { "other-tab" })
                                    .h_full()
                                    .flex()
                                    .items_center()
                                    .border_b_2()
                                    .border_color(rgb(if selected { theme::TEXT_BRIGHT } else { 0x0000_0000 }))
                                    .text_size(px(14.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(if selected { theme::TEXT_BRIGHT } else { theme::TEXT_MUTED }))
                                    .when(i == 0, |d| {
                                        d.cursor_pointer().on_click(cx.listener(
                                            |this, _: &ClickEvent, _, cx| {
                                                this.widget_picker = false;
                                                this.favorite_editor = false;
                                                cx.notify();
                                            },
                                        ))
                                    })
                                    .child(*tab),
                            )
                        },
                    )),
            )
            .child(if (empty || self.widget_picker) && !self.favorite_editor {
                crate::widget_picker::widget_picker(
                    &self.media,
                    Some(Box::new(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.favorite_editor = true;
                        this.widget_picker = false;
                        cx.notify();
                    }))),
                    cx,
                )
            } else {
                div()
                    .flex()
                    .flex_col()
                    .gap(px(16.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(theme::TEXT_BRIGHT))
                                    .child("Your Widgets"),
                            )
                            .child(
                                div()
                                    .id("add-widget")
                                    .h(px(32.))
                                    .px_2()
                                    .flex()
                                    .items_center()
                                    .gap_1()
                                    .rounded(px(theme::RADIUS))
                                    .bg(rgb(theme::BG_ACTIVE))
                                    .cursor_pointer()
                                    .text_size(px(13.))
                                    .text_color(rgb(theme::TEXT_BRIGHT))
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.widget_picker = true;
                                        this.favorite_editor = false;
                                        cx.notify();
                                    }))
                                    .child(icon("plus", 12., theme::TEXT_BRIGHT))
                                    .child("Add Widget"),
                            ),
                    )
                    .child(if self.favorite_editor {
                        crate::favorite_game_editor::editor(&self.media, cx)
                    } else {
                        widgets.into_any_element()
                    })
                    .into_any_element()
            });

        let content = div()
            .id("edit-profile-modal")
            .relative()
            .occlude()
            .w(px(1200.))
            .h(px(800.))
            .flex_none()
            .flex()
            .flex_col()
            .overflow_hidden()
            .rounded(px(theme::RADIUS_MD))
            .bg(rgb(theme::BG_PANEL))
            .border_1()
            .border_color(rgb(theme::BORDER))
            .child(
                div()
                    .h(px(48.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_between()
                    .pl(px(16.))
                    .pr(px(8.))
                    .border_b_1()
                    .border_color(rgb(theme::BORDER))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .text_size(px(16.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(theme::TEXT_BRIGHT))
                            .child("Main Profile")
                            .child(icon("chevron-down", 16., theme::TEXT_MUTED)),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(icon("chevron-right", 20., theme::TEXT_MUTED))
                            .child(
                                div()
                                    .id("edit-profile-close")
                                    .size(px(32.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded(px(theme::RADIUS))
                                    .cursor_pointer()
                                    .hover(|s| s.bg(rgb(theme::BG_HOVER)))
                                    .on_click(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(CloseEditProfileModal)))
                                    .child(icon("x", 20., theme::TEXT_MUTED)),
                            ),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .child(editor)
                    .child(div().flex_none().pl(px(50.)).mr(px(28.)).child(preview))
                    .child(board),
            );

        modal(
            content.into_any_element(),
            Box::new(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(CloseEditProfileModal))),
        )
        .into_any_element()
    }
}
