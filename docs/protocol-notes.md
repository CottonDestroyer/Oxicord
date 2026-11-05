# Protocol notes

Facts confirmed against the official web client (client_build_number 663810,
checked 2026-11-05) and the public API docs. Nothing here is copied code. The
build number goes stale quickly, so treat it as a default that can be
overridden.

## REST

- Base URL: `https://discord.com/api/v9`.
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

- IDENTIFY `d`: `{ token, capabilities, properties, client_state }`.
  - `capabilities` is `1734653` by default, `1767421` when private channel
    obfuscation is enabled.
  - `properties` is the same object as the super properties, plus
    `is_fast_connect`.
  - `client_state` starts as `{ "guild_versions": {} }`.
- Guild subscriptions (member list, typing, activities) go through op 37 as
  `{ subscriptions: { <guild_id>: <options> } }`. The web client splits the
  map so that one frame stays under about 15 KB of JSON.

## Still to confirm

- READY shape for user accounts (merged members, `read_state`,
  `user_guild_settings`, `private_channels`). Will be checked against real
  captures while writing the gateway crate.
- `/auth/login` and MFA request shapes, for the later email login work.
