# Discord reference (live capture 2026-11-07T10:48:31.917Z)

## Friend nickname (captured 2026-11-07)

Entry point: right-click a DM/friend in the DM list (or Friends list) → context
menu item **"Change Friend Nickname"**. (The DM context menu also has
"Add Note\nOnly visible to you", confirming the Note entry point.)

Dialog:
- Title: `Change Friend Nickname`
- Body: `Find a friend faster with a personal nickname. It will only be visible to you in your direct messages.`
- Field label: `Friend Nickname`
- Input: placeholder = current display name, value = existing nickname (empty if none)
- Buttons: `Reset Friend Nickname`, `Cancel`, `Save`

API: `PUT /users/@me/relationships/:id` with body `{ "nick": "..." }`.
Reset sends an empty/none nick. Update broadcast via gateway `RELATIONSHIP_UPDATE`.

## Profile Note field (captured 2026-11-07)

The profile popout/panel has a Note textbox with placeholder **"Click to add a note"**
and helper text "Only visible to you" (in the context menu it is "Add Note\nOnly visible to you").

## Member Ban (documented assumption, 2026-11-07)

Live capture of the guild member popout/ban UI was unstable (page crashes, guild
navigation failed), so the Ban flow is implemented from documented Discord behavior:

- Entry point: the guild member profile card. Discord shows a red **"Ban"** action
  (gated on the `Ban Members` permission and not targeting the server owner), next
  to **"Kick"**.
- Confirmation: Discord opens a confirm dialog titled `Ban @<name>?` (we render
  `Ban <name>?`), with body text warning the member cannot rejoin without an
  unban, and a toggle **"Delete this member's messages for the last 7 days"**.
- Confirm uses `PUT /guilds/:guild/bans/:user` with `{ "delete_message_days": 7 }`
  when the toggle is on, else `0`.
- A failure arrives as an `Event::Notice`.

To verify against live Discord, re-run the member-popout capture once the browser
CDP session is stable.

## Member Timeout (documented assumption, 2026-11-07)

Same live-capture limitation as Ban. Implemented from documented Discord behavior:

- Entry point: the guild member profile card shows a red **"Timeout"** action,
  gated on the `Timeout Members` (`MODERATE_MEMBERS`) permission and not the owner.
- Dialog: titled `Time Out @<name>?`, with preset durations (60 seconds, 5/10
  minutes, 1 hour, 1 day, 1 week, 30 days) and a **"Time Out"** confirm button.
  We also surface a **"Remove Timeout"** button that lifts an active timeout.
- Confirm sends `PATCH /guilds/:guild/members/:user` with
  `{ "communication_disabled_until": "<RFC3339>" }`; lifting sends `null`.
  The model already carries `Member.communication_disabled_until`.

The UI wiring (mod, session event, profile-card action, render child) was landed
in a prior commit; this change adds the API, command, bridge arm, the
`TimeoutDialog` itself, and the `OpenTimeoutMember` event.

## Member Roles (documented assumption, 2026-11-07)

Same live-capture limitation as Ban/Timeout. Implemented from documented Discord
behavior:

- Entry point: the guild member profile card shows a **"Roles"** action, gated on
  the `Manage Roles` (`MANAGE_ROLES`) permission and not targeting the server owner.
- Dialog: titled `Roles — <name>`, lists the server's roles (excluding `@everyone`
  and bot-managed roles) with checkboxes pre-ticked to the member's current roles.
- Save sends `PATCH /guilds/:guild/members/:user` with `{ "roles": [...] }`,
  always including `@everyone` (id == guild id). We don't enforce the role
  hierarchy on the client; the API rejects out-of-reach roles.

## Uploads: spoiler marking (documented assumption, 2026-11-07)

Same live-capture limitation as the moderation work: the browser reaches
Discord but our DOM selectors don't match the current client, so this follows
documented Discord behavior:

- The composer's pending-file chips (already rendered) each get an eye toggle.
  Toggling it on marks that file as a spoiler.
- On send, the bridge prefixes a spoiler file's name with `SPOILER_`, which is
  how Discord hides the attachment until clicked. `read_files` adds the prefix
  only when missing, so an already-prefixed name is left alone.
- The send command now carries `(PathBuf, bool)` per file; the bool is the
  spoiler flag. The `read_files` test covers the prefix behavior.

Alt text for images is a separate, still-open item on the roadmap.



