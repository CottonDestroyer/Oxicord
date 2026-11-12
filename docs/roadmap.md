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
- [x] Edit Profile modal shell, as the live client draws it: full-height left
      panel with the "Main Profile" switcher, the editor sections (nameplate,
      avatar and decoration, banner colour, profile effect and frame, Nitro
      upsell, display name style, bio, pronouns) and the preview card with the
      message row, bio, member since, connections and note over the Board,
      Activity and Wishlist widget tabs. Opened from the account page's Edit
      Profile button and from the settings header pencil. Measured from the
      live capture and the modal CSS at
      `reference/web/live-667580/8e8eccd890e9820b.css`, the frame is 1202 by
      800 at (359, 140) with a 242 left panel (240 inner), a 958 right region
      at 48 left / 49 top / 32 right padding, a 400 by 750 preview card with a
      140 banner and a 120 avatar, and a 487 by 750 board. The 500 by 50 save
      bar sits 24 below the dialog (top 824, left 351) in an 874 high wrapper;
      otherwise its clicks fall through to the scrim. The board header and
      disclaimer are built in `#[inline(never)]` helpers because the inline
      builder chain overflowed the main stack on open. Widget cards are 431
      wide. docs/discord-reference.md. Captures:
      screenshots/reference/edit_profile_modal_picker.png,
      screenshots/reference/edit_profile_modal_live.png,
      screenshots/port/modal_layout5.png and
      screenshots/port/gi_edit_check.png. Verified at 1920 x 1080: the modal
      lands at (359, 140) 1202 by 800, the preview at (649, 189) 400 by 750,
      the board border at 1045, the picker title at (1136, 257) and the first
      card at (1081, 345); Reset re-fills the editors and stays open. Save
      sends the existing EditProfile request; not exercised live (account
      writes rule). Remaining differences: the preview card has no game badge
      row or mutual guild pill (no data source yet), the connections section
      only has its Add Connection row, the nameplate, decoration, effect and
      frame tiles open nothing yet, and the widget cards show real game data
      only.
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
- [x] Favorite Game game picker: Add Game opens the 398 by 354 Browse games
      popover at the add tile's right (8 padding, radius 8, #111214, 1 px
      rgba(151,151,159,0.2) border, a 40 high search field and a 380 by 288
      scrolling list of 40 high rows, radius 4, 16 / 20 weight 500 labels and
      a check on the selected row), and a row selects one game and closes it.
      The suggested covers select their game directly. The selected game
      replaces the add tile with its cover and shows its name where the
      comment fields will go. The 20 game names are bundled from the live
      list. Remaining differences: the search field is not focused on open so
      it has no brand focus ring, and the six games without a captured cover
      show a wash placeholder. Values from the live DOM and
      screenshots/reference/browse_games.png.
- [x] Favorite Game comment and tag editing: a chosen game shows its 87.25
      by 116.33 cover and, to its right, the name, a 300.75 wide comment
      field ("Let everyone know why this is your favorite", 14 / 18, max 200)
      and a 20 high tag row. Tag chips are 20 high, radius 8, 1 px
      rgba(151,151,159,0.2) border, padding 3 8 3 6, with a 12 px tag-award
      icon, a 10 / 12 weight 500 label and a 12 px X. "+ Tags" is a 42.4 by
      20 dashed chip opening a 172.7 by 400 menu, 8 to its right: 36 high
      rows with a 20 px radio, grouped Skill level, Rating and Looking for,
      one value per group, and the menu stays open while picking. Remove Game
      is a 32 by 32 button 8 from the top right. The selected card is 188.3
      high and drops the divider and suggestions. Values from the live DOM and
      screenshots/reference/favorite_game_selected.png. Remaining differences:
      clicking outside the tag menu does not close it, the tag values are
      lowercased labels where the wire values are unverified, and the card's
      remove button uses a new local trash icon copied from the live SVG.
- [x] Favorite Game widget save: the chosen game, comment and tags become a
      `WidgetsSubmission` from `oxicord-model`, which serializes
      `{"widgets":[{"data":{"type":"favorite_games","games":[{"game_id":"...","comment":"...","tags":["..."]}]}}]}`
      (a new widget has no `id`). The editor sends the wire tag values and
      the ids read from the live browse list; a unit test covers the payload.
      Picking a game and returning to Board shows the pending card with its
      cover, name and comment, and Save changes builds the request. Verified at
      1920 x 1080: choosing Lethal Company and clicking Board shows its cover
      and name. Nothing is
      sent from the real account. Remaining differences: the card is the
      simple cover/name/comment row, not the live profile widget, and the
      pending card is lost when the modal reopens.
