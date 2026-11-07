# Discord Web Client — UI/UX Reference (for Oxicord)

Captured via browser automation on 2026-11-06 against the live Discord web client
(`https://discord.com/channels/@me`). **Read-only observation; no messages were sent.**

This document is a structural/a11y-level reference for the Rust rewrite (Oxicord).
It describes layout, regions, controls and states, not pixel-perfect visuals.

## Top-level layout (in-server view)

Regions, left-to-right / top-to-bottom:

1. **Servers sidebar** (far left, vertical)
   - `navigation "Servers sidebar"` containing a `tree`.
   - Top item: **Direct Messages** (`treeitem`, level 1, selectable).
   - Then **Servers** group (`group "Servers"`) with one `treeitem` per server (level 1).
     - Each server item may show: unread badge, mention count badge, voice-call-active indicator, scheduled-event indicator.
     - Selected server is marked `active`/`selected`.
   - Bottom: user area is separate (see User bar).

2. **Channels sidebar** (second column)
   - Server name header (`heading` level 2, e.g. "Neverlose Roblox").
   - `heading "Channels"` (level 2) grouping channel list.
   - Channels grouped into categories (`heading` level 3, e.g. `-main`, `-utils`, `-chats`).
   - Channel items are `treeitem`s (text/voice/forum) with unread/mention state.

3. **Main content area**
   - **Channel header** (`region "Channel header"`):
     - Channel type label (e.g. "Text Channel").
     - Channel name (`heading` level 1, e.g. "Neverlose Roblox: status-🟡").
     - Action buttons: Threads, Notification Settings, Pinned Messages, Show/Hide Member List.
     - Search combobox ("Search <server>").
   - **Message list** (`main "<channel> (channel)"`):
     - Channel intro heading (level 2, e.g. "status-🟡 chat") + "Welcome to … / This is the beginning of this server."
     - `list "Messages in <channel>"` containing message groups.
     - Permission notice when user cannot send: "You do not have permission to send messages in this channel."
   - **Message input** is absent/locked in read-only channels; otherwise a text input at bottom.

4. **Member list** (right sidebar, toggleable)
   - `heading "Members list for <channel>"` (level 2) + `list "Members"`.
   - Role groups as `heading` level 3 with member counts (e.g. "dev — 3", "User — 59").
   - Each member `listitem`: avatar image with presence (e.g. "Do Not Disturb"), display name, optional server tag, optional activity ("Playing …").

5. **User bar** (bottom of servers/channels column)
   - Avatar, username, status.
   - Mute/Deafen switches, Input/Output options.
   - "User Settings" button.

## Direct Messages / Friends view

- Servers sidebar still present; DMs group at top with unread counts.
- Main area `main "Friends"`:
  - Heading "Friends" (level 1).
  - Tablist: **Online**, **All**, **Pending** (with badge count), **Add Friend**.
  - Searchbox "Search".
  - Friend list items:
    - Avatar + presence + activity ("Playing …").
    - Name, server tag badge, activity emoji/text.
    - Buttons: "Message <name>", "More options for <name>".
  - "Looking for accounts you've blocked or ignored? Go to settings" link.

## Common UI elements / states

- **Unread indicators:** bold channel/server names, unread badges, mention count badges (e.g. "7 mentions").
- **Voice activity:** server icons show "Voice call active" / "Screenshare active".
- **Scheduled events:** server icons show "Scheduled event".
- **Presence:** Do Not Disturb, Online, etc.; shown in avatar tooltip/alt text.
- **Activities:** "Playing <game>" with optional emoji.
- **Server tags:** displayed next to usernames (e.g. "Server Tag: ETHR").
- **Promo banner:** top banner with heading + "Claim Offer" + "Dismiss".
- **Top bar buttons:** Inbox, Help (support link).
- **ARIA landmarks used heavily:** banner, navigation (servers/channels/private/settings), main, complementary, region, tree, list, tablist, searchbox, combobox.

## Settings (user settings overlay)

Opened via "User Settings" button; URL stays `/channels/@me`, overlay with "Close".

Navigation `navigation "Settings pages"` grouped:
- **General:** Account (Account Info, Password & Security, Account Status, Family Center), Data & Privacy, Messaging Permissions, Notifications.
- **Billing:** Nitro, Server Boost, Subscriptions, Gift Inventory, Billing.
- **Experience:** Voice & Video, Appearance, Accessibility, System, Language & Time.
- **Games & Apps:** Activity Privacy, Connected Apps.
- **Other:** Developer, Log Out.

Footer: version string (click to copy), Privacy Policy, Terms of Service, More.
Breadcrumb shows current page; content pane shows page heading (e.g. "Account Info").

## Message area details

- Channel header buttons: Threads, Notification Settings, Pinned Messages, Show/Hide Member List, Search.
- Message list shows intro header + "beginning of this server" for new channels.
- Permission-gated channels show "You do not have permission to send messages in this channel."
- Member list role groups show counts; members include presence + activity.

## Notes for Oxicord (Rust) implementation

