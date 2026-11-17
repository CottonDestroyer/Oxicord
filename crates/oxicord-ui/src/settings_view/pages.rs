use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, ClickEvent, Context, FontWeight, InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement as _, StatefulInteractiveElement as _,
    Styled as _, div, px, rgb, rgba,
    component::slider::Slider,
};
use oxicord_settings::{Key, Value};

use super::{
    Page, SettingsView,
    widgets::{
        Tone, blurb, button, group, info_row, radio_row, status_line, switch_row, title,
    },
};
use crate::{
    AccountRequest, Settings,
    account::Load,
    danger_dialog::{Danger, OpenDanger},
    icons::icon,
    session::{OpenChangePassword, OpenTotpSetup},
    theme,
};

/// Bit values of the friend request setting. Everyone is its own bit, so
/// turning a narrower source off also drops it.
const FRIENDS_OF_FRIENDS: u32 = 2;
const SERVER_MEMBERS: u32 = 4;
const EVERYONE: u32 = 8;

/// The services the live Connections page offers, in its order: Riot Games,
/// Battle.net, Bluesky, Letterboxd, PayPal, Reddit, Steam, X, eBay,
/// Crunchyroll, PlayStation Network, Spotify and Xbox, then Amazon Music,
/// Bungie.net, Domain, Epic Games, Facebook, GitHub, Roblox, Twitch and
/// YouTube.
const CONNECTION_SERVICES: [&str; 22] = [
    "riotgames", "battlenet", "bluesky", "letterboxd", "paypal", "reddit", "steam", "twitter", "ebay", "crunchyroll",
    "playstation", "spotify", "xbox", "amazon-music", "bungie", "domain", "epicgames", "facebook", "github", "roblox",
    "twitch", "youtube",
];

/// A line on the account page that explains a drastic action beside its button.
fn danger_row(title: &'static str, line: &'static str, action: AnyElement) -> AnyElement {
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .child(
            div()
                .flex()
                .flex_col()
                .child(div().font_weight(FontWeight::MEDIUM).text_color(rgb(theme::TEXT_BRIGHT)).child(title))
                .child(div().text_size(px(14.)).text_color(rgb(theme::TEXT_MUTED)).child(line)),
        )
        .child(action)
        .into_any_element()
}

/// Flips one friend request source in the stored bit mask.
pub(super) fn toggle_friend_source(flags: u32, source: u32) -> u32 {
    let on = flags & source != 0;
    match (source, on) {
        (EVERYONE, false) => flags | 14,
        (EVERYONE, true) => flags & !EVERYONE,
        (bit, false) => flags | bit,
        (bit, true) => flags & !(bit | EVERYONE),
    }
}

/// "ada@example.com" as "a**@example.com".
pub(super) fn mask_email(email: &str) -> String {
    match email.split_once('@') {
        Some((name, domain)) => {
            let first: String = name.chars().take(1).collect();
            let hidden = "*".repeat(name.chars().count().saturating_sub(1).max(2));
            format!("{first}{hidden}@{domain}")
        }
        None => "*".repeat(email.chars().count()),
    }
}

/// "+44 7700 900123" as "*********0123".
pub(super) fn mask_phone(phone: &str) -> String {
    let count = phone.chars().count();
    let tail: String = phone.chars().skip(count.saturating_sub(4)).collect();
    format!("{}{tail}", "*".repeat(count.saturating_sub(4)))
}

impl SettingsView {
    pub(super) fn render_page(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let body = match self.page {
            Page::AccountInfo | Page::PasswordSecurity | Page::AccountStatus | Page::FamilyCenter => {
                self.account_page(cx)
            }
            Page::ContentSocial => self.content_page(cx),
            Page::DataPrivacy => self.privacy_page(cx),
            Page::Apps => self.apps_page(cx),
            Page::Devices => self.devices_page(cx),
            Page::Connections => self.connections_page(cx),
            Page::Appearance => self.appearance_page(cx),
            Page::VoiceVideo => self.voice_page(cx),
            Page::Chat => self.chat_page(cx),
            Page::Notifications => self.notifications_page(cx),
            Page::System => keybinds_page(),
            Page::Advanced => self.advanced_page(cx),
            Page::Nitro
            | Page::ServerBoost
            | Page::Subscriptions
            | Page::GiftInventory
            | Page::Billing
            | Page::Accessibility
            | Page::LanguageTime
            | Page::ActivityPrivacy => Self::placeholder_page(self.page.label()),
        };
        div().flex().flex_col().gap_2().child(body).into_any_element()
    }

    /// A sidebar row whose page is not implemented yet. Keeps navigation 1:1
    /// while the entries in the live client are filled in one at a time.
    fn placeholder_page(heading: &'static str) -> AnyElement {
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(title(heading))
            .child(blurb("This page is not implemented yet."))
            .into_any_element()
    }

