# Parallel build plan

How a team of coding agents (DeepSeek V4.1 Flash, 8 to 12 running at once,
no vision, no network) finishes Oxicord: every Discord system, matched to the
live client as closely as a non-sighted agent can manage. The list of work is
`docs/roadmap.md`. This file says who does what, in which order, how, and
how the work comes back together.

Written 2026-11-12, at commit 17cc381 (558 tests passing).

Contents:

1. The rules every agent follows
2. The project in one page
3. Roles
4. Lanes, waves and the order of work
5. Worktree setup
6. The task loop
7. Getting measurements without vision
8. Testing without vision
9. Hot files and conflicts
10. Merging
11. Capture and verify queues
12. Lane cards
13. Task brief template
14. Known traps
15. What the agents must never do

---

## 1. The rules every agent follows

Paste this block at the top of every agent's instructions.

```
You are working on Oxicord, a Discord client in Rust and gpui, in a git
worktree of C:\Users\bumbi\Documents\Rust\oxicord. Read docs/plan.md sections
1, 2, 6, 7, 8, 9, 14 and 15 before your first task.

Hard rules:
1. One roadmap item per task, one commit per item. The commit message is the
   item id and a short summary, e.g. "CH-04 Render headings, subtext and
   lists". No body unless something needs explaining. No Co-Authored-By or
   any other attribution line, ever, whatever your tools add by default.
2. Never write the em dash character (U+2014) anywhere: code, comments,
   docs, commit messages. A Rust string that must show one on screen uses
   "\u{2014}". Run the em dash check in section 10 before committing.
3. Never run cargo fmt or rustfmt. Match the style around your edit.
4. No network. Do not run the app against Discord, do not use a token, do not
   read or copy .env, do not call any Discord URL from a test. You have no
   token and need none.
5. Writes to Discord (anything that changes the account, a server, a
   message, a relationship) are built as a pure function that returns the
   request body, unit tested, and wired to the UI. They are never executed.
6. Do not change crates/oxicord-gateway identify, the client properties,
   headers or super properties. See docs/protocol-notes.md "Client
   identity". A token was revoked twice over this.
7. Touch only the files your task needs. Hot files (section 9) get small,
   additive edits in your own area block only.
8. Every measured number in the roadmap item is the spec. If the item has
   none, get numbers from the CSS (section 7) and write where they came from
   in the tick note. Never guess silently.
9. Before you hand off: cargo build, cargo test --workspace, no new warnings
   in files you touched, the em dash check, the roadmap item ticked with the
   note format from section 6, rebased on main.
10. If you are stuck for more than one attempt on the same error, or the
    task needs a live capture, a new dependency, or a hot file change outside
    your area, stop and write docs/agent-notes/<ID>.md (section 11), then
    hand off what you have as a draft or move to the next item in your
    lane.
```

## 2. The project in one page

Workspace (`Cargo.toml`, edition 2024, nightly toolchain):

| crate | what it is | lines |
|---|---|---|
| `oxicord-model` | serde types for every Discord object and gateway event (`gateway.rs` has `DispatchEvent`) | 7.4k |
| `oxicord-api` | REST client, one file per area in `src/routes/`, rate limits, error parsing | 3.9k |
| `oxicord-gateway` | websocket, zlib-stream, heartbeat, resume, identify | 1.4k |
| `oxicord-store` | in memory state, `Store::apply(DispatchEvent) -> Changes`, permissions, read states, member lists, threads | 3.6k |
| `oxicord-ui` | every view (gpui), the `Session` entity, the `Backend` channel types | 45k |
| `oxicord-voice` | voice gateway v8, UDP, Opus, AES-GCM / XChaCha20, DAVE, capture and playback (cpal) | 2.4k |
| `oxicord-markdown` | Discord markdown parser and highlighter | 1.2k |
| `oxicord-media` | CDN urls, image fetch, APNG, type sniffing | 0.4k |
| `oxicord-settings` | the account settings protobuf (`/users/@me/settings-proto/1`), `Key` and `Value` | 0.8k |
| `oxicord` | the binary: `bridge.rs` runs commands on tokio, login, hotkeys, assets | 2.6k |
| `oxicord-browser-login` | the hosted login window | 0.4k |

How data moves:

- The UI never touches the network. A view sends a `Command` through
  `Backend` (`crates/oxicord-ui/src/backend.rs`). The binary's `bridge.rs`
  runs it with `oxicord-api` or the gateway and answers with an `Event`.
  `AccountRequest` / `AdminRequest` (`account.rs`, `admin.rs`) are the area
  enums for account and admin work; after F-01 every area has one.
- Gateway events arrive as `Event::Gateway`, go to `Session::handle`
  (`crates/oxicord-ui/src/session.rs`), which calls `Store::apply` and emits
  `Changes`. Events the model does not type arrive as
  `DispatchEvent::Unknown { name, data }` and are matched by name in
  `Session::handle` (after F-04, in per area handler tables).
