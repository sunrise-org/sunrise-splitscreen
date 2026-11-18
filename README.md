# Sunrise Splitscreen

Local splitscreen for [Sunrise](https://github.com/sunrise-org/sunrise), as an official native mod. Extra seats need
no account of their own and never use the host's credentials. Own-account guest sign-in is not part of the cutover.

The package currently contains the local-seat admission foundation. It does not replace the game's startup or add
player controls. The separate [two-view fixture](docs/guest-fixture.md) tests a guest and compositor on the signed SDK.
See the [cutover plan](docs/plan.md) for what must pass before guests ship.

## Build

On x86-64 Linux, run `bash tools/build-sdk.sh`. It verifies the signed SDK pinned by `sunrise-mod.toml`, checks the mod
with the SDK's Rust toolchain, builds the native patch without rebuilding the game, and writes an unsigned source
package to `target/sdk-<commit>/splitscreen.smod`. Clippy and the unit tests cover the mod crate and seat policy. The
native adapter and smoke drivers compile against the signed SDK; the injected game code is not linted by Clippy.
CI does not run the in-game smokes because they need private game resources. To run both windowed starts locally,
set `SPLITSCREEN_TEST_RESOURCES` to a private copy of a prepared resource directory and rerun the command with Xvfb
and Vulkan (software Vulkan works).

The `Splitscreen` workflow also accepts an `sdk_tag` of the form `sdk-<full commit>`. This rebuilds a scratch copy
against the named SDK without changing the repository's pin. Set `force_hosted: true` on a manual run to use a
GitHub-hosted runner even when the pool lease is active. These builds are not player releases. Once the mod is ready
and signed, each version goes through the normal [sunrise-mods](https://github.com/sunrise-org/sunrise-mods#submitting-a-project)
submission process.

Licence: [PolyForm Shield 1.0.0](LICENSE). See [CONTRIBUTING.md](CONTRIBUTING.md) for the DCO sign-off.