    /// Shown instead of a page's controls until the preferences are here.
    fn prefs_gate(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        match &self.session.read(cx).account.settings {
            Load::Ready(_) => None,
            Load::Failed(error) => Some(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(status_line(
                        "triangle-alert",
                        format!("Couldn't load your preferences. {error}"),
                        theme::DANGER,
                    ))
                    .child(button("retry-prefs", "Try again", Tone::Plain, cx.listener(
                        |this, _: &ClickEvent, _, cx| {
                            let backend = this.backend.clone();
                            this.session.update(cx, |s, cx| {
                                s.request_account(&backend, AccountRequest::LoadSettings, cx);
                            });
                        },
                    )))
                    .into_any_element(),
            ),
            _ => Some(status_line(
                "loader-circle",
                "Loading your preferences...".into(),
                theme::TEXT_MUTED,
            )),
        }
    }

    fn pref_switch(
        &mut self,
        id: &'static str,
        label: &'static str,
        hint: Option<&'static str>,
        key: Key,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let on = self.pref(key, cx).as_bool();
        switch_row(id, label, hint, on, cx.listener(move |this, _: &ClickEvent, _, cx| {
            this.toggle_pref(key, cx);
        }))
    }

    fn number_radios(
        &mut self,
        id: &'static str,
        key: Key,
        current: u32,
        options: &[(u32, &'static str, Option<&'static str>)],
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        options
            .iter()
            .map(|&(value, label, hint)| {
                radio_row((id, value), label, hint, current == value, cx.listener(
                    move |this, _: &ClickEvent, _, cx| this.set_pref(key, Value::Number(value), cx),
                ))
            })
            .collect()
    }

