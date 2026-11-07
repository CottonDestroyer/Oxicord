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