## Home / DM view

```json
{
  "guild_rail": [],
  "channel_list": [],
  "main_header": "Direct Messages",
  "bodySample": "Online tab added\nFriends\nDirect Messages\n6\nNeverlose Roblox\nNeverlose Roblox\nGIF Central\nGIF Central\nUnread messages, Counter Blox | Headwind\nCounter Blox | Headwind\nUnread messages, Constantiam Presidential Elections\nConstantiam Presidential Elections\nProject Aurora\nProject Aurora\nUnread messages, "
}
```

## Message row structure

```json
{
  "found": false
}
```

## User Settings sidebar sections

```json
{
  "sections": [
    "Home",
    "Log In",
    "Download",
    "Nitro",
    "Discover",
    "Quests",
    "Safety",
    "Support",
    "Blog",
    "Developers",
    "Careers",
    "Status",
    "App Directory",
    "About",
    "Jobs",
    "Brand",
    "Newsroom",
    "Fall Release",
    "Feedback",
    "Creators",
    "Community",
    "Official 3rd Party Merch",
    "Terms",
    "Privacy",
    "Cookie Settings",
    "Guidelines",
    "Acknowledgements",
    "Licenses",
    "Company Information"
  ]
}
```

## Profile popout (own) - Note field + actions

```json
{
  "note_placeholders": [
    "Search"
  ],
  "buttons": [
    "Find or start a conversation"
  ],
  "headings": [
    "Direct Messages",
    "Friends",
    "Online — 41",
    "a.oi",
    "Aliolii",
    "AncientFigurine",
    "barrett",
    "bebo",
    "CBRNUNIT",
    "Critaata"
  ]
}
```

## Right-click context menu on a DM

```json
{
  "found": false
}
```

## Inbox view

