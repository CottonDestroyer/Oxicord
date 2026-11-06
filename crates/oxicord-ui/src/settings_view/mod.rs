//! The settings window: pages down the left, the open page on the right.
//! Preferences that Discord stores on the account are read and written
//! through the account settings blob, the rest live in the local file.

mod pages;
pub(crate) mod widgets;

use gpui_kit::{
    AppContext as _, ClickEvent, Context, Entity, EventEmitter, FontWeight, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, StatefulInteractiveElement as _,
    Styled as _, Subscription, Window, div, px, rgb,
    component::input::{Input, InputState, TextareaState},
};
use oxicord_model::Snowflake;
use oxicord_settings::{Key, Value};

use crate::{
    account::Load,
    backend::Backend,
    icons::icon,
    media::Media,
    profile::modal,
    session::Session,
    theme,
    AccountRequest,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Page {
    Account,
    Profile,
    ContentSocial,
    DataPrivacy,
    Apps,
    Devices,
    Connections,
    Appearance,
    Chat,
    Notifications,
    Keybinds,
    Advanced,
}

impl Page {
    const SECTIONS: [(&'static str, &'static [Page]); 2] = [
        (
            "",
            &[
                Page::Account,
                Page::Profile,
                Page::ContentSocial,
                Page::DataPrivacy,
                Page::Apps,
                Page::Devices,
                Page::Connections,
            ],
        ),
        (
            "App settings",
            &[
                Page::Appearance,
                Page::Chat,
                Page::Notifications,
                Page::Keybinds,
                Page::Advanced,
            ],
        ),
    ];

    fn label(self) -> &'static str {
        match self {
            Page::Account => "My Account",
            Page::Profile => "Profile",
            Page::ContentSocial => "Content & Social",
            Page::DataPrivacy => "Data & Privacy",
            Page::Apps => "Authorized Apps",
            Page::Devices => "Devices",
            Page::Connections => "Connections",
            Page::Appearance => "Appearance",
            Page::Chat => "Chat",
            Page::Notifications => "Notifications",
            Page::Keybinds => "Keybinds",
            Page::Advanced => "Advanced",
        }
    }

    /// The icon beside the page in the sidebar.
    fn glyph(self) -> &'static str {
        match self {
            Page::Account => "user",
            Page::Profile => "square-user",
            Page::ContentSocial => "shield-check",
            Page::DataPrivacy => "shield",
            Page::Apps => "layout-grid",
            Page::Devices => "laptop",
            Page::Connections => "link",
            Page::Appearance => "palette",
            Page::Chat => "message-circle",
            Page::Notifications => "bell",
            Page::Keybinds => "keyboard",
            Page::Advanced => "settings",
        }
    }

    /// Words besides the title that should find this page in the search box.
    fn keywords(self) -> &'static str {
        match self {
            Page::Account => "email phone password display name username log out two factor 2fa",
            Page::Profile => "about me bio pronouns",
            Page::ContentSocial => {
                "explicit media filter spoilers dm spam friend requests everyone mutual activity status game"
            }
            Page::DataPrivacy => "direct messages server members local time privacy",
            Page::Apps => "oauth bots integrations deauthorize revoke",
            Page::Devices => "sessions logged in locations",
            Page::Connections => "linked accounts steam github twitch spotify",
            Page::Appearance => "time format 12 hour 24 hour clock members list layout theme",
            Page::Chat => "embeds link previews images attachments reactions media",
            Page::Notifications => "desktop toast mentions alerts",
            Page::Keybinds => "keyboard shortcuts hotkeys keys",
            Page::Advanced => "developer mode reset about version",
        }
    }

    fn matches(self, query: &str) -> bool {
        query.is_empty()
            || self.label().to_lowercase().contains(query)
            || self.keywords().contains(query)
    }
}

/// Emitted when the window asks to go away.
pub struct CloseSettings;

impl EventEmitter<CloseSettings> for SettingsView {}

pub struct SettingsView {
    session: Entity<Session>,
    media: Entity<Media>,
    backend: Backend,
    page: Page,
    search: Entity<InputState>,
    display_name: Entity<InputState>,
    pronouns: Entity<InputState>,
    bio: Entity<TextareaState>,
    /// Whether the profile inputs have been filled from the server yet, so a
    /// refresh doesn't overwrite what the user is typing.
    filled: bool,
    reveal_contact: bool,
    /// The app whose remove button was pressed once already.
    confirm_revoke: Option<Snowflake>,
    _subscriptions: Vec<Subscription>,
}

