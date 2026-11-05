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
    CreateRole { guild: Snowflake, name: String },
    EditRole { guild: Snowflake, role: Snowflake, fields: Value },
    DeleteRole { guild: Snowflake, role: Snowflake },
    RenameEmoji { guild: Snowflake, emoji: Snowflake, name: String },
    DeleteEmoji { guild: Snowflake, emoji: Snowflake },
    CreateChannel { guild: Snowflake, name: String, kind: u8, parent: Option<Snowflake> },
    EditChannel { channel: Snowflake, fields: Value },
    DeleteChannel(Snowflake),
}

/// The answers, each tagged with the server it was for.
#[derive(Debug)]
pub enum AdminEvent {
    Invites(Snowflake, Result<Vec<InviteEntry>, String>),
    Bans(Snowflake, Result<Vec<BanEntry>, String>),
    Audit(Snowflake, Result<AuditLog, String>),
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
            _ => {}
        }
    }
}