- Views are gpui entities. They `observe` or `subscribe` to the `Session`.
  Opening a modal is done by emitting a session event (`OpenServerSettings`,
  `OpenPicker`, ...) that `MainView` (`main_view.rs`) subscribes to and turns
  into an overlay.
- Account preferences go through `session.change_prefs(&backend, &[(Key,
  Value)], cx)` (writes the settings proto), local ones through
  `session.change_settings(&backend, |s| ..., cx)` (writes
  `%APPDATA%\oxicord\settings.json`).
- Look and feel: `theme.rs` (Midnight palette constants: BG_PANEL 0a0a0c,
  BG_CARD 17181b, BG_ACTIVE 242426, BG_MENU 121214, BG_HOVER 121213, BG_CHAT
  000000, ACCENT 5865f2, TEXT_BRIGHT dcdcdf, TEXT d4d5d8, TEXT_MUTED 96979e,
  TEXT_FAINT 81828a, BORDER 1e1e20), `icons::icon(name, size, color)` (loads
  `discord-assets/icons/<name>.svg`), multi colour pictures with
  `img("folder/file.svg")` from `discord-assets/`, the font is gg sans
  (`discord-assets/fonts`). Shared bits today: `popup.rs`, `context_menu.rs`,
  `tip.rs`, `settings_view/widgets.rs`, `scroll.rs`; after F-07, `kit/`.
- Tests: unit tests next to the code, store reducer tests in
  `crates/oxicord-store/tests/reducer.rs`, JSON fixtures in `fixtures/`
  (git ignored, see section 5). After F-05, headless UI tests in
  `crates/oxicord-ui/tests/ui/`.

Reading list for a new agent, in order: this file, the roadmap item, the
roadmap's "How to use this file", `docs/protocol-notes.md`, the matching
section of `docs/discord-reference.md` (read it from the main checkout, it is
git ignored), then the code around the change via codegraph or ripgrep.

## 3. Roles

- **Owner**: the person who owns the repo and the Discord account. Decides
  scope questions, approves new dependencies, does the first real run of
  anything that touches the network.
- **Lead**: one session (the owner, or a Claude session with vision and the
  logged in Playwright browser). Hands out tasks from the lane queues,
  merges (section 10), answers `docs/agent-notes/`, fills capture requests
  (section 11) and does the visual verify passes. There is exactly one lead.
- **Lane agents**: DeepSeek agents, one per lane, each in its own long lived
  worktree. An agent works its lane queue top to bottom, one item at a time,
  and hands each finished item to the lead.

## 4. Lanes, waves and the order of work

Eleven lanes. With 8 agents, merge PROF into APPS, CHAN into SERV, and
VOICE-B into VOICE-A (their queues concatenate in that order).

| lane | scope | roadmap items |
|---|---|---|
| FOUND | foundations | F-01 to F-11 |
| CHAT | messages, composer, markdown, embeds, attachments, pickers, search | CH-01 to CH-24, LO-02 to LO-07 |
| CHAN | channel list rows, threads, forums, media and directory channels, channel menus | TH-01 to TH-11 |
| SERV | rail, server header and menu, discovery, join flows, member list | SV-01 to SV-11, LO-01, LO-08 to LO-12 |
| SOCIAL | friends, DMs, group DMs, inbox, notifications, events leftovers | P8-01, DM-01 to DM-07, LO-13, LO-15, LO-24 |
| PROF | profiles, collectibles, Nitro, Shop, Quests, gifts | PR-01 to PR-10, LO-16, LO-17 |
| APPS | commands, launcher, app directory, components, OAuth2 | AP-01 to AP-08, LO-18 |
| SET | user settings | P8-02 to P8-15, ST-01 to ST-08, LO-19 |
| ADMIN | server and channel settings | AD-01 to AD-18, LO-20 to LO-23 |
| VOICE-A | audio, calls, stage, soundboard, voice UI | VC-01 to VC-03, VC-07 to VC-14 |
| VOICE-B | video, camera, Go Live, clips | VC-04 to VC-06, PL-12 |
| PLAT | sign in, accounts, tray, keybinds, locale, accessibility, presence, gateway robustness | PL-01 to PL-11, PL-13, PL-14 |

LO-14 (gateway capabilities) is an owner decision and goes to no agent.

### Wave 0: unblock (about one agent day)

Only these run. Everything else waits for F-04 because F-01 to F-04 move the
hot files.

| agent | queue |
|---|---|
| A (FOUND) | F-01, F-02, F-03, F-04, strictly in order, each merged before the next starts |
| B | F-10 group 1 (calls, members, stage, soundboard, streams, voice effects) |
| C | F-10 group 2 (saved messages, notification center, guild settings, acks, polls, presences, sessions, thread members, join requests, event exceptions) |
| D | F-10 group 3 (integrations, webhooks, friend suggestions, game relationships, gifts, entitlements, authenticators, oauth2, required action, activities, quests, summaries, boosts, automod raid, command index, prune, channel updates) |
| E | F-11, then F-08 |
| F | F-09 |

