//! Permission math, following the algorithm in the public API docs.

use oxicord_model::{Guild, Overwrite, OverwriteKind, Permissions, Snowflake};

/// What anyone can do in a DM. There are no roles to consult.
pub const DM_PERMISSIONS: Permissions = Permissions::VIEW_CHANNEL
    .union(Permissions::SEND_MESSAGES)
    .union(Permissions::READ_MESSAGE_HISTORY)
    .union(Permissions::ATTACH_FILES)
    .union(Permissions::EMBED_LINKS)
    .union(Permissions::ADD_REACTIONS)
    .union(Permissions::USE_EXTERNAL_EMOJIS);

/// Guild wide permissions, before any channel overwrites.
pub fn base_permissions(guild: &Guild, user: Snowflake, member_roles: &[Snowflake]) -> Permissions {
    if guild.owner_id == Some(user) {
        return Permissions::all();
    }

    // The @everyone role always has the same id as the guild.
    let mut perms = guild
        .roles
        .iter()
        .find(|r| r.id == guild.id)
        .map(|r| r.permissions)
        .unwrap_or_default();

    for role in guild.roles.iter().filter(|r| member_roles.contains(&r.id)) {
        perms |= role.permissions;
    }

    if perms.contains(Permissions::ADMINISTRATOR) {
        Permissions::all()
    } else {
        perms
    }
}

/// Applies a channel's overwrites to the guild wide permissions.
///
/// Order matters: @everyone first, then all of the member's roles combined,
/// then the member's own overwrite.
pub fn channel_permissions(
    base: Permissions,
    guild_id: Snowflake,
    user: Snowflake,
    member_roles: &[Snowflake],
    overwrites: &[Overwrite],
) -> Permissions {
    if base.contains(Permissions::ADMINISTRATOR) {
        return Permissions::all();
    }

    let mut perms = base;

    if let Some(o) = overwrites
        .iter()
        .find(|o| o.kind == OverwriteKind::Role && o.id == guild_id)
    {
        perms = perms.difference(o.deny).union(o.allow);
    }

    let (mut allow, mut deny) = (Permissions::empty(), Permissions::empty());
    for o in overwrites.iter().filter(|o| {
        o.kind == OverwriteKind::Role && o.id != guild_id && member_roles.contains(&o.id)
    }) {
        allow |= o.allow;
        deny |= o.deny;
    }
    perms = perms.difference(deny).union(allow);

    if let Some(o) = overwrites
        .iter()
        .find(|o| o.kind == OverwriteKind::Member && o.id == user)
    {
        perms = perms.difference(o.deny).union(o.allow);
    }

    // A channel you can't see offers nothing else, whatever the bits say.
    if perms.contains(Permissions::VIEW_CHANNEL) {
        perms
    } else {
        Permissions::empty()
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    const GUILD: u64 = 100;
    const ME: Snowflake = Snowflake(1);

    fn guild(owner: u64) -> Guild {
        serde_json::from_value(json!({
            "id": GUILD.to_string(),
            "name": "g",
            "owner_id": owner.to_string(),
            "roles": [
                { "id": "100", "name": "@everyone", "permissions": (1u64 << 10 | 1u64 << 11).to_string() },
                { "id": "200", "name": "mod", "permissions": (1u64 << 13).to_string() },
                { "id": "300", "name": "admin", "permissions": (1u64 << 3).to_string() }
            ]
        }))
        .unwrap()
    }

    fn overwrite(kind: u8, id: u64, allow: Permissions, deny: Permissions) -> Overwrite {
        serde_json::from_value(json!({
            "id": id.to_string(), "type": kind,
            "allow": allow.bits().to_string(), "deny": deny.bits().to_string()
        }))
        .unwrap()
    }

    #[test]
    fn owner_has_everything() {
        let g = guild(1);
        assert_eq!(base_permissions(&g, ME, &[]), Permissions::all());
    }

    #[test]
    fn everyone_role_is_the_floor() {
        let g = guild(9);
        let p = base_permissions(&g, ME, &[]);
        assert_eq!(p, Permissions::VIEW_CHANNEL | Permissions::SEND_MESSAGES);
    }

    #[test]
    fn roles_add_up() {
        let g = guild(9);
        let p = base_permissions(&g, ME, &[Snowflake(200)]);
        assert!(p.contains(Permissions::MANAGE_MESSAGES | Permissions::SEND_MESSAGES));
    }

    #[test]
    fn administrator_bypasses_overwrites() {
        let g = guild(9);
        let base = base_permissions(&g, ME, &[Snowflake(300)]);
        let deny_all = overwrite(0, GUILD, Permissions::empty(), Permissions::VIEW_CHANNEL);
        let p = channel_permissions(base, g.id, ME, &[Snowflake(300)], &[deny_all]);
        assert_eq!(p, Permissions::all());
    }

    #[test]
    fn everyone_deny_hides_the_channel() {
        let g = guild(9);
        let base = base_permissions(&g, ME, &[]);
        let hide = overwrite(0, GUILD, Permissions::empty(), Permissions::VIEW_CHANNEL);
        let p = channel_permissions(base, g.id, ME, &[], &[hide]);
        assert_eq!(p, Permissions::empty());
    }

    #[test]
    fn role_allow_beats_everyone_deny() {
        let g = guild(9);
        let roles = [Snowflake(200)];
        let base = base_permissions(&g, ME, &roles);
        let overwrites = [
            overwrite(0, GUILD, Permissions::empty(), Permissions::VIEW_CHANNEL),
            overwrite(0, 200, Permissions::VIEW_CHANNEL, Permissions::empty()),
        ];
        let p = channel_permissions(base, g.id, ME, &roles, &overwrites);
        assert!(p.contains(Permissions::VIEW_CHANNEL));
    }

    #[test]
    fn member_overwrite_has_the_last_word() {
        let g = guild(9);
        let roles = [Snowflake(200)];
        let base = base_permissions(&g, ME, &roles);
        let overwrites = [
            overwrite(0, 200, Permissions::VIEW_CHANNEL, Permissions::empty()),
            overwrite(1, 1, Permissions::empty(), Permissions::VIEW_CHANNEL),
        ];
        let p = channel_permissions(base, g.id, ME, &roles, &overwrites);
        assert_eq!(p, Permissions::empty());
    }

    #[test]
    fn overwrites_for_roles_we_lack_are_ignored() {
        let g = guild(9);
        let base = base_permissions(&g, ME, &[]);
        let other = overwrite(0, 200, Permissions::empty(), Permissions::VIEW_CHANNEL);
        let p = channel_permissions(base, g.id, ME, &[], &[other]);
        assert!(p.contains(Permissions::VIEW_CHANNEL));
    }

    #[test]
    fn denying_send_keeps_the_rest() {
        let g = guild(9);
        let base = base_permissions(&g, ME, &[]);
        let mute = overwrite(0, GUILD, Permissions::empty(), Permissions::SEND_MESSAGES);
        let p = channel_permissions(base, g.id, ME, &[], &[mute]);
        assert_eq!(p, Permissions::VIEW_CHANNEL);
    }
}
