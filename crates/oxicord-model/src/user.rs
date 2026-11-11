use serde::{Deserialize, Serialize};

use crate::Snowflake;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct User {
    pub id: Snowflake,
    #[serde(default)]
    pub username: String,
    /// "0" for accounts migrated to unique usernames.
    #[serde(default)]
    pub discriminator: Option<String>,
    #[serde(default)]
    pub global_name: Option<String>,
    #[serde(default)]
    pub avatar: Option<String>,
    #[serde(default)]
    pub bot: bool,
    /// A frame drawn around the avatar, bought from the shop.
    #[serde(default, deserialize_with = "crate::serde_util::lenient_default")]
    pub avatar_decoration_data: Option<ShopItem>,
    #[serde(default, deserialize_with = "crate::serde_util::lenient_default")]
    pub collectibles: Option<Collectibles>,
    #[serde(default, deserialize_with = "crate::serde_util::lenient_default")]
    pub primary_guild: Option<PrimaryGuild>,
}

/// A shop item such as a decoration or a nameplate, named by its listing.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct ShopItem {
    pub sku_id: Snowflake,
}

/// Cosmetics a person owns and has switched on.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
pub struct Collectibles {
    #[serde(default, deserialize_with = "crate::serde_util::lenient_default")]
    pub nameplate: Option<ShopItem>,
}

impl User {
    /// The listing of the frame around their avatar, if they have one on.
    pub fn decoration(&self) -> Option<Snowflake> {
        self.avatar_decoration_data.as_ref().map(|d| d.sku_id)
    }

    /// The listing of the banner behind their name in lists, if they have one on.
    pub fn nameplate(&self) -> Option<Snowflake> {
        self.collectibles.as_ref()?.nameplate.as_ref().map(|n| n.sku_id)
    }

    /// The tag they wear after their name, unless they switched it off.
    pub fn server_tag(&self) -> Option<ServerTag> {
        self.primary_guild.as_ref()?.to_tag()
    }

    pub fn display_name(&self) -> &str {
        self.global_name
            .as_deref()
            .filter(|n| !n.is_empty())
            .unwrap_or(&self.username)
    }
}

/// A badge shown next to a name, such as an early supporter mark.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
pub struct Badge {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub description: String,
    /// The file name under the badge icon path on the CDN.
    #[serde(default)]
    pub icon: String,
}

/// A server both of us are in.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct MutualGuild {
    pub id: crate::Snowflake,
    #[serde(default)]
    pub nick: Option<String>,
}

/// An account linked on the profile, like a game or social site.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct LinkedAccount {
    #[serde(rename = "type", default)]
    pub kind: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub verified: bool,
    #[serde(default, deserialize_with = "crate::serde_util::lenient_default")]
    pub metadata: AccountMetadata,
}

/// The extras a linked account can carry. Both arrive as text.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
pub struct AccountMetadata {
    /// When the account was made on the other service.
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub game_count: Option<String>,
}

/// The server tag shown next to a name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerTag {
    pub tag: String,
    pub guild_id: Snowflake,
    /// The badge picture hash, when the tag has one.
    pub badge: Option<String>,
}

/// A game a person put on their profile, with what they wrote about it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameEntry {
    pub id: Snowflake,
    pub comment: String,
    /// Tag keys such as `obsessed` or `looking_for_group`.
    pub tags: Vec<String>,
}

/// What a game lookup returns: enough to draw its cover.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Game {
    pub name: String,
    pub cover_url: Option<String>,
}

/// The body of `PUT /users/@me/widgets`: every widget the board should hold.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct WidgetsSubmission {
    pub widgets: Vec<WidgetSubmission>,
}

/// One widget in that body. A widget that has never been saved has no `id`
/// and the server gives it one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct WidgetSubmission {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Snowflake>,
    pub data: WidgetDataSubmission,
}

/// What kind of widget this is, tagged the way the wire names it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WidgetDataSubmission {
    FavoriteGames {
        games: Vec<GameEntrySubmission>,
    },
    CurrentGames {
        games: Vec<GameEntrySubmission>,
    },
    PlayedGames {
        games: Vec<GameEntrySubmission>,
    },
    WantToPlayGames {
        games: Vec<GameEntrySubmission>,
    },
}

