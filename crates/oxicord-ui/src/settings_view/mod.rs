//! The settings window: pages down the left, the open page on the right.
//! Preferences that Discord stores on the account are read and written
//! through the account settings blob, the rest live in the local file.

mod pages;
mod widgets;

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
    Advanced,
}

impl Page {
    const SECTIONS: [(&'static str, &'static [Page]); 2] = [
        (
            "User settings",
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
            Page::Advanced => "Advanced",
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

    fn render_sidebar(&mut self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let query = self.search.read(cx).value().trim().to_lowercase();

        let mut nav = div().flex().flex_col().gap(px(1.));
        for (heading, pages) in Page::SECTIONS {
            let shown: Vec<Page> = pages.iter().copied().filter(|p| p.matches(&query)).collect();
            if shown.is_empty() {
                continue;
            }
            nav = nav.child(
                div()
                    .px_2()
                    .pt_3()
                    .pb_1()
                    .text_size(px(11.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(theme::TEXT_MUTED))
                    .child(heading.to_uppercase()),
            );
            for page in shown {
                let selected = self.page == page;
                nav = nav.child(
                    div()
                        .id(("settings-page", page as u32))
                        .h(px(32.))
                        .px_2()
                        .flex()
                        .items_center()
                        .rounded(px(6.))
                        .cursor_pointer()
                        .text_color(rgb(if selected { theme::TEXT_BRIGHT } else { theme::TEXT_MUTED }))
                        .bg(rgb(if selected { theme::BG_ACTIVE } else { theme::BG_SIDEBAR }))
                        .hover(|s| s.bg(rgb(theme::BG_HOVER)))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.select(page, cx)))
                        .child(page.label()),
                );
            }
        }

        div()
            .w(px(240.))
            .h_full()
            .flex_none()
            .flex()
            .flex_col()
            .gap_2()
            .p_3()
            .bg(rgb(theme::BG_SIDEBAR))
            .child(Input::new(&self.search))
            .child(div().id("settings-nav").flex_1().overflow_y_scroll().child(nav))
            .child(
                div()
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
        let page = self.render_page(cx);

        let close = cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(CloseSettings));
        let close_button = div()
            .id("settings-close")
            .absolute()
            .top(px(12.))
            .right(px(12.))
            .size(px(32.))
            .flex()
            .items_center()
            .justify_center()
            .rounded_full()
            .border_1()
            .border_color(rgb(theme::BORDER))
            .cursor_pointer()
            .hover(|s| s.bg(rgb(theme::BG_HOVER)))
            .on_click(close)
            .child(icon("x", 16., theme::TEXT_MUTED));

        let content = div()
            .id("settings-window")
            .occlude()
            .relative()
            .m_4()
            .w_full()
            .max_w(px(960.))
            .h_full()
            .max_h(px(660.))
            .flex()
            .overflow_hidden()
            .rounded(px(10.))
            .border_1()
            .border_color(rgb(theme::BORDER))
            .bg(rgb(theme::BG_CHAT))
            .child(sidebar)
            .child(
                div()
                    .id("settings-page-body")
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .overflow_y_scroll()
                    .px_8()
                    .py_6()
                    .child(page),
            )
            .child(close_button)
            .into_any_element();

        modal(
            content,
            Box::new(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(CloseSettings))),
        )
    }
}
