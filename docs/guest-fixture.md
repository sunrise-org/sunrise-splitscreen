# Two-view development fixture

This is an **offline test**, not a playable release. It builds with the signed Linux SDK
`68b51161354df882c903467bd891a7996e08c11f` on the prebuilt route. The `.smod` still ships only
admission. CI compiles the guest fixture in a separate package; it does not install or run it.

The test starts from one integrated world, opens the reserved memory pipe, and constructs a
second `Game` directly, without the normal startup, a second server, or another launcher account.
It disables the stock guest flags for auth and fullscreen. The guest has its own connection,
renderer, viewport, and silent audio. Both games draw offscreen on the same device and the
compositor presents both textures into P1's window. The guest sees only a local loop handle.
Once both players are active and both views have been presented, the test exercises the guest's
F11 mapping, auth, title, TCP, Realms/LAN connection flags, world opening and LAN publication
entrypoints. It then quits the guest, returns the surface to P1, and checks the server roster
before saving and exiting normally. No online account or controller device is used.

Run `bash tools/build-sdk.sh` with `SPLITSCREEN_TEST_RESOURCES` pointing to a **private copy** of
prepared resources, Xvfb installed, and a Vulkan driver on the machine. Lavapipe works. The
runner creates isolated game, config, launcher, and runtime directories. It tests both ordinary
asynchronous loading and the synchronous start (`--frames`) for both fixtures. Do not add the
resources, screenshots or local logs to this repository.

## Replacement table for this pin

Target names below are relative to `sunrise_client::game::`. A head has no callee selector; all
wraps require exactly one match (ordinal 0). A `[[native.module]]` supplies shared fixture code
but does not replace a game function. The admission package applies two patches. The guest
fixture builds those two plus the 21 rows marked **Fixture**. None of the fixture rows ships.

| Scope | Target | Kind / selector | Purpose and refactor risk |
| --- | --- | --- | --- |
| Admission | `developer_launch::Game::lan_host` | Wrap `self.start_lan_hub` | Bind tickets to the integrated server's ordinary login gate. Recheck changes to the gate signature, process/thread of login, or server role dispatch. |
| Admission | `integrated::Game::stop_serving` | Head | Revoke reservations on server teardown. An alternate owner shutdown path could leave tickets live. |
| Fixture | `loading_screen::run_event_loop` | Overwrite | Install the test app for synchronous startup. The fixture keeps the stock shutdown/reporting; audit the entire copied body if it changes. Production must wrap `EventLoop::new()?.run_app` instead. |
| Fixture | `resource_loader::run_loading` | Overwrite | Install the test app for asynchronous startup. Changes to loading, handoff or shutdown invalidate the copied body. Production must wrap `EventLoop::new()?.run_app` instead. |
| Fixture | `client_tick_steps::Game::client_tick_steps` | Wrap `sunrise_client::social::set_presence_status` | Keep P1's process presence unchanged. A new presence producer could bypass the wrap. |
| Fixture | `client_tick_steps::Game::client_tick_steps` | Wrap `sunrise_client::social::tick` | Block guest social work before dispatching workers. Recheck moved or additional account consumers even if the selector still matches. |
| Fixture | `remote::Game::update_remote_skins` | Cancellable head | Use no remote skin downloads from the guest tick. New render/tick paths may dispatch downloads elsewhere. |
| Fixture | `remote::Game::resolve_inline_profiles` | Cancellable head | Don't resolve dynamic inline profiles over the network. A new text component renderer could bypass this entry. |
| Fixture | `item_draw::<Host as SpecialHost>::player_skin` | Cancellable head | Drop dynamic player head skin requests. Changes to item-draw host types or a new resolver need another audit. |
| Fixture | `options::Game::apply_ui_settings` | Cancellable head | The fixture does not apply guest UI settings to the host surface, mixer or global options. A production patch has to preserve guest-local settings; direct device writes would bypass this guard. |
| Fixture | `options::Game::observe_fullscreen` | Cancellable head | Don't copy the shared window's fullscreen state to guest options. Moving the observation into the event handler can bypass this entry. |
| Fixture | `options::Game::toggle_fullscreen` | Cancellable head | Guest F11 cannot change the shared window or write settings. Direct window writes would bypass the guard. |
| Fixture | `native_hooks::Game::show_title` | Cancellable head | Don't install a stock guest title, which exposes server/Realms/social entry. Assigning `title` directly bypasses this guard. |
| Fixture | `activity::Game::report_activity` | Cancellable head | Suppress guest process activity. Re-audit new activity producers or worker dispatch. |
| Fixture | `native_configs::Game::native_configs_tick_client` | Cancellable head | Only P1 ticks the process config watcher. A new watcher or call site that bypasses this method can duplicate work. |
| Fixture | `music::Game::tick_music` | Cancellable head | Don't schedule guest music. Music starting elsewhere would not use this guard. |
| Fixture | `world_session::Game::finish_perf` | Cancellable head | Don't send process world-performance reports for the guest. Recheck separate telemetry events. |
| Fixture | `world_session::Game::open_world` | Cancellable head | Refuse guest world creation/open before any filesystem work or integrated worker startup. A new entry that calls `open_integrated` directly bypasses the head. |
| Fixture | `lan::Game::lan_open` | Cancellable head | Refuse guest LAN publication before any worker starts. Lower-level LAN entry must remain unreachable from the guest. |
| Fixture | `remote::Game::auth` | Cancellable head | No guest account, including when the memory-session initializer asks. Doesn't cover direct process account reads; those paths must stay unreachable. |
| Fixture | `remote::Game::connect_remote_session` | Cancellable head | Permit only the memory route for the scoped guest. New TCP/Realms/session-start paths need review. |
| Fixture | `saving::Game::begin_closing_save` | Cancellable head | Quit drops the guest connection without requesting process exit or saving P1's world. Direct process exit would bypass it. |
| Fixture | `remote::Game::leave_server` | Cancellable head | Drop the guest connection instead of returning the instance to the title. Recheck new disconnect paths. |

## Before enabling guests in a package

- Replace both test-only startup copies with normal operation wraps on the signed SDK carrying
  [the receiver fix](https://github.com/sunrise-org/sunrise/pull/2521). The fixture uses `Split`
  solely to locate and run the primary under the pinned pre-removal dispatcher; it never uses
  stock guest admission or routing. Its offscreen/pacing fields must be re-audited once the
  built-in implementation is removed. `Game::new` and the renderer primitives need the same audit.
- Enter the general engine-owned local-instance scope for each guest's callbacks and stores.
  The fixture's synchronous thread-local flag filters only its explicit effects; it is not an
  engine seat scope. It does not write `set_seat` or initialize a second controller backend.
- Audit all reachable services under real guest input, including skin/profile work, asynchronous
  tasks, render/store changes, world or connection loss, and actual sign-in, Realms, and signed
  chat for P1. The fixture does not prove full service containment or signed-in P1 behavior.
  A direct `open_integrated` head failed `E_UNSUPPORTED_CFG` on this pin; the fixture never
  takes that bootstrap path and guards its UI ingress at `open_world`.
- Run the tests again on the exact signed SDK without the built-in feature. No private build or
  source patch to the engine can replace that check. The [cutover plan](plan.md) gives the release
  tests and the one-, two-, and four-seat performance criteria.
