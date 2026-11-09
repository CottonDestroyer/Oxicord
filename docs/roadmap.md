# Roadmap

What the official client does that Oxicord still does not, found by comparing
its endpoints and gateway events with ours (first pass 2026-11-06).

## How to use this file

- Work top to bottom. Each unchecked line is one feature and one commit.
- Nothing here is "blocked". If an item is too big, split it into smaller lines
  in place, the first needing only reference/ and fixtures, and start that one.
- Endpoint and event names marked (confirm) are best guesses: check them in
  reference/notes/ before writing code and fix the line if wrong.
- When done, tick it and add a short note: remaining differences, pixel diff %,
  or "unverified: <reason>".
- Account and admin writes: build and unit test the request, wire the UI, but
  never send it from the real account. Test against a throwaway server only.

## Phase 0: shared fixes (unlock several items below)

- [x] GPUI scroll viewport swallows child hitboxes. One viewport lives in
      `crates/oxicord-ui/src/scroll.rs` (`flex_1().min_h_0()` in flow, one id).
      The original symptom did not reproduce: clicks reached content in both an
      in-flow and an absolute viewport once the row was inside the scrolled
      content, so the note above was a misdiagnosis. The screening dialog uses
      the shared viewport; other panes adopt it as they are touched.
- [x] Rules screening: agree row inside the scroll area. Verified at
      1920 x 1080: the row scrolls with the rules, toggles on click and the
      submit button enables. Crops: screenshots/port/rules-checkbox.png.
- [x] Screening forms taller than the pane scroll. The manual application
      content uses the same viewport; the 5-question Mossad form scrolls at
      1280 x 720 and its inputs take focus and text.
- [x] Scroll edge fade, as the web client draws it, in the shared scroll
      component. 170 px linear fade to #0a0a0c, drawn at the top while
      scrolled and at the bottom while more content follows. Sampled panel
      background within 1 unit of the reference at every 10 px step.
- [x] Screening answers capped at 150 (text) and 1000 (paragraph)
      characters. Text rejects keys past 150; paragraph trims on change and
      puts the caret back at the end. Verified by typing 160 and 1010
      characters into the Mossad form: the 10 extra letters dropped at each
      limit and the counter stopped at 1000/1000.
- [ ] Avatar and banner upload: unverified. The live client opens an Edit
      Profile modal (nameplate, avatar and decoration, banner color, effects)
      instead of a Profiles settings page, and the port's saved login was
      missing from the credential manager this session, so its Profile page
      could not be opened. Live capture: screenshots/reference/
      avatar_banner.png. The old port capture showed a login screen and was
      removed. Verify after the Edit Profile modal is ported.

## Phase 1: stop going stale (done)

- [x] `USER_SETTINGS_PROTO_UPDATE`: settings changed on another device
- [x] `MESSAGE_REACTION_ADD_MANY`: batched reactions
- [x] `CHANNEL_PINS_UPDATE`: refresh the pins panel
- [x] `USER_UPDATE` (own account). `SESSIONS_REPLACE` skipped, only voice uses it
- [x] `GUILD_BAN_ADD` / `GUILD_BAN_REMOVE`: bans page stays live
- [x] `RECENT_MENTION_DELETE`: cleared mentions leave the inbox
- [x] `GUILD_MEMBERS_CHUNK` (op 8): member lookup by id
- [x] `VOICE_CHANNEL_STATUS_UPDATE`: status text under a voice channel

## Phase 2: everyday gaps

- [x] Custom status dialog from the account popout
- [x] Forward a message
- [x] Who reacted tooltip, super reactions
- [x] Polls: create, voters modal, end early
- [x] Uploads: spoiler, alt text in composer
- [x] Syntax highlighting in code blocks
- [x] Reply ping toggle
- [x] Avatar and banner upload (account and server profile)
- [x] Profile popout and full profile matched to the web client. Known gaps:
      Spotify listen along, RTL bios (gpui has no bidi), logos beyond the 22
      Discord lists
- [x] Private notes and friend nicknames
- [ ] Edit Profile modal, as the live client draws it: nameplate, avatar and
      decoration, banner color, profile effect and frame, widgets board
- [x] `USER_NOTE_UPDATE`: notes changed on another device. Handled in
      `session.rs`, which updates the cached profile card.
- [x] `RELATIONSHIP_UPDATE`: friend nickname changed on another device. The
      gateway parses it with `RELATIONSHIP_ADD` and the store replaces the
      whole relationship, nickname included. Added a reducer test for a
      nickname being set and cleared.

## Phase 3: servers you can't fully use yet

- [x] Rules screening: banner and full dialog (diff 5.67%, crops PASS)
- [x] Screening questions: TEXT_INPUT, PARAGRAPH, MULTIPLE_CHOICE (checked
      against the Mossad fixture; live modal unreachable)
