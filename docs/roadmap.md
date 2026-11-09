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
- [x] Polls: create, voters modal, end early (`/polls/:id/expire`)
- [x] Uploads: mark as spoiler (`SPOILER_` filename); alt text input in composer
- [x] Syntax highlighting in code blocks
- [x] Reply ping toggle (`allowed_mentions.replied_user`)
- [x] Avatar and banner upload (account and server profile) (`PATCH /users/@me`, `PATCH /guilds/:id/members/@me`); pixel diff 7.19% (window size mismatch); visual verification partial
- [x] Profile cards matched to the web client: the popout beside a name and the full profile (board, activity, mutual friends and servers, connections with each service's logo, note), the server's own bio, banner, avatar and roles, avatar decorations, nameplates in the member list that play on hover, profile effects over both cards, a popout message box that sends, and our own card with the status bubble. Not done: Spotify's listen along and connect buttons, right to left text in bios (gpui has no bidi), logos for services beyond the 22 Discord lists
- [x] Private user notes (`/users/@me/notes/:id`) and friend nicknames (`PATCH /users/@me/relationships/:id`), both done; `USER_NOTE_UPDATE` and `RELATIONSHIP_UPDATE` from other devices are not handled yet

## Phase 3: servers you can't fully use yet

- [x] Rules screening: the "Complete" banner replaces the composer while our
      member is `pending`, and the full dialog (server card, icon palette
      gradient, numbered rules, agree checkbox, submit states, close) posts to
      `/guilds/:id/requests/@me`. Verified against the live SCP | Roleplay
      dialog; pixel diff 5.67 % on the entry banner crop, visual crop pairs
      PASS. Remaining differences: the agree row sits fixed above the footer
      because GPUI's scroll viewport swallowed its hitbox (identical when
      scrolled), the scroll edge fade is not drawn, and Windows subpixel text
      anti-aliasing differs from the web client's grayscale AA. The banner image
      is cropped from the centre with a 13 px offset.
- [x] Screening questions other than TERMS: TEXT_INPUT, PARAGRAPH and
      MULTIPLE_CHOICE fields, the manual header and subtitle, required-field
      validation, paragraph character count, radio groups and focus borders.
      The live manual modal cannot be opened for an account already through
      screening, so its values come from the client modules and CSS and the port
      was checked against the real Mossad fixture (962385052778512434, five
      fields); pixel diff not applicable. Remaining differences: written answers
      are not capped at 150/1000 characters, and a form taller than the right
      pane does not scroll because GPUI's scroll viewport swallowed the field
      hitboxes.
- [ ] Onboarding: prompts, channel and role selection, and the
      `GUILD_ONBOARDING_*` updates.
- [ ] Welcome screen: the server home with its description and featured
      channels when no channel is selected (`/guilds/:id/welcome-screen`).
- [x] Member moderation: kick, ban, timeout, roles, in the three dots menu of the profile cards (not the right click menu), each gated on its own permission. Kick has no confirmation yet and there is no role hierarchy check

## Phase 4: bots

- [ ] Modals opened by components (`INTERACTION_MODAL_CREATE`) (blocked: interaction framework not implemented)
- [ ] Slash command option autocomplete (`APPLICATION_COMMAND_AUTOCOMPLETE_RESPONSE`) (blocked: autocomplete framework needed)
- [ ] Thinking and failed states after a click (`INTERACTION_SUCCESS` / `INTERACTION_FAILURE`) (blocked: interaction state tracking needed)

## Phase 5: DMs, inbox, events

- [ ] Message requests and the spam inbox (blocked: inbox rework needed)
- [x] Saved messages (`/users/@me/saved-messages`): bookmark and remove from the message menu, listed in the inbox with reminders and scheduled messages
- [x] Inbox "For You" tab (notification center items) (NOTIF_CENTER_ITEMS)
- [ ] Scheduled events: list, RSVP, live updates (`GUILD_SCHEDULED_EVENT_*`) (blocked: scheduled event model/API needed)
- [x] Create a server (`POST /guilds`): API, bridge and UI dialog wired, sends from the server rail plus button
- [x] Announcement channels: publish and follow (`/crosspost`, `/followers`) (follow via channel id input)

## Phase 6: account and admin

- [ ] Change password, enable and disable 2FA (blocked: auth flows needed)
- [x] Log out other devices, add and remove connections (remove connection done)
- [ ] Disable or delete the account (blocked: account deletion flow needed)
- [ ] Server admin: channel permission overwrites, AutoMod, webhooks, integrations,
      welcome screen and onboarding setup, templates, prune (blocked: admin UI breadth)

## Voice (added 2026-11-07)

- [x] Join and leave voice channels and private calls, hear everyone (native voice protocol v8, Opus, AES-GCM or XChaCha20 transport, end-to-end encryption through DAVE)
- [x] Speak: microphone capture with a simple voice gate, mute and deafen sent to Discord
- [x] Live check: joined a real call from the app and heard it connect (audio quality not judged)
- [x] Call UI matched to the web client: connection block, tiles colored from the avatar, name pills, speaking ring (derived from the audio, so it ends when talking ends), control bar, the page before joining, call clock, text chat beside the call, microphone and speaker choice from the arrows
- [x] The user card floats across the rail and the list like the official one
- [ ] Camera, screen share, activities and soundboard (buttons are drawn, they do nothing) (blocked: media capture pipeline needed)
- [ ] Pop out, Krisp noise suppression, input volume, push to talk, per-user volume (blocked: audio device controls needed)
- [ ] Resume a call after a dropped voice websocket (today it ends and has to be joined again) (blocked: voice reconnect logic needed)
- [ ] Video, screen share and Go Live (blocked: video pipeline needed)