    fn account_page(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let (name, username, details, device_count) = {
            let session = self.session.read(cx);
            let me = session.store.me();
            (
                me.map(|m| m.display_name().to_owned()).unwrap_or_default(),
                me.map(|m| m.username.clone()).unwrap_or_default(),
                session.account.details.clone(),
                match &session.account.sessions {
                    Load::Ready(sessions) => sessions.len(),
                    _ => 0,
                },
            )
        };
        let reveal = self.reveal_contact;
        let show = |value: &str, masked: String| if reveal { value.to_owned() } else { masked };
        let mut account_rows: Vec<AnyElement> = vec![
            Self::field_row("Username", username, Some(button("edit-username", "Edit", Tone::Plain, cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(super::OpenEditProfile))))),
        ];
        match &details {
            Load::Ready(d) => {
                account_rows.push(Self::field_row(
                    "Email",
                    d.email.as_deref().map_or("Not set".into(), |e| show(e, mask_email(e))),
                    Some(button("edit-email", "Edit", Tone::Plain, cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(super::OpenEditProfile)))),
                ));
                account_rows.push(Self::field_row(
                    "Phone",
                    d.phone.as_deref().map_or("Not set".into(), |p| show(p, mask_phone(p))),
                    Some(button("edit-phone", "Edit", Tone::Plain, cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(super::OpenEditProfile)))),
                ));
                account_rows.push(Self::field_row(
                    "Email verified",
                    if d.verified { "Yes" } else { "No" }.into(),
                    None,
                ));
                account_rows.push(
                    div().child(button(
                        "reveal-contact",
                        if reveal { "Hide" } else { "Reveal" },
                        Tone::Plain,
                        cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.reveal_contact = !this.reveal_contact;
                            cx.notify();
                        }),
                    )).into_any_element(),
                );
            }
            Load::Failed(error) => account_rows.push(status_line("triangle-alert", error.clone(), theme::DANGER)),
            _ => account_rows.push(status_line("loader-circle", "Loading...".into(), theme::TEXT_MUTED)),
        }

        let session_for_password = self.session.clone();
        let session_for_totp = self.session.clone();
        let mut security_rows: Vec<AnyElement> = vec![
            Self::field_row(
                "Password",
                "••••••••".into(),
                Some(button("edit-password", "Edit", Tone::Plain, move |_, _, cx| {
                    session_for_password.update(cx, |_, cx| cx.emit(OpenChangePassword));
                })),
            ),
        ];
        if let Load::Ready(d) = &details {
            let action = if d.mfa_enabled {
                button("remove-totp", "Remove Authenticator App", Tone::Danger, move |_, _, cx| {
                    session_for_totp.update(cx, |_, cx| cx.emit(OpenDanger(Danger::RemoveTotp)));
                })
            } else {
                button("set-up-mfa", "Set up", Tone::Plain, move |_, _, cx| {
                    session_for_totp.update(cx, |_, cx| cx.emit(OpenTotpSetup));
                })
            };
            security_rows.push(Self::field_row(
                "Multi-Factor Authentication",
                if d.mfa_enabled { "Enabled" } else { "Not enabled" }.into(),
                Some(action),
            ));
        }
        security_rows.push(Self::field_row(
            "Logged-in Devices",
            format!("{device_count} device{}", if device_count == 1 { "" } else { "s" }),
            Some(button("open-devices", "View", Tone::Plain, cx.listener(|this, _: &ClickEvent, _, cx| {
                this.select(Page::Devices, cx);
            }))),
        ));

        let session_for_disable = self.session.clone();
        let session_for_delete = self.session.clone();
        let status_rows = vec![
            Self::field_row("Age Group", "Not set".into(), Some(button("confirm-age", "Confirm Age", Tone::Plain, |_, _, _| {}))),
            Self::field_row("Account Standing", "All good".into(), None),
            Self::field_row("Family Center", "Not set up".into(), Some(button("set-up-family", "Set up Family Center", Tone::Plain, |_, _, _| {}))),
        ];
        let danger_rows = vec![
            danger_row(
                "Disable your account",
                "Temporarily disable your account.",
                button("disable-account", "Disable Account", Tone::Danger, move |_, _, cx| {
                    session_for_disable.update(cx, |_, cx| cx.emit(OpenDanger(Danger::DisableAccount)));
                }),
            ),
            danger_row(
                "Close your account",
                "Permanently close your account.",
                button("delete-account", "Delete Account", Tone::DangerFilled, move |_, _, cx| {
                    session_for_delete.update(cx, |_, cx| cx.emit(OpenDanger(Danger::DeleteAccount)));
                }),
            ),
        ];
        let _ = name;

        div()
            .flex()
            .flex_col()
            .gap(px(24.))
            .child(Self::section("Account Info", account_rows))
            .child(Self::section("Password & Security", security_rows))
            .child(Self::section("Account Status", status_rows))
            .child(Self::section("", danger_rows))
            .into_any_element()
    }

    /// One labelled field with an optional action on the right.
    fn field_row(label: &'static str, value: String, action: Option<AnyElement>) -> AnyElement {
        div()
            .flex()
            .items_center()
            .justify_between()
            .gap_3()
            .py(px(8.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(
                        div()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(theme::TEXT_MUTED))
                            .child(label),
                    )
                    .child(div().text_color(rgb(theme::TEXT_BRIGHT)).child(value)),
            )
            .children(action)
            .into_any_element()
    }

    /// A titled block of account fields.
    fn section(title: &'static str, rows: Vec<AnyElement>) -> AnyElement {
        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .when(!title.is_empty(), |block| {
                block.child(
                    div()
                        .text_size(px(20.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(theme::TEXT_BRIGHT))
                        .child(title),
                )
            })
            .child(div().flex().flex_col().children(rows))
            .into_any_element()
    }

    fn content_page(&mut self, cx: &mut Context<Self>) -> AnyElement {
        if let Some(gate) = self.prefs_gate(cx) {
            return gate;
        }

        let filter = self.pref(Key::ExplicitContentFilter, cx).as_number();
        let filter_rows = self.number_radios(
            "explicit",
            Key::ExplicitContentFilter,
            filter,
            &[
                (0, "Don't scan any media", Some("Messages and images are never checked.")),
                (1, "Scan media from everyone without a role", None),
                (2, "Scan media from everyone", Some("Recommended.")),
            ],
            cx,
        );

        let spam = match self.pref(Key::DmSpamFilter, cx).as_number() {
            0 => 2,
            n => n,
        };
        let spam_rows = self.number_radios(
            "spam",
            Key::DmSpamFilter,
            spam,
            &[
                (1, "Don't filter direct messages", None),
                (2, "Filter messages from people who aren't your friends", None),
                (3, "Filter all direct messages", None),
            ],
            cx,
        );

        let flags = self.pref(Key::FriendSourceFlags, cx).as_number();
        let sources = [
            ("friends-everyone", "Everyone", EVERYONE),
            ("friends-fof", "Friends of friends", FRIENDS_OF_FRIENDS),
            ("friends-server", "Server members", SERVER_MEMBERS),
        ];
        let friend_rows: Vec<AnyElement> = sources
            .into_iter()
            .map(|(id, label, bit)| {
                switch_row(id, label, None, flags & bit != 0, cx.listener(
                    move |this, _: &ClickEvent, _, cx| {
                        let now = this.pref(Key::FriendSourceFlags, cx).as_number();
                        this.set_pref(
                            Key::FriendSourceFlags,
                            Value::Number(toggle_friend_source(now, bit)),
                            cx,
                        );
                    },
                ))
            })
            .collect();

        let restricted = self.pref(Key::DefaultGuildsRestricted, cx).as_bool();
        let dm_rows = vec![
            Self::field_row(
                "Allow direct messages from server members",
                if restricted { "Off" } else { "On" }.into(),
                Some(switch_row(
                    "allow-server-dms-messaging",
                    "",
                    None,
                    !restricted,
                    cx.listener(|this, _: &ClickEvent, _, cx| {
                        let restricted = this.pref(Key::DefaultGuildsRestricted, cx).as_bool();
                        this.set_pref(Key::DefaultGuildsRestricted, Value::Bool(!restricted), cx);
                    }),
                )),
            ),
        ];
        let games_rows = vec![Self::field_row(
            "Messaging in Connected Games",
            "Choose who can message you while you play.".into(),
            Some(button("connected-games", "Manage", Tone::Plain, |_, _, _| {})),
        )];
        let block_rows = vec![Self::field_row(
            "Ignore & Block",
            "Manage the people you have ignored or blocked.".into(),
            Some(button("manage-blocks", "Manage", Tone::Plain, |_, _, _| {})),
        )];

        div()
            .flex()
            .flex_col()
            .gap(px(24.))
            .child(Self::section("Content Filters", filter_rows))
            .child(Self::section("Spam Filters", spam_rows))
            .child(Self::section("Direct Message permissions", dm_rows))
            .child(Self::section("Friend Request permissions", friend_rows))
            .child(Self::section("Messaging in Connected Games", games_rows))
            .child(Self::section("Ignore & Block", block_rows))
            .into_any_element()
    }

    fn privacy_page(&mut self, cx: &mut Context<Self>) -> AnyElement {
        if let Some(gate) = self.prefs_gate(cx) {
            return gate;
        }

        // Stored the other way round: true means servers may not message us.
        let restricted = self.pref(Key::DefaultGuildsRestricted, cx).as_bool();
        let rows = vec![
            Self::field_row(
                "Allow direct messages from server members",
                if restricted { "Off" } else { "On" }.into(),
                Some(switch_row(
                    "allow-server-dms",
                    "",
                    None,
                    !restricted,
                    cx.listener(|this, _: &ClickEvent, _, cx| {
                        let restricted = this.pref(Key::DefaultGuildsRestricted, cx).as_bool();
                        this.set_pref(Key::DefaultGuildsRestricted, Value::Bool(!restricted), cx);
                    }),
                )),
            ),
            Self::field_row(
                "Share your detected activity as a status",
                if self.pref(Key::ShowCurrentGame, cx).as_bool() { "On" } else { "Off" }.into(),
                Some(self.pref_switch(
                    "show-game",
                    "",
                    None,
                    Key::ShowCurrentGame,
                    cx,
                )),
            ),
            Self::field_row(
                "Show your local time on your profile",
                if self.pref(Key::ShowLocalTime, cx).as_bool() { "On" } else { "Off" }.into(),
                Some(self.pref_switch(
                    "show-local-time",
                    "",
                    None,
                    Key::ShowLocalTime,
                    cx,
                )),
            ),
        ];

        let data_rows = vec![
            Self::field_row(
                "How Discord Uses My Data",
                "We use your data to make Discord work, personalise your experience and keep the service safe.".into(),
                Some(button("request-data", "Request my data", Tone::Plain, |_, _, _| {})),
            ),
            Self::field_row(
                "Activity Privacy",
                "Choose who can see the activity you share.".into(),
                Some(button("open-activity-privacy", "Open", Tone::Plain, cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.select(Page::ActivityPrivacy, cx);
                }))),
            ),
        ];

        div()
            .flex()
            .flex_col()
            .gap(px(24.))
            .child(Self::section("Data", data_rows))
            .child(Self::section("Profile Privacy", rows))
            .into_any_element()
    }

    fn apps_page(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let mut page = div()
            .flex()
            .flex_col()
            .gap_2()
            .child(title("Authorized Apps"))
            .child(blurb("Apps and bots you have given access to your account."));

        let apps = self.session.read(cx).account.apps.clone();
        let rows: Vec<AnyElement> = match &apps {
            Load::Ready(apps) if apps.is_empty() => {
                vec![status_line("circle-check", "You haven't authorized any apps.".into(), theme::TEXT_MUTED)]
            }
            Load::Ready(apps) => apps
                .iter()
                .map(|app| {
                    let id = app.id;
                    let armed = self.confirm_revoke == Some(id);
                    let scopes = match app.scopes.len() {
                        0 => "No permissions listed".to_owned(),
                        n if n > 3 => format!("{} and {} more", app.scopes[..3].join(", "), n - 3),
                        _ => app.scopes.join(", "),
                    };
                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .child(div().text_color(rgb(theme::TEXT_BRIGHT)).child(app.application.name.clone()))
                                .child(
                                    div()
                                        .truncate()
                                        .text_size(px(12.))
                                        .text_color(rgb(theme::TEXT_MUTED))
                                        .child(scopes),
                                ),
                        )
                        .child(button(
                            ("revoke", id.0 as u32),
                            if armed { "Confirm" } else { "Deauthorize" },
                            Tone::Danger,
                            cx.listener(move |this, _: &ClickEvent, _, cx| {
                                if this.confirm_revoke == Some(id) {
                                    this.confirm_revoke = None;
                                    let backend = this.backend.clone();
                                    this.session.update(cx, |s, cx| {
                                        s.request_account(&backend, AccountRequest::RevokeApp(id), cx);
                                    });
                                } else {
                                    this.confirm_revoke = Some(id);
                                    cx.notify();
                                }
                            }),
                        ))
                        .into_any_element()
                })
                .collect(),
            Load::Failed(error) => vec![status_line("triangle-alert", error.clone(), theme::DANGER)],
            _ => vec![status_line("loader-circle", "Loading...".into(), theme::TEXT_MUTED)],
        };
        page = page.child(group("Apps", rows));
        page.into_any_element()
    }

    fn devices_page(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let sessions = self.session.read(cx).account.sessions.clone();
        let rows: Vec<AnyElement> = match &sessions {
            Load::Ready(list) if list.is_empty() => {
                vec![status_line("monitor", "No other sessions are listed.".into(), theme::TEXT_MUTED)]
            }
            Load::Ready(list) => list
                .iter()
                .map(|s| {
                    let device = [s.client_info.platform.as_deref(), s.client_info.os.as_deref()]
                        .into_iter()
                        .flatten()
                        .collect::<Vec<_>>()
                        .join(" on ");
                    let device = if device.is_empty() { "Unknown device".to_owned() } else { device };
                    let place = s.client_info.location.clone().unwrap_or_default();
                    let when = s
                        .approx_last_used_time
                        .as_deref()
                        .map(|t| format!("Last used {}", t.get(..10).unwrap_or(t)))
                        .unwrap_or_default();
                    div()
                        .flex()
                        .flex_col()
                        .child(div().text_color(rgb(theme::TEXT_BRIGHT)).child(device))
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(rgb(theme::TEXT_MUTED))
                                .child(format!("{place}  {when}").trim().to_owned()),
                        )
                        .into_any_element()
                })
                .collect(),
            Load::Failed(error) => vec![status_line("triangle-alert", error.clone(), theme::DANGER)],
            _ => vec![status_line("loader-circle", "Loading...".into(), theme::TEXT_MUTED)],
        };
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(title("Devices"))
            .child(blurb("Where your account is signed in. Sign out of other devices from Discord's own settings."))
            .child(group("Sessions", rows))
            .child(div().mt_3().child(button("logout-others", "Log Out Other Devices", Tone::Danger, cx.listener(
                |this, _: &ClickEvent, _, cx| {
                    let backend = this.backend.clone();
                    this.session.update(cx, |s, cx| {
                        s.request_account(&backend, AccountRequest::LogoutOtherSessions, cx);
                    });
                },
            ))))
            .into_any_element()
    }

    fn connections_page(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let connections = self.session.read(cx).account.connections.clone();

        // The grid the live page opens with: 44 px tiles 8 apart, each with the
        // service's logo at 28.
        let tiles: Vec<AnyElement> = CONNECTION_SERVICES
            .iter()
            .map(|kind| {
                let (backend, session) = (self.backend.clone(), self.session.clone());
                let kind = (*kind).to_owned();
                let logo = crate::profile::logo_file(&kind);
                div()
                    .id(gpui_kit::ElementId::Name(format!("add-connection-{kind}").into()))
                    .size(px(44.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(8.))
                    .bg(rgba(0x97979f1f))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgba(0x97979f33)))
                    .on_click(move |_, _, cx| {
                        let (backend, kind) = (backend.clone(), kind.clone());
                        session.update(cx, |s, cx| {
                            s.request_account(&backend, AccountRequest::AuthorizeConnection(kind), cx);
                        });
                    })
                    .child(match logo {
                        Some(file) => gpui_kit::img(gpui_kit::SharedString::from(format!("connections/{file}")))
                            .size(px(28.))
                            .into_any_element(),
                        None => icon("link", 20., theme::TEXT_MUTED).into_any_element(),
                    })
                    .into_any_element()
            })
            .collect();

        let linked: Vec<AnyElement> = match &connections {
            Load::Ready(list) => list
                .iter()
                .map(|c| {
                    let kind: String = c
                        .kind
                        .chars()
                        .enumerate()
                        .map(|(i, ch)| if i == 0 { ch.to_ascii_uppercase() } else { ch })
                        .collect();
                    let backend = self.backend.clone();
                    let session = self.session.clone();
                    let kind_for_remove = c.kind.clone();
                    let id_for_remove = c.id.clone();
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .child(div().text_color(rgb(theme::TEXT_BRIGHT)).child(kind))
                                .child(
                                    div()
                                        .text_size(px(12.))
                                        .text_color(rgb(theme::TEXT_MUTED))
                                        .child(c.name.clone()),
                                ),
                        )
                        .children(c.verified.then(|| {
                            div()
                                .text_size(px(12.))
                                .text_color(rgb(theme::ONLINE))
                                .child("Verified")
                        }))
                        .child(button("remove-connection", "Remove", Tone::Danger, move |_, _, cx| {
                            let backend = backend.clone();
                            let session = session.clone();
                            let kind = kind_for_remove.clone();
                            let id = id_for_remove.clone();
                            session.update(cx, |s, cx| {
                                s.request_account(&backend, AccountRequest::RemoveConnection { kind, id }, cx);
                            });
                        }))
                        .into_any_element()
                })
                .collect(),
            Load::Failed(error) => vec![status_line("triangle-alert", error.clone(), theme::DANGER)],
            Load::Loading => vec![status_line("loader-circle", "Loading...".into(), theme::TEXT_MUTED)],
            Load::Idle => Vec::new(),
        };

        let empty = matches!(&connections, Load::Ready(list) if list.is_empty());
        div()
            .flex()
            .flex_col()
            .child(
                div()
                    .text_size(px(24.))
                    .line_height(px(32.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(theme::TEXT_BRIGHT))
                    .child("Connections"),
            )
            .child(
                div()
                    .mt(px(4.))
                    .text_size(px(14.))
                    .line_height(px(18.))
                    .text_color(rgb(theme::TEXT_MUTED))
                    .child(
                        "These are accounts from other platforms. Connecting them here can unlock new features \
                         on Discord, like the ability to share what you're currently listening to, get paid \
                         safely and securely, and more.",
                    ),
            )
            .child(
                div()
                    .mt(px(29.))
                    .text_size(px(16.))
                    .line_height(px(20.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(theme::TEXT_BRIGHT))
                    .child("Add a new connection"),
            )
            .child(div().mt(px(16.)).flex().flex_wrap().gap_x(px(8.)).gap_y(px(9.)).children(tiles))
            .child(div().mt(px(24.)).flex().flex_col().gap_2().children(linked))
            .children(empty.then(|| {
                div()
                    .mt(px(64.))
                    .flex()
                    .flex_col()
                    .items_center()
                    .child(
                        div()
                            .text_size(px(20.))
                            .line_height(px(24.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(theme::TEXT_MUTED))
                            .child("No Connections"),
                    )
                    .child(
                        div()
                            .mt(px(8.))
                            .text_size(px(16.))
                            .line_height(px(20.))
                            .text_color(rgb(theme::TEXT_MUTED))
                            .child("Connect your accounts to unlock special Discord integrations"),
                    )
            }))
            .into_any_element()
    }

    fn appearance_page(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let mut page = div().flex().flex_col().gap_2().child(title("Appearance"));

        let members = self.session.read(cx).settings.show_members;
        let layout = switch_row(
            "layout-members",
            "Show the member list",
            Some("Keep the list of people beside server channels."),
            members,
            cx.listener(|this, _: &ClickEvent, _, cx| {
                let backend = this.backend.clone();
                this.session.update(cx, |s, cx| {
                    s.change_settings(&backend, |st| st.show_members = !st.show_members, cx);
                });
            }),
        );
        page = page.child(group("Layout", vec![layout]));

        match self.prefs_gate(cx) {
            Some(gate) => page = page.child(gate),
            None => {
                let cycle = self.pref(Key::HourCycle, cx).as_number();
                let rows = self.number_radios(
                    "hour-cycle",
                    Key::HourCycle,
                    cycle,
                    &[
                        (0, "Automatic", Some("24 hour clock.")),
                        (1, "12 hour", Some("3:30 PM")),
                        (2, "24 hour", Some("15:30")),
                    ],
                    cx,
                );
                page = page.child(group("Time format", rows));
            }
        }
        page.child(blurb("Oxicord only has its dark theme for now."))
            .into_any_element()
    }

    fn chat_page(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let mut page = div().flex().flex_col().gap_2().child(title("Chat"));
        if let Some(gate) = self.prefs_gate(cx) {
            return page.child(gate).into_any_element();
        }

        let embeds = vec![
            self.pref_switch(
                "render-embeds",
                "Show website previews",
                Some("Preview links as cards beneath the message."),
                Key::RenderEmbeds,
                cx,
            ),
            self.pref_switch(
                "embed-media",
                "Show images and videos from links",
                None,
                Key::InlineEmbedMedia,
                cx,
            ),
            self.pref_switch(
                "attachment-media",
                "Show uploaded images inline",
                Some("Otherwise they appear as file cards."),
                Key::InlineAttachmentMedia,
                cx,
            ),
            self.pref_switch("render-reactions", "Show reactions on messages", None, Key::RenderReactions, cx),
        ];

        let spoilers = self.pref(Key::RenderSpoilers, cx).as_text().to_owned();
        let spoiler_rows: Vec<AnyElement> = [
            ("ALWAYS", "Always", "Spoilers are shown without clicking."),
            ("ON_CLICK", "On click", "Click a spoiler to reveal it."),
            ("IF_MODERATOR", "In servers I moderate", "Everywhere else they are hidden."),
        ]
        .into_iter()
        .enumerate()
        .map(|(i, (value, label, hint))| {
            radio_row(("spoilers", i as u32), label, Some(hint), spoilers == value, cx.listener(
                move |this, _: &ClickEvent, _, cx| {
                    this.set_pref(Key::RenderSpoilers, Value::Text(value.into()), cx);
                },
            ))
        })
        .collect();

        page = page
            .child(group("Links and media", embeds))
            .child(group("Show spoiler content", spoiler_rows));
        page.into_any_element()
    }

    fn voice_page(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let percent = self.session.read(cx).settings.input_volume;
        let slider = div()
            .flex()
            .items_center()
            .gap_3()
            .child(div().flex_1().child(Slider::new(&self.input_volume).accessibility_label("Input Volume")))
            .child(
                div()
                    .w(px(48.))
                    .flex_none()
                    .text_right()
                    .text_color(rgb(theme::TEXT))
                    .child(format!("{percent}%")),
            )
            .into_any_element();
        let suppression = switch_row(
            "noise-suppression",
            "Enable noise suppression",
            Some("Takes keyboard clatter, fans and hiss out of your microphone. Uses RNNoise, not Krisp."),
            self.session.read(cx).settings.noise_suppression,
            cx.listener(|this, _: &ClickEvent, _, cx| {
                let backend = this.backend.clone();
                this.session.update(cx, |s, cx| s.change_settings(&backend, |st| st.noise_suppression = !st.noise_suppression, cx));
            }),
        );
        let (on, key) = {
            let settings = &self.session.read(cx).settings;
            (settings.push_to_talk, settings.push_to_talk_key.clone())
        };
        let toggle = switch_row(
            "ptt-enabled",
            "Push to talk",
            Some("Your microphone is sent only while you hold the key, even when another program has the focus."),
            on,
            cx.listener(|this, _: &ClickEvent, _, cx| {
                let backend = this.backend.clone();
                this.session.update(cx, |s, cx| s.change_settings(&backend, |st| st.push_to_talk = !st.push_to_talk, cx));
            }),
        );
        let capturing = self.capturing_key;
        let shown = if capturing {
            "Press a key".to_owned()
        } else if key.is_empty() {
            "Not set".to_owned()
        } else {
            crate::talk_key::label(&key)
        };
        let key_button = div()
            .id("ptt-key")
            .track_focus(&self.key_focus)
            .h(px(32.))
            .min_w(px(96.))
            .px_3()
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(theme::RADIUS))
            .border_1()
            .border_color(rgb(if capturing { theme::ACCENT } else { theme::BG_ACTIVE }))
            .bg(rgb(theme::BG_ACTIVE))
            .text_color(rgb(theme::TEXT_BRIGHT))
            .cursor_pointer()
            .hover(|s| s.opacity(0.85))
            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                this.capturing_key = true;
                window.focus(&this.key_focus, cx);
                cx.notify();
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                if !this.capturing_key {
                    return;
                }
                let name = event.keystroke.key.as_str();
                let usable = crate::talk_key::virtual_key(name).is_some();
                if usable {
                    let name = name.to_owned();
                    let backend = this.backend.clone();
                    this.session.update(cx, |s, cx| s.change_settings(&backend, |st| st.push_to_talk_key = name, cx));
                }
                // Escape gives up, any key that cannot be used is ignored.
                if usable || name == "escape" {
                    this.capturing_key = false;
                    cx.stop_propagation();
                    cx.notify();
                }
            }))
            .child(shown);
        let shortcut = div()
            .flex()
            .items_center()
            .justify_between()
            .gap_3()
            .child(
                div()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(theme::TEXT_MUTED))
                    .child("Shortcut"),
            )
            .child(key_button)
            .into_any_element();
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(title("Voice & Video"))
            .child(group("Input Volume", vec![slider]))
            .child(blurb("How loud your microphone is for everyone in the call."))
            .child(group("Noise Suppression", vec![suppression]))
            .child(blurb("Applies before your voice is sent, so everyone in the call hears it."))
            .child(group("Input Mode", vec![toggle, shortcut]))
            .child(blurb(
                "Push to talk needs a shortcut. Any letter, number or function key works. Escape cancels.",
            ))
            .into_any_element()
    }

    fn notifications_page(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let on = self.session.read(cx).settings.desktop_notifications;
        let mut overview = Vec::new();
        overview.push(Self::field_row(
            "Enable desktop notifications",
            if on { "On" } else { "Off" }.into(),
            Some(switch_row(
                "notify-desktop",
                "",
                None,
                on,
                cx.listener(|this, _: &ClickEvent, _, cx| {
                    let backend = this.backend.clone();
                    this.session.update(cx, |s, cx| {
                        s.change_settings(
                            &backend,
                            |st| st.desktop_notifications = !st.desktop_notifications,
                            cx,
                        );
                    });
                }),
            )),
        ));
        overview.push(Self::field_row(
            "Do Not Disturb",
            "Silences notifications entirely.".into(),
            None,
        ));
        let sounds = vec![Self::field_row(
            "Message notifications",
            "The sound played for a new message.".into(),
            Some(button("message-sound", "Change", Tone::Plain, |_, _, _| {})),
        )];
        let badges = vec![Self::field_row(
            "Show mention badges",
            "Show a badge on the app icon for unread mentions.".into(),
            None,
        )];
        let email = vec![
            Self::field_row(
                "Email notifications",
                "Receive an email when you miss a mention or direct message.".into(),
                None,
            ),
            Self::field_row(
                "Marketing emails",
                "Product news and announcements.".into(),
                Some(button("unsubscribe-email", "Unsubscribe from all marketing emails", Tone::Plain, |_, _, _| {})),
            ),
        ];
        let advanced = vec![Self::field_row(
            "Advanced",
            "More notification controls.".into(),
            Some(button("show-advanced-notifications", "Show Advanced Notification Settings", Tone::Plain, |_, _, _| {})),
        )];
        div()
            .flex()
            .flex_col()
            .gap(px(24.))
            .child(Self::section("Overview", overview))
            .child(Self::section("Sounds", sounds))
            .child(Self::section("Badges", badges))
            .child(Self::section("Email", email))
            .child(Self::section("Advanced", advanced))
            .into_any_element()
    }

    fn advanced_page(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let mut page = div().flex().flex_col().gap_2().child(title("Advanced"));

        if self.prefs_gate(cx).is_none() {
            let developer = self.pref_switch(
                "developer-mode",
                "Developer mode",
                Some("Stored on your account. Lets tools show IDs, which this app doesn't use yet."),
                Key::DeveloperMode,
                cx,
            );
            page = page.child(group("Developers", vec![developer]));
        }

        let reset = button("reset-local", "Reset app settings", Tone::Danger, cx.listener(
            |this, _: &ClickEvent, _, cx| {
                let backend = this.backend.clone();
                this.session.update(cx, |s, cx| {
                    s.change_settings(&backend, |st| *st = Settings::default(), cx);
                });
            },
        ));
        page.child(group(
            "This app",
            vec![
                info_row("Version", env!("CARGO_PKG_VERSION").to_owned()),
                reset,
            ],
        ))
        .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn everyone_turns_on_every_source_and_off_only_itself() {
        assert_eq!(toggle_friend_source(0, EVERYONE), 14);
        assert_eq!(toggle_friend_source(14, EVERYONE), 6);
    }

    #[test]
    fn dropping_a_narrow_source_drops_everyone_too() {
        assert_eq!(toggle_friend_source(14, SERVER_MEMBERS), 2);
        assert_eq!(toggle_friend_source(2, SERVER_MEMBERS), 6);
        assert_eq!(toggle_friend_source(6, FRIENDS_OF_FRIENDS), 4);
    }

    #[test]
    fn contact_details_are_masked() {
        assert_eq!(mask_email("ada@example.com"), "a**@example.com");
        assert_eq!(mask_email("a@example.com"), "a**@example.com");
        assert_eq!(mask_phone("+447700900123"), "*********0123");
    }
}

