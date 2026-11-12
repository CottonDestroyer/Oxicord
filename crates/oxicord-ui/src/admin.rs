//! Server management requests and the lists its settings pages show. Edits
//! need no state here: the gateway reports the change and the store has it.

use oxicord_model::{
    Snowflake,
    admin::{AuditLog, AutoModRule, BanEntry, IntegrationEntry, InviteEntry, WebhookEntry},
};
use serde_json::Value;

use crate::account::Load;

/// What the UI asks the backend to do about a server.
#[derive(Debug)]
pub enum AdminRequest {
    LoadInvites(Snowflake),
    RevokeInvite { guild: Snowflake, code: String },
    LoadBans(Snowflake),
    Unban { guild: Snowflake, user: Snowflake },
    LoadAudit(Snowflake),
    EditGuild { guild: Snowflake, fields: Value },
    /// The screening form and the card beside it.
    LoadMemberVerification(Snowflake),
    /// The server's own tag, for the header menu. A read of its public profile.
    LoadGuildTag(Snowflake),
    /// Answers the screening form. The gateway clears `pending` on success.
    SubmitMemberVerification {
        guild: Snowflake,
        form: oxicord_model::VerificationForm,
        answers: Vec<oxicord_model::VerificationResponse>,
    },
    SetServerAvatar { guild: Snowflake, avatar: Option<String> },
    SetServerBanner { guild: Snowflake, banner: Option<String> },
    CreateRole { guild: Snowflake, name: String },
    EditRole { guild: Snowflake, role: Snowflake, fields: Value },
    DeleteRole { guild: Snowflake, role: Snowflake },
    RenameEmoji { guild: Snowflake, emoji: Snowflake, name: String },
    DeleteEmoji { guild: Snowflake, emoji: Snowflake },
    CreateChannel { guild: Snowflake, name: String, kind: u8, parent: Option<Snowflake> },
    EditChannel { channel: Snowflake, fields: Value },
    DeleteChannel(Snowflake),
    /// What a role (`kind` 0) or member (1) may do in a channel. The gateway
    /// reports the change back as a channel update.
    SetOverwrite {
        channel: Snowflake,
        target: Snowflake,
        kind: u8,
        allow: oxicord_model::Permissions,
        deny: oxicord_model::Permissions,
    },
    /// Back to inheriting for a role or member in a channel.
    ClearOverwrite { channel: Snowflake, target: Snowflake },
    LoadAutomod(Snowflake),
    /// A full rule body, as `oxicord_model::admin::keyword_rule_body` builds it.
    CreateAutomod { guild: Snowflake, body: Value },
    /// Some fields of a rule: its name, `enabled`, its words.
    EditAutomod { guild: Snowflake, rule: Snowflake, fields: Value },
    DeleteAutomod { guild: Snowflake, rule: Snowflake },
    LoadIntegrations(Snowflake),
    DeleteIntegration { guild: Snowflake, integration: Snowflake },
    LoadWelcome(Snowflake),
    /// A body as `WelcomeScreen::body` builds it.
    SaveWelcome { guild: Snowflake, body: Value },
    LoadOnboardingSetup(Snowflake),
    /// A body as `Onboarding::edit_body` builds it.
    SaveOnboardingSetup { guild: Snowflake, body: Value },
    LoadTemplates(Snowflake),
    /// A body as `oxicord_model::template_body` builds it.
    CreateTemplate { guild: Snowflake, body: Value },
    EditTemplate { guild: Snowflake, code: String, body: Value },
    SyncTemplate { guild: Snowflake, code: String },
    DeleteTemplate { guild: Snowflake, code: String },
    /// How many members a prune of this many days would remove.
    PreviewPrune { guild: Snowflake, days: u32 },
    /// Removes them. Admin write, never sent from the real account in tests.
    PruneMembers { guild: Snowflake, days: u32 },
    LoadWebhooks(Snowflake),
    CreateWebhook { guild: Snowflake, channel: Snowflake, name: String },
    EditWebhook { guild: Snowflake, webhook: Snowflake, name: String, channel: Snowflake },
    DeleteWebhook { guild: Snowflake, webhook: Snowflake },
}

/// The answers, each tagged with the server it was for.
#[derive(Debug)]
pub enum AdminEvent {
    Invites(Snowflake, Result<Vec<InviteEntry>, String>),
    Bans(Snowflake, Result<Vec<BanEntry>, String>),
    Webhooks(Snowflake, Result<Vec<WebhookEntry>, String>),
    Integrations(Snowflake, Result<Vec<IntegrationEntry>, String>),
    Automod(Snowflake, Result<Vec<AutoModRule>, String>),
    Welcome(Snowflake, Result<oxicord_model::WelcomeScreen, String>),
    OnboardingSetup(Snowflake, Result<oxicord_model::Onboarding, String>),
    Templates(Snowflake, Result<Vec<oxicord_model::GuildTemplate>, String>),
    /// A prune preview (`done` false) or its result (true), with its days.
    Prune(Snowflake, u32, bool, Result<u64, String>),
    Audit(Snowflake, Result<AuditLog, String>),
    MemberVerification(Snowflake, Box<Result<oxicord_model::VerificationInfo, String>>),
    /// How answering the screening form went.
    Verified(Snowflake, Result<(), String>),
    /// The tag a server hands out, or none when it has no tag.
    GuildTag(Snowflake, Option<oxicord_model::ServerTag>),
}

