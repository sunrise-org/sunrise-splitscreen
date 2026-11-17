# Replacing the built-in splitscreen

The signed SDK currently contains built-in splitscreen. Its seats keep using stock admission while
we prove the replacement. The mod does not call `Integrated::join_split` or `leave_split`, and the
released mod will not keep the temporary stock-seat exception.

## Build the replacement

1. **Admission and SDK checks.** Reserve one exact in-process pipe for one validated offline UUID
   and name. Only that pipe may use its single-use ticket, which expires with the request, view, or
   world. With no reservation and no ticket, defer to the game's own login gate, including its
   published-world policy. Never admit a ticket over TCP, pass P1's credentials, assign an operator
   level, or change ordinary LAN permissions. Pending Sunrise's decision on how to count local
   connections, the mod uses the existing eight-player hub cap for P1, local seats and LAN guests
   together; 16 views are a layout test only. If Sunrise instead adopts a generic network cap
   that excludes in-process connections, revisit the gameplay cap. The device bus's 16 indices
   are not an admission rule. CI tests the ticket policy and builds a separate private-resource
   smoke driver. `build` and `dco` are required checks; the build uses the configured pool with
   a live lease on eligible events, or a GitHub-hosted runner otherwise.
2. **Compose without replacing startup.** Wrap the two `run_app` calls with `WRAP_OPERATION` once a
   signed SDK carries the general [`?` receiver fix](https://github.com/sunrise-org/sunrise/pull/2521).
   Delegate P1 to the plain `Game`; the wrapper owns the other `Game` instances, offscreen views
   and composition. Leave stock loading, reporting and shutdown alone; headless does not use
   winit. No app factory or splitscreen-specific hook, registry, or `Game` field.
3. **Move the player-facing behavior.** The mod owns view and monitor layout, joining and leaving,
   seat HUD, setup, profile storage and per-view focus. Its native window path draws existing
   vanilla widgets before and after world entry. Keep profile drafts separate and reject names
   that duplicate an existing profile regardless of case. Keep one controller backend and use
   only general input, glyph and output interfaces. The cutover ships credential-less local
   guest profiles. Defer own-account sign-in for another local player and call out that change
   in the release notes; do not depend on the launcher's bulk account export.

## Local instance identity and authority

A general engine prerequisite for hosting more than one `Game` in a process: evolve the private
role in [#2483](https://github.com/sunrise-org/sunrise/pull/2483) into an **instance role** with
`Primary` and `Secondary { local_player_index }`. Keep this separate from server/client execution
mode. Only the primary client owns process-wide effects. Window authority comes from actual
window ownership, not the fact that an instance has a window reference: a secondary must not
resize or fullscreen a shared window, but a separately owned window may be controlled by its
owner. The compositor and its frame pacing belong to the host, not the instance role.

Construction supplies each `Game` its own optional account. The primary takes its account from
normal launcher bootstrap. The mod creates a secondary with `None`; `Game::auth()` reads only the
instance's account, never falls back to the process account, and the local guest session passes
`None` too. Guest-reachable title, Realms, social and LAN-hosting paths must use the explicit
instance account or refuse the operation; asynchronous work must take the account with it. Keep
process-scoped account status, social and presence updates primary-only. Secondary clients in
this mod join over memory only. A request to join an online server or open Realms must not start
an authenticated request as P1. The native route is trusted engine code, not a mod sandbox; this
boundary prevents the game's own secondary instance from using P1's session implicitly.

The engine owns `seat()` as the current local-player index (0 when playing alone). It scopes the
index from the instance for construction, frame, tick, window events and settings-store load/save,
restoring the previous index on return. That covers copying P1's profile defaults and then
loading the guest's own stores outside its first frame. The native wrapper never calls
`engine::set_seat`. See the [general input spec](https://github.com/sunrise-org/sunrise/issues/2498).

### Existing call sites to audit

Engine baseline: `817fe45`. Paths below are relative to `crates/sunrise-client/src` unless named
otherwise. These are responsibilities to preserve or move, not new public hooks.

| Responsibility | Baseline sites | Boundary at cutover |
| --- | --- | --- |
| Primary sign-in bootstrap and chat-key preparation | `game/runtime/launch_options.rs:133–138`; `sunrise-client-render/src/auth.rs:126–134,470` | Consume the launcher's account only at primary bootstrap. Pass the instance account onward. |
| Remote/replay identity, skin/profile and signed chat | `game.rs:1109`; `game/network/remote/mod.rs:990,1298,2089–2094,5477`; `game/network/remote_chat.rs:185,203,215` | Use the instance account only, including for profile lookup and key selection. Never use P1's keys on a guest path. |
| Title status and notices, Realms, social requests, world/LAN host identity | `title.rs:2392,3557–3586`; `realms.rs:164–166`; `sunrise-client-render/src/social/service.rs:82–93`; `game/runtime/client_tick_steps.rs:17–18`; `game/world/integrated.rs:464–479`; `lan/mod.rs:208–216` | Do not read P1's account on a secondary path. Keep process-wide account notices and social work on the primary; use explicit account/authority at any shared entry. |
| Quit, save on process stop, crash and server report | `game/world/world_session.rs:21–29`; `game/runtime/control.rs:1369–1379`; `game/runtime/window_handler.rs:259–270`; `game.rs:9473`; `game/world/saving.rs:260`; `game/world/integrated.rs:1523–1539`; `game/network/remote/mod.rs:982–986,1598–1600` | Closing a secondary leaves only that instance. Primary owns process shutdown, owner-world crash exit and process-level server crash details. |
| One-time client lifecycle, registrations and config watcher | `game/runtime/launch_options.rs:93–95`; `game/runtime/loading_screen.rs:29–32,44–47`; `game/runtime/window_handler.rs:252–254`; `game/modding/initial_client_models.rs:5`; `game/modding/native_configs.rs:188–202`; `game/runtime/activity.rs:17–18` | Process-level client initialization and shutdown run once for the primary, not once per local view. Per-instance session/events remain per instance. |
| Process audio, presence and telemetry | `game/interface/music.rs:14–24`; `game/interface/options.rs:652–658`; `game/interface/startup_menu.rs:493`; `game/runtime/activity.rs:23–28`; `game/world/world_session.rs:195–199`; `sunrise-discord/src/lib.rs:92–99` | Primary controls music, mixer volumes, narrator settings, presence and the process telemetry report. |
| Window observation, fullscreen, pointer and frame ownership | `game/interface/options.rs:565–569,720–755,810–813`; `game/interface/pointer_capture.rs:274`; `game/runtime/window_handler.rs:461–471`; `game/interface/render_settings.rs:58–63`; `game.rs:7502` | Preserve actual window ownership and input focus. A view must not reconfigure a surface or pace frames it does not own. Keep detached-window behavior in the wrapper. |
| Shared cache and seat-local state | `game/world/world_update.rs:61–75`; `game/runtime/split/players.rs:128–142,311–330`; `sunrise-client-render/src/settings.rs:344–350`; `sunrise-hooks/src/lib.rs:230–246` | Do not let a secondary write the primary's persistent server terrain cache. Load and save stores under the intended instance index. |

## Remove the built-in implementation

Compile the mod against the exact cutover candidate SDK and drop its temporary stock-seat fallback
and every split-only reference. Audit the deletion on that head, not just the SDK pinned today.

| Area | Remove or move to the mod | Keep or generalize |
| --- | --- | --- |
| Startup and client | `game/runtime/split/`, `mod split`, its direct and loading startup branches, `split_guest`, `split_auth`, `split_window`, `split_in_main_window`, `paced_by_split`, `split_hud` and the split-only title/menu/pad branches. | Both windowed starts and the loading handoff own a plain `Game`. Preserve solo, offscreen and headless behavior with the instance boundary above. Shift ordinary controller notices out of the split HUD. |
| Render, settings and UI | Move `split_layout`, `split_map`, `split_profiles` and `split_ui`. Remove re-exports, `SplitSetup`, `SplitHelp`, `SPLIT_SETTINGS`, `SPLIT_TWO`/`THREE`, split-only option pages, labels and flags. Move or retire `sunrise.split.*` and `sunrise.pages.{open.,}split_*` locale keys. | Keep `gfx/compositor.rs` as a blit/composition primitive for offscreen views, with neutral docs, labels and errors. Preserve vanilla widgets and ordinary controller notices. |
| Integrated world | Remove `Flags::split_players`, `Integrated::{join_split, leave_split, has_split_players}`, the `owner_link.rs` split requests and handlers, inherited ops, `bypasses_limit` for seats, the `split_seat` gate branch, and the split-specific pause rule. | Keep memory-only owner login and the ordinary LAN gate. While another local client is active the mod keeps the world ticking, then lets stock pause policy resume. If pipe binding still uses the mod's worker adapter, require each SDK bump to check the signed `Lan::connect_local` and `serve_local` sources; fail on changes until the adapter is re-audited and both smokes pass. Do not add a feature-specific token. |
| Hooks and shared state | Remove built-in splitscreen assumptions from `sunrise-hooks` seat/page docs and mod-vocab client-session, lifecycle and frame-hook docs. Audit shared input, HUD and presence code. | Use the engine-owned local instance index for existing per-view stores, pages and rendering. Preserve the hook contracts. |
| Launcher | Remove the `SUNRISE_ACCOUNT_SESSIONS` export, its split-session list and renewal, `GameSession.env` plumbing used only for it, both defensive scrubs and related tests at cutover. | Preserve primary sign-in, renewal and session transport. Before removing the scrubs, prove old supported launch targets cannot inherit stale extra-account credentials; if that cannot be proven, block this step pending a general launch-environment fix. |
| Coverage and docs | Move split smoke and control cases, split-specific fixtures and docs describing a built-in feature. | Keep vanilla pointer, input, and owner/LAN gate coverage. Describe splitscreen as a separate mod. |

Both searches below must produce no matches in the candidate's crates and launcher, including the
mod API. This catches named remnants, not renamed behavior: also review the diff against the
inventory and prove that there is no built-in join/UI path without the mod. Audit tools and docs
for references to the old built-in behavior separately.

```sh
pattern='splitscreen|split[-_ ]?(guest|hud|seats?|layout|map|profiles|ui|players|sessions|setup|settings|window|auth|in_main_window)|join[_ ]?split|leave[_ ]?split|paced_by_split|Split(Setup|Help)\b|SPLIT_(TWO|THREE|SETTINGS)|sunrise\.(split|pages\.(open\.)?split_)|(super|crate|runtime)::split::|\bmod split\b|split::(Split|Hud)|SUNRISE_(ACCOUNT_SESSIONS|SPLIT_)|runtime/split/'
git grep -inE "$pattern" -- crates launcher
git ls-files crates launcher | grep -iE "$pattern"
```

## Cutover checks

- Without the mod: unchanged solo and headless play, no work allocated to extra views. With the
  mod: both windowed starts, closed and published worlds, the shared eight-player boundary under
  the current default, pause and resume, title/world transitions, profile and focus behavior,
  2 and 4 seats, resize, monitor changes and device loss. Exercise 16 views in layout only.
- With P1 signed in, assert the secondary's `auth()` and memory session have no account. Attempt
  Realms and remote multiplayer from its title: no P1 identity, credentials, chat signing or
  account-specific request. Guests changing fullscreen, audio, or quitting must not resize P1's
  window, change the process mixer or end the game. Verify guest settings affect only its profile.
- On the same hardware, scene, settings and render resolution, use the existing performance
  platform to collect total frame time p50/p99 and CPU and GPU frame time p50. Repeat captures to
  establish the run-to-run noise band. The candidate without the mod must stay within that band
  of pre-cutover main at one seat. The candidate with the mod loaded at one seat must be no more
  than 3% worse than the same candidate without it on each metric. The mod at 2 and 4 seats must
  be no more than 3% worse than built-in splitscreen at the corresponding seat count on each
  metric. Record tick time and memory alongside these gates.
- Coordinate the engine removal and official mod release only after the pair passes. Admission
  alone is not enough. Track the move in [#505](https://github.com/sunrise-org/sunrise/issues/505)
  and [#617](https://github.com/sunrise-org/sunrise/issues/617).