- [x] Games I Like widget editor, opened from its card in the picker. The card
      is the same 431 wide, radius 16, #17181b, 1 px
      rgba(151,151,159,0.16) border, 12 16 16 padding, 8 gap. Header row 32
      high (34 once a game is chosen): "Games I Like" 14 / 18 weight 500
      #dededf, the "Add up to 20 games" subtitle 12 / 16 #9b9ca3 once games
      are chosen, and a 32 by 32 Add Game button (radius 8, rgba(151,151,159,
      0.12), 1 px border, 16 px plus). Empty body 64 high with the two line
      "Add up to 20 games. This Widget won't show up on your profile until you
      add at least 1 game." 12 / 16 #9b9ca3. Suggestions keep the 4 top
      margin, 16 top padding, 8 gap, "Suggested for you" 12 / 16 weight 500
      #9b9ca3 with an 18 px X, and six 56.16 by 74.89 covers 12 apart (Dark
      and Darker, Chained Together, Sid Meier's Civilization VI, MapleStory,
      Left 4 Dead 2, Call of Duty: MWIII). Chosen games form a 16 gap wrap of
      87.25 by 116.33 covers, four per row, each with a 32 by 32 Remove Game
      button 4 from its top right that appears on hover. Add Game opens the
      same 398 by 354 Browse games popover, centred under the button and 8
      below it (199 from the card's left, 53 from its top), with the 20 game
      browse order captured live. Up to 20 games; the entry saves as
      `played_games` through `WidgetsSubmission`. Verified at 1920 x 1080:
      opening the card shows the empty editor and its suggestions, and
      choosing Dark and Darker shows its 87.25 by 116.33 tile and the
      "Add up to 20 games" subtitle. Nothing is sent from the real account.
      Remaining differences: the Add Game popover keeps the same game list as
      the empty state rather than dropping chosen games, and the suggestion
      block does not rotate in new games as it does live.
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
- [x] Welcome screen view: description and featured channels when no channel
      is selected. The stable 668923 bundle has no view component (only the
      store, actions and route), and the live client redirects an existing
      member to a real channel, so the layout is unverified: it is built from
      the endpoint and theme tokens, and verified only by temporarily clearing
      the channel. Content is a 480 wide centred column: 64 guild icon,
      "Welcome to <server>!" 24 / 30, the description 16 / 22, then
      FEATURED CHANNELS and 480 by 56 rows (radius 8, BG_CARD, hash or unicode
      emoji, name 16 / 20, description 14 / 18) that select the channel when
      clicked. Unknown channels are skipped. Data:
      fixtures/welcome-screen.json; screenshot: screenshots/port/
      welcome_screen.png. docs/discord-reference.md.
- [x] Onboarding fixture from `GET /guilds/:id/onboarding`, confirmed and
      captured for SCP | Site Roleplay Community in fixtures/onboarding.json:
      one optional single-select prompt with one option, 19 default channels.
      `Onboarding`, `OnboardingPrompt`, `OnboardingOption` and
      `OnboardingResponse` parse it and `ApiClient::onboarding` calls
      `GET /guilds/:id/onboarding`. Unit test against the fixture. Payload
      shape in docs/discord-reference.md.
- [x] Onboarding prompts view: questions and options, single and multi select.
      Opened from the server name dropdown's "Server Guide" item. A 620 by
      700-at-most dialog: 48 header, 16 body padding, each prompt a radius 12
      card with title 16/20, type 12, and option rows 12/10 with a 20 px ring
      (single) or box (multi) that fills with the accent when picked. Required
      prompts carry an 18 high chip. Selections are local; no answer is sent
      yet (next item). Unverified live: this account completed onboarding and
      the reachable @home Server Guide shows the channel list instead. Verified
      against E.T.T.R.A's four active prompts by forcing the dialog to that
      server: screenshots/port/onboarding_prompts.png, empty state in
      screenshots/port/onboarding_empty.png. docs/discord-reference.md.
- [x] Onboarding channel and role selection applied, responses sent. Endpoint
      confirmed in the web client: `PUT /guilds/:id/onboarding-responses`
      (POST only during the onboarding flow), body
      `{ onboarding_responses: [option ids],
      onboarding_prompts_seen: { prompt id: ms },
      onboarding_responses_seen: { option id: ms } }`. `Onboarding::submission`
      builds it from the picked options and `ApiClient::update_onboarding`
      sends it; the Save button shows "Saving...", closes on success and shows
      the error on failure. Unit test checks the JSON shape. The Save button
      was never pressed against the real account (account-write rule); channels
      and roles are granted by the server and arrive through the gateway, so
      there is no local application step. docs/discord-reference.md.
- [x] Onboarding live updates: `GUILD_ONBOARDING_*` events. Confirmed there
      are none: the official gateway event list has no onboarding entries,
      and the web client's onboarding store has only REST fetch actions plus
      local selection dispatch. The port already asks `GET
      /guilds/:id/onboarding` every time the guide opens, which is the
      client's refresh path. No gateway handler to add.
      docs/discord-reference.md.

## Phase 4: bots

- [x] Interaction state tracking: nonce on `POST /interactions`, handle
      `INTERACTION_CREATE`, `INTERACTION_SUCCESS`, `INTERACTION_FAILURE`.
      Nonce is made by `Snowflake::nonce()` in the UI and sent with the click;
      the model parses the three events (id, nonce, type, reason_code) into
      `DispatchEvent::InteractionCreate/Success/Failure`; `Session.interactions`
      maps the nonce to the message, custom id, component type and state
      (Queued, Created, removed on success, Failed). Unit test on the three
      payloads. docs/discord-reference.md.
- [x] "Thinking" state on the clicked component and the message. The clicked
      button shows a 16 px ellipsis at 70% opacity instead of its label, and
      the message shows a 12 px "Thinking..." line with a 14 px ellipsis under
      the components. Verified by forcing the state on Bloxlink's buttons in
      SCP #get-roles: screenshots/port/thinking_state.png. Not clicked live:
      a real click sends an interaction to the bot (write).
- [x] "Interaction failed" state and its retry. `INTERACTION_FAILURE` sets the
      entry to Failed; the clicked button keeps its label with a 1 px DANGER
      border, and the message shows a 14 px question glyph, "Interaction
      failed" in DANGER and a Retry link. Retry re-sends the same component
      type, custom id and values with a fresh nonce; the old failed entry is
      dropped. Verified by forcing the state on Bloxlink's buttons:
      screenshots/port/interaction_failed.png. A real failure needs a live
      click, which is a write, so it is unverified live.
      docs/discord-reference.md.
- [x] Modal view from an `INTERACTION_MODAL_CREATE` fixture: title, text inputs
      (short and paragraph), required, min and max length. `modal` loads the
      fixture: 480 wide, 12 radius, 48 header with a 20/24 title and 40 close,
      body labels 16/20 and a 12/16 counter, footer 40 high buttons. The frame
      follows the live Create a Poll modal (the same newer modal component) at
      1920 x 1080; the bot modal component is a lazy chunk absent from
      reference/web and no bot in the account opened one (Ticket King creates
      tickets directly, Ticket Tool /panel timed out), so its exact width,
      header and footer are unverified. Verified in the port at 1920 x 1080:
      empty required fields keep Submit disabled, 5 and 13 characters enable
      it, and 85/80 turns the hint and counter red and disables Submit again.
      Captures: screenshots/port/interaction_modal_v8.png,
      interaction_modal_filled.png, interaction_modal_too_long.png.
- [x] Modal submit (interaction type 5) and its success and failure states.
      Submit sends every input, blank ones included, as one action row per text
      input in `POST /interactions` type 5, with the modal custom id, app,
      channel, gateway session and a new nonce. The modal stays open and Submit
      dims until the gateway answers; `INTERACTION_SUCCESS` closes it, while
      `INTERACTION_FAILURE`, a failed request or ten seconds without an answer
      keeps the answers and shows "Something went wrong. Try again." above the
      fields. The request body is unit tested. The live bot modal component is
      a missing lazy chunk and no bot in the account opened a modal, so the
      live in-flight and error presentation are unverified; the port's error
      banner is styled from Discord's critical feedback colours and captured in
      screenshots/port/interaction_modal_failed.png.
- [x] Slash option autocomplete request (interaction type 4) while typing.
      Typing in an option the bot marks `autocomplete` sends `POST
      /interactions` type 4 with a fresh nonce, the other filled options and
      the typed one marked `focused`, and `guild_id` in `data` as the web
      client's module 721768 does (without it the bot failed the request).
      Body unit tested. Verified live in the Dyno DM: typing "rai" into
      `/help command` is answered by the bot. A failed request or an
      INTERACTION_FAILURE for the nonce shows "Couldn't load options.".
- [x] Autocomplete results list from
      `APPLICATION_COMMAND_AUTOCOMPLETE_RESPONSE`. The event (choices, nonce)
      parses to `DispatchEvent::AutocompleteResponse`; the form shows an
      "Options matching <query>" list under the field and a row puts its name
      in the box and keeps its value for the run (no second request fires).
      Stale answers are dropped by nonce. Verified at 1296 x 808 against the
      live Dyno DM: screenshots/port/autocomplete_dm_results.png and
      autocomplete_dm_picked.png, reference
      screenshots/reference/autocomplete-live.png. Remaining differences: the
      web list floats above the composer and the option shows as an inline
      chip, the port keeps the form's own field. Run was not pressed (it
      would send a command to the bot).

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
- [x] Scheduled events fixture and model. The web client never calls
      `GET /guilds/:id/scheduled-events`: the events ride on each guild in
      READY and `GUILD_CREATE` as `guild_scheduled_events`, and
      `GET /users/@me/scheduled-events?guild_ids=` only returns our own
      RSVPs (an empty list here). RSVP totals come from
      `GET /guilds/:id/scheduled-events/:id/users/counts` (answer
      `{guild_scheduled_event_count, guild_scheduled_event_exception_counts}`).
      fixtures/scheduled-events.json holds SCP | Roleplay's 4 events, read from
      the live READY. `ScheduledEvent` parses id, guild, channel, name,
      description, start and end, status, entity type, location, image hash
      and recurrence, and `Guild.scheduled_events` keeps them (a bad event is
      dropped, not the guild). Unit tests on the fixture and the guild.
- [x] Scheduled events list and event card. A "4 Events" row (calendar glyph,
      CHANNELS_DEFAULT) sits above the channels while the server has events
      running or coming, and opens the Events modal: 604 wide frame (radius
      12, max 802 high) with a 48 high header, 24 side margins, 554 wide
      cards 16 apart (BG_CARD, radius 8, 1 px border), a 120 high cover from
      `cdn.discordapp.com/guild-events/:id/:hash`, a status row (green
      "Happening Now" or the blue "Fri Nov 20th · 7:00 PM"), the 20/24 name,
      the markdown description capped at 4 lines, a divider and a footer with
      the location, a "..." button, Copy Link (copies
      discord.com/events/:guild/:event) and Interested on upcoming events.
      Verified at 1920 x 1080 against the live SCP | Roleplay modal: frame,
      card heights (about 351) and gaps match. Captures:
      screenshots/reference/events_modal.png and
      screenshots/port/events_modal.png, events_sidebar_row.png. Remaining
      differences: no RSVP pill or interested avatars yet (counts come with
      the RSVP item), the sidebar "Happening Now" card above the channel list
      is not drawn, and the "..." and Interested buttons do nothing yet.
- [x] Event details modal, opened by clicking a card in the Events modal and
      closing back to the list. 680 wide frame (radius 12, height the window
      minus 155, max 952, 8 top and 16 bottom padding), a 40 by 40 close
      button, the 20/24 weight 600 title, a 630 by 240 cover (radius 8), the
      Event Info and Interested tabs (16/20 weight 500, 32 apart, a 2 px
      track with a brand underline on the active one), the date row (blue
      calendar and "Fri Nov 20th · 7:00 PM, a dash, Fri Dec 4th · 7:00 PM", or
      green "Happening Now" for a running event), the name again, a 28 pitch
      info list (server with its icon, location or voice channel), the
      markdown description and a footer with a 40 high "...", Copy Link and
      Interested (not on a running event). Values from the live DOM and
      screenshots/reference/event_details_modal.png and
      event_details_active.png; port capture screenshots/port/
      event_details.png at 1920 x 1080. Remaining differences: the "people
      are interested" and "Created by" rows need the counts and a user
      lookup (next item), the Interested tab is not clickable yet, list
      items sit closer together than the web client's markdown, and the
      sidebar "Happening Now" card with its Event Details button is not
      drawn (`EventsDialog::opened_event` is ready for it).
- [x] RSVP (interested) and its count. Confirmed in the web client's module
      496092: the count is `GET /guilds/:id/scheduled-events/:id/users/counts`
      (`{guild_scheduled_event_count, ...}`), our own answers are `GET
      /users/@me/scheduled-events?guild_ids=` (entries carry
      `guild_scheduled_event_id` and `response`, 1 interested), and the toggle
      is `PUT .../scheduled-events/:id/users/@me` with `{"response":1}` or
      `DELETE` on the same path, flipped by module 929267 (no answer sends 1,
      an answer deletes). The dialog asks the counts and our answers when it
      opens: each card shows a black pill with a people glyph and the total, the
      details modal shows "1,108 people are interested" and an "N Interested"
      tab label. Interested flips the button and the count at once and undoes
      both if the request fails. Request body unit tested; never sent from the
      real account. Pressed look checked with the dev command `interested
      <event>` (local only): screenshots/port/event_interested.png, 1296 x 808.
      Remaining differences: the pressed style (accent border) is unverified
      against the web client because its button lives in a lazy chunk and a
      live click would write to the account, recurring events skip the
      occurrence picker, and the Interested tab does not list people yet.
- [x] Live updates: `GUILD_SCHEDULED_EVENT_CREATE`, `_UPDATE`, `_DELETE`,
      `_USER_ADD`, `_USER_REMOVE`. Payloads read from the web client's
      gateway handlers (web bundle line 382133): create, update and delete
      carry the whole event, and the two user events carry `user_id`,
      `guild_id`, `guild_scheduled_event_id` and `response`. The first three
      parse to `DispatchEvent::ScheduledEventUpsert` and `ScheduledEventDelete`
      and the store replaces, adds or drops the event in its guild, marking
      the channel list so the "N Events" row follows. The user events stay
      `Unknown` and `session.rs` moves the interested total once we know it,
      and our own pressed button when the user is us, without counting the echo
      of our own click twice. Reducer test for create, update, delete and the
      quiet misses, and unit tests for the count movement. Unverified live: no
      event changed in a server while the app ran, so only the tests exercise
      it. Not handled: `GUILD_SCHEDULED_EVENT_EXCEPTION_*` (recurring event
      occurrences).