```json
{
  "tabs": [
    "Direct Messages\n6\nNeverlose Roblox\nNeverlose Roblox\nGIF Central\nGIF Central\nUnread messages, Counter Blox | Headwind\nCounter Blox | Headwind\nUnread messages, Constantiam Presidential Elections\nConstantiam Presidential Elections\nProject Aurora\nProject Aurora\nUnread messages, The Order\nThe Order\nUnread messages, Sovereign Constantiam, Voice call active\nSovereign Constantiam\nUnread messages, FishAnarchy.net\nFishAnarchy.net\nUnread messages, 𖤐 Japsterdam 𖤐\n𖤐 Japsterdam 𖤐\nGorilla Mercenary Services\nGorilla Mercenary Services\nUnread messages, anarchy configs\nanarchy configs\nUnread messages, Torn Exchange\nTorn Exchange\nUnread messages, 2b2t Census Bureau\n2b2t Census Bureau\nUnread messages, HiddenDevs | Roblox, Scheduled event\nHiddenDevs | Roblox\nUnread messages, Hard Working Sloths\nHard Working Sloths\n1 mention, Void | Roblox Trading Tools\n1\nVoid | Roblox Trading Tools\nnotantque's server\nns\nnotantque's server\nnotantque's server\nns\nnotantque's server\nnotantque's server\nns\nnotantque's server\nUnread messages, 2b2t/Constantiam Chest Shop\n2b2t/Constantiam Chest Shop\nnotantque's server\nns\nnotantque's server\nUnread messages, Parliament of Minecraft Anarchy\nParliament of Minecraft Anarchy\nUnread messages, Rolimon's | Roblox Trading\nRolimon's | Roblox Trading\nUnread messages, Constantiam Tier List\nConstantiam Tier List\nUnread messages, Evil space\nEvil space\nUnread messages, Natro Macro, Voice call active\nNatro Macro\nUnread messages, Grave/Digger Community Server\nGrave/Digger Community Server\n8 mentions, Alt Hub Fanclub™, Scheduled event\n8\nAlt Hub Fanclub™\n5 mentions, BBCHUB\n5\nBBCHUB\nUnread messages, aspect.sys\naspect.sys\nUnread messages, specter.club\nspecter.club\nUnread messages, Potassium, Voice call active\nPotassium\nUnread messages, astra [BETA]\nastra [BETA]\n1 mention, Tomboy.Hook\n1\nTomboy.Hook\n24 mentions, Site 80 \"Jacoby\" | Faction Hub\n24\nSite 80 \"Jacoby\" | Faction Hub\nUnread messages, Five-O Community\nFive-O Community\n1 mention, The GIA, Voice call active\n1\nThe GIA\n4 mentions, Grand Alfheim\n4\nGrand Alfheim\nUnread messages, Constantiam Base Archive\nConstantiam Base Archive\nUnread messages, The Rishi Verdhana\nThe Rishi Verdhana\nUnread messages, Research Site-73 | SCP:RP\nResearch Site-73 | SCP:RP\nUnread messages, 10builders10tools Official\n10builders10tools Official\nUnread messages, Vanity Services\nVanity Services\nUnread messages, TORN\nTORN\nEmpire Studios\nEmpire Studios\nUnread messages, Constantiam Chat Room\nConstantiam Chat Room\nUnread messages, InfraNet\nInfraNet\nUnread messages, Mush Technologia\nMush Technologia\nnotantque's server\nns\nnotantque's server\n337 mentions, SCP:RP | Installation - 45\n337\nSCP:RP | Installation - 45\nWrath's Discord\nWrath's Discord\n17 mentions, Site 80 \"Jacoby\" | The New Beginning\n17\nSite 80 \"Jacoby\" | The New Beginning\n3 mentions, Blackhub\n3\nBlackhub\nConst Mapart Contests\nConst Mapart Contests\nUnread messages, Edp Watch\nEdp Watch\nUnread messages, Map Artists of Constantiam\nMap Artists of Constantiam\nUnread messages, SCP | Factions Hub Reports and Appeals\nSCP | Factions Hub Reports and Appeals\nUnread messages, The Care Home\nThe Care Home\nUnread messages, Map Artists of 2b2t\nMap Artists of 2b2t\nRubiRedstone's Community Hub\nRubiRedstone's Community Hub\nUnread messages, The Hunter’s Union of Constantiam\nThe Hunter’s Union of Constantiam\nUnread messages, Ascension Studios\nAscension Studios\nUnread messages, [M.E.G] Major Explorer Group\n[M.E.G] Major Explorer Group\n2 mentions, 🎨 ⋆ Octolinguski ⋆ ✨\n2\n🎨 ⋆ Octolinguski ⋆ ✨\nUnread messages, Bite By Night Support Server\nBite By Night Support Server\n3 mentions, 𝐒𝐈𝐓𝐄 - 64 (wip)\n3\n𝐒𝐈𝐓𝐄 - 64 (wip)\n2 mentions, .gg/riths\n2\n.gg/riths\nUnread messages, Imperials of Constantiam\nImperials of Constantiam\nUnread messages, Constantiam Highway Builders\nConstantiam Highway Builders\nPhoenix Corp.\nPhoenix Corp.\nUnread messages, rfresh technologies\nrfresh technologies\nUnread messages, Amadeus\nAmadeus\nConstantiam Chat Broadcast, Constantiam Farms, folder\nnotantque's server\nns\nnotantque's server\nUnread messages, mioclient.me | DLC\nmioclient.me | DLC\n1 mention, DupersUnited\n1\nDupersUnited\nUnread messages, Constantiam trading platform/travel outposts\nConstantiam trading platform/travel outposts\nUnread messages, Constantiam Info\nConstantiam Info\nUnread messages, The Constantiam Alliance\nThe Constantiam Alliance\nUnread messages, Rusher Community\nRusher Community\n9 mentions, valentinee471 HOOD\n9\nvalentinee471 HOOD\nDD | Support\nDD | Support\n7 mentions, Defuse Division\n7\nDefuse Division\nfreaks only\nfreaks only\nUnread messages, Dew Drinker Discord\nDew Drinker Discord\n248 mentions, ◥ SITE-53 | ARCHIVED | ꜱᴄᴘ:ʀᴘ ◤\n248\n◥ SITE-53 | ARCHIVED | ꜱᴄᴘ:ʀᴘ ◤\nUnread messages, R.R.T. | IV Branch\nR.R.T. | IV Branch\n40 mentions, Site-47 | \"A New Life\"\n40\nSite-47 | \"A New Life\"\nLow iq gaymers\nLow iq gaymers\nlocked in\nlocked in\nUnread messages, TempleOS, Scheduled event\nTempleOS\nnotantque's server\nns\nnotantque's server\nUnread messages, Starscape Community\nStarscape Community\nUnread messages, Hapless Studios, Voice call active\nHapless Studios\nSprawa M.W i M.K\nSM.WiM.K\nSprawa M.W i M.K",
    "Friends\n5\nMessage Requests\n1\nNitro\nOFFER\nShop\nQuests\nDirect Messages\nmy cute little bf\nServer Tag: ZOE\nZOE\nHarcèlement Informatique\nCotton \nServer Tag: LWIQ\nLWIQ\nhekknaw\n‧₊˚✧[NYX]✧˚₊‧ \nServer Tag: TFR\nTFR\nGabriel \nServer Tag: LWIQ\nLWIQ\nZharrs\nRimWorld\n•\nje me fait chier\nMore_Orgasm\nGeneric GC V2\n10 Members\nBloxlink\n/invite | blox.link\n/invite | blox.link\ngray \nServer Tag: TUNE\nTUNE\nlil fat bro \nServer Tag: ᶻz𐰁\nᶻz𐰁\n747 (carti edition)\nDiscord\nOFFICIAL DISCORD MESSAGE\nOFFICIAL\nOfficial Discord Message\nRaika\nServer Tag: Ado\nAdo\nSCP Secret Laboratory with Medal\n•\nJust remember in the winter Far beneath the bitter snows Lies the seed that with the sun's love In the spring becomes the rose.\nHΛNTΞL \nServer Tag: G4S\nG4S\nOris\nServer Tag: MYMY\nMYMY",
    "Friends",
    "Online",
    "All",
    "Pending, 5 new",
    "Add Friend",
    "Looking for accounts you've blocked or ignored? Go to settings\nOnline — 41\na.oi\nServer Tag: EXAM\nEXAM\nROBLOX\nROBLOX\n•\nrasonia\nAliolii\nDo Not Disturb\nAncientFigurine\nmeow\nbarrett\nServer Tag: NANO\nNANO\nRoblox with Medal\n+1\nRoblox with Medal\nthe screams of the damned\nbebo\nDo Not Disturb\nCBRNUNIT\nIdle\nCritaata\nServer Tag: SCP\nSCP\nDo Not Disturb\nDel\nServer Tag: UN\nUN\nOnline\nevylen\nDo Not Disturb\nfiremaster\nServer Tag: FEET\nFEET\nMeteor Client\nMeteor Client\n•\nbye\nFlan\nJarona!\nFlandreu\nServer Tag: IMPS\nIMPS\nOnline\nfluff\nDo Not Disturb\ngoopdestroyer3700\nDo Not Disturb\nhhugo\nIdle\nIgor\nServer Tag: RIP\nRIP\nOnline\nActive Now\nMaddie 🦃, Better TTS, and 1 other\nIn a Voice Channel\nSovereign Constantiam\nvoice 1\nMaddie 🦃\nBetter TTS\ndarq\nSera\nROBLOX – 12h\nRoblox\n1 Person\nSera\nbarrett\nRoblox with Medal – 38m\nClipping Roblox\nwith Medal\n38:30 elapsed\nfiremaster\nMeteor Client – 34m\nF1remast3r\nMeteor on Crack!\n34:48 elapsed\nStarman\nPlaying on Xbox\nBlizzard Arcade Collection\n1 Person\nStarman\nrynim\nCounter-Strike 2 – 18m\nCounter-Strike 2\n1 Person\nrynim\nZenith\nROBLOX – 6m\nRoblox\n1 Person\nZenith\na.oi\nROBLOX – 11m\nRoblox\n1 Person\na.oi\nRaika\nSCP Secret Laboratory with Medal – 1h\nClipping SCP Secret Laboratory\nwith Medal\n01:57:24 elapsed\nunderd0sed\nMinecraft – 5h\nMinecraft\n1 Person\nunderd0sed\nZharrs\nRimWorld – 1h\nRimWorld\n1 Person\nZharrs",
    "Online — 41\na.oi\nServer Tag: EXAM\nEXAM\nROBLOX\nROBLOX\n•\nrasonia\nAliolii\nDo Not Disturb\nAncientFigurine\nmeow\nbarrett\nServer Tag: NANO\nNANO\nRoblox with Medal\n+1\nRoblox with Medal\nthe screams of the damned\nbebo\nDo Not Disturb\nCBRNUNIT\nIdle\nCritaata\nServer Tag: SCP\nSCP\nDo Not Disturb\nDel\nServer Tag: UN\nUN\nOnline\nevylen\nDo Not Disturb\nfiremaster\nServer Tag: FEET\nFEET\nMeteor Client\nMeteor Client\n•\nbye\nFlan\nJarona!\nFlandreu\nServer Tag: IMPS\nIMPS\nOnline\nfluff\nDo Not Disturb\ngoopdestroyer3700\nDo Not Disturb\nhhugo\nIdle\nIgor\nServer Tag: RIP\nRIP\nOnline"
  ],
  "headers": [
    "Direct Messages",
    "Friends",
    "Online — 41",
    "Active Now"
  ]
}
```

