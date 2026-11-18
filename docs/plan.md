# Replacing the built-in splitscreen

**Route M: one process, one window, no splitscreen code in the engine.** Use the mod API and
native mixins on the prebuilt game. The mod owns guest instances and their composition. It adds
no factory, account or process role, or feature-specific extension point. The only new field on
`Game` this plan requires is the neutral local-instance index from #2498 step 13. That work is
owned by #2498, not splitscreen. Start with two local players in one window. Ship only the exact
SDK builds on which containment and primary behavior are verified.

The pinned signed SDK still contains built-in splitscreen, but Sunrise is removing it now from
its engine, mod API and launcher, ahead of mod parity. Splitscreen will be unavailable on those
builds until the mod ships. The foundation remains pinned to the existing signed SDK. It leaves
stock admission alone on that pin; it does not call `Integrated::join_split` or `leave_split`.
Drop the temporary stock-seat exception when moving to an SDK without the built-in feature.

## Build the replacement

1. **Admission and SDK checks.** Reserve one exact in-process pipe for one validated offline UUID
   and name. Only that pipe may use its single-use ticket, which expires with the request, view, or
   world. Refuse an issued pipe if its lease is gone. With no reservation, a genuinely untagged
   connection goes through the game's own login gate, including its published-world policy.
   Never admit a ticket over TCP, pass P1's credentials, assign an operator level, or change
   ordinary LAN permissions. Our provisional default, not a cap decided by
   Umut, is the existing eight-player hub cap for P1, local seats and LAN guests together.
   Sixteen views are a layout test only. If Sunrise instead adopts a generic network cap that
   excludes in-process connections, revisit the gameplay cap and the device bus table size
   together. The device bus's 16 indices are not an admission rule. CI tests the ticket policy
   and builds a separate private-resource smoke driver. `build` and `dco` are required checks; the build uses the configured pool with
   a live lease on eligible events, or a GitHub-hosted runner otherwise. After this workflow
   reaches the default branch, dispatch `.github/workflows/build.yml` once with `force_hosted: true`
   and record the hosted build result. Ordinary lease routing stays unchanged.

   The mod refuses conflicts with playing guests when issuing a seat reservation; the smoke tests
   this after dropping a guest's lease while keeping the guest online. The pinned SDK cannot guarantee
   refusal of a UUID held by a login still in configuration. The stock gate runs before the engine
   claims the UUID, and the roster is populated after configuration. A published-world login can
   therefore pass the stock gate before the seat reserves the same UUID, and the seat can hit the
   stock duplicate-kick path. Do not ship guest admission until a general native route can refuse
   that conflict where the engine claims the UUID, without changing ordinary duplicate policy.