/// The lists last loaded, for the server whose settings are open.
#[derive(Default)]
pub struct AdminState {
    pub guild: Option<Snowflake>,
    pub invites: Load<Vec<InviteEntry>>,
    pub bans: Load<Vec<BanEntry>>,
    pub webhooks: Load<Vec<WebhookEntry>>,
    pub integrations: Load<Vec<IntegrationEntry>>,
    pub automod: Load<Vec<AutoModRule>>,
    pub welcome: Load<oxicord_model::WelcomeScreen>,
    pub onboarding: Load<oxicord_model::Onboarding>,
    pub templates: Load<Vec<oxicord_model::GuildTemplate>>,
    pub prune: Load<oxicord_model::PruneState>,
    pub audit: Load<AuditLog>,
}

impl AdminState {
    /// Starts fresh for another server so its lists don't show stale rows.
    pub fn switch_to(&mut self, guild: Snowflake) {
        if self.guild != Some(guild) {
            *self = Self { guild: Some(guild), ..Self::default() };
        }
    }

    pub fn apply(&mut self, event: AdminEvent) {
        match event {
            AdminEvent::Invites(g, r) if self.guild == Some(g) => self.invites = Load::from_result(r),
            AdminEvent::Bans(g, r) if self.guild == Some(g) => self.bans = Load::from_result(r),
            AdminEvent::Webhooks(g, r) if self.guild == Some(g) => self.webhooks = Load::from_result(r),
            AdminEvent::Integrations(g, r) if self.guild == Some(g) => self.integrations = Load::from_result(r),
            AdminEvent::Automod(g, r) if self.guild == Some(g) => self.automod = Load::from_result(r),
            AdminEvent::Welcome(g, r) if self.guild == Some(g) => self.welcome = Load::from_result(r),
            AdminEvent::OnboardingSetup(g, r) if self.guild == Some(g) => self.onboarding = Load::from_result(r),
            AdminEvent::Templates(g, r) if self.guild == Some(g) => self.templates = Load::from_result(r),
            AdminEvent::Prune(g, days, done, r) if self.guild == Some(g) => {
                self.prune = Load::from_result(r.map(|count| oxicord_model::PruneState { days, count, done }));
            }
            AdminEvent::Audit(g, r) if self.guild == Some(g) => self.audit = Load::from_result(r),
            AdminEvent::MemberVerification(..) | AdminEvent::Verified(..) | AdminEvent::GuildTag(..) => {}
            _ => {}
        }
    }

    /// GUILD_BAN_ADD and GUILD_BAN_REMOVE: keeps a loaded ban list current.
    /// The gateway doesn't carry the reason, so a new row has none.
    pub fn apply_ban(&mut self, guild: Snowflake, user: oxicord_model::User, banned: bool) {
        let Load::Ready(bans) = &mut self.bans else { return };
        if self.guild != Some(guild) {
            return;
        }
        bans.retain(|b| b.user.id != user.id);
        if banned {
            bans.push(BanEntry { user, reason: None });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user(id: u64) -> oxicord_model::User {
        serde_json::from_value(serde_json::json!({ "id": id.to_string(), "username": "u" })).unwrap()
    }

    #[test]
    fn a_prune_answer_lands_for_the_open_server_only() {
        let mut state = AdminState::default();
        state.switch_to(Snowflake(1));
        state.apply(AdminEvent::Prune(Snowflake(2), 7, false, Ok(4)));
        assert!(matches!(state.prune, Load::Idle), "another server's answer is dropped");
        state.apply(AdminEvent::Prune(Snowflake(1), 7, false, Ok(4)));
        let Load::Ready(preview) = &state.prune else { panic!("not loaded") };
        assert_eq!((preview.days, preview.count, preview.done), (7, 4, false));
        state.apply(AdminEvent::Prune(Snowflake(1), 7, true, Err("no".into())));
        assert!(matches!(state.prune, Load::Failed(_)));
    }

    #[test]
    fn templates_land_for_the_open_server_only() {
        let mut state = AdminState::default();
        state.switch_to(Snowflake(1));
        let template = oxicord_model::GuildTemplate { code: "abc".into(), ..Default::default() };
        state.apply(AdminEvent::Templates(Snowflake(2), Ok(vec![template.clone()])));
        assert!(!matches!(state.templates, Load::Ready(_)), "another server's answer is dropped");
        state.apply(AdminEvent::Templates(Snowflake(1), Ok(vec![template])));
        let Load::Ready(list) = &state.templates else { panic!("not loaded") };
        assert_eq!(list[0].code, "abc");
    }

    #[test]
    fn bans_follow_the_gateway_for_the_open_server_only() {
        let mut state = AdminState::default();
        state.switch_to(Snowflake(1));
        state.apply(AdminEvent::Bans(Snowflake(1), Ok(Vec::new())));

        state.apply_ban(Snowflake(1), user(5), true);
        state.apply_ban(Snowflake(1), user(5), true);
        state.apply_ban(Snowflake(2), user(6), true);
        assert_eq!(state.bans.ready().unwrap().len(), 1, "no duplicates, no other servers");

        state.apply_ban(Snowflake(1), user(5), false);
        assert!(state.bans.ready().unwrap().is_empty());
    }
}
