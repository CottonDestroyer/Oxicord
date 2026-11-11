//! Server management requests and the lists its settings pages show. Edits
//! need no state here: the gateway reports the change and the store has it.

use oxicord_model::{
    Snowflake,
    admin::{AuditLog, BanEntry, InviteEntry},
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
}

/// The answers, each tagged with the server it was for.
#[derive(Debug)]
pub enum AdminEvent {
    Invites(Snowflake, Result<Vec<InviteEntry>, String>),
    Bans(Snowflake, Result<Vec<BanEntry>, String>),
    Audit(Snowflake, Result<AuditLog, String>),
    MemberVerification(Snowflake, Box<Result<oxicord_model::VerificationInfo, String>>),
    /// How answering the screening form went.
    Verified(Snowflake, Result<(), String>),
}

/// The lists last loaded, for the server whose settings are open.
#[derive(Default)]
pub struct AdminState {
    pub guild: Option<Snowflake>,
    pub invites: Load<Vec<InviteEntry>>,
    pub bans: Load<Vec<BanEntry>>,
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
            AdminEvent::Audit(g, r) if self.guild == Some(g) => self.audit = Load::from_result(r),
            AdminEvent::MemberVerification(..) | AdminEvent::Verified(..) => {}
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