## Phase 6: account (UI and request building only, see the rule at the top)

- [x] Log out other devices, remove a connection
- [x] Add a connection. The Connections settings page is now the live
      "Connected Apps" page (stable 670026, measured through the logged in web
      client at 1920 x 1080): a 24 / 32 weight 500 title, the 14 / 18
      description, "Add a new connection" at 16 / 20 weight 500, and 44 by 44
      tiles (radius 8, rgba(151,151,159,0.12)) with a 28 px logo, 8 apart across
      and 53 down, in the live order of 22 services (Riot Games to Xbox, then
      Amazon Music to YouTube). Linked accounts list under the grid and an
      account with none shows "No Connections" with its 20 / 24 and 16 / 20
      lines. A tile sends `GET /connections/:platform/authorize` (module 77468)
      and opens the returned URL in the browser, where Discord's own callback
      page finishes the link; `USER_CONNECTIONS_UPDATE` reloads the list. The
      answer shape is unit tested; no tile was clicked from the real account
      (account writes rule). Captures: screenshots/reference/connected_apps.png
      and screenshots/port/connections_page.png. Remaining differences: the
      Nitro Rewards promo banner is not drawn, the empty state sits closer to
      the grid than the live one, and a linked account row is still the old
      plain row because this account has none to compare.
- [x] Change password dialog, as the live "Update your password" modal draws
      it (measured through the logged in web client at 1920 x 1080, stable
      670026): a 480 wide frame (radius 12, 1 px rgba(151,151,159,0.2) border,
      8 top and 16 bottom padding), a 16 / 24 / 8 header with the 20 / 24
      weight 600 title and the 16 / 20 muted line, three 40 high fields 20
      apart with 16 / 20 weight 500 labels, a red asterisk and an eye toggle,
      a 40 by 40 close at 8 from the top right, and Cancel and Done at 211 by
      40, 8 apart and 24 under the last field. An Edit button on the My Account
      page opens it. Done checks all three fields are filled and the new ones
      match before anything is sent, then sends `PATCH /users/@me` with
      `{password, new_password}` (module 631670). The answer carries the token
      that replaces the old one, so a success signs in again with it under the
      saved identity and closes the dialog; a server error is shown on the
      dialog. Request body and answer unit tested, never sent from the real
      account (account writes rule). Verified in the port: the frame, title,
      labels, fields and buttons sit within a pixel of the live offsets, and
      Done on empty fields shows "All three passwords are required." without
      sending. Captures: screenshots/reference/change_password_dialog.png and
      screenshots/port/change_password_dialog.png, change_password_error.png.
      Remaining differences: the focused field keeps gpui's border instead of
      the brand coloured one, field errors are one line under the fields rather
      than under each field, the settings page has no live "Password &
      Security" section (an Edit row sits in the Account group), and the
      re-login after a success was not exercised.
