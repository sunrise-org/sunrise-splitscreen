# Replacing the built-in splitscreen

The signed SDK currently contains built-in splitscreen. Its seats keep using stock admission while
we prove the replacement. The mod does not call `Integrated::join_split` or `leave_split`, and the
released mod will not keep the temporary stock-seat exception.

## Build the replacement

1. **Admission and SDK checks.** Reserve one exact in-process pipe for one validated offline UUID
   and name. Only that pipe may use its single-use ticket, which expires with the request, view, or
   world. With no reservation and no ticket, defer to the game's own login gate, including its
   published-world policy. Never admit a ticket over TCP, pass P1's credentials, assign an operator
   level, or change ordinary LAN permissions. The existing eight-player hub cap includes P1,
   extra local seats and LAN guests together; exercise 16 views in layout tests only. If Sunrise
   adopts a general rule that excludes in-process connections from its network cap, revisit both
   limits together. CI runs the ticket tests and builds a separate private-resource smoke driver.
   `build` and `dco` are required checks; the build uses the configured pool with a live lease on
   eligible events, or a GitHub-hosted runner otherwise.
2. **Compose without replacing startup.** Wrap the two `run_app` calls with `WRAP_OPERATION` once a
   signed SDK carries the general [`?` receiver fix](https://github.com/sunrise-org/sunrise/pull/2521).
   The wrapper delegates P1 to the plain `Game` and owns the other `Game` instances, offscreen
   views and composition. Leave stock loading, reporting and shutdown alone; headless does not
   use winit. No app factory, splitscreen-specific hook, registry, or field on `Game`.
3. **Move the player-facing behavior.** The mod owns view and monitor layout, joining and leaving,
   seat HUD, setup, profile storage and per-view focus. Its native window path draws existing
   vanilla widgets before and after world entry. Keep profile drafts separate and reject names
   that duplicate an existing profile regardless of case. Keep one controller backend and use
   only general input, glyph and output interfaces. The cutover ships credential-less local
   guest profiles. Own-account sign-in for an extra seat is a later decision; the mod does not
   build it now or depend on the launcher's bulk account export.

## Remove the built-in implementation

Compile the mod against the exact cutover candidate SDK and drop its temporary stock-seat fallback
and every split-only reference. Audit the deletion on that head, not just the SDK pinned today.

| Area | Remove or move to the mod | Keep or generalize |
| --- | --- | --- |
| Startup and client | `game/runtime/split/`, its direct and loading startup branches, split-only roles/options (`split_guest`), `split_hud`, and split-only state and call sites in window, pointer capture, music, settings, screens, pad actions, world/session and shutdown code. | Both windowed starts and the loading handoff own a plain `Game`. Preserve solo, offscreen and headless behavior. Shift ordinary controller notices out of the split HUD; retain ordinary input, focus, recovery, audio and exit behavior. |
| Render, settings and UI | Move `split_layout`, `split_map`, `split_profiles` and `split_ui` and remove their re-exports, option pages, HUD labels, feature flags and translation keys. | Keep `gfx/compositor.rs` as the general blit/composition primitive for offscreen views; its errors, labels and docs describe surfaces and views, not splitscreen. Keep vanilla widgets, unrelated render settings and keys reused by ordinary controller notices (renamed neutrally where needed). |
| Integrated world | Remove `Flags::split_players`, `Integrated::{join_split, leave_split, has_split_players}`, the `owner_link.rs` split requests/handlers, inherited ops, `bypasses_limit` use for seats, the `split_seat` branch of `refused()`, and the split-specific pause rule. | Preserve the memory-only owner and ordinary LAN gate and cap. The mod keeps the world ticking while another local client is active, then lets stock pause policy resume. Use the general memory-connection path; retire the copied worker and thread-local ticket transport if an approved general connection token can replace them without weakening pipe binding. |
| Hooks and shared state | Remove built-in splitscreen assumptions from `sunrise-hooks` seat/page docs and mod-vocab client-session, lifecycle and frame-hook docs. Audit shared input, HUD and presence code for feature-specific assumptions and comments. | Keep `seat()` as a generic current local-player index for existing per-view settings, page and renderer state. The native wrapper sets the index through the existing `engine::set_seat` for each `Game` turn and restores the previous index afterwards; standalone play stays at 0. Preserve the hook contracts. |
| Launcher | Remove the `SUNRISE_ACCOUNT_SESSIONS` export, split-session list/renewal, its `GameSession.env` plumbing where solely used for split, and related tests. Check old launch targets before removing defensive scrubs. | Preserve P1's normal sign-in, renewal and session transport. There is no guest sign-in handoff at cutover. |
| Coverage and docs | Move split smoke/control cases and split-specific fixtures, split-only strings and options, and docs describing a built-in feature. | Keep regression coverage for vanilla pointer behavior, input and owner/LAN gates. Describe splitscreen as a separate mod in user-facing docs. |

Before the cutover, both searches must produce no matches in the game repository's crates and
launcher, including the mod API. Audit the docs and tools separately for references to the old
built-in behavior.

```sh
pattern='splitscreen|split[-_ ]?(guest|hud|seats?|layout|map|profiles|ui|players|sessions)|join[_ ]?split|leave[_ ]?split|SUNRISE_(ACCOUNT_SESSIONS|SPLIT_)|runtime/split/'
git grep -inE "$pattern" -- crates launcher
git ls-files crates launcher | grep -iE "$pattern"
```

## Cutover checks

- Without the mod: unchanged solo and headless play, no work allocated to extra views. With the
  mod: both windowed starts, signed-in P1 with no guest account, closed and published worlds,
  capacity shared with LAN guests, pause and resume, title/world transitions, profile and focus
  behavior, 2 and 4 seats, the 8-player boundary, resize, monitor changes and device loss. Verify
  layout through 16 views without connecting 16 players.
- Use the existing performance platform on the same hardware, scenario, settings and render
  resolution. Compare the candidate without the mod to pre-cutover main at one seat: no measurable
  regression. Compare the candidate with the mod loaded at one seat to the same candidate without
  it: at most 3% overhead. Repeat the captures to distinguish overhead from run-to-run noise.
  Record median and p95 CPU/GPU frame time, server tick time and memory. Compare 2- and 4-seat frame times against the built-in implementation at
  the same seat counts. Record the baselines and results before deleting the built-in feature.
- Coordinate the engine removal and official mod release only after the pair passes. Admission
  alone is not enough. Track the move in [#505](https://github.com/sunrise-org/sunrise/issues/505)
  and [#617](https://github.com/sunrise-org/sunrise/issues/617).