impl SettingsView {
    pub fn new(
        session: Entity<Session>,
        media: Entity<Media>,
        backend: Backend,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search"));
        let display_name = cx.new(|cx| InputState::new(window, cx).placeholder("Display name"));
        let pronouns = cx.new(|cx| InputState::new(window, cx).placeholder("Add your pronouns"));
        let bio = cx.new(|cx| {
            TextareaState::new(window, cx)
                .auto_grow(3, 8)
                .placeholder("Tell people about yourself")
        });
        let subscriptions = vec![
            cx.observe(&session, |_, _, cx| cx.notify()),
            cx.observe(&media, |_, _, cx| cx.notify()),
            cx.observe(&search, |_, _, cx| cx.notify()),
        ];
        Self {
            session,
            media,
            backend,
            page: Page::Account,
            search,
            display_name,
            pronouns,
            bio,
            filled: false,
            reveal_contact: false,
            confirm_revoke: None,
            _subscriptions: subscriptions,
        }
    }

    /// Called when the window is shown: fetch what the open page needs.
    pub fn opened(&mut self, cx: &mut Context<Self>) {
        self.reveal_contact = false;
        self.confirm_revoke = None;
        self.load_page(cx);
    }

    /// Opens straight on the Keybinds page, for its own shortcut.
    pub fn open_keybinds(&mut self, cx: &mut Context<Self>) {
        self.page = Page::Keybinds;
        self.opened(cx);
        cx.notify();
    }

    /// Opens straight on the Profile page, for the account popout.
    pub fn open_profile(&mut self, cx: &mut Context<Self>) {
        self.page = Page::Profile;
        self.opened(cx);
        cx.notify();
    }

    fn select(&mut self, page: Page, cx: &mut Context<Self>) {
        self.page = page;
        self.confirm_revoke = None;
        self.load_page(cx);
        cx.notify();
    }

    /// Lists and account details are fetched fresh each time their page is
    /// opened. Preferences are shared by several pages and only fetched
    /// when we don't have them.
    fn load_page(&mut self, cx: &mut Context<Self>) {
        let (settings_missing, request) = {
            let account = &self.session.read(cx).account;
            let missing = !matches!(account.settings, Load::Ready(_) | Load::Loading);
            let request = match self.page {
                Page::Account | Page::Profile => Some(AccountRequest::LoadDetails),
                Page::Apps => Some(AccountRequest::LoadApps),
                Page::Devices => Some(AccountRequest::LoadSessions),
                Page::Connections => Some(AccountRequest::LoadConnections),
                _ => None,
            };
            (missing, request)
        };
        let backend = self.backend.clone();
        self.session.update(cx, |session, cx| {
            if settings_missing {
                session.request_account(&backend, AccountRequest::LoadSettings, cx);
            }
            if let Some(request) = request {
                session.request_account(&backend, request, cx);
            }
        });
    }

    fn pref(&self, key: Key, cx: &gpui_kit::App) -> Value {
        self.session.read(cx).account.pref(key)
    }

    fn set_pref(&mut self, key: Key, value: Value, cx: &mut Context<Self>) {
        let backend = self.backend.clone();
        self.session
            .update(cx, |session, cx| session.change_prefs(&backend, &[(key, value)], cx));
    }

    fn toggle_pref(&mut self, key: Key, cx: &mut Context<Self>) {
        let on = self.pref(key, cx).as_bool();
        self.set_pref(key, Value::Bool(!on), cx);
    }