## Open Friends tab

```json
{
  "clickedFriends": true
}
```

## Profile popout via clicking a friend row

```json
{
  "noteFields": [
    {
      "ph": "Search",
      "aria": "Search",
      "cls": "input_0PLWhS"
    }
  ],
  "buttons": [
    "Find or start a conversation"
  ],
  "headings": [
    "Direct Messages",
    "Friends",
    "Online — 43",
    "a.oi",
    "Aliolii",
    "AncientFigurine",
    "barrett",
    "CBRNUNIT"
  ],
  "noteRegion": []
}
```

## Open a DM channel

```json
{
  "header": "Direct Messages"
}
```

## Message row + right-click context menu

```json
{
  "found": false
}
```

## Server member list and moderation (2026-11-07, read-only)

Observed in `notantque's server` > `#general` (read-only; no state changed).

- A `complementary` region labelled `Members list for <channel>` holds a `list "Members"`.
- Members are grouped under role/status headings, e.g. `heading "Online, N member"`.
  Each entry shows the avatar, display name, server-tag pill and (for the owner)
  a `Server Owner` badge.
- Opening a member's profile popout (clicking their name/avatar) is where Discord
  surfaces moderation actions for people you can manage: a red **Kick <name>** and
  **Ban <name>** item, plus **Timeout** and role actions, gated by your
  `KICK_MEMBERS` / `BAN_MEMBERS` permission and role hierarchy. The server owner
  is never kickable/bannable.
