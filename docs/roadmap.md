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
- [x] Avatar and banner upload. Verified against the live Edit Profile modal
      (screenshots/reference/edit_profile_modal.png, 2026-11-09): the modal's
      avatar tile and banner swatch carry the same shapes and sizes as the
      port's (screenshots/port/edit_profile_modal.png). The upload requests
      stay unit tested only, per the account writes rule.

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
- [x] Edit Profile modal shell, as the live client draws it: header with the
      "Main Profile" switcher, the editor column (nameplate, avatar and
      decoration, banner colour, profile effect and frame, Nitro upsell,
      display name style, bio, pronouns, save) and the preview card with the
      message row, bio, member since, connections and note over the Board,
      Activity and Wishlist widget tabs. Opened from the account page's Edit
      Profile button and from the settings header pencil. Measured from the
      live desktop capture at 1:1 (the capture is CSS pixels: the nameplate
      dialog's tile measures exactly the 106 by 80 its CSS declares), so the
      frame is 1200 by 800 with a 240 editor column, 99 tiles, a 400 preview
      card with a 140 banner and a 120 avatar, and widgets cards 431 wide:
      docs/discord-reference.md. Captures: screenshots/reference/
      edit_profile_modal.png, screenshots/reference/edit_profile_modal_dialog.png
      and screenshots/port/edit_profile_modal.png. Save sends the existing
      EditProfile request; not exercised live (account writes rule). Remaining
      differences: the preview card has no game badge row or mutual guild pill
      (no data source yet), the connections section only has its Add
      Connection row, the nameplate, decoration, effect and frame tiles open
      nothing yet, and the widget cards show real game data only.
- [x] Nameplate picker dialog, opened from the nameplate row in the Edit
      Profile modal: 598 by 501 frame with "Change Nameplate" and a close, the
      left column (Your Nameplates over the None and Shop controls at 106 by
      80 with a 2px brand ring on the selected one, the owned nameplates, then
      See What's in Shop over the shop nameplates with padlocks) and the right
      preview pane (276 wide over a 190 tall card preview with five member
      rows), over a right aligned Cancel and Apply footer with Apply dimmed
      until the pick changes. Values from the live capture and the inventory
      CSS in reference/web/assets/5257 (docs/discord-reference.md). The catalog is
      oxicord-model's NameplateCatalog, written by hand from the collectibles
      product shape because the live payload was not captured: the artwork is
      the palette colour rather than the real gradient. Apply sets the nameplate the
      editor shows; the endpoint that stores it is unconfirmed so nothing is
      sent. Captures: screenshots/reference/edit_profile_modal_dialog.png and
      screenshots/port/nameplate_picker.png. Remaining differences: the Shop
      control has no storefront glyph yet, and shop rows are dimmed rather
      than tinted behind their padlock.
- [x] Avatar decoration picker, opened from the decoration tile in the Edit
      Profile modal: 604 by 542 frame, "Change Avatar Decoration", the 3 column
      tile grid (75.33 by 80, 2 px brand inset ring when selected) split into
      Your Decorations, Exclusive to Nitro and See What's in Shop, the 280 wide
      preview with the 120 avatar and the 40/32 repeat column, and a right
      aligned Cancel and Apply footer with Apply dimmed until the pick changes.
      Values from the live capture (screenshots/reference/decoration_picker.png)
      and the picker CSS. The catalog is oxicord-model's DecorationCatalog,
      copied from the live dialog (3 Nitro, 18 shop). Apply stores the pick in
      the editor and Save sends PATCH /users/@me with avatar_decoration_data
      (request built and unit tested only, no live account write). Captures:
      screenshots/port/decoration_picker.png. Remaining differences: the Shop
      tile and the locked Nitro and shop tiles open nothing (the live client
      opens the shop or the Nitro upsell), and the account owns no decoration,
      so the selected and applied states could not be compared live.
- [x] Profile effect picker, opened from the Profile Effect tile in the Edit
      Profile modal: 663 by 802 frame, "Change Profile Effect", the 3 column
      83.33 by 80 tile grid (2 px outer brand ring when selected) split into
      Your Effects, Exclusive to Nitro and See What's in Shop, the 300 wide
      preview card (105 banner, 80 avatar, name, username and tag row, bio and
      the Example Button), and a right aligned Cancel and Apply footer with
      Apply dimmed until the pick changes. Values from the live capture
      (screenshots/reference/effect_picker.png) and the picker CSS. The catalog
      is oxicord-model's EffectCatalog, copied from the live dialog (2 Nitro,
      24 shop). Apply stores the pick in the editor and Save sends PATCH
      /users/@me with profile_effect_sku_id (request built and unit tested
      only, no live account write). Captures: screenshots/port/effect_picker.png.
      Remaining differences: the preview banner uses the port's accent fallback
      instead of the stored banner colour, and the Shop and locked Nitro and
      shop tiles open nothing (the live client opens the shop or the Nitro
      upsell). The account owns no effect, so the selected and applied states
      could not be compared live.
- [x] Profile frame picker, opened from the Profile Frame tile in the Edit
      Profile modal: 663 by 802 frame, "Change Profile Frame", the 3 column
      83.33 by 80 tile grid (2 px outer brand ring when selected) split into
      Your Frames and See What's in Shop, the 300 wide preview card (105
      banner, 80 avatar, name, username and tag row, bio and the Example
      Button), and a right aligned Cancel and Apply footer with Apply dimmed
      until the pick changes. Values from the live capture
      (screenshots/reference/frame_picker.png) and the picker DOM. A frame
      tile is a 55.33 by 68 preview with padding from the frame's overflow,
      the 1.95 ratio sample portrait drawn top aligned inside, and the top and
      bottom pictures absolutely placed around it so they spill past the
      portrait. The catalog is oxicord-model's FrameCatalog, copied from the
      live dialog (30 shop frames, none owned). Apply stores the pick in the
      editor and Save sends PATCH /users/@me with profile_frame_sku_id
      (request built and unit tested only, no live account write). Captures:
      screenshots/port/frame_picker.png. Remaining differences: the preview
      banner uses the port's accent fallback instead of the stored banner
      colour, and the Shop and locked frame tiles open nothing (the live
      client opens the shop). The account owns no frame, so the selected and
      applied states could not be compared live.