2. **Compose without replacing startup.** Wrap the two `run_app` calls with `WRAP_OPERATION` once a
   signed SDK carries the general [`?` receiver fix](https://github.com/sunrise-org/sunrise/pull/2521).
   Delegate P1 to the plain `Game`; the wrapper owns the other `Game` instances, offscreen views
   and composition. Leave stock loading, reporting and shutdown alone; headless does not use
   winit. No app factory or splitscreen-specific hook, registry or `Game` field. The only new
   `Game` field used is #2498's neutral local-instance index.
3. **Move the player-facing behavior.** The mod owns view and monitor layout, joining and leaving,
   seat HUD, setup, profile storage and per-view focus. Its native window path draws existing
   vanilla widgets before and after world entry. Keep profile drafts separate and reject names
   that duplicate an existing profile regardless of case. Keep one controller backend and use
   only general input, glyph and output interfaces. The cutover ships credential-less local
   guest profiles. Defer own-account sign-in for another local player and call out that change
   in the release notes; do not depend on the launcher's bulk account export.

## Guests play only in the host's world

The mod uses native function replacement on the exact signed game build. It adds no engine
instance role and does not change P1's account or sign-in. The mod owns the secondary instances
and identifies them before secondary setup or callbacks run. It never gives a secondary P1's
launch options, a developer control socket, or an event-loop handle that can end the process.

A guest has a separate offline profile and can only enter P1's current integrated world through
the mod's reserved memory pipe. Mod-owned Setup and Profiles are not the stock title or server
list. A guest never enters those screens, Realms, friends/social, another world, or LAN/integrated
hosting. Guard the underlying entry points as well as the UI. Block guest social dispatch before
it starts a worker; a game-thread seat check cannot guard a worker's process-global account read.
Refuse guest TCP connections and return `None` from `Game::auth()` for a secondary, including
when the stock local-session initializer asks. The guest session also receives `None` explicitly.
Leaving the host's world, or losing its connection, disposes of the guest and lease; no stock
leave-to-title path runs for it.

Fullscreen, process quit and signals, mixer output and settings, activity, crash details and
once-per-client work stay with P1. Guests contribute their listener cameras to the shared mixer.
A guest may act only on a window it actually owns.

[Step 13 of #2498](https://github.com/sunrise-org/sunrise/issues/2498) starts with an engine-owned
local-instance index and `in_seat`, independent of #2483. Set the neutral index once at `Game`
construction: 0 for the first `Game` in the process, distinct indices for later local clients.
The index carries no account or behavior. The mod passes a guest's index only at construction.
The engine enters and restores `seat()` from the index around construction, frame, tick, input
and settings-store calls. Guards use the instance index or, without a receiver, the active seat
scope. The mod never calls `engine::set_seat`. Controller input uses only the general device bus
or mod API; the mod installs no second backend. Guest turns wait for a signed SDK with this scope.

### Native replacement checklist

Use the smallest set of patches that handles the paths a guest can actually reach. The guest-view
PR records every applied patch's exact target, selector, purpose and what an engine refactor could
break. Keep `MOD-API-GAPS.md` in the engineering handoff: record the exact SDK, file/line or tool
error, and a general fix for each missing capability. Do not add a feature-specific workaround to
the engine. The two startup wraps remain blocked until a signed SDK ships the receiver fix.

The candidate inventory below was traced on Sunrise `817fe45`; the foundation implements none of
these guest patches. Paths start at `crates/sunrise-client/src/`; targets start at `sunrise_client::`.
W means `WRAP_OPERATION` with the named call selector; H means a cancellable `HEAD` on the named
function, with no call selector. Confirm the targets and exact match counts against the cutover SDK. Do not start guests
if a required patch is missing. At each SDK update, also audit newly reachable calls: an old
selector still matching does not prove that no new guest path reaches an account service.

| Sites | Target, file:line and selector |
| ---: | --- |
| 2 | `game::loading_screen::run_event_loop` (`game/runtime/loading_screen.rs:36`) and `game::resource_loader::run_loading` (`game/runtime/resource_loader.rs:295`): W `EventLoop::new()?.run_app` in each. |
| 3 | `game::options::Game::{apply_fullscreen,observe_fullscreen,toggle_fullscreen}` (`game/interface/options.rs:565,720,739`): H each, skip shared-window writes/observation for the secondary. |
| 4 | `game::Game::render_with` (`game.rs:8154`): W `self.follow_frame`; `game::options::Game::apply_ui_settings` (`game/interface/options.rs:647`): W `r.update_output`; `game::options::Game::limit_frame` (`:810`): H; `game::pointer_capture::Game::internal_grab` (`game/interface/pointer_capture.rs:273`): H. |
| 4 | `game::saving::Game::begin_closing_save` (`game/world/saving.rs:44`): H for guest `quit`; `game::native_hooks::shared::Game::show_title` (`game/native_hooks/shared.rs:362`): H, refuse stock title installation and detach; `game::window_handler::Game::idle` (`game/runtime/window_handler.rs:261`): W `sunrise_threads::stop_requests`; `game::integrated::Game::exit_after_server_crash` (`game/world/integrated.rs:1523`): H, not a wrap of the `!`-returning exit. |
| 4 | `game::loading_window::start_game` (`game/runtime/loading_window.rs:406,408`): W `game.publish_initial_client_models` and `game.native_configs_client_start`; `game::client_tick_steps::Game::client_tick_steps` (`game/runtime/client_tick_steps.rs:43`): W `self.native_configs_tick_client`; `game::window_handler::Game::shut_down` (`game/runtime/window_handler.rs:253`): W `self.native_client_stopping`. |
| 6 | `game::client_tick_steps::Game::client_tick_steps` (`game/runtime/client_tick_steps.rs:50`): W `self.tick_music`; `game::options::Game::apply_ui_settings` (`game/interface/options.rs:597,598,656,658`): W `sunrise_client::parallel::set_setting`, `sunrise_client_render::mipmap::set_dither_cutouts`, `sunrise_client::settings::set_source_volumes`, `super::narrator::sync`; `game::startup_menu::Game::apply_menu_settings` (`game/interface/startup_menu.rs:493`): W `super::narrator::sync`. |
| 4 | `game::activity::Game::report_activity` (`game/runtime/activity.rs:28`): W `sunrise_hooks::engine::activity`; `game::client_tick_steps::Game::client_tick_steps` (`game/runtime/client_tick_steps.rs:17–18`): W `sunrise_client::social::set_presence_status` and `sunrise_client::social::tick`; `game::world_session::Game::finish_perf` (`game/world/world_session.rs:195`): H. |
| 1 | `game::remote::Game::leave_server` (`game/network/remote/mod.rs:1530`): H, schedule guest disposal instead of stock leave-to-title. This skips `clear_detail` at `:1599` and title construction at `:1626–1645`; `set_detail` at `:983` is excluded by the no-TCP gate below. |
| 4 | `game::remote::Game::connect_remote_session` (`game/network/remote/mod.rs:954`): H, refuse TCP for secondaries before reading identity; `game::world_session::Game::open_world` (`game/world/world_session.rs:86`): H before filesystem work; `game::integrated::Game::open_integrated` (`game/world/integrated.rs:464`): H before auth/server worker creation; `game::lan::Game::lan_open` (`game/network/lan/mod.rs:1181`): H. |
| 2 | `game::startup_menu::Game::draw_menu` (`game/interface/startup_menu.rs:1397`) and `game::native_hooks::shared::Game::settle_title_toasts` (`game/native_hooks/shared.rs:865`): W `title.poll_backup` in each. Mod never routes secondary input to stock Title/Realms; use its own setup screen. |
| 2 | `game::world_update::Game::prepare_lod` (`game/world/world_update.rs:74`): W `std::env::current_dir`, short-circuit only the guest persistent-cache path; `game::options::Game::options_path` (`game/interface/options.rs:98`): H returning the scoped guest path. `options_path()` has no `self`; use the active `in_seat` scope, including during construction. |
| 1 | `game::remote::Game::auth` (`game/network/remote/mod.rs:2089–2094`): H returns `None` for the scoped secondary. P1's ordinary auth path stays untouched. |

**Deferred, only if guests ever get online features:** give each `Game` its own optional account,
attach the launcher account only at primary construction, and remove the process fallback from
`Game::auth()`. Audit direct global readers as part of that work. Do not build or require this
engine change for memory-only guests.

## Remove the built-in implementation

This deletion is separate and does not wait for the mod. Audit it on the removal PR's head. Once
there is a signed SDK from that tree, compile the mod against it and drop the mod's temporary
stock-seat exception and any other dependency on the old implementation.

| Area | Remove or move to the mod | Keep or generalize |
| --- | --- | --- |
| Startup and client | `game/runtime/split/`, `mod split`, its direct and loading startup branches, `split_guest`, `split_auth`, `split_window`, `split_in_main_window`, `paced_by_split`, `split_hud` and the split-only title/menu/pad branches. | Both windowed starts and the loading handoff own a plain `Game`. Preserve #2498's neutral local-instance index and solo, offscreen and headless behavior. Shift ordinary controller notices out of the split HUD. |
| Render, settings and UI | Move `split_layout`, `split_map`, `split_profiles` and `split_ui`. Remove re-exports, `Page::SplitSetup`, `split_setup`, `FrameDraw::SplitHelp`, `SPLIT_SETTINGS`, `open_split_settings`, `SPLIT_TWO`, `SPLIT_THREE`, split-only option pages, labels and flags. Move or retire `sunrise.split.*` and `sunrise.pages.{open.,}split_*` locale keys. | Keep `gfx/compositor.rs` as a blit/composition primitive for offscreen views, with neutral docs, labels and errors. Preserve vanilla widgets and ordinary controller notices. |
| Audio (`sunrise-client-render/src/audio.rs`) | Remove only built-in feature wording. | **Kept: the generic multi-listener mixer.** Retain `Audio::share`, `listeners`, `cameras`, the nearest-listener rule and the halved panning with more than one camera. The mod uses these behaviors; document them without feature-specific terms. |
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
  the current default, pause and resume, leaving the host world, profile and focus behavior,
  2 and 4 seats, resize, monitor changes and device loss. Exercise 16 views in layout only.
- On the exact signed SDK with the startup fix, run both windowed paths with P1 signed in. Prove
  guest auth is absent for the reserved memory connection. Drive guest input and direct entry
  attempts for the stock title, server list/TCP, Realms, friends/social, world open, integrated
  hosting and LAN opening. No screen, request or account worker may start. Close the host world
  and force a lost guest connection; in both cases the affected guest is destroyed, not taken
  to a title screen. Verify no guest attempt uses P1's name, account or chat key.
- On both starts, prove P1 can still sign in, open Realms and use signed online chat. Verify guest
  fullscreen, audio and quit do not resize P1's window, change the process mixer or end the game.
  Verify that a guest hears nearby world sounds from its own camera and P1 still hears sounds
  from the primary camera. Lifecycle and signals remain primary-only. Guest settings affect only
  its profile.
- Capture the built-in baseline before deletion. Use the existing performance platform to record
  total frame time p50/p99 and CPU and GPU frame time p50. Repeat captures to establish the
  run-to-run noise band. Use the same GPU host and method recorded in the engineering handoff's
  `BASELINE.md`, with the same scene, settings and resolution. Software rendering (including
  lavapipe) does not count toward the gate. The candidate without the mod must stay within the
  noise band of the last build with built-in splitscreen at one seat. The candidate with the mod
  loaded at one seat must be no more than 3% worse than the same candidate without it on each
  metric. The mod at 2 and 4 seats must be no more than 3% worse than built-in splitscreen at the
  corresponding seat count on each metric. Record tick time and memory alongside these gates.
- Release the mod only after it passes these checks on an exact signed SDK without built-in
  splitscreen. The engine deletion is not gated on these checks. Admission alone is not enough. Track the move in [#505](https://github.com/sunrise-org/sunrise/issues/505)
  and [#617](https://github.com/sunrise-org/sunrise/issues/617).