- API used to remove a member: `DELETE /guilds/{guild.id}/members/{user.id}`
  (mirrors the existing `unban` call shape in this repo).
- Implemented: profile popout now shows a red **Kick** button for guild members
  the viewer may remove (has `KICK_MEMBERS`, target is not the guild owner).

## Server header (captured 2026-11-07, read-only in notantque's server)

Inside a guild, the sidebar header (`CHANNEL_HEADER_HEIGHT`, border-bottom) holds
two controls, left-aligned:

- The **server name** as a button (heading level 2). Clicking it opens the server
  actions menu (the same `Target::Guild` menu Oxicord already opens).
- A dedicated **"Invite to Server"** button (icon button, `user-plus`) immediately
  to its right. It opens the invite creator for the current channel.

Below it, the channel list shows categories ("Text Channels", "Voice Channels",
each collapsible with a "Create Channel" affordance) and, on hover, each text
channel row reveals two icon buttons: **Invite to Channel** (`user-plus`) and
**Edit Channel** (pencil). Oxicord already renders the hover **Invite to Channel**
button; the header **Invite to Server** button and the hover **Edit Channel**
button are the missing pieces.

For the smallest first-run change: add the header **Invite to Server** button
reusing the existing `OpenInviteDialog(channel)` event + `InviteDialog`.

## Channel hover "Edit Channel" (captured 2026-11-07)

On hovering a text channel row in the sidebar, Discord reveals two icon buttons on
the right: **Invite to Channel** (`user-plus`) and **Edit Channel** (pencil).
Clicking the pencil opens a small "Edit Channel" dialog with the channel **name**
and **topic** fields and Save/Cancel. The gateway reports the change back
(`CHANNEL_UPDATE`), so the sidebar name updates.

Oxicord already had the hover Invite button. Implemented: a hover **Edit
Channel** pencil (shown for guild channels, hidden for DMs) that emits a new
`OpenChannelSettings(channel)` session event, opening `EditChannelDialog`. The
dialog prefills name + topic from the store and saves via the existing
`AdminRequest::EditChannel` -> `api.edit_channel` admin path.

## User panel mute / deafen (captured 2026-11-07)

The user panel (bottom-left) shows, between the account card and the settings
gear: a **mute** button (mic → mic-off when active) and a **deafen** button
(headphones → headphone-off when active). Active toggles are drawn in red
(`DANGER`), matching Discord. The toggles are independent. In Discord they drive
the voice gateway's `self_mute`/`self_deafen`; Oxicord shows the local intent for
now and will send it once a voice connection exists.