- [x] Remove the Profiles settings page. The sidebar lists My Account,
      Content & Social, Data & Privacy, Authorized Apps, Devices and
      Connections, matching the live capture, and the Profile entry, its page,
      its bio and pronoun editors and the unused helpers behind them are gone.
      "Edit Profiles" beside the account name and the account popout's Edit
      Profile both open the Edit Profile modal, as in the live client. That
      open is deferred one effect cycle with cx.defer_in: opening the modal in
      the same update that closes settings killed the process in 2 of 3 tries
      before the defer, and the deferred build survived three tries.
      Captures: screenshots/port/settings_no_profile.png and
      screenshots/port/settings_edit_profile.png.
- [x] Add Widget picker: the Board tab's empty state, and what Add Widget
      opens, is the live widget library. "Customize your profile with
      Widgets" 16 / 20 weight 500 over "Choose from our library of Widgets to
      share more about yourself and your interests" 14 / 18 in a 320 wide
      centered block, then a 24 gap and a 431 wide grid of 207.5 by 100
      cards, two columns 16 apart, radius 8, background
      rgba(151,151,159,0.12). Each card shows the add glyph or the link glyph
      for an account that has to be connected first, then a 20 high title row
      with the game icon and the label 14 / 20 weight 500. Create your own is
      first with a mic glyph and a Beta badge (16 high, 0 8 padding, full
      radius, #5865f2, 12 / 16 weight 600 white). Values from the live DOM
      and screenshots/reference/widget_picker.png. Add Widget sets the
      picker on and the Board tab sets it off.
      Remaining differences: the port draws no placeholder profile card or
      hero cover art inside the cards yet, and the Beta badge takes a flow
      row so the personal card's glyph sits lower than the live card's.
- [x] Favorite Game widget editor shell, opened from its card in the picker:
      the 431 wide inline card (radius 16, #17181b, 1 px
      rgba(151,151,159,0.16) border, 12 16 16 padding), the 14 / 18 weight 500
      title, the 132.3 high game row with the 87.25 by 116.33 add tile and the
      293.75 wide two line description, the 1 px divider, and the suggested
      games block with "Suggested for you" and six 56.2 by 74.9 covers
      (Lethal Company, Overwatch, Stardew Valley, Destiny 2, Among Us,
      Escape from Tarkov: Arena). Add Widget opens the picker again and the
      Board tab returns to the widgets. Values from the live DOM and
      screenshots/reference/favorite_game_editor.png. Remaining differences:
      the divider sits about 16 px lower and the card is about 13 px taller
      than the live one, the board header above the card shows only Add
      Widget where the live also has Create Your Own, and the add tile and
      suggested covers are not wired yet.
- [ ] Favorite Game game picker: Add Game and the suggested covers open a
      search, choose one game
- [ ] Favorite Game comment and tag editing, saved with the widget
- [ ] Games I Like widget editor, opened from its card in the picker
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
- [x] Follow an announcement channel with a server and channel picker. The
      Edit Channel dialog lists servers, then each server's text and
      announcement channels; Follow sends `POST /channels/:source/followers`.
      Functional check at 1920 x 1080: picking Velocity then download enabled
      Follow. No web capture: the live Follow button needs MANAGE_WEBHOOKS,
      which this account lacks. Port: screenshots/port/announcement_follow.png.
- [x] Message requests: channel flags, fixture and store lists. `Channel`
      parses `is_message_request`, `is_message_request_timestamp` and
      `is_spam`; the DM list hides request and spam channels;
      `message_requests()` and `spam_channels()` list them newest first.
      Reducer test against fixtures/message-requests.json. Unverified
      visually: the page module (171133 MessageRequestPage) is absent from
      reference/web and this account has no pending requests, so the live
      page cannot be opened.
- [x] Message requests page: sidebar row with the count badge and the list
      page. Row under Friends, drawn only while a request or spam channel
      exists, badge counts the plain requests; the page lists requester,
      arrival time and spam tag, a click opens the conversation. Checked at
      1920 x 1080 with a locally injected request channel (dev command
      `fake-request`, screenshots/port/message_requests_row.png and
      message_requests_page.png). Layout unverified against the official
      page: its module (171133 MessageRequestPage) is absent from
      reference/web and the live page cannot be opened with 0 requests.
- [x] Message requests: accept and ignore. Accept sends
      `PUT /channels/:id/recipients/@me` with `consent_status: 2` and hides
      the row at once (an accepted-optimistic set, like the client's);
      ignore sends `DELETE /channels/:id/recipients/@me` and the server
      answers with CHANNEL_UPDATE, a message ACK and CHANNEL_DELETE. Built
      and unit tested per the account writes rule: the requests are never
      sent from this account (a synthetic channel 404s at best). Buttons
      checked on the page with a locally injected request:
      screenshots/port/message_requests_actions.png. Button and page layout
      unverified against the official page, see above.
- [x] Spam inbox tab. The requests page has two tabs, Message Requests and
      Spam, styled like the friends page tabs; each lists only its own
      channels (requests = `is_message_request && !is_spam`, spam =
      `is_spam`). Verified on the port with a locally injected request and
      a locally injected spam channel: each tab shows exactly its own row,
      screenshots/port/spam_tab_*.png. The official page module was never
      captured, so tab layout is our own, like the page itself.
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