- Mirror the three-column layout + toggleable member list.
- Preserve ARIA roles/labels for accessibility and testability.
- Unread/mention/voice indicators are first-class state in the sidebar models.
- Settings should be an overlay with categorized navigation and a content pane.
- Friends view needs tabbed filtering (Online/All/Pending/Add Friend) with search.
- Member list should group by role with counts and show presence + activity.
- Channel header should expose Threads/Pins/Notifications/Search/Member-list toggle.
- Keep permission notices inline in the message area.
- Activities and server tags are part of member rendering.

## Direct Message conversation layout

Observed on a 1:1 DM ("my cute little bf").

- Channel header (`region "Channel header"`):
  - Label "Direct Message".
  - Partner display name + username (heading level 1).
  - Optional activity line (emoji + text, e.g. "Harcèlement Informatique").
  - Action buttons: **Start Voice Call**, **Start Video Call**, Pinned Messages, Add to DM.
  - "Show User Profile" button (may be disabled).
  - Search combobox ("Search <partner>").
- Main area `main "<partner> (channel)"`:
  - Heading (level 2) "<partner> chat".
  - `list "Messages in <partner>"`.
  - Message input textbox "Message @<partner>" at bottom.
- No member list; right side absent.

## Voice channel (in-server) layout

Observed in "Low iq gaymers" → voice channel **General** (joined for observation).

- Channel list entry:
  - Voice channel item labelled "Voice (Limited) General" with an "Open Chat" affordance and a live timer (e.g. "4:03:25").
  - "Set a channel status" link.
- When connected, a **Voice Details** panel appears:
  - "Voice Connected" + "General / <server>".
  - Controls: Camera (Unavailable), Share Your Screen, Start An Activity.
  - Current user row with presence + "In voice"; Mute/Deafen/Unmute controls.
  - Other users in voice listed with presence (e.g. "In voice (General)", muted indicators).
- Voice state is reflected on the server icon ("Voice call active") and in the member list ("In voice (General)").
- Main content area stays on the last text channel; voice connection is a global overlay state, not a page navigation.
- "Voice (Limited)" indicates a limited-capacity voice channel variant.

## Observed voice-related states

- User bar shows "In voice" status and Mute/Deafen switches while connected.
- Member list marks users "In voice (General)" and shows muted/deafened icons.
- Activities (e.g. "Playing …") continue to display for voice participants.

## Notes for Oxicord (Rust) implementation (additions)

- DM view needs voice/video call launch buttons in the channel header.
- Voice connection is global state: keep text channel view mounted while showing a Voice Details panel.
- Voice channel list items need an "Open Chat" action and live duration timer.
- Limited voice channels should be visually distinguished ("Voice (Limited)").
- Member list must reflect in-voice presence and mute/deafen per user.

## Direct Message voice call

Observed by starting a voice call to "Cotton" (then disconnected).

- DM header already exposes **Start Voice Call** / **Start Video Call** buttons.
- When connected, a **Voice Details** panel appears (same as in-server voice):
  - "Voice Connected" + partner name.
  - Controls: Mute/Unmute, Deafen, Share Your Screen, Start An Activity.
  - Disconnect button to hang up.
- DM voice call is a global voice state; the DM chat view remains mounted.

## Notes for Oxicord (Rust) implementation (additions)

- DM voice calls reuse the same Voice Details panel as server voice channels.
- Header must switch from "Start Voice Call" to connected-state controls.

This file is a reference snapshot; Discord's UI changes over time. Re-capture as needed.


## User Profile Popout / Full Profile

Observed in the logged-in browser (read-only, no posting).

Mini profile popout labels:
- "Manage profile and status"
- Custom status prompt: "What fictional world would you live in?"
- Avatar aria-label: "View Full Profile"
- "Server Tag: LWIQ"
- "Edit Profile"
- "Your Status: Do Not Disturb"
- "Badges" / "150 Orbs" / "new"
- "Switch Accounts"
- "Copy User ID"

Full profile editor ("Your full profile") sections:
- "Profile styles panel"
- "Select profile"
- "Main Profile"
- "Nameplate"
- "Avatar & Decoration"
- "Banner Color"
- "Profile Effect & Frame"
- "Level up your look, only with Nitro"
- "Preview Nitro"
- "Display Name Style"
- "Profile Theme"
- "Profile Banner"
- "Animated Avatar"

Oxicord profile modal already covers: avatar, display name, username, pronouns, activity status, bio, presence, badges, mutual friends count, mutual servers list, joined date, boost date, linked accounts, banner, accent color, copy user ID/avatar/banner URLs.

## User Profile Note

Observed in Cotton's full profile.

- Section label: "Note (only visible to you)".
- Input placeholder: "Note".
- Stored value is per-viewer and fetched separately from `/users/@me/notes/{user_id}`.
- Response shape: `{"user_id":..., "note_user_id":..., "note":"..."}`.

Oxicord now fetches this note while loading a profile and shows it as "Note: ..." in the profile modal.

## Message context menu

Observed on a normal text message in a DM.

Menu items:
- Add Reaction
- Edit Message / Reply / Forward
- Copy Text
- Pin Message
- Bookmark Message
- Create Reminder
- Apps
- Mark Unread
- Copy Message Link
- Speak Message
- Delete Message
- Copy Message ID

Bookmarking calls `PUT /users/@me/saved-messages/{channel}/{message}` with body `{"source":"context_menu"}`.
