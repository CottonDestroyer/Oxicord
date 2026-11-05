//! URL builders for Discord's CDN. Always PNG, which every size and kind of
//! avatar or icon is available as, and which a still frame of an animated
//! one falls back to.

use oxicord_model::Snowflake;

const CDN: &str = "https://cdn.discordapp.com";

pub fn user_avatar(user: Snowflake, hash: Option<&str>, size: u32) -> String {
    match hash {
        Some(hash) => format!("{CDN}/avatars/{user}/{hash}.png?size={size}"),
        None => default_avatar(user),
    }
}

/// Accounts with no avatar get one of six, picked from the account's
/// creation time.
pub fn default_avatar(user: Snowflake) -> String {
    format!("{CDN}/embed/avatars/{}.png", (user.0 >> 22) % 6)
}

pub fn guild_icon(guild: Snowflake, hash: &str, size: u32) -> String {
    format!("{CDN}/icons/{guild}/{hash}.png?size={size}")
}

pub fn group_icon(channel: Snowflake, hash: &str, size: u32) -> String {
    format!("{CDN}/channel-icons/{channel}/{hash}.png?size={size}")
}

pub fn emoji(id: Snowflake, size: u32) -> String {
    format!("{CDN}/emojis/{id}.png?size={size}&quality=lossless")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn avatar_with_hash_uses_the_user_path() {
        assert_eq!(
            user_avatar(Snowflake(5), Some("abc"), 64),
            "https://cdn.discordapp.com/avatars/5/abc.png?size=64"
        );
    }

    #[test]
    fn avatar_without_hash_falls_back_to_one_of_six_defaults() {
        // 175928847299117063 >> 22 is 41944705796, and that mod 6 is 2.
        let url = user_avatar(Snowflake(175_928_847_299_117_063), None, 64);
        assert_eq!(url, "https://cdn.discordapp.com/embed/avatars/2.png");
        for id in [0u64, 1 << 22, 5 << 22, 6 << 22] {
            assert!(default_avatar(Snowflake(id)).ends_with(".png"));
        }
    }

    #[test]
    fn icon_urls_carry_their_owner() {
        assert!(guild_icon(Snowflake(9), "h", 96).contains("/icons/9/h.png"));
        assert!(group_icon(Snowflake(9), "h", 96).contains("/channel-icons/9/h.png"));
        assert!(emoji(Snowflake(9), 48).contains("/emojis/9.png"));
    }
}