Implemented: the two toggle buttons on the user panel with local state and the
icon/colour swap. No voice plumbing yet.

## Category "Create Channel" button + modal (captured 2026-11-07)

Each category header in the server sidebar shows a **Create Channel** button on
its right edge (always visible, not hover-revealed — it sits as a sibling of the
category name/chevron). Clicking it opens a **"Create Channel"** dialog scoped to
that category (banner reads "in <Category>"):

- **Channel Type** radio group: Text (default, "Send messages, images, GIFs,
  emoji, opinions, and puns"), Voice ("Hang out together with voice, video, and
  screen share"), Forum ("Create a space for organized discussions").
- **Channel Name** textbox, placeholder `new-channel` (Discord uses kebab-case).
- Footer: **Cancel** + **Create Channel** (disabled until a name is typed).

The create is a `POST /guilds/:id/channels` with `name`, `type`, and `parent_id`.
Oxicord reuses `AdminRequest::CreateChannel { guild, name, kind, parent }` ->
`api.create_channel`.

Implemented: a "Create Channel" button on every guild category header (emits
`OpenCreateChannel(guild, Some(category_id))`) opening `CreateChannelDialog` with
the Text/Voice/Forum type cards, a name input, and Cancel/Create buttons (Create
disabled while the name is empty). The gateway reports the new channel back.

## Composer "Send a gift" button + gift modal (captured 2026-11-07)

The message composer toolbar (left to right in Discord): **gift**, GIF, sticker,
emoji, Apps. The **gift** button (tooltip "Upgrade your friends! Gift them awesome
chat perks with Nitro.") opens a **"Send a Gift"** dialog:

- Heading "Send a Gift" with a Close (✕) button.
- Two tier cards:
  - **Nitro** — "2-WEEK TRIAL" badge; perks: 1GB uploads, Custom emoji anywhere,
    Unlimited Super Reactions, HD video streaming, 2 Server Boosts, Custom profiles
    and more!
  - **Nitro Basic** — `€3.03` /month, `€30.41` /year; perks: 50MB uploads, Custom
    emoji anywhere, Unlimited Super Reactions, Special Nitro badge on your profile.
  - Each card has a "Gift <Tier>" button.
- Footer: "1 gift away from the Patron Badge!".

Discord processes payment on the web; the client only shows the upsell. Oxicord has
no payment backend, so this is a **read-only** view — selecting a tier closes the
dialog without charging anything (documented in `gift_dialog.rs`).

Implemented: a **gift** button on the composer (emits `OpenGift`) opening
`GiftDialog`, a centered modal with the Nitro / Nitro Basic tier cards, perks, and
the Patron Badge footer. Closing via ✕, Esc, or backdrop emits `CloseGift`.
## Rules screening dialog (captured 2026-11-08, live)

Server used: SCP | Roleplay (605462595239149568), 1920x1080, dark theme, 100% zoom.
The modal is reached from the channel banner "You must complete a few more steps
before you can talk." and its "Complete" button, and is the web client's
`in-app-member-verification` modal.

### Entry banner

- Sits where the composer would be, inset 8 px from the message area on each
  side and 8 px above the bottom.
- Box: 1263 x 58, radius 8, border 1 px rgba(150,151,159,0.2), background
  #000000, padding 12 px 0.
- Text: 16 px / 20 px, weight 600, #dcdcdf, 16 px from the left.
- Button: 83 x 32, radius 8, border 1 px rgba(255,255,255,0.08), background
  #5865f2, text 14 px / 20 px weight 500 white, 13 px from the right.
- Button hover is #4752c4 (web), the port uses the same.

### Backdrop and frame

- Scrim: black at 72 % (rgba(0,0,0,0.72)).
- Card: 1280 x 968, centered, radius 12, border 1 px rgba(150,151,159,0.2),
  background #0a0a0c, shadow 0 12 px 24 px rgba(0,0,0,0.24).
- Card padding is visual only: the left pane has 8 px / 16 px outer margins,
  the right column runs the full height.

### Left pane and server card

- Pane: 419 x 950, 16 px side margins, 8 px top/bottom, radius 8, clipped.
- Background: linear-gradient(315deg, dominant icon colour, generated accent),
  opacity 0.33 over the card background. The accent is the first generated
  colour of the web client: take the icon's most common colour (skip alpha <
  125 and pixels above 250 in all channels, sample every tenth pixel), convert
  to HSL, add 30 to saturation when below 30, subtract 40 from lightness above
  80, add 15 below 20, then rotate hue by 90 degrees.
- Server card: 282 wide, radius 16, background #17181b, border 1 px
  rgba(150,151,159,0.2), clipped. Vertically centered.
- Banner: 280 x 120, background `discovery-splashes/{guild}/{custom_banner_hash}.jpg?size=512`,
  cover.
- Avatar: 64 px round, 16 px from the left, centered in a 70 px row that overlaps
  the banner by 34 px.
- Name: 16 px / 20 px weight 600 #dcdcdf, row 21 px high, 4 px gaps to the
  discoverable badge (18 x 18) and the server tag chip.
- Tag chip: 16 px high, 4 px side padding, radius 4, background
  rgba(151,151,159,0.24), 14 px clan badge image, 12 px / 16 px weight 600 text.
- Counts: 14 px / 18 px #96979e, 8 px round dots, online #3d9e60, members
  #9d9ea5, 4 px between dot and text, 8 px between the two groups.
- Established line: 14 px / 18 px #96979e.
- Description: 14 px / 18 px #96979e, 10 px above, 16 px side padding, 16 px
  bottom padding.
- Displayed counts use comma grouping (41,786 and 182,412).

### Right column

- Column: 688 wide, centered in the remaining pane, padding 60 px top, 24 px
  sides, 36 px bottom. Everything above the footer scrolls.
- Heading: "Before you can talk here...", 32 px / 40 px weight 500 #dcdcdf,
  8 px top and bottom margin.
- Subtitle: "You'll need to complete the required steps below.", 14 px / 18 px
  #d4d5d8.
- Question: 40 px below the subtitle. Title "Agree to rules" 16 px / 20 px
  #dcdcdf, 12 px below.
- Terms field: background rgba(151,151,159,0.12), radius 8, padding 10 px 17 px.
- Rule row: 4 px side padding, 16 px top and bottom (first row 8 px top, last
  8 px bottom), border bottom 1 px rgba(150,151,159,0.2) except the last.
  Number 16 px / 20 px #81828a, 12 px before the text, text 16 px / 20 px
  #d4d5d8.
- Checkbox row: 16 px below the field, 20 px high, padding-left 32, 12 px gap.
  In the captured scrolled state the whole row is drawn at about 63 % opacity,
  making the fill and label read dimmer than their tokens. Box 20 x 20,
  radius 4. Unchecked background rgba(0,0,0,0.08), border 1 px
  rgba(151,151,159,0.8). Checked background #4452bb, no visible border, white
  check mark 13 px. Label 16 px / 20 px #d4d5d8.
- Scroll edges: the content has a 120 px black fade at the top once scrolled
  and at the bottom whenever more rules follow (the number column dims from
  #81828a to about #4f505a at the edge).
- Footer: 56 px high, padding 8 px 0, right aligned.
- Submit: 100 x 40, radius 8, border 1 px rgba(255,255,255,0.08), background
  #008545, text 16 px / 20 px weight 500 white. Hover #006c37. Disabled is the
  same green at opacity 0.5. Disabled hint (tooltip): "All required fields must
  be filled, including agreeing to the rules."
- Close: 40 x 40 at 32 px from the top and right, radius 8, X icon 20 px
  #96979e, hover background #121213.

### Endpoints and state

- `GET /guilds/:id/member-verification?with_guild=false` returns `version`,
  `description` and `form_fields` (TERMS fields carry `values`).
- `GET /guilds/:id/profile` fills the card: `name`, `icon_hash`,
  `online_count`, `member_count`, `description`, `custom_banner_hash`, `tag`,
  `badge`, `badge_hash`, `premium_tier`, `premium_subscription_count`.
- `PUT /guilds/:id/requests/@me` answers the form with
  `{ "version": ..., "form_fields": [ { ...field, "response": true } ] }`.
  Success clears `pending` on our member through `GUILD_MEMBER_UPDATE`.
- Our own member carries `pending: true` until the form is accepted; that is
  what shows the banner and replaces the composer.
- Only TERMS questions are filled in so far. TEXT_INPUT, PARAGRAPH and
  MULTIPLE_CHOICE questions still need their own inputs.