/// Every shortcut, grouped the way Discord's page groups them.
fn keybinds_page() -> AnyElement {
    let mut page = div()
        .flex()
        .flex_col()
        .gap_2()
        .child(title("Keybinds"))
        .child(blurb("Discord's default shortcuts, for what this app can do. They can't be changed yet."));
    let mut groups: Vec<&'static str> = Vec::new();
    for s in crate::keys::SHORTCUTS {
        if !groups.contains(&s.group) {
            groups.push(s.group);
        }
    }
    for heading in groups {
        let rows = crate::keys::SHORTCUTS
            .iter()
            .filter(|s| s.group == heading)
            .map(|s| {
                let caps = crate::keys::caps(s.keys).into_iter().map(|cap| {
                    div()
                        .min_w(px(24.))
                        .h(px(24.))
                        .px_2()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(4.))
                        .border_1()
                        .border_b_2()
                        .border_color(rgb(theme::BORDER))
                        .bg(rgb(theme::BG_ACTIVE))
                        .text_size(px(12.))
                        .text_color(rgb(theme::TEXT_BRIGHT))
                        .child(cap)
                });
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(div().text_color(rgb(theme::TEXT)).child(s.label))
                    .child(div().flex().gap_1().children(caps))
                    .into_any_element()
            })
            .collect();
        page = page.child(group(heading, rows));
    }
    page.into_any_element()
}