/// One game as the save request names it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct GameEntrySubmission {
    pub game_id: Snowflake,
    pub comment: String,
    pub tags: Vec<String>,
}

/// The extra details behind a profile card. Discord nests the bio under
/// `user_profile`, and older responses put it on the user, so take either.
/// Everything is optional: a profile we only half understand still shows.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Profile {
    pub bio: String,
    pub pronouns: String,
    pub banner: Option<String>,
    pub accent_color: Option<u32>,
    pub server_tag: Option<ServerTag>,
    pub badges: Vec<Badge>,
    pub mutual_guilds: Vec<MutualGuild>,
    /// The people we both know, when Discord listed them.
    pub mutual_friend_users: Vec<User>,
    pub mutual_friends: u32,
    pub accounts: Vec<LinkedAccount>,
    /// When they started boosting or subscribed, as sent.
    pub premium_since: Option<String>,
    /// When they joined the server the card was opened from.
    pub joined_server: Option<String>,
    /// The server the member details below belong to, set by whoever asked.
    pub member_guild: Option<Snowflake>,
    /// Their roles in that server, as the profile lists them.
    pub member_roles: Vec<Snowflake>,
    /// A picture they set for that server only.
    pub server_avatar: Option<String>,
    /// A banner they set for that server only.
    pub server_banner: Option<String>,
    /// Their nickname there.
    pub member_nick: Option<String>,
    /// The note we wrote about them, visible only to us.
    pub note: String,
    /// The games of the three profile widgets.
    pub favorite_games: Vec<GameEntry>,
    pub played_games: Vec<GameEntry>,
    pub want_to_play_games: Vec<GameEntry>,
    /// Names and covers of those games, filled in after a lookup.
    pub games: std::collections::HashMap<Snowflake, Game>,
    /// The shop listing of the effect they have on their profile.
    pub effect: Option<Snowflake>,
    /// What that effect looks like, filled in after a lookup.
    pub effect_art: Option<crate::ProfileEffect>,
    /// The shop listing of the frame around their profile.
    pub frame: Option<Snowflake>,
    /// What that frame looks like, filled in after a lookup.
    pub frame_art: Option<crate::ProfileFrame>,
}

impl Profile {
    /// Every game id the widgets mention, once each.
    pub fn game_ids(&self) -> Vec<Snowflake> {
        let mut ids: Vec<Snowflake> = Vec::new();
        for entry in self.favorite_games.iter().chain(&self.played_games).chain(&self.want_to_play_games) {
            if !ids.contains(&entry.id) {
                ids.push(entry.id);
            }
        }
        ids
    }
}

#[derive(Default, Deserialize)]
struct RawProfile {
    #[serde(default)]
    user_profile: Option<ProfileFields>,
    #[serde(default)]
    user: Option<ProfileFields>,
    #[serde(default, deserialize_with = "crate::serde_util::lenient_vec")]
    badges: Vec<Badge>,
    #[serde(default, deserialize_with = "crate::serde_util::lenient_vec")]
    mutual_guilds: Vec<MutualGuild>,
    #[serde(default, deserialize_with = "crate::serde_util::lenient_vec")]
    mutual_friends: Vec<User>,
    #[serde(default)]
    mutual_friends_count: Option<u32>,
    #[serde(default, deserialize_with = "crate::serde_util::lenient_vec")]
    connected_accounts: Vec<LinkedAccount>,
    #[serde(default)]
    premium_since: Option<String>,
    #[serde(default)]
    guild_member: Option<GuildMemberFields>,
    /// The bio, pronouns and banner set for the server asked about.
    #[serde(default)]
    guild_member_profile: Option<ProfileFields>,
    #[serde(default)]
    primary_guild: Option<PrimaryGuild>,
    #[serde(default, deserialize_with = "crate::serde_util::lenient_vec")]
    widgets: Vec<WidgetFields>,
}

#[derive(Default, Deserialize)]
struct GuildMemberFields {
    #[serde(default)]
    joined_at: Option<String>,
    #[serde(default, deserialize_with = "crate::serde_util::lenient_vec")]
    roles: Vec<Snowflake>,
    #[serde(default)]
    nick: Option<String>,
    #[serde(default)]
    avatar: Option<String>,
    #[serde(default)]
    banner: Option<String>,
    #[serde(default)]
    bio: Option<String>,
}