- [x] Member moderation: kick, ban, timeout, roles in the profile card menu
- [x] Kick confirmation dialog. Implemented in b442a41 and wired to
      `DELETE /guilds/:id/members/:user`. Unverified: the account has no
      `KICK_MEMBERS` in any joined server, so the live client shows no Kick
      action to open it from.
- [x] Role hierarchy check: hide or disable actions on members above you.
      `card_actions` only offers kick, ban, timeout and roles when our highest
      role sits above theirs, with administrators exempt and the owner never
      a target. Visual check blocked by the same missing permission.
- [x] Welcome screen fixture from `GET /guilds/:id/welcome-screen`.
      fixtures/welcome-screen.json, captured from SCP | Roleplay.
- [x] Welcome screen model and endpoint: `WelcomeScreen` and
      `WelcomeChannel` parse the fixture, and `ApiClient::welcome_screen`
      calls `GET /guilds/:id/welcome-screen`. Unit test against
      fixtures/welcome-screen.json.
- [ ] Welcome screen view: description and featured channels when no channel
      is selected. Unverified layout: /channels/<guild> redirects to a real
      channel for an existing member, so this account cannot open the live
      welcome screen. Data: fixtures/welcome-screen.json.
- [ ] Onboarding fixture from `GET /guilds/:id/onboarding` (confirm)
- [ ] Onboarding prompts view: questions and options, single and multi select
- [ ] Onboarding channel and role selection applied, responses sent (confirm
      endpoint)
- [ ] Onboarding live updates: `GUILD_ONBOARDING_*` events (confirm names)

## Phase 4: bots

- [ ] Interaction state tracking: nonce on `POST /interactions`, handle
      `INTERACTION_CREATE`, `INTERACTION_SUCCESS`, `INTERACTION_FAILURE`
- [ ] "Thinking" state on the clicked component and the message
- [ ] "Interaction failed" state and its retry
- [ ] Modal view from an `INTERACTION_MODAL_CREATE` fixture: title, text inputs
      (short and paragraph), required, min and max length
- [ ] Modal submit (interaction type 5) and its success and failure states
- [ ] Slash option autocomplete request (interaction type 4) while typing
- [ ] Autocomplete results list from
      `APPLICATION_COMMAND_AUTOCOMPLETE_RESPONSE`

## Phase 5: DMs, inbox, events

- [x] Saved messages, with reminders and scheduled messages in the inbox
- [x] Inbox "For You" tab
- [x] Create a server from the server rail
- [x] Announcement channels: publish and follow
- [ ] Follow an announcement channel with a server and channel picker, as the
      web client does, instead of typing a channel id
- [ ] Message requests: list from fixtures (find the flag and endpoints in
      reference/notes/)
- [ ] Message requests: accept and ignore
- [ ] Spam inbox tab
- [ ] Scheduled events fixture from `GET /guilds/:id/scheduled-events`
- [ ] Scheduled events list and event card
- [ ] Event details modal
- [ ] RSVP (interested) and its count (confirm endpoint)
- [ ] Live updates: `GUILD_SCHEDULED_EVENT_CREATE`, `_UPDATE`, `_DELETE`,
      `_USER_ADD`, `_USER_REMOVE`

## Phase 6: account (UI and request building only, see the rule at the top)

- [x] Log out other devices, remove a connection
- [ ] Add a connection
- [ ] Change password dialog
- [ ] Enable 2FA: QR code and backup codes screens
- [ ] Disable 2FA dialog
- [ ] Disable account dialog
- [ ] Delete account dialog

## Phase 7: server admin (test server only)

- [ ] Channel permission overwrites: list roles and members
- [ ] Channel permission overwrites: edit allow, deny, inherit
- [ ] Webhooks: list, create, edit, delete
- [ ] AutoMod rules: list
- [ ] AutoMod rules: create and edit
- [ ] Integrations list
- [ ] Welcome screen setup
- [ ] Onboarding setup
- [ ] Server templates
- [ ] Prune members: count preview and confirm

## Voice

- [x] Join, leave and hear voice channels and private calls (voice v8, Opus,
      AES-GCM or XChaCha20, DAVE)
- [x] Speak: mic capture, voice gate, mute and deafen
- [x] Live call check
- [x] Call UI matched to the web client
- [x] Floating user card across the rail and list
- [ ] Input volume slider
- [ ] Per-user volume (local gain) from the user's right click menu
- [ ] Push to talk with a configurable key
- [ ] Resume a call after the voice websocket drops (voice resume opcode)
- [ ] Pop out the call into its own window
- [ ] Noise suppression toggle (Krisp is proprietary: use RNNoise and note
      the difference)

## Out of scope

Video, camera, screen share, Go Live, stage, activities, soundboard and voice
messages. Their buttons stay drawn and do nothing. Spotify listen along.
