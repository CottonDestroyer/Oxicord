# Roadmap

What the official web client does that Oxicord still does not, found by
comparing its endpoints and gateway events with ours (2026-11-06). Video, screen
share, stage, Go Live, soundboard and voice messages are left out on purpose.

Work through the phases in order. Tick items off as they land.

## Phase 1: stop going stale

Gateway events we ignore today, so things go out of date until a restart.

- [x] `USER_SETTINGS_PROTO_UPDATE`: settings changed on another device (mutes, folders, preferences)
- [x] `MESSAGE_REACTION_ADD_MANY`: batched reactions in busy servers, currently lost
- [x] `CHANNEL_PINS_UPDATE`: refresh the pins panel
- [x] `USER_UPDATE` and `SESSIONS_REPLACE`: own account done (the gateway names it USER_UPDATE, we listened for CURRENT_USER_UPDATE); sessions skipped, only voice uses them
- [x] `GUILD_BAN_ADD` / `GUILD_BAN_REMOVE`: keep the bans page live
- [x] `RECENT_MENTION_DELETE`: drop cleared mentions from the inbox
- [x] `GUILD_MEMBERS_CHUNK` (op 8 request): member lookup by id instead of one REST call per user
- [x] `VOICE_CHANNEL_STATUS_UPDATE`: the status text under a voice channel

## Phase 2: everyday gaps

- [x] Set your own custom status (dialog from the account popout, saved in the settings blob)
- [x] Forward a message
- [x] Who reacted, tooltip done (`GET /channels/:id/messages/:id/reactions/:emoji`), super reactions
- [ ] Polls: create, list voters (`/polls/:id/answers/:id`), end early (`/polls/:id/expire`)
- [x] Uploads: mark as spoiler (`SPOILER_` filename); alt text still missing
- [x] Syntax highlighting in code blocks
- [x] Reply ping toggle (`allowed_mentions.replied_user`)
- [ ] Avatar and banner upload, (server nickname done) (`PATCH /guilds/:id/members/@me`), server profile
- [x] Private user notes (`/users/@me/notes/:id`) and friend nicknames (`PATCH /users/@me/relationships/:id`), both done; `USER_NOTE_UPDATE` and `RELATIONSHIP_UPDATE` from other devices are not handled yet

## Phase 3: servers you can't fully use yet

- [ ] Rules screening (`/guilds/:id/member-verification`), onboarding, welcome screen
- [x] Member moderation: kick, ban, timeout, roles, as buttons on the profile card (not the right click menu), each gated on its own permission. Kick has no confirmation yet and there is no role hierarchy check

## Phase 4: bots

- [ ] Modals opened by components (`INTERACTION_MODAL_CREATE`)
- [ ] Slash command option autocomplete (`APPLICATION_COMMAND_AUTOCOMPLETE_RESPONSE`)
- [ ] Thinking and failed states after a click (`INTERACTION_SUCCESS` / `INTERACTION_FAILURE`)

## Phase 5: DMs, inbox, events

- [ ] Message requests and the spam inbox
- [x] Saved messages (`/users/@me/saved-messages`): bookmark and remove from the message menu, listed in the inbox with reminders and scheduled messages
- [ ] Inbox "For You" tab (notification center)
- [ ] Scheduled events: list, RSVP, live updates (`GUILD_SCHEDULED_EVENT_*`)
- [ ] Create a server (`POST /guilds`): API call and bridge command exist, nothing in the UI sends it yet
- [ ] Announcement channels: publish (`/crosspost`), follow (`/followers`)

## Phase 6: account and admin

- [ ] Change password, enable and disable 2FA
- [ ] Log out other devices, add and remove connections
- [ ] Disable or delete the account
- [ ] Server admin: channel permission overwrites, AutoMod, webhooks, integrations,
      welcome screen and onboarding setup, templates, prune

## Voice (added 2026-11-07)

- [x] Join and leave voice channels and private calls, hear everyone (native voice protocol v8, Opus, AES-GCM or XChaCha20 transport, end-to-end encryption through DAVE)
- [x] Speak: microphone capture with a simple voice gate, mute and deafen sent to Discord, speaking ring in the channel list
- [ ] Verified live against a real call (written from the protocol docs, not yet tried against Discord)
- [ ] Choose input and output devices, input volume, push to talk, per-user volume
- [ ] Resume a call after a dropped voice websocket (today it ends and has to be joined again)
- [ ] Video, screen share and Go Live