    /// Copies the account's name and bio into the editors the first time
    /// they are known.
    fn fill_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.filled {
            return;
        }
        let Some(details) = self.session.read(cx).account.details.ready().cloned() else {
            return;
        };
        let name = details.global_name.unwrap_or_default();
        let pronouns = details.pronouns.unwrap_or_default();
        let bio = details.bio.unwrap_or_default();
        self.display_name.update(cx, |i, cx| i.set_value(name, window, cx));
        self.pronouns.update(cx, |i, cx| i.set_value(pronouns, window, cx));
        self.bio.update(cx, |i, cx| i.set_value(bio, window, cx));
        self.filled = true;
    }

    /// The profile card at the top of the sidebar: who we are, and a way to
    /// jump to editing it.
    fn render_identity(&mut self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let (name, url) = {
            let store = &self.session.read(cx).store;
            match store.me() {
                Some(me) => (me.display_name().to_owned(), oxicord_media::cdn::user_avatar(me.id, me.avatar.as_deref(), 96)),
                None => (String::new(), String::new()),
            }
        };
        div()
            .id("settings-identity")
            .flex()
            .items_center()
            .gap(px(12.))
            .px(px(8.))
            .py(px(8.))
            .child(crate::media::avatar(&self.media, &url, 48., &name, cx))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .truncate()
                            .text_size(px(16.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(theme::TEXT_BRIGHT))
                            .child(name),
                    )
                    .child(
                        div()
                            .id("edit-profiles")
                            .flex()
                            .items_center()
                            .gap(px(4.))
                            .cursor_pointer()
                            .text_size(px(14.))
                            .text_color(rgb(theme::TEXT_MUTED))
                            .hover(|s| s.text_color(rgb(theme::TEXT_BRIGHT)))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.select(Page::Profile, cx)))
                            .child("Edit Profiles")
                            .child(icon("pencil", 12., theme::TEXT_MUTED)),
                    ),
            )
    }

    fn render_sidebar(&mut self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let query = self.search.read(cx).value().trim().to_lowercase();

        let mut nav = div().flex().flex_col().gap(px(4.));
        let mut first = true;
        for (heading, pages) in Page::SECTIONS {
            let shown: Vec<Page> = pages.iter().copied().filter(|p| p.matches(&query)).collect();
            if shown.is_empty() {
                continue;
            }
            // Groups are told apart by a line and, after the first, a title.
            if !first {
                nav = nav.child(div().my(px(8.)).mx(px(8.)).h(px(1.)).bg(rgb(theme::BORDER)));
            }
            if !heading.is_empty() {
                nav = nav.child(
                    div()
                        .px(px(8.))
                        .pt(px(8.))
                        .pb(px(4.))
                        .text_size(px(14.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(rgb(theme::TEXT_MUTED))
                        .child(heading),
                );
            }
            first = false;
            for page in shown {
                let selected = self.page == page;
                let color = if selected { theme::TEXT_BRIGHT } else { theme::TEXT_MUTED };
                nav = nav.child(
                    div()
                        .id(("settings-page", page as u32))
                        .h(px(36.))
                        .px(px(10.))
                        .flex()
                        .items_center()
                        .gap(px(10.))
                        .rounded(px(theme::RADIUS))
                        .cursor_pointer()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(rgb(color))
                        .bg(rgb(if selected { theme::BG_ACTIVE } else { theme::BG_PANEL }))
                        .hover(|s| s.bg(rgb(if selected { theme::BG_ACTIVE } else { theme::BG_HOVER })).text_color(rgb(theme::TEXT_BRIGHT)))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.select(page, cx)))
                        .child(icon(page.glyph(), 20., color))
                        .child(page.label()),
                );
            }
        }

        let identity = self.render_identity(cx);
        div()
            .w(px(252.))
            .h_full()
            .flex_none()
            .flex()
            .flex_col()
            .gap(px(8.))
            .px(px(16.))
            .pt(px(24.))
            .pb(px(12.))
            .bg(rgb(theme::BG_PANEL))
            .child(identity)
            .child(Input::new(&self.search).prefix(icon("search", 18., theme::TEXT_MUTED).mr(px(6.))))
            .child(div().id("settings-nav").flex_1().overflow_y_scroll().child(nav))
            .child(
                div()
                    .px(px(8.))
                    .text_size(px(11.))
                    .text_color(rgb(theme::TEXT_FAINT))
                    .child(format!("Oxicord {}", env!("CARGO_PKG_VERSION"))),
            )
    }
}

impl Render for SettingsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.fill_inputs(window, cx);
        let sidebar = self.render_sidebar(cx);
        let title = self.page.label();
        // About 40px of air around it, up to the size of the official one.
        let viewport = window.viewport_size();
        let width = (f32::from(viewport.width) - 80.).min(1400.);
        let height = (f32::from(viewport.height) - 80.).min(1246.);
        let page = self.render_page(cx);

        // The title bar over the page: its name on the left, close on the right.
        let bar = div()
            .h(px(theme::CHANNEL_HEADER_HEIGHT - 1.))
            .flex_none()
            .flex()
            .items_center()
            .justify_between()
            .pl(px(16.))
            .pr(px(16.))
            .border_b_1()
            .border_color(rgb(theme::BORDER))
            .child(
                div()
                    .text_size(px(16.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(theme::TEXT_BRIGHT))
                    .child(title),
            )
            .child(
                div()
                    .id("settings-close")
                    .size(px(32.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(theme::RADIUS))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(theme::BG_HOVER)))
                    .on_click(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(CloseSettings)))
                    .child(icon("x", 24., theme::TEXT_MUTED)),
            );

        let content = div()
            .id("settings-window")
            .occlude()
            .w(px(width))
            .h(px(height))
            .flex()
            .overflow_hidden()
            .rounded(px(theme::RADIUS_MD))
            .border_1()
            .border_color(rgb(theme::BORDER))
            .bg(rgb(theme::BG_PANEL))
            .child(sidebar)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .flex()
                    .flex_col()
                    .border_l_1()
                    .border_color(rgb(theme::BORDER))
                    .child(bar)
                    .child(
                        div()
                            .id("settings-page-body")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .flex()
                            .justify_center()
                            .px(px(40.))
                            .py(px(40.))
                            // The page sits in a column about 700px wide.
                            .child(div().w_full().max_w(px(700.)).child(page)),
                    ),
            )
            .into_any_element();

        modal(
            content,
            Box::new(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(CloseSettings))),
        )
    }
}