B, C and D only add files and tests in `oxicord-model` (new modules plus
one `pub use` and one `DispatchEvent::from_name` arm each, which the lead
merges by hand if they collide). Meanwhile the lead fills the first capture
requests: the server settings pages for ADMIN (AD-01) and the live menus
for F-07.

### Wave 1: kit and harness (about one to two agent days)

| agent | queue |
|---|---|
| A (FOUND) | F-05, then F-06 |
| E | F-07 (the kit, including P8-16 the menu restyle) |
| CHAT | CH-04 (markdown crate only), CH-16 model and fixtures part |
| CHAN | TH-01 API and model part |
| APPS | AP-01 API, model and store part |
| VOICE-A | VC-01 device enumeration in `oxicord-voice` (no UI) |
| VOICE-B | VC-04 RTP depacketizers and their tests (no UI) |
| PLAT | PL-02 remote auth crypto and protocol in a new module (no UI), PL-13 store parts |

Rule for Wave 1: lane agents only touch their crate level code (model, api,
store, voice, markdown) and new files. No view work until F-05 and F-07 are
merged, because every view should be built on the kit and checked with the
harness.

### Wave 2 onward: lanes run their queues

Every lane works its card in section 12 top to bottom. The lead merges as
items come in, at least every couple of hours, so lanes rebase on fresh
main often. A lane that empties its queue takes the oldest unclaimed item
from the "any lane" leftovers or helps the lane with the longest queue
(ask the lead first).

Cross lane dependencies (the dependent item waits until the other is
merged):

- Everything with menus waits for F-07 (CH-01, TH-10, SV-06, the Apps
  submenus).
- P8-07 (SET, Voice & Video page) waits for VC-01 (VOICE-A).
- DM-06 and VC-07 wait for F-09 (sounds).
- CH-06 voice message playback uses the `oxicord-voice` output; ask VOICE-A
  for the API if it is not there, do not write a second audio path.
- CH-07 and CH-06 inline video wait for VC-04 (VOICE-B); until then they open
  the browser.
- SV-04 is used by PL-05; SV-03 is used by LO-15.
- PR-06 and PR-07 are used by LO-16 and PR-10.
- AP-05 is used by AP-04.
- ST-07 needs PL-06.

## 5. Worktree setup

The repo has git ignored inputs every agent needs: `reference/` (the web
client bundle), `fixtures/` (JSON used by tests via `include_str!`),
`docs/discord-reference.md` and `docs/protocol-notes.md`. A plain worktree
lacks them, so link them in. One worktree per lane, reused for every task in
that lane.

PowerShell, run by the lead once per lane (`<lane>` in lower case):

```powershell
$main = 'C:\Users\bumbi\Documents\Rust\oxicord'
$wt   = "C:\Users\bumbi\Documents\Rust\oxicord-wt\<lane>"
git -C $main worktree add --detach $wt main
cmd /c mklink /J "$wt\reference" "$main\reference"
cmd /c mklink /J "$wt\fixtures"  "$main\fixtures"
# docs/discord-reference.md and docs/protocol-notes.md: agents read them
# from $main\docs, read only. Do not copy them into the worktree.
```

Each worktree keeps its own `target/` (the default). The first build of a
worktree is slow; later ones are incremental. Do not share one
`CARGO_TARGET_DIR` between agents: cargo holds a lock on it and the agents
would wait on each other. If disk runs short, delete the `target/` of a lane
that is idle.

Per task, inside the lane worktree:

```bash
git switch --detach main          # main is shared by all worktrees, always fresh
git switch -c agent/<ID>
# ... work ...
git rebase main                   # before handing off
git switch --detach               # frees the branch so the lead can merge it
```

## 6. The task loop

For each item:

1. Read the item in `docs/roadmap.md` in full, including the notes of any
   ticked item it builds on.
2. Look up the facts: endpoints in `reference/notes/endpoints.txt`, event
   names in `reference/notes/gateway-events.txt`, field names by searching
   `reference/web/modules-main` (section 7), the live captures in the main
   checkout's `docs/discord-reference.md` and `docs/captures/`.
3. Find the code to change with codegraph (`codegraph explore "<names>"`) or
   ripgrep. Read the area before writing.
4. Write the data side first: model types with a JSON test, the API route
   with a body builder test, store handling with a reducer test.
5. Then the view, built from the kit (`kit/` after F-07), with every
   measured number as a named constant or an inline literal next to a
   comment saying where it came from only if that is not obvious.
6. Add a headless UI test (section 8) that asserts the key boxes of the new
   view and that the right `Command` is sent on click.
7. `cargo build`, `cargo test --workspace`. Fix new warnings in touched
   files. Run the em dash check.
