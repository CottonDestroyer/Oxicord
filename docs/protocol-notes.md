# Protocol notes

Facts confirmed against the official web client (client_build_number 663810,
checked 2026-11-05) and the public API docs. Nothing here is copied code. The
build number goes stale quickly, so treat it as a default that can be
overridden.

## Client identity (read this before changing anything below)

Discord revokes user tokens whose session looks scripted. A session that
identified exactly like the official web client (with `capabilities` and
`client_state`, API v9, a current build number) had its token revoked twice
in live testing, once by pasted token and once after hosted login. We now
mirror the identity of an existing native client instead:

- API and gateway version 10.
- Chrome 128 user agent (Safari on macOS), build number 634304,
  `os_version` empty.
- REST sends `X-Super-Properties`, `X-Discord-Locale`, `X-Discord-Timezone`,
  `Accept-Language`, `Origin` and `Referer` on every call, and never follows
  redirects.
- IDENTIFY is exactly `{ token, compress: false, properties, presence }`.

Which of these differences matters is not known, only that the combination
works better. Change them together, not one at a time.

## REST

- Base URL: `https://discord.com/api/v10`.
- Auth for user accounts: raw token in the `Authorization` header, no `Bot`
  prefix.
- Message ack: `POST /channels/{channel_id}/messages/{message_id}/ack`.
- Attachment upload slot: `POST /channels/{channel_id}/attachments`.
- The client sends `X-Super-Properties`, a base64 encoded JSON object. Fields
  seen: `os`, `browser`, `device`, `system_locale`, `browser_user_agent`,
  `browser_version`, `os_version`, `release_channel`, `client_build_number`,
  `client_event_source`, `has_client_mods`, `client_launch_id`.

## Gateway

- Compression: `zlib-stream` (one persistent inflate context per connection,
  frames end with the `00 00 FF FF` suffix).
- Opcodes in use:

  | op | name |
  |----|------|
  | 0 | DISPATCH |
  | 1 | HEARTBEAT |
  | 2 | IDENTIFY |
  | 3 | PRESENCE_UPDATE |
  | 4 | VOICE_STATE_UPDATE |
  | 6 | RESUME |
  | 7 | RECONNECT |
  | 8 | REQUEST_GUILD_MEMBERS |
  | 9 | INVALID_SESSION |
  | 10 | HELLO |
  | 11 | HEARTBEAT_ACK |
  | 13 | CALL_CONNECT |
  | 37 | GUILD_SUBSCRIPTIONS_BULK |

- IDENTIFY `d` as the official web client sends it: `{ token, capabilities,
  properties, client_state }` with `capabilities` `1734653`. We do not send
  this, see the client identity section above. Ours is
  `{ token, compress, properties, presence }`.
- `READY_SUPPLEMENTAL` follows READY. Its `merged_members` is indexed in
  parallel with its own `guilds` list and carries our member (with roles)
  per guild.
- Guild subscriptions (member list, typing, activities) go through op 37 as
  `{ subscriptions: { <guild_id>: <options> } }`. The web client splits the
  map so that one frame stays under about 15 KB of JSON.

## Still to confirm

- READY shape for user accounts (merged members, `read_state`,
  `user_guild_settings`, `private_channels`). Will be checked against real
  captures while writing the gateway crate.
- `/auth/login` and MFA request shapes, for the later email login work.

## Features added from the official client's behaviour

Facts only, confirmed against the public web client's request code. None of
that code is reproduced here or in the repository.

- **Account preferences** live in one protobuf blob at
  `/users/@me/settings-proto/1` (`{ settings: base64 }`, patched with
  `{ settings, required_data_version }`). Sections used: 6 text and images,
  8 privacy, 11 status, 13 appearance. Booleans and numbers are wrapped in
  `BoolValue` or `UInt32Value` messages, enums and a few scalars are bare
  varints. A patch repeats every field of a section it does not change.
- **Notification settings** arrive in READY as `user_guild_settings`, either
  `{ entries, partial }` or a bare list, and as `USER_GUILD_SETTINGS_UPDATE`.
  `channel_overrides` may be a list or an object keyed by channel id. They are
  changed with `PATCH /users/@me/guilds/:id/settings`. Resolution order is
  channel, category, server, then the server default.
- **Threads** are channels with a parent. READY lists active threads under
  each guild. `THREAD_LIST_SYNC`, `THREAD_MEMBER_UPDATE` and
  `THREAD_MEMBERS_UPDATE` track membership. `GET /channels/:id/threads/search`
  lists threads and forum posts (with first messages), joining is
  `POST` or `DELETE /channels/:id/thread-members/@me`. A forum post is created
  with `POST /channels/:id/threads` carrying a nested `message`.
- **Message components** (types 1 to 14, 17) sit on a message. A click is
  `POST /interactions` with type 3, the gateway session id and the message id.
  A slash command run is type 2. The command list is
  `GET /channels/:id/application-command-index` and may answer 202 while it is
  built.
- **Polls**: `poll` on the message, results in `poll.results.answer_counts`,
  votes with `PUT /channels/:id/polls/:message/answers/@me`, live updates as
  `MESSAGE_POLL_VOTE_ADD` and `_REMOVE`.
- **Bulk read**: `POST /read-states/ack-bulk` with up to 100 read states.
- **Large servers** report channel activity in `PASSIVE_UPDATE_V2` instead of
  one event per message.
- **Pins**: `GET /channels/:id/messages/pins`, `PUT` and `DELETE`
  `/channels/:id/messages/pins/:message`.
- **Search**: `GET /guilds/:id/messages/search` (or the channel form for DMs),
  with `author_id`, `channel_id`, `mentions` and `has` filters. It answers
  with a retry wait while a server is indexing.
- **GIFs**: `GET /gifs/trending-gifs` and `GET /gifs/search`. The provider is
  now Klipy. Still previews come as AVIF, which we do not decode, so the
  animated copy is used for thumbnails.
- **Invites**: `GET /invites/:code?with_counts=true` to look one up,
  `POST /invites/:code` with the session id to join.
- **Stickers**: a server's stickers come on its READY or `GUILD_CREATE` object
  (only because we identify without `client_state`; the official client gets
  them through its cache sync instead) and are replaced wholesale by
  `GUILD_STICKERS_UPDATE` (`guild_id`, `stickers`). Sending one is a normal
  message with `sticker_ids` and may have empty `content`. `available: false`
  marks one past the slots the server's boost level pays for.
