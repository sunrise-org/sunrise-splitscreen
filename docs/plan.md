# Replacing the built-in splitscreen

This repository provides the one local-seat implementation after the cutover. The first SDK still
contains built-in splitscreen; its existing seats continue to use stock admission while we prove the
new path. The mod does not call `Integrated::join_split` or `leave_split`. The temporary stock-seat
exception in the gate is not part of the released mod on the replacement SDK.

1. **Admission and SDK checks.** Reserve one exact local pipe for one validated offline UUID and
   name. Consume its ticket only once and expire it with the request, view, or world. Use the
   existing networking worker and protocol; never send P1's credentials, admit a ticket over TCP,
   raise the LAN cap, or assign an operator level. A published world's ordinary guest command
   settings still apply. CI tests the policy and builds a separate private-resource smoke driver.
2. **Compose without replacing startup.** Wrap the two existing `run_app` calls through
   `WRAP_OPERATION` once an SDK supports their `?` receiver. The wrapper delegates P1 to the plain
   client and owns the other `Game` instances, the offscreen views, and their composition using the
   existing renderer and `Compositor`. Leave the loading handoff, report and shutdown in their
   stock functions; headless does not use a winit loop. No app factory, Split-shaped engine hook,
   registry, or `Game` field. Until a signed SDK carries the general wrapping fix, neither startup
   function is patched by this mod.
3. **Move the player-facing behavior.** Port the built-in view/monitor layout, joins and leaves,
   setup and profile pages, and seat HUD state into the mod. Keep private options and per-view
   widget/focus state in the mod; the existing vanilla widgets and the native window path draw the
   pages before and after world entry. Resolve duplicate profiles case-insensitively at commit and
   keep drafts separate. Keep one controller backend in the game; only use an existing or separately
   approved general route for per-device input, glyphs and output. Never use a guest account handoff.
4. **Prove the pair before deleting built-in Split.** The engine cutover patch switches both
   windowed starts and the loading handoff to a plain `Game`; it removes `runtime/split/`, the
   split-only options and UI, `Flags::split_players`, `Integrated::{join_split, leave_split}` and
   their bypass/permission logic. The mod is compiled against that exact candidate SDK and drops
   its stock-seat fallback and references to split-only fields (including `split_guest`). Use the
   game’s general local connection path and an explicit credential-less guest session instead of
   relying on the old role to suppress auth. The plain renderer, compositor, memory transport,
   vanilla widgets, stock LAN/auth behavior, and P1's launcher sign-in remain in the game.
5. **Cut over only after the replacement passes.** Run without the mod (unchanged solo/headless,
   no split work) and with it on the same candidate build: both starts, signed-in P1 with no extra
   accounts, closed and published LAN, pause, title/world transitions, focus and profiles, 2/4/16
   seats, resize/monitor changes and device loss. Then coordinate engine removal and the official
   mod release together. A passing admission test alone is not a reason to remove the built-in
   feature. Track the move in [#505](https://github.com/sunrise-org/sunrise/issues/505) and
   [#617](https://github.com/sunrise-org/sunrise/issues/617).