/// The server a person wears the tag of, as users carry it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
pub struct PrimaryGuild {
    #[serde(default)]
    tag: Option<String>,
    #[serde(default)]
    identity_enabled: Option<bool>,
    #[serde(default)]
    identity_guild_id: Option<Snowflake>,
    #[serde(default)]
    badge: Option<String>,
}

impl PrimaryGuild {
    fn to_tag(&self) -> Option<ServerTag> {
        if self.identity_enabled == Some(false) {
            return None;
        }
        Some(ServerTag {
            tag: self.tag.clone().filter(|t| !t.is_empty())?,
            guild_id: self.identity_guild_id?,
            badge: self.badge.clone().filter(|b| !b.is_empty()),
        })
    }
}

#[derive(Default, Deserialize)]
struct WidgetFields {
    #[serde(default)]
    data: WidgetData,
}

#[derive(Default, Deserialize)]
struct WidgetData {
    #[serde(rename = "type", default)]
    kind: String,
    #[serde(default, deserialize_with = "crate::serde_util::lenient_vec")]
    games: Vec<WidgetGame>,
}

#[derive(Default, Deserialize)]
struct WidgetGame {
    #[serde(rename = "game_id", default)]
    game_id: Option<Snowflake>,
    #[serde(default)]
    comment: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
}

#[derive(Default, Deserialize)]
struct ProfileFields {
    #[serde(default)]
    bio: Option<String>,
    #[serde(default)]
    pronouns: Option<String>,
    #[serde(default)]
    banner: Option<String>,
    #[serde(default)]
    accent_color: Option<u32>,
    /// Newer responses carry the server tag on the user.
    #[serde(default)]
    primary_guild: Option<PrimaryGuild>,
    /// The effect over the card, named by the shop listing it came from.
    #[serde(default, deserialize_with = "crate::serde_util::lenient_default")]
    profile_effect: Option<EquippedEffect>,
    /// Newer responses list the shop items worn, effects among them.
    #[serde(default, deserialize_with = "crate::serde_util::lenient_vec")]
    collectibles: Vec<WornItem>,
}

#[derive(Deserialize)]
struct EquippedEffect {
    /// The catalog calls it `sku_id`; older responses called it `id`.
    #[serde(alias = "sku_id")]
    id: Snowflake,
}

/// An entry of `collectibles`, whose type 1 is a profile effect.
#[derive(Deserialize)]
struct WornItem {
    sku_id: Snowflake,
    #[serde(rename = "type")]
    kind: u8,
}

impl<'de> Deserialize<'de> for Profile {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        // One odd field must not lose the whole profile, so read loosely.
        let value = serde_json::Value::deserialize(d)?;
        Ok(serde_json::from_value::<RawProfile>(value).unwrap_or_default().into())
    }
}