8. Tick the item in `docs/roadmap.md`: change `- [ ]` to `- [x]` and add,
   indented under it:

   ```
         Done: <what was built, in two to five lines>.
         Numbers from: <roadmap | captures/<ID>.md | CSS <file or class> |
         bundle module <id>>.
         Tests: <names of the main tests>.
         Unverified: <what nobody has seen yet, always present for UI>.
         Verify: <what the lead should compare live, one line per screen>.
         Remaining: <differences you know of, or "none known">.
   ```

9. Commit (one commit, message `<ID> <summary>`), rebase on main, detach,
   tell the lead: `READY <ID> agent/<ID> <short sha>` plus anything the lead
   must know.

An item too big for one commit is split in place in the roadmap into
`<ID>a`, `<ID>b`, ... (first one needing only data work), each its own
commit. Never leave a half feature on main: hide unfinished UI behind its
entry point until the last part lands.

## 7. Getting measurements without vision

Priority order. Use the first source that has the number.

1. **The roadmap item and the notes of ticked neighbours.** Phase 8 notes
   were measured on the live client at 1920x1080 and are exact.
2. **`docs/captures/<ID>.md` and the main checkout's
   `docs/discord-reference.md`.** Live measurements made by the lead.
3. **The CSS in `reference/web/assets/*.css`.** Class names are hashed
   (`messageContent_c19a55`). Search by the readable prefix:

   ```bash
   node scripts/ref/class.js messageContent      # after F-11
   node scripts/ref/css-rule.js 'embedFull_'
   node scripts/ref/token.js theme-midnight 'background-mod'
   # before F-11, by hand:
   grep -o '"messageContent[^"]*": "[^"]*"' reference/web/classes.json
   grep -n 'messageContent_c19a55' reference/web/assets/*.css
   ```

   Colours are often `var(--something)`. Resolve them in the
   `.theme-midnight` block (Midnight is the theme we copy) or the root
   block. `oklab(...)` values convert to hex; write the hex in Rust.
   Common text roles: header primary dcdcdf, text normal d4d5d8, text muted
   96979e, channels default 81828a, interactive muted 4e5058.
4. **The JS component.** When the CSS rule is not obvious, find the module
   that renders the thing: search the English text
   (`node scripts/ref/find-string.js "Create Message"` after F-11, or ripgrep
   the text in `reference/web/modules-main`), follow the intl hash to the
   component, read which classes and text variants it uses
   (`"body-md/medium"` means 16px medium, `"text-sm/normal"` 14px normal,
   `"heading-lg/semibold"` 20px semibold, `"text-xs"` 12px), then the CSS for
   those classes. `node reference/tools/show.js <module id>` prettifies a
   module.
5. **Known scales.** Text: 10, 12, 14, 16, 18, 20, 24, 32 with line heights
   (12/16, 14/18, 16/20, 20/24, 24/30). Spacing: 4, 8, 12, 16, 20, 24, 32.
   Radius: 4, 8, 12, 16. Row heights: menu rows 36 (after F-07), channel
   rows 32 in 34, DM rows 42 in 44, member rows 42 in 44, buttons 32 / 38 /
   44. Modals: 440 small, 480 standard, 600 large.
6. **Nothing found.** Build with the kit defaults, write `Numbers from:
   kit defaults (no source found)` in the tick note, and file a capture
   request (section 11) so the lead can measure it later.

Lazy chunks: `reference/web` is a login page scrape. Settings (user and
server), many modals and the shop are lazy chunks that are not in it. For
those, captures by the lead are the source; until a capture exists, use the
bundle's strings for wording and the kit for layout.

Icons: copy from the bundle (F-08 script) instead of drawing or guessing.
Name the file after what our code calls it.

## 8. Testing without vision

What "done" means when nobody can look:

- **Model**: a test that parses a JSON sample with every field the bundle
  reads, plus missing optional fields and unknown extra fields. Write the
  sample by hand from the field names in the bundle; mark it
  `// shape from module <id>`.
- **API**: every route is `fn <name>_body(...) -> serde_json::Value` (or a
  query builder) plus the call. The test asserts the exact JSON. See
  `friend_request_body` in `crates/oxicord-api/src/routes/relationships.rs`
  and `password_body` in `routes/account.rs`.
- **Store**: a reducer test in `crates/oxicord-store/tests/reducer.rs` style:
  apply events, assert the state and the `Changes` flags.