- [x] Enable 2FA: QR code and backup codes screens. The live "Enable
      Authenticator App" modal (Multi-Factor Authentication page, stable
      670026, measured at 1920 x 1080 without pressing Activate) shows all
      three steps at once, and the port draws the same: a 480 wide frame with
      the password dialog's header, three rows 20 apart divided by 1 px
      lines, a 156 wide picture column and a 254 wide text column 20 beside it.
      Step 1 is the 100 px authenticator picture with "Download an
      authenticator app" 14 / 18 weight 600 and the Authy and Google
      Authenticator links, step 2 a 128 square code on a 144 white tile with
      the 2FA key in groups of four (Consolas 16 / 20), step 3 the 100 by 85
      phone picture, "Log in with your code", a 146 wide "000 000" field and a
      100 by 40 Activate. The key and address follow the web client's module
      464477: 20 random bytes as base32, lowercased in groups of four, and
      `otpauth://totp/Discord:<email>?secret=<KEY>&issuer=Discord`; the code is
      painted from the `qrcode` crate on a canvas. Activate checks for six
      digits, then sends `POST /users/@me/mfa/totp/enable` with
      `{secret, code}` and shows the backup codes the answer carries (two
      columns of 209 by 40 tiles, a used code struck through, Copy Codes and
      Done); the answer's new token signs in again like a password change and
      leaving the codes reloads the account. The two pictures are in
      discord-assets/mfa. Opened from an "Enable Authenticator App" button in
      the My Account page's Account group while two-factor is off. Verified
      in the port: the frame is 593 high against the live 592 and the rows,
      code, key and Activate line up within a pixel; Activate on an empty
      field shows the six-digit message without sending. Captures:
      screenshots/reference/mfa_enable_step1.png and
      screenshots/port/totp_setup.png, totp_setup_error.png and
      totp_backup_codes.png (dev command `totp-codes` shows the codes screen
      with made-up codes). Unit tested: key length and alphabet, grouping, the
      address, the code matrix, the request body and the answer. Unverified:
      the body's exact fields (the lazy chunk that sends it was not captured,
      so a password may also be required), the backup codes screen's wording
      and layout (unreachable without turning the authenticator on, built
      from the web client's), and the re-login after a success. Never sent
      from the real account (account writes rule). Remaining differences:
      the step 1 link text wraps one word earlier than live, the focused code
      field keeps gpui's border, and the Security Keys section and the
      breadcrumbed Multi-Factor page are not built.
- [x] Disable 2FA dialog, the third kind of the shared `DangerDialog`
      ("Remove Authenticator App"). Unverified against the live client: the
      account has no authenticator, so the live dialog cannot be opened, and
      the title, the label "Enter Discord Auth/Backup Code" and the button
      "Remove" are from the web client's wording as remembered. It uses the
      same 480 wide frame, header, field offsets and 211 by 40 red button as
      the other two with no amber box, and a plain field with a "000 000" hint
      because a code is read as typed. The My Account page shows a "Remove
      Authenticator App" button in place of "Enable Authenticator App" when
      two-factor is on. Remove checks a code was typed, strips its spaces and
      sends `POST /users/@me/mfa/totp/disable` with `{code}`; the answer's new
      token signs in again like enabling it and the account reloads. Unit
      tested: the body, the kind's request and its spaces. Never sent from the
      real account (account writes rule). Verified in the port with the dev
      command `danger totp` (also `danger disable` and `danger delete`):
      Remove on an empty field shows "Enter a code from your authenticator app
      or a backup code." without sending. Captures:
      screenshots/port/remove_totp_dialog.png and remove_totp_error.png.
      Remaining differences: the exact endpoint body and the live dialog's
      layout are unconfirmed, and the focused field keeps gpui's border.
- [x] Disable account dialog, as the live "Disable account" modal draws it
      (opened from the Account page's Disable Account button and measured
      without submitting, stable 670026, 1920 x 1080): a 480 by 350 frame
      (radius 12, 8 top and 16 bottom padding) with the 20 / 24 weight 600
      title, an amber warning box 24 under it (430 wide, 10 padding, radius 8,
      rgba(250,166,26,0.08), text #ce8000 at 16 / 20), a required Password
      field 20 under that with the eye toggle, and Cancel beside a 211 by 40
      red button (#d22d39, 1 px rgba(255,255,255,0.08) border) 44 under the
      field. One shared `DangerDialog` in `danger_dialog.rs` draws it and will
      take the delete and two-factor dialogs. The My Account page gains an
      "Account status" group with a "Disable your account" row and its button.
      Confirm checks a password was typed, then sends `POST
      /users/@me/disable` with `{password}` (module 631670); on success the
      saved login is cleared and the app returns to sign-in, a failure shows on
      the dialog. Request body unit tested; never sent from the real account
      (account writes rule). Verified in the port: the frame is 350 high from
      365 to 715 like the live one, the title, box, field, buttons and close
      sit at the live offsets, and Confirm on an empty field shows "Enter your
      password to go on." without sending. Captures:
      screenshots/reference/disable_account_dialog.png and
      screenshots/port/disable_account_dialog.png, disable_account_error.png.
      Remaining differences: the focused field keeps gpui's border, the
      warning box wraps its text one word earlier than live, and the live page
      lists the disable and delete rows at the very bottom with no heading.
- [x] Delete account dialog, the second kind of the shared `DangerDialog`.
      The live "Delete account" modal (stable 670026, measured without
      submitting) has the same 480 by 350 frame and offsets as Disable account
      with the title "Delete account", the warning "Are you sure that you want
      to delete your account? This will immediately log you out of your
      account and you will not be able to log in again." and a red "Delete
      Account" button. The My Account page's "Account status" group gains the
      live "Close your account / Permanently close your account." row with the
      filled red `Tone::DangerFilled` button (#d22d39). Confirm checks a
      password was typed, then sends `POST /users/@me/delete` with
      `{password}` (module 631670); success forgets the saved login and returns
      to sign-in through the same `end_session` path as disable, a failure
      shows on the dialog. Request kind and wording unit tested, body shared
      with disable and tested; never sent from the real account (account
      writes rule). Verified in the port: frame 365 to 715 like the live one,
      and Confirm on an empty field shows "Enter your password to go on."
      without sending. Captures: screenshots/reference/delete_account_dialog.png
      and screenshots/port/delete_account_dialog.png, delete_account_error.png.
      Remaining differences: the focused field keeps gpui's border, and the
      warning box wraps its text one word earlier than live.

## Phase 7: server admin (test server only)

- [x] Channel permission overwrites: list roles and members. The live
      channel settings is a full screen with its own sidebar (Overview,
      Permissions, Invites, Integrations, Delete Channel). It was read, view
      only, on a channel of the one server where this account has Manage
      Channels (found with a temporary dev command, since removed), at 1920 x
      1080, stable 670026. The new `ChannelSettings` screen draws its
      Permissions tab: a 264 wide navigation column (60 top padding, 32 high
      items, the selected one rgba(151,151,159,0.24) at radius 4, a divider and
      a red Delete Channel with a trash glyph), a 660 wide content column 48
      beside it and the close circle with "ESC" 40 past that. Content: the 20 /
      24 weight 600 title and 14 / 18 line, the sync banner (66 high card, a
      warning glyph, "Permissions not synced with category: <name>" and a Sync
      Now button, or the synced wording), the 82 high Private Channel card
      with its 48 by 24 switch (on when @everyone is denied View Channel), a
      divider 40 below and the "Advanced permissions" 20 / 24 expander. Open,
      it lists Roles/Members on the left (209 wide, 32 high rows, a colour dot
      for coloured roles, roles in server order then members by name, @everyone
      last and selected by default, a divider and the "Need help with
      permissions?" link) and on the right the groups with each row's name,
      description and the 96 by 28 segmented deny, inherit and allow control
      showing the selected overwrite's state. The group names, permission names
      and descriptions are scraped from the live page for a text channel and a
      voice channel (`channel_permissions.rs`), so voice channels list Voice
      Channel Permissions, Voice Channel Chat Permissions and Events
      Permissions. Nine permission flags the live list uses (pin messages,
      bypass slowmode, send voice messages, create polls, use external apps,
      soundboard, external sounds, create events, voice channel status) were
      added to `Permissions`. Reached from a Permissions button in the older
      Edit Channel dialog (and `chperm <channel>` for checks), whose Overview
      item here reopens that dialog. Verified at 1920 x 1080 on the same
      channel: the banner, cards, expander, list, group titles and the View
      Channel row sit at the live offsets plus a constant 8 px from the port's
      window frame, selecting Quarantine shows its deny on View Channel and
      Create Invite, and the voice channel scrolls through its own groups.
      Unit tested: the tri-state read, the private test, category sync in any
      order, and that every listed permission is one known flag and none
      repeats. Captures: screenshots/reference/channel_permissions.png and
      channel_permissions_advanced.png, screenshots/port/channel_permissions.png,
      channel_permissions_role.png and channel_permissions_voice.png. Remaining
      differences: the controls are display only (editing is the next item),
      Sync Now, the add button and the other tabs do nothing, members show
      without an avatar, the live page greys Manage Permissions for this account
      where the port enables it (its rule for what an account may grant was not
      worked out), and the add glyph is filled rather than outlined.
- [x] Channel permission overwrites: edit allow, deny, inherit. Each segment
      of the deny, inherit and allow control now sets that permission for the
      selected role or member: the flag leaves both sets and joins the one that
      matches, an overwrite left saying nothing is removed, and a click shows
      at once while the gateway's channel update confirms it (the on-screen edit
      is dropped once the store agrees). The Private Channel switch sets View
      Channel (plus Connect on a voice channel) for `@everyone` between deny and
      allow, and Sync Now gives the channel its category's overwrites. Rows the
      account cannot grant ignore clicks. The calls are `PUT` and `DELETE` on
      `/channels/{id}/permissions/{target}` with the allow and deny sets as
      strings, and Sync Now is a channel update carrying `permission_overwrites`.
      Built and unit tested, never sent: the screen was checked only in a dry
      run (`chperm <channel>` is dry unless given `live`), where edits stay
      local, so nothing was changed on the real server. Unit tested: the set
      move for each state, editing several flags over an existing overwrite,
      keeping a row's place and dropping an empty one, the string JSON, and the
      request body. Captures: screenshots/port/channel_permissions_edit.png and
      channel_permissions_synced.png. Remaining differences: the add button for
      roles and members does nothing, the request bodies and the private switch's
      off state (allow versus inherit) were not seen on the wire, a failed
      request does not roll the on-screen edit back until the screen reopens, and
      the other tabs still do nothing.
- [x] Webhooks: list, create, edit, delete. Integrations now opens on the
      live client's two cards (Webhooks with its count, Channels Followed with
      its count) and the Webhooks card opens a page with the intro, New Webhook,
      the webhooks grouped under "Posting to #channel", each row with avatar,
      name and "Created on <date> by <tag>", and a row editor with Name, a
      Channel menu, Save Changes (only when something changed), Copy Webhook URL
      and a two click Delete Webhook. The calls are `GET /guilds/{id}/webhooks`,
      `POST /channels/{id}/webhooks` with the name, `PATCH /webhooks/{id}` with
      name and channel, and `DELETE /webhooks/{id}`, taken from the web client's
      module 824953. New Webhook uses the first text channel and a random name
      as the web client does. Measured on the live page (view only):
      screenshots/reference/integrations.png, webhooks.png and webhook_edit.png.
      Read-only check in the port against the same server: the list, the
      editor and the channel menu show the same webhooks, dates and creators
      (captures in screenshots/port/integrations.png, webhooks.png,
      webhook_edit.png and webhook_channel_menu.png). Create, edit and delete
      were built and unit tested only, never sent: the model read, the link
      build, the grouping, the changed test and the edit body. Remaining
      differences: the avatar upload and Remove controls are not built, the
      Channels Followed card has no Learn More button or page, Copy Webhook URL
      does nothing for application webhooks (they list without a token), the
      Integrations item is missing from the port's left list for this account
      (the port's rule for which pages an account sees differs from the web
      client's), and the list does not show the new webhook expanded after New
      Webhook.
- [x] AutoMod rules: list. The AutoMod page loads `GET
      /guilds/{id}/auto-moderation/rules` (route names from the web client's
      module 652215 and its REST helpers in module 179832) and lists each rule
      as a card with its name, what it watches for (custom words, spam,
      commonly flagged words, mention spam, member profile), its actions,
      its exemption count and an On or Off mark. An empty list, a loading line
      and a failure line are handled. `automod <server id>` in the dev control
      opens it. Unit tested: the rule parse, the trigger and action words and
      the off-by-default flag. NOT measured and NOT seen on screen: the test
      account has no AutoMod page on the live client (it lacks Manage Server),
      the AutoMod screen code is not in the extracted reference, and the
      running app could not be rebuilt without closing it. So the card layout,
      the wording and the blurb are a best guess and still need a pass against
      an account that can open the page. Also missing: the per trigger setup
      cards the live page shows above the list, and the toggle to switch a
      rule on or off (that belongs to the next item).
- [x] AutoMod rules: create and edit. A rule's card opens an editor with its
      name, its words (custom word rules only), Save Changes (shown only when
      something changed), Turn On or Turn Off and a two click Delete Rule. A
      form above the list makes a custom word rule from a name and comma
      separated words: it watches messages, blocks the ones holding a word and
      starts on. The requests are `POST`, `PATCH` and `DELETE` on
      `/guilds/{id}/auto-moderation/rules[/{rule}]`, and each re-fetches the
      list. Unit tested: the create body, the edit body (only name and words
      travel, so actions and exemptions stay), the change check and the word
      splitting. Never sent from the real account and NOT seen on screen (same
      reasons as the list). Missing against the live page: spam, mention spam,
      commonly flagged and member profile rules cannot be created, the actions
      (alert channel, timeout length) and exemptions cannot be edited, the
      block message custom text is absent, and `rules/validate` is not called.
- [x] Integrations list. The Integrations page loads `GET
      /guilds/{id}/integrations` (route from the web client's module 652215)
      beside the webhooks and lists each Twitch, YouTube or app integration as
      a card below Webhooks and Channels Followed: name, then its kind, linked
      account, subscriber count, a Disabled mark and who added it. Remove is
      two clicks and sends `DELETE /guilds/{id}/integrations/{id}`, then
      re-fetches the list. Nothing shows while the list loads, fails or is
      empty, which is what the live client shows for the test account (its
      Integrations page has only the two cards, so the list could not be
      measured). Unit tested: the model read, the kind names and the detail
      line. Never sent from the real account and NOT seen on screen. Missing:
      the card layout is a best guess, Twitch and YouTube sync, enable and
      subscriber role settings are not built, there is no per integration
      page, and the Channels Followed card still has no Learn More button.
- [x] Welcome screen setup. A Welcome Screen page sits after Enable Community
      for accounts with Manage Server. It loads `GET /guilds/{id}/welcome-screen`
      and edits a draft: an enable switch, the server description, and up to
      five featured text or announcement channels, each with its own
      description, added from a channel menu and dropped with Remove. Save
      Changes (shown only when the draft differs) sends `PATCH
      /guilds/{id}/welcome-screen` with `enabled`, `description` and
      `welcome_channels`, the body from the web client's module 225315, and
      shows what the server answers. Unit tested: the body (trimmed text, blank
      description as null, emoji fields) and the add rules (no duplicates, five
      at most). Never sent from the real account. Seen on a non-Community server: the live GET answers 404 and the page shows "That no longer exists.", which misleads (a Community prompt would be better); the loaded layout is unseen, and the
      layout and wording are a best guess and the live page was not measured.
      Missing: channel emoji cannot be picked (a loaded one is kept), the page
      is listed even for servers without Community, and there is no preview.
- [x] Onboarding setup (Onboarding page under MANAGE_GUILD: enable switch,
      default channels with an add menu, questions with title, single answer,
      required and "ask when someone joins" switches, and answers with title
      and description. Save sends `PUT /guilds/:id/onboarding` with the body
      shape of the web client's serializer, only when the draft differs from
      the loaded setup, and the answer replaces the loaded state. A question
      or answer with no title is left out of the body. Unit tested: body
      shape, blank rows, question cap, default channel toggle.
      Missing: the channels and roles an answer grants cannot be picked yet
      (loaded ones are kept and shown as a count), answer emoji cannot be
      picked, no dropdown question type or Advanced mode switch, no server
      requirement check (7 default channels) before enabling, and the page
      route was read from the web client but the settings chunk that sends it
      is not in the reference. Layout is a best guess. Never sent and not
      seen as the empty, disabled state on a real server (loads, switch off, Add Channel and Add Question).)
- [x] Server templates (Server Template page: loads `GET /guilds/:id/templates`;
      with none, a name (prefilled with the server name) and description and a
      Generate a Template Link button (`POST`); with one, the discord.new link,
      use count, last synced day, a notice when the server is dirty, Save
      Changes (`PATCH`, only when the text changed), Copy Link, Sync Template
      (`PUT`) and a two click Delete Template (`DELETE`). Every write reloads
      the list. Routes and the `{name, description}` body come from the web
      client's template module; the name and description caps (100 and 120)
      are from the public API docs, not the reference. Unit tested: parsing,
      body trimming and caps, the changed check, per server list handling.
      Missing: the settings chunk that draws the page is not in the reference,
      so layout and wording are a best guess, and the web client's
      "Template has been synced" toasts and the dirty dot on the settings
      button are not there. Never sent. Seen on screen (dev `settings` command): only the 403 state, the one server this account manages lacks Manage Server, so the loaded layout is still unseen.)
- [x] Prune members (a Prune group on the Members page: 1, 7 or 30 days, Count
      Members sends `GET /guilds/:id/prune?days=N`, then the page says how many
      would go and offers a two click Prune Members that sends `POST
      /guilds/:id/prune` with `{days, compute_prune_count: true}` and reports
      how many went. Changing the days or reopening the page clears the
      result. Unit tested: the body and its day clamp, the answer with or
      without a count, the plural label, per server event handling. Seen on
      screen: the group, the day picker and the live count request, which this
      account gets a 403 for (no Kick Members), shown as an error. Never sent
      as a write and the count and confirm states are unseen. Missing: the
      include-roles option, the async v2 route and its `GUILD_PRUNE_UPDATE`
      progress event, and the settings chunk that draws the real dialog is not
      in the reference, so layout and wording are a best guess.)

## Voice

- [x] Join, leave and hear voice channels and private calls (voice v8, Opus,
      AES-GCM or XChaCha20, DAVE)
- [x] Speak: mic capture, voice gate, mute and deafen
- [x] Live call check
- [x] Call UI matched to the web client
- [x] Floating user card across the rail and list
- [x] Input volume slider (a new Voice & Video page in app settings, after
      Appearance, with an Input Volume slider from 0 to 100 and the percent
      beside it, default 100 like the web client. Dragging changes the live
      call at once, letting go saves it to the local settings file, and the
      saved value is sent to every call on join. The microphone frames are
      scaled after the voice gate, so the gate still reacts to the raw mic.
      Like the web client's slider it only turns the mic down. Unit tested:
      the frame scaling and its cap, the default for an old settings file.
      Seen on screen: the page, dragging, the percent label and the value
      landing in the settings file. Not checked in a live call, so the effect
      on what others hear is unverified. Missing: the rest of the web page
      (input and output device pickers, output volume, mic test, camera),
      which come with the later voice items, and the slider's own wording and
      spacing are a best guess because the settings chunk is not in the
      reference.)
- [x] Per-user volume (local gain) from the user's right click menu (right
      clicking a person who is in our call, on their call tile or anywhere a
      person menu opens, shows a User Volume slider from 0 to 200 percent with
      the value beside it, between Message and the friend actions. Dragging
      changes how loud they are played at once, by scaling their decoded audio
      with clipping, and letting go saves it per user in the local settings
      file. Saved volumes are sent again on every join and 100 is not stored.
      Unit tests cover the gain, the settings round trip and when the menu
      offers the slider. Seen on screen through a dev command that opens the
      menu as if they were in the call: the slider, the percent label, 46
      landing in the settings file. Not checked in a live call, so whether the
      sound really changes is unverified. Missing: the web client's Mute
      checkbox for a person, and the slider is not offered from the voice list
      in the sidebar. The 0 to 200 range and the label are from memory of the
      web client, not the reference.)
- [x] Push to talk with a configurable key (the Voice & Video page has an
      Input Mode group: a Push to talk switch and a Shortcut button that waits
      for the next key, Escape cancels, and the key is kept in the local
      settings file. With it on, the microphone is sent only while the key is
      held and the voice activity gate is skipped; the key is polled every 8
      ms with GetAsyncKeyState so it works while another program has the
      focus, on Windows only. Turning it on with no usable key changes
      nothing. The key and switch are sent again on every join. Unit tests
      cover the send decision, the key names and the settings rule. Seen on
      screen: the page, the Press a key state, F8 landing in the settings
      file. Not tried in a live call and the held key itself was not
      exercised, so whether the mic really opens on the key is unverified.
      Missing: modifier keys and mouse buttons as the key, key combinations,
      the release delay and the sound the official client plays, other
      platforms. Only letters, digits, function keys, space, arrows and a few
      others can be picked.)
- [x] Resume a call after the voice websocket drops (voice resume opcode)
      (when the voice websocket breaks without a goodbye, times out on
      heartbeats, or closes with 1001, 1006 or 4015, the call is no longer
      ended. The UDP socket, keys, playback and microphone stay up, the socket
      is reopened and op 7 resume is sent with the server id, session id,
      token and last sequence, instead of identify. Up to five attempts, 0.5 s
      doubling to 8 s, and the counter resets when the server answers Resumed.
      Settings changes and leaving still work during the wait. Kicked (4014),
      invalid session (4006), timeout (4009) and bad token still end the call
      with the same messages. Unit tests cover which close codes resume and
      the resume payload. Not tried against a real dropped connection, so the
      server accepting the resume is unverified. Missing: a visible
      reconnecting state in the call panel, and a fresh identify when the
      session has expired instead of ending the call.)
- [x] Pop out the call into its own window (the pop out button on the call
      bar opens a second window with the tiles, mute, hang up and a return
      button, and the main window shows "Your call is in another window" with
      Bring it back. Closing the pop out, or leaving the call, puts it back;
      closing the main window quits the app. Checked live against a real
      call. Missing: the pop out has no device menus, camera or share
      buttons, and no right click menu on a tile, and it is a native
      titlebar window rather than the official always-on-top mini window.)
- [x] Noise suppression toggle (Krisp is proprietary: use RNNoise and note
      the difference)
      (a Noise Suppression switch on the Voice & Video page, saved with the
      settings and off by default. It runs the microphone through RNNoise
      via the pure Rust nnnoiseless crate, two 10 ms halves per 20 ms frame,
      before the voice gate, input volume and Opus, and applies to the
      running call at once. The page says it is RNNoise, not Krisp, which
      cleans differently. A unit test checks steady noise is quieted and the
      frame length kept. The page and the saved setting were checked live and
      a call was joined with it on, but muted, so the denoised microphone
      audio itself was not heard. Missing: the official Krisp and Standard
      choice, and the official default of on.)

## Phase 8: parity audit against live build 670026 (2026-11-11)

Found by opening the live web client (build 670026, newer than the 668923
snapshot in reference/web) and our debug app side by side, both at 1920 x 1080
(web viewport set to 1920 x 1080, our window sized to match), on the same
server (Low iq gaymers) and the same account. Each line is a gap seen in those
two captures or on the live settings pages. Work top to bottom; each line is
one feature and one commit. Re-capture the live page before starting an item,
the build moves.

### Messages and chat

- [x] Server nickname and role colour on author names. The store keeps a
      nickname and role list per server and person (`Store::member_info`),
      learned from messages that carry a member, `GUILD_MEMBERS_CHUNK`
      (now parsing `nick` and `roles`), `GUILD_MEMBER_UPDATE` for anyone, and
      the member list as a fallback. `Store::member_look` gives the name
      (nickname, else global name) and the top role colour. The row, the
      reply line, mentions and the typing line use it; an author we have no
      info for is asked for once through the existing member request. Tested
      in the store (chunk, update and message paths). Checked at 1920 x 1080
      on Low iq gaymers #yap-yap: "Marshmallow" blue, "Gabriel" and "Cotton"
      red, "Mervo" magenta in the reply line, matching the live client.
      Remaining differences: gradient and holographic role styles draw as the
      flat role colour; the member list still reads its own entries; reply
      line text, server tag chips and timestamps are their own items below.
- [x] Reply line as the live client draws it. The quoted line now sits above
      the name (the avatar column drops by 22 px), 18 px high, with the 2 px
      connector (`#383940`, 6 px radius, from the avatar centre line to the
      line), a 16 px avatar of the replied person, their server nickname in the
      top role colour at 0.64 opacity, an APP badge for bots, and the text as
      `markdown::preview`: first line only, mentions, timestamps and formatting
      resolved through the same path as messages, custom emoji as 18 px
      pictures, truncated. Variants: "Original message was deleted" and
      "Blocked message" (blocked author, reply badge instead of the avatar), both
      italic, and "Click to see attachment" with an image icon or "Click to see
      command" when the original has no text. Checked at 1920 x 1080 on Low iq
      gaymers #yap-yap against the live DOM (sizes, colours, opacity).
      Remaining differences: the server tag chip after the name (next item);
      gradient role names draw flat; the connector does not brighten when the
      line is hovered over the text, only over itself; rows with a reply may be
      a few pixels off the live spacing; "Message could not be loaded" (original
      not fetched) is shown as deleted; the deleted variant was not on screen in
      the live channel so it is unverified against it.
- [x] Server tag chip after names in messages, reply lines, the member list and
      the DM list. `User` now reads `primary_guild` (so message authors, DM
      recipients and member list users carry it; a tag switched off by
      `identity_enabled: false`, or odd JSON, shows nothing), and one helper,
      `member_list::server_tag_chip`, draws the chip: 16 px high, 4 px side
      padding, 4 px radius, `rgba(151,151,159,0.24)` fill, 12 px semibold
      `#d4d5d8` letters, the clan badge at 14 px (12 px in a reply line, where
      the chip is at 0.64 opacity like the name) with 2 px before the letters.
      Measured against the live DOM at 1920 x 1080 (message header, reply
      line, member list, DM list) and compared by screenshot on Low iq
      gaymers #yap-yap and Home. The live DM header has no chip, so ours has
      none either. Unit test for the model side.
      Remaining differences: the chip is not clickable (live opens the tag
      popout); the Friends page rows and the Active Now list also carry it and
      are covered by the Friends page item; the muted variant on dimmed DM
      rows is the row's own fade only; the tag is not shown in the profile
      column header, it is the existing pill.
- [x] Composer placeholder "Message #channel-name" (and "Message @user" in a
      DM). Ours says only "Message". Also the Apps button (sparkle grid) at the
      right end of the composer icons, which opens the app launcher.
      Done: the placeholder follows the open channel (checked against live in
      #yap-yap and the Gabriel DM; a group DM uses its name, a thread uses
      "#name", both unverified live since the account has neither open). The
      Apps button replaces our poll button, with the live glyph. The plus
      button now opens a menu like live (Upload a File, Create Poll, Use Apps)
      instead of going straight to the file picker. Remaining differences: the
      Apps button only starts a slash command in the box, it does not open the
      launcher popout (search, Recents, "Apps in this Server", Promoted);
      the plus menu lacks Create Thread and Schedule Message and uses our
      shared popup, which puts glyphs on the right and is 220px wide where
      live is content sized with glyphs on the left; the restyle is the last Phase 8 item.
- [x] Message timestamps in the live format: time only ("5:33 PM") on today's
      rows with the full date in the tooltip, then "Yesterday at" and the date
      for older rows, following the 12 hour or 24 hour setting. Ours writes
      "Today at 17:44" on every row. Confirm the live rule before copying it.
      Done: live checked in #yap-yap (today rows read "5:33 PM", tooltip
      "Wednesday, November 11, 2026 at 5:33 PM") and the Gabriel DM (older rows
      read "8/19/26, 9:17 PM"). Ours now matches: time only today, short US
      date plus time for older days, hover tooltip with the full date. Remaining:
      the "Yesterday at" form is unverified (no yesterday row on the account;
      the live bundle has no such string, so it may be the short date too),
      and the date shape is fixed to the US short form while live follows the
      browser locale. The 12 hour or 24 hour clock follows the account setting,
      so our window shows 24 hour where the live browser shows 12 hour.
- [x] Blocked and ignored messages collapse into "N blocked messages - Show"
      rows (the live client groups consecutive ones). Verify what ours does.
      Done: live checked in the DM with a blocked account (two bars, one per
      day, each "1 blocked message" with a slash circle in the 72px gutter, 16px
      text, "Show" in the muted color, 32px from the day line to the text; once
      opened the action reads "Hide" and the bar and message sit on one 12%
      panel). Ours had nothing: blocked authors were drawn as normal rows. Now
      consecutive blocked messages fold into one `Row::Blocked` bar (split by a
      day divider, the NEW line or a visible message), clicking the action opens
      or closes the run, and the opened messages share the panel. The open state
      was checked against our window once; the closed state matches live.
      Remaining: ignored users are not modelled in the store, so only blocked
      authors fold; the action has no hover style (not captured live); the
      panel does not cover the gap between messages exactly; the day divider
      above paints 24px low in our list, so the bar adds 16px of top padding to
      land on live's spacing, and the gap below a bar is about 6px more than
      live; live shows a blocked-DM composer ("You cannot send messages to a user
      you have blocked." and Unblock) and an invite embed under the revealed
      message, neither of which ours does.
- [x] Search box in the channel header: placeholder "Search <server name>",
      wider than ours, with the filter chips dropdown on focus. Ours is a small
      "Search" field.
      Done: live checked in #yap-yap (244x32 box at rest in a server, 8px
      radius, 24% muted border, placeholder "Search Low iq gaymers", search glyph
      on the right; the box does not widen on focus in this build). Focus opens
      a 356px list under the header, 4px in from the right edge, with a "Filters"
      heading and five 44px rows with a 20px glyph, a title and a "key: hint"
      line (from:, in:, has:, mentions:, and a "More filters" row). Ours was a
      144px button in servers that opened a panel with a second input. Now the
      box is a real input in the header (244px everywhere, placeholder names the
      server or the DM person), the filter list shows while it is focused and
      empty, picking a row types its key, Enter runs the search and opens the
      results panel, Escape closes the list, and Ctrl+F focuses the box. The
      side panel lost its own input. Remaining: the list closes as soon as text
      is typed (live then shows matching users, channels and history), "More
      filters" is inert (live opens a date and author type picker), the
      placeholder updates on channel change only, so a server rename shows
      later, and the results panel is still our plain list, not the live
      "N Results" panel.
- [x] Header icon set: threads, mute toggle drawn with a slash when muted,
      pinned messages with the new dot, members. Compare glyphs and tooltips
      with the live header.
      Done: live checked in #yap-yap and in a DM at 1920x1080. The toolbar is
      bare 32px boxes, 8px apart, with 24px glyphs (20px for pins), no hover
      square: the glyph goes from muted to light on hover and to the strong
      tone while its panel shows (CSS: icon-muted, icon-subtle, icon-strong).
      Server order is threads, notification settings, pins, members. A DM is
      voice call, video call, pins, add to DM, user profile toggle, so the two
      extra profile buttons we had are gone. New glyph files copied from the
      live svgs: bell-off (the slash bell), phone, people-add, user-profile;
      threads, pins and members were already the live paths. The bell is drawn
      with the slash whenever the effective level is not All Messages, muted
      or mentions-only alike (live shows it in the mentions-only #yap-yap and
      a plain bell in an All Messages server, both matched). Pins draws the
      8px blurple dot at the bottom right in a ring of the page colour when
      something was pinned after we last looked: the channel's
      last_pin_timestamp is compared with the read state's, CHANNEL_PINS_UPDATE
      and CHANNEL_PINS_ACK keep both current, and opening the pins panel acks
      them locally and posts /channels/:id/pins/ack. Tooltips use the existing
      text tip with the live aria labels (Show/Hide Member List, Show/Hide User
      Profile, Start Voice Call and the rest). Store test covers the dot rule.
      Remaining differences: the dot was seen drawn here only with a forced
      flag, because the live session had already acked this channel's pins
      (the live pin showed no dot afterwards, ours agrees); the ack request is
      built but was never sent from the real account. The notch around the dot
      is a filled ring, not live's SVG mask, so it differs by a pixel on
      non-black backgrounds. Tooltip chrome is the old Tip style until the
      shared popup item. The inbox and help icons still sit in our header; live
      puts them in the title bar, which belongs to the Home items.

### Server list and channel list

- [x] Server banner and boost goal in the channel list header: the banner image
      fills the header, the name sits over it with a chevron, and the "Boost
      Goal" bar (progress and "36 Boosts") sits below it. Ours is a plain text
      header with no banner.
      Done: Guild now carries banner, premium_tier,
      premium_subscription_count and premium_progress_bar_enabled (flat or
      nested layout, kept current by GUILD_UPDATE). The channel list starts
      with the 130px banner behind the 49px header, as live measures it at
      1920x1080 (build 670026): white 16px semibold name, the level badge
      (scalloped flower with the pink gem) before it when the server is
      boosted, the chevron right after the name, the invite button at the
      right edge, a dark fade at the top for legibility. The banner scrolls
      away with the list and the header turns solid once it has slid out from
      under it. The Boost Goal pill sits 15px under the banner (22px gradient
      pill, "Boost Goal" with the party popper left, "62 Boosts" and a
      chevron right) and only draws when the server turned the bar on. With
      a banner the channels start 3px under it, which matched live in true
      larpers (no goal) and Low iq gaymers (goal). Unit tests cover the
      model reading and the bar's fill and label.
      Remaining differences: the banner is the still frame of the animated
      picture, so it shows a different frame than the playing live one. The
      header has no drop shadow behind the name and no blurred glass layers.
      The level tooltip ("Level 3", "62 Boosts") is not drawn. The pill has no
      click action yet, because the Server Boost page belongs to the header
      menu item. The lower tiers (label "n/goal Boosts" with goals 2, 7, 14,
      bar filled by that share) and a boosted server without a banner were
      not checked live, since neither test server is in that state. The
      "Events" row under the pill is the next item.
- [x] Events row under the header ("Events", calendar glyph), opening the
      existing Events dialog. Done, checked at 1920x1080 (build 670026) in
      Low iq gaymers: the row (calendar 20px, medium 16px label in the muted
      channel color, 32px tall) and the 1px line under it (96979e at 20%, 12px
      under the row, 16px in from the left, 7px from the right) now land on
      the same pixel rows as live, and so do Static and rules below it. The
      live component reads "N Events" while some are running or coming and
      plain "Events" when none are; ours does the same. Live shows the row on
      Low iq gaymers with no upcoming events, and not on true larpers. Neither
      server has events, so the show rule is inferred from the account: the
      row draws for anyone who may create or manage events, or when something
      is upcoming (a unit test covers the rule and the label). I could not
      confirm the rule in the live bundle, so a server where the account can
      create events but live hides the row would still differ. Also fixed the
      category header sitting 2px above its first channel in a group. Live
      stacks them with no gap.
      Remaining differences: no unread dot or mention count on the row, and no
      mute-events state (the live row has both). No right click menu on the
      row. The line is drawn only under the Events row; whether live also
      draws it under the Boost Goal pill alone is not checked.
- [x] Server header dropdown menu, as the live client draws it (grouped, with
      icons): Server Boost, Server Tag, Invite to Server, Server Settings,
      Create Event, App Directory, Notification Settings, Privacy Settings,
      Edit Per-server Profile, Hide Muted Channels (checkbox), Leave Server
      (red), Copy Server ID. Done, checked at 1920x1080 (build 670026) in Low
      iq gaymers and true larpers. The header now opens its own menu
      (`Target::GuildHeader`, rows and groups in `header_menu.rs`, drawing in
      `context_menu.rs`); the older list stays for right click on a server
      icon. Geometry matches live: 220px card at 114,85 (centred under the
      header, 4px below it) with a 1px 2c2c30 frame, 8px radius, 8px padding,
      36px rows (34 for the tag row), 20px glyph at 8px, label at 36px in 14px
      medium, muted glyph that turns bright on hover, 222225 hover (211516 on
      Leave), 1px group lines with 8px above and below. A pixel scan of the
      Low iq gaymers crop shows no differing row outside the label text (font
      anti-aliasing), and the card's bottom edge lands on the same row. The
      chevron beside the name turns up while the menu is open. Rows follow the
      live component: Boost always; Server Tag when the server has one (a
      read of `GET /guilds/:id/profile`, asked once on first open, new
      `AdminRequest::LoadGuildTag`); Invite always (live opens a different
      sheet without the right); Server Settings for anyone with one of manage
      server, manage roles, admin, ban, manage nicknames, create or manage
      expressions, webhooks, audit log or insights (read from the live
      bundle, now `server_settings::can_access`; the Emoji page opens for
      create expressions); Create Event with create or manage events; Leave
      hidden for the owner. Added the `CREATE_GUILD_EXPRESSIONS` bit and four
      glyphs from live (server-boost, calendar-plus, eye-off, id-badge). True
      larpers shows the live list exactly (no tag, settings or event rows).
      Remaining differences, all unverified against a live click since the
      menu rows write or open modals: Server Boost shows a notice (live opens
      the boost modal); Create Event opens the events list (no creation form
      yet); App Directory opens the web page in the browser; the checked
      state of Hide Muted Channels is drawn from the live indicator's CSS,
      not from a live capture; live also adds Create Channel and Create
      Category (with manage channels), Server Insights, Active Threads,
      Friends, Use Server Theme and Report Raid rows when their conditions
      hold, none built. Follow-ups are listed below.
- [x] Create Event dialog behind the header menu row (topic, location or
      channel, start, description, cover). Build and unit test only, no live
      write.
      Done: the row now opens a three step wizard like the live one (strings
      read from the live bundle): "Where is your event?" with Voice Channel,
      Stage Channel and Somewhere Else cards (a channel list or a location
      field under the pick), "What's your event about?" (Event Topic, Start
      Date, Start Time, an "Add end date and time" toggle, Event Description)
      and a Review preview with Create Event. `NewEvent` in the model builds
      the `POST /guilds/:id/scheduled-events` body and names the first
      problem in the web client's words (start in the future, end after
      start, end required for an external place, location required); the API
      call and the `Command::CreateEvent` bridge handler are built and unit
      tested, never sent. Dev command `create-event <guild>` opens the form
      because the account may not manage events and the menu hides the row.
      Checked in the debug app at 1920x1080 for the steps, the channel
      picker and the validation message. Remaining differences: the live
      modal could not be captured (the account lacks the permission), so
      the layout is built from the strings and the shared modal style, not
      measured; the date and time are typed text (YYYY-MM-DD and 7:00 PM)
      where live has a calendar and time dropdown; no repeat picker, no
      cover image upload, no Stage "Good for" hint.
- [x] Server Notification Settings modal behind the header menu row (mute
      toggle, All Messages, Only @mentions, Nothing, the suppress toggles,
      Mute New Events, Mobile Push, per channel overrides). Captured live in
      Low iq gaymers at 1920x1080, modal 480px wide with a Done button.
      Done: the row opens `notification_dialog.rs`, a 480px card with the
      live header (20px title, 40px close), a scrolling body between two
      hairlines and a full width Done. Sections as live: "Mute <server>"
      switch, "Server Notification Settings" with the three radios (disabled
      while muted), Suppress @everyone and @here, Suppress All Role @mentions,
      Suppress Highlights (with its blurb and "Learn more about Highlights"
      link, forced on and disabled while muted), Mute New Events, Mobile Push
      Notifications (off and disabled while muted), then the "Select a channel
      or category" picker and the override table (All, Mentions, Nothing, Mute
      columns, the dashed placeholder row). Behaviour read from the live
      bundle (module 131527): the mute switch only flips `muted` and a
      "Mute duration" select appears once on; Highlights writes
      `notify_highlights` 1 (off) or 2 (on); a channel shows its server's
      level until it has its own; removing an override sends level 3 and
      unmuted. New model fields `mute_scheduled_events`, `mobile_push`
      (default on), `notify_highlights` and `selected_time_window`,
      `ChannelOverride::is_set`, `Store::guild_settings`, and six new
      `menus::Action` bodies with unit tests. The duration labels now use the
      live capitalization ("For 15 Minutes"). Right click on a server icon
      now opens the older list (that handler never existed; only the old
      Notification Settings swap reached it). Dev command `notifications
      <guild>` opens the modal. Checked at 1920x1080 on Low iq gaymers against
      a live capture (card 720,140 to 1200,940, dividers, switches, radios and
      the Done button land within a pixel; text sits 1px lower, the usual font
      metric difference). Every control sends a settings patch, so none was
      clicked on the real account; only the picker (local state) was driven.
      Remaining differences: no scrollbar thumb on the body (live shows one);
      the picker list opens inline and pushes the table down where live
      floats it over the modal, with no type to filter; the override row is
      built from the bundle, not captured (live's add click was blocked as a
      write, so the row's removal button position and the category glyph,
      closed folder live against our open one, are unchecked); the muted state
      and the duration select are not seen live or here because muting is an
      account write; the new-notifications experiment's "@mentions + Unreads"
      option is not drawn.
- [x] Leave Server confirm modal and the Server Tag modal, from the header
      menu rows.
      Done: new `server_prompts.rs`, one entity for both, drawn from live
      captures at 1920x1080 on Low iq gaymers. Leave Server is a 400x206
      card (12px radius, 8px top padding, 20px title "Leave 'name'" with a
      40px close, the muted sentence with the server name in semibold, a 64px
      footer with Cancel and a red Leave Server button 171x40 each, 8px
      apart). The server menu row and the rail right click both open it, so
      the old "click again to leave" arming and `Action::Leave` are gone. The
      Server Tag row opens the "Adopt this server's tag" card, 706x322: the
      title and blurb, the server row with its icon, name and a 26px tag chip,
      a full width Adopt Tag and Edit Profile, and a black right half with the
      five sample chat lines (names, colors and lines read from the live
      bundle; the four avatars are the client's own, saved under
      discord-assets/tag-preview). Adopt Tag is drawn disabled when the tag is
      already the one we wear; Edit Profile opens the per-server profile
      page. New `Command::AdoptServerTag` sends `PUT /users/@me/clan` with
      `identity_guild_id` and `identity_enabled` (endpoint and body read
      from the web bundle) through `ApiClient::set_guild_identity`; the leave
      body and the identity body have unit tests. Neither request was sent
      from the real account; only the modals were opened. Dev commands
      `leave-prompt <guild>` and `tag-prompt <guild>`. Compared at 1920x1080:
      the cards, rows, buttons and chip land on the live boxes (the window has
      its usual 8px frame offset); text sits about 1px lower, the usual font
      metric. Remaining differences: the live guild icon is clipped by the
      client's tag icon mask where ours is a circle; the red inline error box
      the live Leave card reveals on a failed request is not drawn; the
      adopted state and its tooltip "You've already adopted this server's
      tag" are from the bundle only, because this account has not adopted
      the tag.
- [x] Invite glyph in the header and on the hovered channel row, as drawn live.
      Done: the header button now draws the live two person people-add glyph
      at 20px in the 32px button (white over a banner, as live); the channel
      row shows the single person user-plus at 16px in a 20x16 box flush with
      the row's right padding, bright on row hover, 8px from the row edge as
      live. Live shows the row invite for anyone who may create an invite or
      when the server has a vanity link (read from the live bundle and
      confirmed on Low iq gaymers, whose account lacks the permission but the
      server has vanity `lowiq`), so the guild now keeps `vanity_url_code` and
      `Store::can_invite` decides it, also used to pick the header menu's
      invite channel (unit test in the store reducer tests). The row's edit
      shortcut now needs Manage Channels like live, where ours showed a pencil
      on every channel. Compared at 1920x1080 on Low iq gaymers #yap-yap:
      glyph rows and columns line up with live. Remaining differences: the
      live sidebar in the playwright session is 303px wide against our 293,
      which shifts everything on its right edge by 10px (not touched here, it
      may be the session's resized sidebar); the edit shortcut glyph and the
      invite sheet shown to someone without the permission are not captured
      (the account manages no channel and the sheet writes).

### Home and direct messages

- [x] Nitro, Shop and Quests rows under Friends in the DM sidebar. Done: the
      rows sit at the live pitch (Friends, Nitro, Shop, Quests at y 109, 149,
      189, 229 at 1920x1080) with the live glyphs (`nitro.svg`, `store.svg`,
      new `quests.svg`). Each opens a read only shell in
      `crates/oxicord-ui/src/home_pages.rs` and the title bar names it. Nitro
      has the live nav strip (Home, What's New, Best of Nitro, Plans, Compare,
      Gift Nitro) over the purple hero with the two buttons. Shop has its nav
      strip (Featured, Browse, Orbs Shop, Game Shops, Game Servers, search) and
      a note instead of the catalog. Quests is the full pane "Opening Discord
      App." card, as the live /quest-home draws it. The NEW pill on Nitro and
      Shop goes away once the row is opened, remembered in settings
      (`seen_home_tabs`, unit tested). Remaining differences: the live Shop is
      a real catalog (hero, bundles, Orbs) and the live Nitro page has the
      What's New and plans sections, none of which are drawn; the Nitro
      headline is gg sans italic where the live one is an image in a display
      face; the heart and Gift Nitro glyphs, the Orbs counter and the BETA tag
      are missing (no gift or heart icon file yet); the pill colour, size and
      placement are unverified because the live account has already opened both
      rows and shows no pill (the pill is blurple 11px bold from memory of the
      earlier audit). The live gradient is pinker at the top than ours.
- [x] DM rows: status or activity line under the name (custom status, a bot's
      "/help" text, "Dungeon crawling | Use /raid") and the server tag chip.
      Done: one to one rows show `friends::status_line` as a 12px muted line
      under the name, the tag chip already existed, and group rows keep their
      "N Members" line in the same 12px style (was 11px). Rows stay 42px with a
      2px gap, as live. Remaining differences and unverified parts: the line
      never showed in the debug app because Oxicord identifies without the
      gateway capabilities on purpose (a token was revoked once with them), so
      the server sends presences for friends only and the DM list of this
      account, mostly bots and non friends, shows everyone offline, where live
      shows Ticket King, Bloxlink and others online with their lines. The line
      appears for anyone whose presence we do receive (friends, people met in
      a member list). The layout is from live measurements, not a capture of
      ours. Live captured at 1920x1080 only.
- [x] Friends page: the tab strip collapses to "Friends", Pending and Add
      Friend when no one is online, plus the "Looking for accounts you've
      blocked or ignored? Go to settings" banner, the search field and the
      "Received - N" heading. Verify the full layout on an account that has
      friends before copying it.
      Done: the strip follows the live client's own rule (read from the
      bundle): Online shows only while a friend is online, All while there is
      a friend, Pending while a request is open (with the red incoming
      count), Add Friend always, and a tab that leaves the strip falls back to
      the first one left. Header is the 32px friends icon, the title, a 4px
      dot and 32px pills (selected grey, Add Friend filled, or tinted when
      open). Below it: the dismissible banner (close is remembered in
      settings), the 40px Search field (filters by name or username, with a
      clear X), section headings "Received - N", "Sent - N", "Online - N" and
      "All friends - N" in 14px medium, 61px rows under a hairline with a 32px
      avatar, the 16px name, the server tag chip, the username and 36px
      round actions. Active Now is 420px wide with the 20px heading and the
      live empty text. Measured against live at 1920x1080 on the Pending page
      (right edges, vertical positions within 1px). Remaining differences and
      unverified parts: this account has no friends, so the Online and All
      rows are not compared with live and keep the status line under the name;
      the banner dismissed layout is assumed (dismissing writes the real
      account setting); live has no Blocked tab (that list is in its
      settings), ours keeps one while someone is blocked until the settings
      rebuild has a restricted users page, and "Go to settings" opens Data &
      Privacy for now; accept and decline icons do not tint green and red on
      hover. The Add Friend page itself is still the old layout (see the next
      item).
- [ ] Add Friend page rebuild: live shows a 20px heading, the line "You can
      add friends with their Discord username." with the Wumpus picture at
      the right, one 16px rounded box holding the username row, a "Personalize
      your request (Optional)" note field with a 120 counter and the line
      "What you write here will also show up in your DM if you become
      friends.", then "Other Places to Make Friends" with an "Explore
      Discoverable Servers" row. The request body with a note is built and
      unit tested only, never sent.
- [ ] New group DM button at the right end of the friends header, and the
      inbox and help icons in the title strip.

### User settings (the sidebar was rebuilt in the live client)

The live sidebar, top to bottom: Account (sub-entries Account Info, Password &
Security, Account Status, Family Center), Data & Privacy, Messaging
Permissions, Notifications; Billing group: Nitro, Server Boost, Subscriptions,
Gift Inventory, Billing; Experience group: Voice & Video, Appearance,
Accessibility, System, Language & Time; Games & Apps group: Activity Privacy,
Connected Apps; then Developer and a red Log Out row, and a footer with the
build line ("stable 670026"), Privacy Policy, Terms of Service and More. Ours
is the older layout (My Account, Content & Social, Data & Privacy, Authorized
Apps, Devices, Connections, then App settings: Appearance, Voice & Video, Chat,
Notifications, Keybinds, Advanced).

- [ ] Rebuild the settings sidebar to the live structure: group headings, the
      Account sub-entries that scroll to a section, the Log Out row, the
      version footer, and the search box matching every page below.
- [ ] Account page as one scrolling page: Account Info (Username, Email and
      Phone with Reveal and Edit), Password & Security (Password Edit,
      Multi-Factor Authentication "Set up", Logged-in Devices "N devices"),
      Account Status (Age Group with Confirm Age, Account Standing "All
      good"), Family Center ("Set up Family Center"), and the Disable and
      Close account rows. The existing dialogs stay, only their entry points
      move. Devices becomes the Logged-in Devices detail page.
- [ ] Data & Privacy page: How Discord Uses My Data (use data to make Discord
      work), Request my data, Profile Privacy, and the related Activity Privacy
      link.
- [ ] Messaging Permissions page (replaces Content & Social): Content Filters
      (sensitive content filters), Spam Filters, Direct Message permissions,
      Friend Request permissions, Messaging in Connected Games, and the Ignore
      & Block list.
- [ ] Notifications page as drawn live: Overview, Sounds, Badges, Email (with
      "Unsubscribe from all marketing emails"), and the Advanced group behind
      "Show Advanced Notification Settings".
- [ ] Voice & Video page additions: the Voice group with "Show Advanced Voice
      Settings", Camera, Streaming, Sounds (with "Show 18 more sounds"),
      Soundboard with its server and sound pickers, and the Advanced group with
      "Show Diagnostic Settings" and "Reset all Voice & Video settings". Video,
      soundboard playback and streaming stay inert (see Out of scope).
- [ ] Appearance page as drawn live: Theme with Default Themes and Color
      Themes (locked ones link to Nitro), App Icon, Messages (display mode and
      spacing), Chat Box, Search, Streamer Mode. Ours has a time format and a
      members list switch only.
- [ ] Accessibility page: the live preview message, Text Readability, Visual
      Density, Color & Contrast, Reduced Motion, Audio & Screen Reader.
- [ ] System page (the old Keybinds page): Custom Keybinds and Default
      Keybinds, with the record a key control.
- [ ] Language & Time page: language and time zone choices. Capture the live
      page first, its headings did not read back.
- [ ] Activity Privacy page: What Activity I Share, Where I Share Activity,
      Activity Joining.
- [ ] Connected Apps page: Connections with Nitro Rewards, then the Authorized
      Apps list (Verification and Security). Merges our Connections and
      Authorized Apps pages.
- [ ] Developer page (Developer Mode and the related switches). Merges our
      Advanced page.
- [ ] Billing group shells, read only, no purchase flow: Nitro (the Nitro Basic
      and Nitro plan cards with the perks list), Server Boost (Server Boosts,
      perks per level, Boosting FAQ), Subscriptions, Gift Inventory (Redeem
      Codes, Gifts I Purchased), Billing (Payment Methods, Transaction
      History).
- [ ] Menu style shared by all popup and context menus: live draws the glyph at
      the left of each label, sizes the card to its content (201px for the
      plus menu), rounds it at 8px and gives rows 36px height with a 2px
      radius. Ours is 220px wide, rows 32px with a 4px radius and glyphs on
      the right. Measure the
      live message, user, server and plus menus first, then restyle
      `popup.rs` and `context_menu.rs` once.

## Out of scope

Video, camera, screen share, Go Live, stage, activities, soundboard and voice
messages. Their buttons stay drawn and do nothing. Spotify listen along.