impl From<RawProfile> for Profile {
    fn from(raw: RawProfile) -> Self {
        // What was set for the server wins over what was set everywhere.
        let mut server = raw.guild_member_profile.unwrap_or_default();
        if let Some(member) = &raw.guild_member {
            server.bio = server.bio.filter(|b| !b.is_empty()).or_else(|| member.bio.clone());
            server.banner = server.banner.filter(|b| !b.is_empty()).or_else(|| member.banner.clone());
        }
        let server_banner = server.banner.clone().filter(|b| !b.is_empty());
        // The banner of the server is a different picture from the account's.
        let account_banner = [&raw.user_profile, &raw.user]
            .into_iter()
            .flatten()
            .find_map(|s| s.banner.clone().filter(|b| !b.is_empty()));
        server.banner = None;
        let mut sources = [Some(server), raw.user_profile, raw.user];
        let primary_guild = raw
            .primary_guild
            .or_else(|| sources.iter_mut().flatten().find_map(|s| s.primary_guild.take()));
        // The server's own effect wins, then the one everywhere.
        let effect = sources.iter().flatten().find_map(|s| {
            s.profile_effect
                .as_ref()
                .map(|e| e.id)
                .or_else(|| s.collectibles.iter().find(|item| item.kind == 1).map(|item| item.sku_id))
        });
        // The server's own frame wins, then the one everywhere.
        let frame = sources.iter().flatten().find_map(|s| {
            s.collectibles.iter().find(|item| item.kind == 3).map(|item| item.sku_id)
        });
        let first = |field: fn(&ProfileFields) -> Option<&String>| {
            sources
                .iter()
                .flatten()
                .find_map(|s| field(s).filter(|v| !v.is_empty()))
                .cloned()
        };
        let games = |kind: &str| -> Vec<GameEntry> {
            raw.widgets
                .iter()
                .filter(|widget| widget.data.kind == kind)
                .flat_map(|widget| &widget.data.games)
                .filter_map(|game| {
                    Some(GameEntry {
                        id: game.game_id?,
                        comment: game.comment.clone().unwrap_or_default(),
                        tags: game.tags.clone(),
                    })
                })
                .collect()
        };
        let (favorite_games, played_games, want_to_play_games) =
            (games("favorite_games"), games("played_games"), games("want_to_play_games"));
        Self {
            bio: first(|s| s.bio.as_ref()).unwrap_or_default(),
            pronouns: first(|s| s.pronouns.as_ref()).unwrap_or_default(),
            banner: account_banner,
            accent_color: sources.iter().flatten().find_map(|s| s.accent_color),
            badges: raw.badges,
            mutual_guilds: raw.mutual_guilds,
            mutual_friends: raw.mutual_friends_count.unwrap_or(0).max(raw.mutual_friends.len() as u32),
            mutual_friend_users: raw.mutual_friends,
            accounts: raw.connected_accounts,
            premium_since: raw.premium_since,
            member_guild: None,
            server_avatar: raw.guild_member.as_ref().and_then(|m| m.avatar.clone()).filter(|a| !a.is_empty()),
            server_banner,
            joined_server: raw.guild_member.as_ref().and_then(|m| m.joined_at.clone()),
            member_nick: raw.guild_member.as_ref().and_then(|m| m.nick.clone()).filter(|n| !n.is_empty()),
            member_roles: raw.guild_member.map(|m| m.roles).unwrap_or_default(),
            server_tag: primary_guild.and_then(|g| g.to_tag()),
            favorite_games,
            played_games,
            want_to_play_games,
            games: Default::default(),
            effect,
            effect_art: None,
            frame,
            frame_art: None,
            note: String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cosmetics_are_read_and_odd_ones_ignored() {
        let user: User = serde_json::from_str(
            r#"{"id":"1","avatar_decoration_data":{"asset":"a_x","sku_id":"7"},
                "collectibles":{"nameplate":{"sku_id":"8","asset":"n/","palette":"cobalt"}}}"#,
        )
        .unwrap();
        assert_eq!((user.decoration(), user.nameplate()), (Some(Snowflake(7)), Some(Snowflake(8))));
        let none: User = serde_json::from_str(r#"{"id":"1","avatar_decoration_data":null,"collectibles":null}"#).unwrap();
        assert_eq!((none.decoration(), none.nameplate()), (None, None));
        let odd: User = serde_json::from_str(r#"{"id":"1","avatar_decoration_data":"x","collectibles":{"nameplate":5}}"#).unwrap();
        assert_eq!((odd.decoration(), odd.nameplate()), (None, None));
    }

    #[test]
    fn profile_prefers_the_nested_bio_and_tolerates_gaps() {
        let nested: Profile = serde_json::from_str(
            r#"{"user":{"id":"1","bio":"old"},"user_profile":{"bio":"hello","pronouns":"they/them"}}"#,
        )
        .unwrap();
        assert_eq!(nested.bio, "hello");
        assert_eq!(nested.pronouns, "they/them");

        let flat: Profile = serde_json::from_str(r#"{"user":{"bio":"just this"}}"#).unwrap();
        assert_eq!(flat.bio, "just this");
        assert_eq!(serde_json::from_str::<Profile>("{}").unwrap(), Profile::default());
    }

    #[test]
    fn the_richer_fields_are_read_and_bad_ones_dropped() {
        let p: Profile = serde_json::from_str(
            r#"{"user":{"id":"1","accent_color":255},"user_profile":{"bio":"hi","banner":"abc"},
                "badges":[{"id":"x","description":"Early","icon":"i1"},"junk"],
                "mutual_guilds":[{"id":"5","nick":"N"},{"id":"bad"}],"mutual_friends_count":3,
                "connected_accounts":[{"type":"steam","name":"me","verified":true}],
                "premium_since":"2020-01-01","guild_member":{"joined_at":"2021-02-03"}}"#,
        )
        .unwrap();
        assert_eq!(p.banner.as_deref(), Some("abc"));
        assert_eq!(p.accent_color, Some(255));
        assert_eq!(p.badges.len(), 1);
        assert_eq!(p.mutual_guilds.len(), 1);
        assert_eq!(p.mutual_friends, 3);
        assert_eq!(p.accounts[0].name, "me");
        assert_eq!(p.joined_server.as_deref(), Some("2021-02-03"));
    }

    #[test]
    fn widgets_the_server_tag_and_friend_lists_are_read() {
        let p: Profile = serde_json::from_str(
            r#"{"user":{"id":"1","primary_guild":{"identity_guild_id":"9","identity_enabled":true,"tag":"LWIQ","badge":"b1"}},
                "mutual_friends":[{"id":"7","username":"pal"},"junk"],
                "guild_member":{"nick":"Cookie","roles":["3","4"]},
                "connected_accounts":[{"type":"steam","name":"me","metadata":{"created_at":"2020-03-26T17:41:16","game_count":"49","item_count_tf2":"-1"}},
                                      {"type":"odd","name":"x","metadata":{"game_count":5}}],
                "widgets":[
                  {"data":{"type":"favorite_games","games":[{"game_id":"5","comment":"peak","tags":["obsessed"]}]}},
                  {"data":{"type":"played_games","games":[{"game_id":"5","comment":null,"tags":[]},{"game_id":"6"}]}},
                  {"data":{"type":"want_to_play_games","games":[{"game_id":"8"}]}}]}"#,
        )
        .unwrap();
        let tag = p.server_tag.as_ref().unwrap();
        assert_eq!((tag.tag.as_str(), tag.guild_id, tag.badge.as_deref()), ("LWIQ", Snowflake(9), Some("b1")));
        assert_eq!((p.mutual_friend_users.len(), p.mutual_friends), (1, 1));
        assert_eq!(p.accounts[0].metadata.game_count.as_deref(), Some("49"));
        // A metadata block we can't read costs only the metadata.
        assert_eq!(p.accounts[1].metadata, AccountMetadata::default());
        assert_eq!(p.favorite_games[0].comment, "peak");
        assert_eq!(p.member_roles, [Snowflake(3), Snowflake(4)]);
        assert_eq!(p.member_nick.as_deref(), Some("Cookie"));
        assert_eq!(p.favorite_games[0].tags, ["obsessed"]);
        assert_eq!(p.played_games.len(), 2);
        assert_eq!(p.want_to_play_games[0].id, Snowflake(8));
        // Five is in two widgets but is looked up once.
        assert_eq!(p.game_ids(), [Snowflake(5), Snowflake(6), Snowflake(8)]);
    }

    #[test]
    fn a_favorite_game_widget_serializes_as_the_save_request() {
        let body = WidgetsSubmission {
            widgets: vec![WidgetSubmission {
                id: None,
                data: WidgetDataSubmission::FavoriteGames {
                    games: vec![GameEntrySubmission {
                        game_id: Snowflake(1_167_674_267_748_540_516),
                        comment: "peak".into(),
                        tags: vec!["obsessed".into()],
                    }],
                },
            }],
        };
        assert_eq!(
            serde_json::to_value(&body).unwrap(),
            serde_json::json!({
                "widgets": [{"data": {"type": "favorite_games", "games": [{
                    "game_id": "1167674267748540516",
                    "comment": "peak",
                    "tags": ["obsessed"]
                }]}}]
            })
        );
        let saved = WidgetSubmission {
            id: Some(Snowflake(7)),
            data: WidgetDataSubmission::PlayedGames { games: Vec::new() },
        };
        assert_eq!(
            serde_json::to_value(&saved).unwrap(),
            serde_json::json!({"id": "7", "data": {"type": "played_games", "games": []}})
        );
    }

    #[test]
    fn what_was_set_for_the_server_wins_and_keeps_its_own_banner() {
        let p: Profile = serde_json::from_str(
            r#"{"user":{"id":"1","banner":"acct"},
                "user_profile":{"bio":"everywhere","pronouns":"he/him"},
                "guild_member":{"avatar":"av","banner":"srv","bio":"","nick":"N"},
                "guild_member_profile":{"bio":"here","pronouns":""}}"#,
        )
        .unwrap();
        assert_eq!(p.bio, "here");
        // An empty server pronoun falls back to the account's.
        assert_eq!(p.pronouns, "he/him");
        assert_eq!(p.banner.as_deref(), Some("acct"));
        assert_eq!(p.server_banner.as_deref(), Some("srv"));
        assert_eq!(p.server_avatar.as_deref(), Some("av"));

        let plain: Profile = serde_json::from_str(r#"{"user_profile":{"bio":"everywhere"}}"#).unwrap();
        assert_eq!((plain.bio.as_str(), plain.server_banner, plain.server_avatar), ("everywhere", None, None));
    }

    #[test]
    fn a_message_author_carries_the_tag_they_wear() {
        let on: User = serde_json::from_str(
            r#"{"id":"1","primary_guild":{"identity_guild_id":"9","identity_enabled":true,"tag":"LWIQ","badge":"b1"}}"#,
        )
        .unwrap();
        assert_eq!(on.server_tag().map(|t| (t.tag, t.guild_id, t.badge)), Some(("LWIQ".into(), Snowflake(9), Some("b1".into()))));
        let off: User = serde_json::from_str(
            r#"{"id":"1","primary_guild":{"identity_guild_id":"9","identity_enabled":false,"tag":"LWIQ"}}"#,
        )
        .unwrap();
        assert_eq!(off.server_tag(), None);
        let none: User = serde_json::from_str(r#"{"id":"1","primary_guild":null}"#).unwrap();
        assert_eq!(none.server_tag(), None);
        let odd: User = serde_json::from_str(r#"{"id":"1","primary_guild":"x"}"#).unwrap();
        assert_eq!(odd.server_tag(), None);
    }

    #[test]
    fn a_server_tag_the_person_turned_off_is_hidden() {
        let p: Profile = serde_json::from_str(
            r#"{"primary_guild":{"identity_guild_id":"9","identity_enabled":false,"tag":"LWIQ"}}"#,
        )
        .unwrap();
        assert_eq!(p.server_tag, None);
    }

    #[test]
    fn a_worn_item_map_on_the_user_does_not_lose_the_profile() {
        // The `user` object lists worn cosmetics as a map, while
        // `user_profile` lists the same key as a list. Reading the map as a
        // list used to fail the whole profile and show an empty card.
        let p: Profile = serde_json::from_str(
            r#"{"user":{"id":"1","collectibles":{"nameplate":{"sku_id":"8"}}},
                "user_profile":{"bio":"hello","collectibles":[{"type":1,"sku_id":"9"}]}}"#,
        )
        .unwrap();
        assert_eq!(p.bio, "hello");
        assert_eq!(p.effect, Some(Snowflake(9)));
    }

    #[test]
    fn the_effect_is_read_from_either_place_it_is_sent() {
        let named: Profile =
            serde_json::from_str(r#"{"user_profile":{"profile_effect":{"id":"77","expires_at":null}}}"#).unwrap();
        assert_eq!(named.effect, Some(Snowflake(77)));

        let sku: Profile =
            serde_json::from_str(r#"{"user_profile":{"profile_effect":{"sku_id":"78","expires_at":null}}}"#).unwrap();
        assert_eq!(sku.effect, Some(Snowflake(78)));

        let worn: Profile = serde_json::from_str(
            r#"{"user_profile":{"profile_effect":null,"collectibles":[{"type":3,"sku_id":"5"},{"type":1,"sku_id":"88"}]}}"#,
        )
        .unwrap();
        assert_eq!(worn.effect, Some(Snowflake(88)));

        // The one set for the server wins.
        let both: Profile = serde_json::from_str(
            r#"{"user_profile":{"profile_effect":{"id":"1"}},"guild_member_profile":{"profile_effect":{"id":"2"}}}"#,
        )
        .unwrap();
        assert_eq!(both.effect, Some(Snowflake(2)));

        let none: Profile = serde_json::from_str(r#"{"user_profile":{"profile_effect":null,"collectibles":[]}}"#).unwrap();
        assert_eq!(none.effect, None);
    }

#[test]
fn a_frame_is_read_from_collectibles() {
    let framed: Profile = serde_json::from_str(
        r#"{"user_profile":{"collectibles":[{"type":3,"sku_id":"99"}]}}"#,
    )
    .unwrap();
    assert_eq!(framed.frame, Some(Snowflake(99)));
}
}
