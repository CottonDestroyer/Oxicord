//! The settings window: pages down the left, the open page on the right.
//! Preferences that Discord stores on the account are read and written
//! through the account settings blob, the rest live in the local file.

mod pages;
pub(crate) mod widgets;

use gpui_kit::{
    AppContext as _, ClickEvent, Context, Entity, EventEmitter, FocusHandle, FontWeight, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, StatefulInteractiveElement as _,
    Styled as _, Subscription, Window, div, px, rgb,
    component::{input::{Input, InputState}, slider::{SliderEvent, SliderState}},
};
use oxicord_model::Snowflake;
use oxicord_settings::{Key, Value};

use crate::{
    account::Load,
    backend::{Backend, Command},
    icons::icon,
    media::Media,
    profile::modal,
    session::Session,
    theme,
    AccountRequest,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Page {
    AccountInfo,
    PasswordSecurity,
    AccountStatus,
    FamilyCenter,
    DataPrivacy,
    ContentSocial,
    Notifications,
    Nitro,
    ServerBoost,
    Subscriptions,
    GiftInventory,
    Billing,
    VoiceVideo,
    Appearance,
    Accessibility,
    System,
    LanguageTime,
    ActivityPrivacy,
    Apps,
    Chat,
    Devices,
    Connections,
    Advanced,
}

impl Page {
    /// The sidebar as the live client draws it: group heading, then pages.
    const SECTIONS: [(&'static str, &'static [Page]); 5] = [
        (
            "Account",
            &[
                Page::AccountInfo,
                Page::PasswordSecurity,
                Page::AccountStatus,
                Page::FamilyCenter,
                Page::DataPrivacy,
                Page::ContentSocial,
                Page::Notifications,
            ],
        ),
        (
            "Billing",
            &[
                Page::Nitro,
                Page::ServerBoost,
                Page::Subscriptions,
                Page::GiftInventory,
                Page::Billing,
            ],
        ),
        (
            "Experience",
            &[
                Page::VoiceVideo,
                Page::Appearance,
                Page::Accessibility,
                Page::System,
                Page::LanguageTime,
            ],
        ),
        (
            "Games & Apps",
            &[Page::ActivityPrivacy, Page::Apps],
        ),
        ("Developer", &[Page::Advanced]),
    ];

    /// Every page the search can reach, including ones without a sidebar row.
    const ALL: &'static [Page] = &[
        Page::AccountInfo,
        Page::PasswordSecurity,
        Page::AccountStatus,
        Page::FamilyCenter,
        Page::DataPrivacy,
        Page::ContentSocial,
        Page::Notifications,
        Page::Nitro,
        Page::ServerBoost,
        Page::Subscriptions,
        Page::GiftInventory,
        Page::Billing,
        Page::VoiceVideo,
        Page::Appearance,
        Page::Accessibility,
        Page::System,
        Page::LanguageTime,
        Page::ActivityPrivacy,
        Page::Apps,
        Page::Chat,
        Page::Devices,
        Page::Connections,
        Page::Advanced,
    ];

    fn label(self) -> &'static str {
        match self {
            Page::AccountInfo => "Account Info",
            Page::PasswordSecurity => "Password & Security",
            Page::AccountStatus => "Account Status",
            Page::FamilyCenter => "Family Center",
            Page::DataPrivacy => "Data & Privacy",
            Page::ContentSocial => "Messaging Permissions",
            Page::Notifications => "Notifications",
            Page::Nitro => "Nitro",
            Page::ServerBoost => "Server Boost",
            Page::Subscriptions => "Subscriptions",
            Page::GiftInventory => "Gift Inventory",
            Page::Billing => "Billing",
            Page::VoiceVideo => "Voice & Video",
            Page::Appearance => "Appearance",
            Page::Accessibility => "Accessibility",
            Page::System => "System",
            Page::LanguageTime => "Language & Time",
            Page::ActivityPrivacy => "Activity Privacy",
            Page::Apps => "Connected Apps",
            Page::Chat => "Chat",
            Page::Devices => "Devices",
            Page::Connections => "Connections",
            Page::Advanced => "Developer",
        }
    }

    /// The window title can differ from the sidebar label: the live account
    /// page is titled "My Account" but its first section is "Account Info".
    fn title(self) -> &'static str {
        if self == Page::AccountInfo { "My Account" } else { self.label() }
    }

    /// The icon beside the page in the sidebar.
    fn glyph(self) -> &'static str {
        match self {
            Page::AccountInfo => "user",
            Page::PasswordSecurity => "shield-check",
            Page::AccountStatus => "id-badge",
            Page::FamilyCenter => "users",
            Page::DataPrivacy => "shield",
            Page::ContentSocial => "messages-square",
            Page::Notifications => "bell",
            Page::Nitro => "nitro",
            Page::ServerBoost => "server-boost",
            Page::Subscriptions => "store",
            Page::GiftInventory => "tag-award",
            Page::Billing => "file",
            Page::VoiceVideo => "mic",
            Page::Appearance => "palette",
            Page::Accessibility => "circle-user-round",
            Page::System => "keyboard",
            Page::LanguageTime => "discovery",
            Page::ActivityPrivacy => "activity",
            Page::Apps => "layout-grid",
            Page::Chat => "message-circle",
            Page::Devices => "laptop",
            Page::Connections => "link",
            Page::Advanced => "settings",
        }
    }

    /// Words besides the title that should find this page in the search box.
    fn keywords(self) -> &'static str {
        match self {
            Page::AccountInfo => "email phone display name username global name",
            Page::PasswordSecurity => "password two factor 2fa authentication logged in devices",
            Page::AccountStatus => "age group standing good",
            Page::FamilyCenter => "family center parental controls",
            Page::ContentSocial => {
                "content filters spam dm direct message friend requests connected games ignore block"
            }
            Page::DataPrivacy => "direct messages server members local time privacy",
            Page::Apps => "oauth bots integrations deauthorize revoke connections",
            Page::Devices => "sessions logged in locations",
            Page::Connections => "linked accounts steam github twitch spotify",
            Page::Appearance => "time format 12 hour 24 hour clock members list layout theme",
            Page::VoiceVideo => "microphone input volume mic sound audio push to talk key",
            Page::Chat => "embeds link previews images attachments reactions media",
            Page::Notifications => "desktop toast mentions alerts",
            Page::Nitro => "subscription premium perks",
            Page::ServerBoost => "boost level perks",
            Page::Subscriptions => "billing renew payment",
            Page::GiftInventory => "gifts codes inventory",
            Page::Billing => "payment methods invoices",
            Page::Accessibility => "text readability motion reduce animations",
            Page::System => "keyboard shortcuts hotkeys keys",
            Page::LanguageTime => "language timezone time zone date",
            Page::ActivityPrivacy => "activity share status game",
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

/// The account page asked to open the Edit Profile modal.
pub struct OpenEditProfile;

impl EventEmitter<OpenEditProfile> for SettingsView {}

pub struct SettingsView {
    session: Entity<Session>,
    media: Entity<Media>,
    backend: Backend,
    page: Page,
    search: Entity<InputState>,
    display_name: Entity<InputState>,
    /// The Voice & Video page's Input Volume slider, 0 to 100.
    input_volume: Entity<SliderState>,
    /// Push to talk: the key button waiting for the next key to be pressed.
    capturing_key: bool,
    key_focus: FocusHandle,
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
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search settings"));
        let display_name = cx.new(|cx| InputState::new(window, cx).placeholder("Display name"));
        let volume = f32::from(session.read(cx).settings.input_volume);
        let input_volume = cx.new(|_| SliderState::new().min(0.).max(100.).step(1.).default_value(volume));
        let subscriptions = vec![
            cx.observe(&session, |_, _, cx| cx.notify()),
            cx.observe(&media, |_, _, cx| cx.notify()),
            cx.observe(&search, |_, _, cx| cx.notify()),
            cx.subscribe(&input_volume, |this, _, event: &SliderEvent, cx| {
                // Dragging changes the call live, letting go saves the file.
                let (value, save) = match event {
                    SliderEvent::Change(value) => (value, false),
                    SliderEvent::Release(value) => (value, true),
                };
                let percent = value.start().round().clamp(0., 100.) as u8;
                let backend = this.backend.clone();
                this.session.update(cx, |s, cx| s.set_input_volume(&backend, percent, save, cx));
            }),
        ];
        Self {
            session,
            media,
            backend,
            page: Page::AccountInfo,
            search,
            display_name,
            input_volume,
            capturing_key: false,
            key_focus: cx.focus_handle(),
            filled: false,
            reveal_contact: false,
            confirm_revoke: None,
            _subscriptions: subscriptions,
        }
    }

    /// Opens straight on the Data & Privacy page.
    pub fn open_data_privacy(&mut self, cx: &mut Context<Self>) {
        self.page = Page::DataPrivacy;
        self.opened(cx);
        cx.notify();
    }

    /// Opens straight on the Activity Privacy page.
    pub fn open_activity_privacy(&mut self, cx: &mut Context<Self>) {
        self.page = Page::ActivityPrivacy;
        self.opened(cx);
        cx.notify();
    }

    /// Opens straight on the Language & Time page.
    pub fn open_language_time(&mut self, cx: &mut Context<Self>) {
        self.page = Page::LanguageTime;
        self.opened(cx);
        cx.notify();
    }

    /// Opens straight on the System page.
    pub fn open_system(&mut self, cx: &mut Context<Self>) {
        self.page = Page::System;
        self.opened(cx);
        cx.notify();
    }

    /// Opens straight on the Accessibility page.
    pub fn open_accessibility(&mut self, cx: &mut Context<Self>) {
        self.page = Page::Accessibility;
        self.opened(cx);
        cx.notify();
    }

    /// Opens straight on the Appearance page.
    pub fn open_appearance(&mut self, cx: &mut Context<Self>) {
        self.page = Page::Appearance;
        self.opened(cx);
        cx.notify();
    }

    /// Opens straight on the Voice & Video page.
    pub fn open_voice(&mut self, cx: &mut Context<Self>) {
        self.page = Page::VoiceVideo;
        self.opened(cx);
        cx.notify();
    }

    /// Opens straight on the Notifications page.
    pub fn open_notifications(&mut self, cx: &mut Context<Self>) {
        self.page = Page::Notifications;
        self.opened(cx);
        cx.notify();
    }

    /// Opens straight on the Messaging Permissions page.
    pub fn open_messaging(&mut self, cx: &mut Context<Self>) {
        self.page = Page::ContentSocial;
        self.opened(cx);
        cx.notify();
    }

    /// Called when the window is shown: fetch what the open page needs.
    pub fn opened(&mut self, cx: &mut Context<Self>) {
        self.reveal_contact = false;
        self.confirm_revoke = None;
        self.load_page(cx);
    }

    /// Opens straight on the Keybinds page, for its own shortcut.
    pub fn open_keybinds(&mut self, cx: &mut Context<Self>) {
        self.page = Page::System;
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
                Page::AccountInfo => Some(AccountRequest::LoadDetails),
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

    /// Puts the slider where the saved volume is, for when something else
    /// changed it, like resetting the app settings.
    fn sync_volume(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let saved = f32::from(self.session.read(cx).settings.input_volume);
        if self.input_volume.read(cx).value().start().round() != saved {
            self.input_volume.update(cx, |slider, cx| slider.set_value(saved, window, cx));
        }
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
        self.display_name.update(cx, |i, cx| i.set_value(name, window, cx));
        self.filled = true;
    }

    fn render_sidebar(&mut self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let query = self.search.read(cx).value().trim().to_lowercase();

        let mut nav = div().flex().flex_col();
        for (heading, pages) in Page::SECTIONS {
            let shown: Vec<Page> = pages.iter().copied().filter(|p| p.matches(&query)).collect();
            if shown.is_empty() {
                continue;
            }
            nav = nav.child(
                div()
                    .pt(px(16.))
                    .pb(px(4.))
                    .px(px(8.))
                    .text_size(px(12.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(theme::TEXT_MUTED))
                    .child(heading),
            );
            for page in shown {
                let selected = self.page == page;
                let color = if selected { theme::TEXT_BRIGHT } else { theme::TEXT_MUTED };
                nav = nav.child(
                    div()
                        .id(("settings-page", page as u32))
                        .h(px(32.))
                        .px(px(8.))
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .rounded(px(theme::RADIUS))
                        .cursor_pointer()
                        .text_size(px(16.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(rgb(color))
                        .bg(rgb(if selected { theme::BG_ACTIVE } else { theme::BG_PANEL }))
                        .hover(|s| {
                            s.bg(rgb(if selected { theme::BG_ACTIVE } else { theme::BG_HOVER }))
                                .text_color(rgb(theme::TEXT_BRIGHT))
                        })
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.select(page, cx)))
                        .child(icon(page.glyph(), 20., color))
                        .child(page.label()),
                );
            }
        }

        // The search box filters every page, including ones without a sidebar
        // row yet, so the old pages remain reachable while they are replaced.
        if !query.is_empty() {
            for page in Page::ALL.iter().copied() {
                if Page::SECTIONS.iter().any(|(_, pages)| pages.contains(&page)) || !page.matches(&query) {
                    continue;
                }
                nav = nav.child(
                    div()
                        .id(("settings-page-search", page as u32))
                        .h(px(32.))
                        .px(px(8.))
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .rounded(px(theme::RADIUS))
                        .cursor_pointer()
                        .text_size(px(16.))
                        .text_color(rgb(theme::TEXT_MUTED))
                        .hover(|s| {
                            s.bg(rgb(theme::BG_HOVER)).text_color(rgb(theme::TEXT_BRIGHT))
                        })
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.select(page, cx)))
                        .child(icon(page.glyph(), 20., theme::TEXT_MUTED))
                        .child(page.label()),
                );
            }
        }

        let backend = self.backend.clone();
        div()
            .w(px(240.))
            .h_full()
            .flex_none()
            .flex()
            .flex_col()
            .bg(rgb(theme::BG_SIDEBAR))
            .child(
                div()
                    .px(px(12.))
                    .pt(px(16.))
                    .pb(px(8.))
                    .child(Input::new(&self.search).prefix(icon("search", 18., theme::TEXT_MUTED).mr(px(6.)))),
            )
            .child(div().id("settings-nav").flex_1().px(px(8.)).overflow_y_scroll().child(nav))
            .child(
                div()
                    .id("settings-log-out")
                    .mx(px(8.))
                    .mb(px(8.))
                    .h(px(32.))
                    .px(px(8.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .rounded(px(theme::RADIUS))
                    .cursor_pointer()
                    .text_size(px(16.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(theme::DANGER))
                    .hover(|s| s.bg(rgb(theme::BG_HOVER)))
                    .on_click(cx.listener(move |_, _: &ClickEvent, _, _| backend.send(Command::Logout)))
                    .child(icon("log-out", 20., theme::DANGER))
                    .child("Log Out"),
            )
            .child(
                div()
                    .px(px(16.))
                    .pb(px(12.))
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(rgb(theme::TEXT_FAINT))
                            .child("stable 670026"),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(rgb(theme::ACCENT))
                            .child("Privacy Policy  |  Terms of Service  |  More"),
                    ),
            )
    }
}

impl SettingsView {
    /// The settings window without the screen overlay, for headless previews.
    pub fn preview_content(&mut self, window: &mut Window, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        self.fill_inputs(window, cx);
        self.sync_volume(window, cx);
        let sidebar = self.render_sidebar(cx);
        let title = self.page.title();
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

        div()
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
            .into_any_element()
    }
}

impl Render for SettingsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        modal(
            self.preview_content(window, cx),
            Box::new(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(CloseSettings))),
        )
    }
}
