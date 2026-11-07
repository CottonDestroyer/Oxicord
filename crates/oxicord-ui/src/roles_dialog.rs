//! The "Edit Roles" dialog: Discord lists a server's roles so a moderator can
//! toggle which ones a member holds.

use std::collections::HashSet;

use gpui_kit::{
    ClickEvent, Context, Entity, EventEmitter, FontWeight, InteractiveElement as _, IntoElement, KeyDownEvent,
    ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _, Window, div, px, rgb,
};
use oxicord_model::{Role, Snowflake};

use crate::{backend::{Backend, Command}, profile::modal, session::Session, theme};

pub struct CloseRoles;

impl EventEmitter<CloseRoles> for RolesDialog {}

pub struct RolesDialog {
    session: Entity<Session>,
    backend: Backend,
    target: Option<(Snowflake, Snowflake, String)>,
    /// Every assignable role in the guild (sorted highest first), excluding
    /// `@everyone` and bot-managed roles.
    roles: Vec<Role>,
    /// The ids the moderator has ticked, including `@everyone`.
    checked: HashSet<Snowflake>,
}

impl RolesDialog {
    pub fn new(session: Entity<Session>, backend: Backend, _: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self { session, backend, target: None, roles: Vec::new(), checked: HashSet::new() }
    }

    /// Opens for `user` in `guild`, pre-ticking their current roles.
    pub fn opened(&mut self, guild: Snowflake, user: Snowflake, name: String, _: &mut Window, cx: &mut Context<Self>) {
        let (all, current) = {
            let session = self.session.read(cx);
            let store = &session.store;
            let all: Vec<Role> = store.guild(guild).map(|g| g.roles.clone()).unwrap_or_default();
            let current: Vec<Snowflake> = store
                .member_in_guild(guild, user)
                .map(|(_, roles)| roles.iter().map(|r| r.id).collect())
                .unwrap_or_default();
            (all, current)
        };
        // Exclude @everyone (id == guild) and managed roles from the list; they
        // can't be assigned by hand. @everyone is always implicitly present.
        self.roles = all
            .into_iter()
            .filter(|r| r.id != guild && !r.managed)
            .collect();
        self.checked = current.into_iter().collect();
        self.checked.insert(guild); // @everyone
        self.target = Some((guild, user, name));
        cx.notify();
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        let Some((guild, user, _)) = self.target else { return };
        let roles: Vec<Snowflake> = self.checked.iter().copied().collect();
        self.backend.send(Command::SetMemberRoles { guild, user, roles });
        cx.emit(CloseRoles);
    }
}

impl Render for RolesDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (name, has_target) = match &self.target {
            Some((_, _, name)) => (name.clone(), true),
            None => (String::new(), false),
        };
        let roles = &self.roles;
        let checked = &self.checked;

        let list = div()
            .id("roles-list")
            .flex()
            .flex_col()
            .gap(px(2.))
            .max_h(px(360.))
            .overflow_y_scroll()
            .children(roles.iter().map(|role| {
                let id = role.id;
                let is_checked = checked.contains(&id);
                let color = if role.color != 0 { rgb(role.color) } else { rgb(theme::TEXT_MUTED) };
                div()
                    .id(("role-row", id.0))
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .h(px(36.))
                    .px(px(8.))
                    .rounded(px(theme::RADIUS))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(theme::BG_HOVER)))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        if this.checked.contains(&id) {
                            this.checked.remove(&id);
                        } else {
                            this.checked.insert(id);
                        }
                        cx.notify();
                    }))
                    .child(
                        div()
                            .size(px(18.))
                            .rounded(px(4.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .border_1()
                            .border_color(rgb(theme::BORDER))
                            .bg(if is_checked { rgb(theme::ACCENT) } else { rgb(theme::BG_HOVER) })
                            .child(if is_checked { "✓" } else { "" }),
                    )
                    .child(
                        div().text_size(px(14.)).text_color(color).child(role.name.clone()),
                    )
            }));

        let card = div()
            .id("roles-card")
            .occlude()
            .w(px(440.))
            .p(px(24.))
            .flex()
            .flex_col()
            .gap(px(16.))
            .rounded(px(theme::RADIUS))
            .border_1()
            .border_color(rgb(theme::BORDER_MUTED))
            .bg(rgb(theme::BG_PANEL))
            .capture_key_down(cx.listener(|_, event: &KeyDownEvent, _, cx| {
                if event.keystroke.key == "escape" {
                    cx.emit(CloseRoles);
                    cx.stop_propagation();
                }
            }))
            .child(
                div()
                    .text_size(px(18.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(theme::HEADER_PRIMARY))
                    .child(format!("Roles — {name}")),
            )
            .child(
                div()
                    .text_size(px(13.))
                    .text_color(rgb(theme::TEXT_MUTED))
                    .child("Toggle the roles this member should have."),
            )
            .child(list)
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(8.))
                    .child(
                        div()
                            .id("roles-cancel")
                            .h(px(40.))
                            .px(px(20.))
                            .flex()
                            .items_center()
                            .rounded(px(theme::RADIUS))
                            .cursor_pointer()
                            .text_size(px(14.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(theme::TEXT_BRIGHT))
                            .bg(rgb(theme::BG_HOVER))
                            .hover(|s| s.opacity(0.85))
                            .child("Cancel")
                            .on_click(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(CloseRoles))),
                    )
                    .child(
                        div()
                            .id("roles-save")
                            .h(px(40.))
                            .px(px(20.))
                            .flex()
                            .items_center()
                            .rounded(px(theme::RADIUS))
                            .cursor_pointer()
                            .text_size(px(14.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(0xffffff))
                            .bg(rgb(theme::ACCENT))
                            .hover(|s| s.opacity(0.85))
                            .child("Save")
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.save(cx))),
                    ),
            );

        if has_target {
            modal(card.into_any_element(), Box::new(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(CloseRoles))))
        } else {
            div().into_any_element()
        }
    }
}