- **Views (after F-05)**: a headless test that builds the view from the test
  world, renders at 1920x1080 (or the view's own size), and asserts:
  - every element id the item names exists,
  - the key boxes with `assert_box(id, x, y, w, h, 1.0)` using the numbers
    from the item, relative to the view origin,
  - clicking the action sends the expected `Command` (`sent_commands()`) and
    never anything else,
  - empty, loading, error and disabled states render (one test each).
  Give every element you want to assert an `.id(...)`. Ids are cheap.
- **Writes**: the test asserts the request is built; nothing in tests can
  reach the network (the fake backend has no API client).
- **Voice and video**: codec and packet code gets byte level tests from the
  RFC examples or packets the test builds itself. Devices are behind a trait
  so tests use a fake device.

No screenshots, no drive.ps1, no Playwright for agents. The lead does those
in the verify passes.

## 9. Hot files and conflicts

Files many lanes touch. Rules:

| file | rule |
|---|---|
| `crates/oxicord-ui/src/lib.rs` | add your `mod` line in alphabetical order, nothing else |
| `crates/oxicord-ui/src/backend/<area>.rs` (after F-01) | only your area's file |
| `crates/oxicord/src/bridge/<area>.rs` (after F-02) | only your area's file |
| `crates/oxicord-ui/src/main_view/<area>.rs` (after F-03) | only your area's file; `MainView::new` gets one line |
| `crates/oxicord-ui/src/session/<area>.rs` (after F-04) | only your area's file |
| `crates/oxicord-ui/src/theme.rs` | new constants only, at the end, in a block headed by your lane name; reuse existing ones first |
| `crates/oxicord-model/src/lib.rs`, `gateway.rs` | add `pub use` and `from_name` arms only |
| `crates/oxicord-ui/src/kit/*` (after F-07) | FOUND owns it; other lanes ask the lead for a new widget instead of editing it, or add a local widget and note it |
| `Cargo.toml` (any) | new dependencies need the lead's approval in `docs/agent-notes/<ID>.md` first |
| `docs/roadmap.md` | only your item's lines (tick and note); split items in place |
| `discord-assets/` | new files only, never rename or edit existing ones |

Area names for F-01 to F-04: `chat`, `channels`, `servers`, `social`,
`profile`, `apps`, `account` (SET), `admin`, `voice`, `platform`. A lane owns
the area of the same name; VOICE-A and VOICE-B share `voice` and coordinate
through the lead.

Files owned by one lane (others do not edit them; ask in agent notes):

- CHAT: `chat/` (except `chat/view/commands.rs`, `chat/view/components.rs`,
  `chat/command_form.rs` which are APPS), `oxicord-markdown`, `emoji_panel.rs`,
  `forward_dialog.rs`, `create_poll_dialog.rs`, `side_panel.rs` (search and
  pins parts), `search_query.rs`, `media.rs` viewer parts.
- CHAN: `forum.rs`, thread parts of `side_panel.rs`, the channel list module
  that F-03 splits out of `main_view.rs`, `edit_channel_dialog.rs`,
  `create_channel_dialog.rs`.
- SERV: `rail.rs`, `header_menu.rs`, `member_list.rs`, `join_server.rs`,
  `invite_dialog.rs`, `create_server_dialog.rs`, `rules_screening.rs`,
  `server_prompts.rs`, `onboarding_dialog.rs`, `switcher.rs`.
- SOCIAL: `friends.rs`, `dm_picker.rs`, `inbox.rs`, `message_requests.rs`,
  `notify.rs`, `events_dialog.rs`, `create_event_dialog.rs`,
  `friend_nick_dialog.rs`, `home_pages.rs`.
- PROF: `profile/`, `edit_profile_modal.rs`, `decoration_dialog.rs`,
  `effect_dialog.rs`, `frame_dialog.rs`, `nameplate_dialog.rs`,
  `widget_picker.rs`, `favorite_game_editor.rs`, `games_i_like_editor.rs`,
  `status_dialog.rs`, `gift_dialog.rs`.
- APPS: `interaction_modal.rs`, `chat/view/commands.rs`,
  `chat/view/components.rs`, `chat/command_form.rs`, new `apps/`.
- SET: `settings_view/`, `password_dialog.rs`, `totp_dialog.rs`, `totp.rs`,
  `danger_dialog.rs`, `account.rs`, `settings.rs`.
- ADMIN: `server_settings.rs`, `channel_settings.rs`,
  `channel_permissions.rs`, `admin.rs`, `ban_dialog.rs`, `kick_dialog.rs`,
  `timeout_dialog.rs`, `roles_dialog.rs`, `nickname_dialog.rs`,
  `notification_dialog.rs`.
- VOICE-A / VOICE-B: `oxicord-voice`, `main_view/call_window.rs`, the voice
  parts F-03 splits out, `talk_key.rs`, `crates/oxicord/src/voice.rs`.
- PLAT: `oxicord-gateway`, `oxicord-browser-login`, `crates/oxicord/src/`
  (`main.rs`, `credentials.rs`, `hotkey.rs`, `settings.rs`, `assets.rs`),
  `login.rs`, `app.rs`, `title_bar.rs`, `keys.rs`, `dev_control.rs`.
- FOUND: `kit/`, `scroll.rs`, `popup.rs`, `context_menu.rs`, `menus.rs`,
  `tip.rs`, `icons.rs`, `theme.rs` (structure), `tests/ui/world.rs`.

Big shared views (`chat/view.rs`, `server_settings.rs`) should be split by
their owning lane early, the same way F-03 splits `main_view.rs`, before the
lane piles features into them.

## 10. Merging

The lead merges one branch at a time, oldest READY first, in the main
checkout (which stays on `main`):

```bash
cd /c/Users/bumbi/Documents/Rust/oxicord
git switch main
git rebase main agent/<ID>            # replays the branch on main
cargo build 2>&1 | tail -3
cargo test --workspace 2>&1 | grep -E "test result|FAILED|panicked"
# em dash check over the branch's changes
git diff main...agent/<ID> | grep -n $'\xe2\x80\x94' && echo "EM DASH FOUND"
# attribution check
git log -1 --format=%B agent/<ID> | grep -i "co-authored" && echo "TRAILER FOUND"
git switch main
git merge --ff-only agent/<ID>
git branch -d agent/<ID>
```

Reject (send back with a note) when: the build or a test fails, an em dash
or trailer is present, the roadmap tick note is missing its fields, a hot
file rule is broken, a write is executed instead of built, or files outside
the task changed without reason. Small conflicts in `lib.rs`, `theme.rs`,
`roadmap.md` and model `from_name` arms the lead resolves directly.

Every few merges, and always before a verify pass, the lead runs the whole
suite on a clean build.

## 11. Capture and verify queues

**Agent notes**: `docs/agent-notes/<ID>.md`, one file per item, so there are
no merge conflicts. An agent writes one when it needs a decision, a new
dependency, a hot file change outside its area, or a live capture. Format:

```
# <ID> <title>
Kind: capture | decision | dependency | blocked
Asked by: <lane>
Question: <one paragraph>
For a capture: page or route, how to open it (read only clicks), which
elements, which numbers (box, font size and weight, colours, radius, gaps).
Answer:
```

The lead answers under `Answer:` and commits the file; the agent reads it
after its next rebase. Captures go to `docs/captures/<ID>.md` (tracked) with
numbers only, no account names, emails, ids or message text.

Captures the lead should do in Wave 0, because whole lanes wait on them:

- Server settings: the sidebar and every page in AD-01 to AD-18.
- User settings pages P8-03 to P8-15 (the Account page is partly done:
  window 1400x936 at 260,72 on a 1920x1080 viewport, radius 12, 1px
  `rgba(151,151,159,0.2)` border; sidebar items 220 wide, 36 tall, 4 apart,
  radius 8, selected fill `rgba(151,151,159,0.2)`, text 16/16 at x 313 with
  the 20px glyph; Account sub entries 20 tall every 36 at x 313 with a
  vertical line at x 295; group headings 14/18 medium muted; page titles
  24/32 medium at x 739, field labels 16/20 medium, "Edit" buttons 100 by
  40 secondary, "Reveal" 16/20 medium link colour).
- The message, user, server, channel and plus menus (F-07).
- The New Message modal (P8-01).
- The Shop, Nitro and Quests pages (PR-06 to PR-08).
- The App launcher and App Directory (AP-02, AP-04).
- Voice: the connected panel, the call view, the soundboard picker, the Go
  Live modal (in a test server call).

**Verify passes**: after a batch of merged UI items, the lead runs the debug
app (`scripts/drive.ps1`, window 1936x1088 for a 1920x1080 client area) and
the live client in Playwright at 1920x1080 on the same account and server,
compares each item's `Verify:` lines, fixes small things directly or files
`LO-` items in the roadmap for the lane, and edits the tick note's
`Unverified:` line. Restore `%APPDATA%\oxicord\settings.json` after each
run and delete screenshots afterwards.

**Owner runs**: anything that needs the network for real (PL-01 email sign
in, PL-02 QR sign in, VC-04 to VC-06 media, VC-07 calls) gets one owner run
after merge, on a test server or a spare account where possible.

## 12. Lane cards

Each card: what the lane owns, its queue in order, and lane specific notes.
Items marked (data) touch no view and may start in Wave 1.

### CHAT

Queue: CH-04 (data, markdown crate), CH-16 (model and fixtures first), CH-01,
CH-02, CH-03, CH-18, CH-15, CH-05, CH-06, CH-07, CH-08, CH-17, CH-22, CH-23,
CH-13, CH-14, CH-11, CH-12, CH-09, CH-10, CH-19, CH-20, CH-21, CH-24, LO-02,
LO-03, LO-04, LO-05, LO-06, LO-07.

Notes:
- Split `chat/view.rs` (3k lines) into `chat/view/rows.rs`,
  `chat/view/composer.rs`, `chat/view/header.rs` before CH-01, as one commit
  `CHAT-split`, no behaviour change.
- The cozy row numbers: padding 2/24/2/72, avatar 40 at x 16, group top
  margin 17, name 16/20 medium, time 12 medium 73747d, text 16/22 d4d5d8,
  hover time 11 medium 81828a in a 56 gutter, reactions 30 tall radius 8.
- Message types and flags are in `oxicord-model/src/message.rs`. Add every
  missing `MessageType` variant with its number from the bundle.
- `markdown::preview` is used by reply lines and the inbox; keep its output
  stable when changing the parser.

### CHAN

Queue: TH-01 (data first), TH-10, TH-09, TH-08, TH-02, TH-03, TH-04, TH-05,
TH-06, TH-07, TH-11.

Notes:
- The channel list lives in `main_view.rs` today; F-03 moves it to
  `main_view/servers.rs` or its own module. CHAN owns channel rows, SERV owns
  the header, banner and rail. Agree the boundary in the F-03 commit.
- Thread data is in `oxicord-store/src/threads.rs`.
- Channel row numbers: 34 slot, 32 link, padding 4/8, glyph 20, gap 8, name
  16 medium, idle 7a7b83, unread name dcdcdf, selected all dcdcdf on
  `rgba(151,151,159,0.24)`; category row 40 (pt 16, h 24), 14 medium, the
  12px chevron after the text.

### SERV

Queue: SV-06, SV-01, SV-02, SV-04, SV-05, SV-03, SV-07, SV-08, SV-09, SV-10,
SV-11, LO-01, LO-08, LO-09, LO-10, LO-11, LO-12.

Notes:
- Rail: 72 wide, icons 40, pitch 48, separator 32x1, pill 8 / 20 hover / 40
  selected.
- Member list: 264 wide, rows 42 in 44, offline 30% opacity until hover,
  group header 14 medium sentence case "Name \u{2014} N" (use the escape).
- Folder and order writes go through the settings proto (`guild_folders`,
  field 14), built only.

### SOCIAL

Queue: P8-01, DM-04, DM-03, DM-01, DM-02, DM-05, DM-07, DM-06, LO-13, LO-15,
LO-24.

Notes:
- P8-01 numbers are in the roadmap note of the ticked group DM item: 480
  wide centred modal, title 20, subtitle "Group DMs can have up to 10
  members.", search hint "Add friends or server members to group DMs.",
  empty "No results found." / "Search your friends or people from your
  servers.", Cancel and Create Message. Keep `dm_picker.rs` as the create
  flow behind it.
- Friends page numbers are in the Friends page and Add Friend notes.
- The account has no friends; the test world must have some.

### PROF

Queue: PR-01, PR-04, PR-05, PR-03, PR-02, PR-10, PR-06, PR-07, PR-08, PR-09,
LO-16, LO-17.

Notes:
- Profile popout and full profile are documented in docs/discord-reference.md
  ("Profile popout", "Edit Profile modal") and in the profile cards memory
  of earlier sessions; the code is `profile/`.
- APNG decoding exists in `oxicord-media/src/apng.rs`; reuse it for
  decorations.
- Shop, Nitro and Quests are read only; every buy, claim or enrol is built
  and tested, then opens the browser.

### APPS

Queue: AP-01 (data first), AP-03, AP-02, AP-06, AP-07, AP-05, AP-04, AP-08,
LO-18.

Notes:
- The interaction path (nonce, INTERACTION_SUCCESS / FAILURE, modals,
  autocomplete) is done; reuse it. Never press Run in any manual test.
- Command index endpoints are in endpoints.txt as
  `APPLICATION_COMMAND_INDEX_*`.

### SET

Queue: P8-02, P8-03, ST-02, ST-05, P8-04, P8-05, P8-06, P8-08, P8-09, P8-10,
P8-11, P8-12, P8-13, P8-14, P8-07 (after VC-01), P8-15, ST-01, ST-03, ST-04,
ST-06, ST-07 (after PL-06), ST-08, LO-19.

Notes:
- The live settings window at 1920x1080 is 1400x936 at (260, 72); see the
  capture list in section 11 for the sidebar numbers. The Account page is
  one scrolling page whose sidebar sub entries scroll to sections.
- Every preference has a `Key` in `oxicord-settings`; add missing keys with
  their proto field numbers from the bundle (the settings proto schema is
  in the bundle; the earlier pass read it from there).
- Account writes are built only (password, email, phone, 2FA, disable,
  delete already follow this).

### ADMIN

Queue: AD-01, AD-02, AD-03, AD-04, AD-05, AD-06, AD-07, AD-08, AD-09, AD-10,
AD-11, AD-12, AD-13, AD-14, AD-15, AD-16, AD-17, AD-18, LO-20, LO-21, LO-22,
LO-23.

Notes:
- Split `server_settings.rs` (3.2k lines) into `server_settings/<page>.rs`
  as the first commit (`ADMIN-split`), no behaviour change.
- Each page waits for its capture; start with the pages whose capture
  exists and fall back to strings plus kit with "Unverified" otherwise.
- Every admin write is built and tested only. The test server rule in the
  roadmap applies to the owner, not to agents.

### VOICE-A

Queue: VC-01 (data first: device lists behind a trait), VC-02, VC-03, VC-07,
VC-09, VC-10, VC-08, VC-11, VC-12, VC-14, VC-13.

Notes:
- `oxicord-voice` works live; do not refactor `connection.rs`, `dave.rs` or
  `transport.rs` beyond what an item needs, and keep their tests green.
- Sounds come from F-09.
- Soundboard play and stage moderation are writes: built only.

### VOICE-B

Queue: VC-04a RTP depacketizers (VP8, VP9, H264, AV1), VC-04b decoders,
VC-04c frames in tiles, VC-05 camera, VC-06a Go Live protocol, VC-06b
capture and encode, VC-06c viewer, PL-12.

Notes:
- New code in `oxicord-voice/src/video/` and `src/stream/`.
- Candidate crates (ask the lead before adding): `openh264` (H264 encode and
  decode, ships the Cisco binary), `vpx-encode` / `env-libvpx-sys`,
  `dav1d`, `nokhwa` (camera), `windows-capture` (screen). Prefer ones that
  build on Windows without a manual SDK install.
- DAVE applies to video frames too; reuse `dave.rs`.
- All of this is unverifiable without a call; the owner does one test run
  per merged sub item.

### PLAT

Queue: PL-13, PL-06, PL-07, PL-08, PL-09, PL-04, PL-01, PL-02, PL-03, PL-05,
PL-10, PL-14, PL-11.

Notes:
- Do not change identify, properties, headers or the build number (rule 6).
- PL-01 and PL-02: unit test every step with fixed inputs; the owner does
  the first real sign in.
- Windows APIs come through `windows-sys` (0.61, already a dependency of
  the binary for `hotkey.rs`); add features to it rather than a second
  Windows crate.

### FOUND

Queue: F-01, F-02, F-03, F-04, F-05, F-06, F-07 (with P8-16), then help the
longest lane. F-08 to F-11 go to other agents in Wave 0.

Notes:
- F-01 to F-04 are pure moves: the diff should be mostly moved lines, with
  every test passing before and after. Do them as `git mv` plus edits so
  history follows.
- F-05 decides the test helper names every lane uses; keep them small and
  document them in a doc comment at the top of `tests/ui/world.rs`.

## 13. Task brief template

What the lead sends an agent for each item:

```
Task: <ID> <title>
Lane: <lane>   Worktree: C:\Users\bumbi\Documents\Rust\oxicord-wt\<lane>
Branch: agent/<ID> from current main
Read first: docs/roadmap.md item <ID> (and <related ids>),
  docs/captures/<ID>.md (if present), docs/discord-reference.md section
  "<name>" in the main checkout.
Facts already known: <endpoints, events, numbers the lead has>.
Files you will likely touch: <list>. Hot files you may touch: <list>.
Depends on: <merged ids>. Do not start <x> part until <y> is merged.
Done when: the item's text is built, tests in section 8 exist for the
  model, request bodies and view boxes, cargo test --workspace passes,
  the roadmap item is ticked with the note format, one commit "<ID> ...".
Hand off: "READY <ID> agent/<ID> <sha>" plus notes.
```

## 14. Known traps

- gpui here is `gpui-pre` 0.3.11 pulled in by `gpui-kit` 0.9.0; read API
  source from `~/.cargo/registry/src/*/gpui-pre-0.3.11` and
  `gpui-component-0.9.0`, not `gpui-0.2.2` which also sits in the registry.
- Never toggle `.hidden()` with `group_hover(.., |s| s.flex())`: painting it
  on hover panics with "must call prepaint before paint". Use `.invisible()`
  with `group_hover(.., |s| s.visible())`.
- A bad key string in a `KeyBinding` panics at startup; `keys.rs` has a test
  that parses every shortcut, keep it passing.
- Inputs: `Input::new(&state).large()` (with `component::Sizable as _`)
  gives 16px text and a 44px box; the default is 14px. `appearance(false)`
  and `bordered(false)` drop gpui's own chrome.
- Multi colour SVGs: `img("folder/file.svg")`. `icons::icon` tints a single
  colour glyph and loses colours.
- Files mix LF and CRLF line endings. Keep each file's existing ending; when
  scripting edits, detect and restore it.
- `discord-assets/` is embedded with rust-embed; a debug build reads the
  folder from disk, a release build embeds it.
- Tests that read `fixtures/` need the junction from section 5.
- Discord colour values in the CSS are often oklab; convert them, do not
  copy oklab into Rust.
- The em dash in "Name \u{2014} N" style headings must be the escape, never
  the literal character.
- Windows only paths for now (hotkeys, tray, capture); gate them with
  `#[cfg(windows)]` and keep the rest of the crate building elsewhere.

## 15. What the agents must never do

- Run the app or any test against Discord, use or read a token, read `.env`.
- Execute an account, server, message or relationship write.
- Change the gateway identify, client properties, headers or build number.
- Take screenshots of anything, or use a browser.
- Commit to `main` directly, push anywhere, or rewrite history on `main`.
- Add a Co-Authored-By line or any attribution.
- Write an em dash.
- Run `cargo fmt`.
- Edit another lane's owned files or the kit without the lead.
- Copy JavaScript from `reference/` into the repo. Read it for facts
  (endpoints, field names, enum values, numbers, strings) and write our own
  Rust.
- Commit anything from `reference/`, `fixtures/`, `.playwright-mcp/`,
  `screenshots/` or `target*/`.
