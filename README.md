# Sunrise Splitscreen

Local splitscreen for [Sunrise](https://github.com/sunrise-org/sunrise), as an official native mod. Extra seats play on
the host's account; a second account is optional, never required.

This repository currently contains the native startup and local-seat admission foundation. It does not yet add a way
for players to open or control a second view.

## Build

On x86-64 Linux, run `bash tools/build-sdk.sh`. It verifies the signed SDK pinned by `sunrise-mod.toml`, checks the mod
with the SDK's Rust toolchain, builds the native patch without rebuilding the game, and writes an unsigned source
package to `target/sdk-<commit>/splitscreen.smod`. No game resources or player account are required for this check.

The `Splitscreen` workflow also accepts an `sdk_tag` of the form `sdk-<full commit>`. This rebuilds a scratch copy
against the named SDK without changing the repository's pin. These builds are not player releases. Once the mod is
ready and signed, each version goes through the normal [sunrise-mods](https://github.com/sunrise-org/sunrise-mods#submitting-a-project)
submission process.

Licence: [PolyForm Shield 1.0.0](LICENSE). See [CONTRIBUTING.md](CONTRIBUTING.md) for the DCO sign-off.
