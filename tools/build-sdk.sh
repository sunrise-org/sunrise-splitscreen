#!/usr/bin/env bash
set -euo pipefail

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
build=${DYNA_CI_BUILD:-$root/target}
mkdir -p "$build" "$root/target"
scratch=$(mktemp -d "$build/splitscreen.XXXXXXXX")
trap 'rm -rf -- "$scratch"' EXIT
project=$scratch/project
mkdir -p "$project" "$scratch/work"
cp -a "$root/"{Cargo.toml,Cargo.lock,LICENSE,README.md,sunrise-mod.toml,src} "$project/"
cp -- "$root/LICENSES/MIT-Horizon.txt" "$project/LICENSE-MIT.txt"
for dir in assets data; do
    if [[ -d "$root/$dir" ]]; then cp -a "$root/$dir" "$project/"; fi
done
sdk=$(python3 "$root/tools/fetch-sdk.py" "$project" "$scratch" "${SDK_TAG:-}")
read -r rust tag <<< "$sdk"

host=x86_64-unknown-linux-gnu
if [[ -n ${SPLITSCREEN_RUST_DIR:-} ]]; then
    toolchain=$SPLITSCREEN_RUST_DIR
else
    url=https://static.rust-lang.org/rustup/dist/$host/rustup-init
    curl_args=(--fail --silent --show-error --location --retry 2 --proto '=https')
    export RUSTUP_HOME="$scratch/rustup"
    mkdir "$scratch/bootstrap"
    (
        export CARGO_HOME="$scratch/bootstrap/cargo"
        cd -- "$scratch/bootstrap"
        curl "${curl_args[@]}" "$url" -o rustup-init
        curl "${curl_args[@]}" "$url.sha256" -o rustup-init.sha256
        sha256sum --strict --check rustup-init.sha256
        chmod +x rustup-init
        ./rustup-init -y --no-modify-path --default-toolchain none
    )
    CARGO_HOME="$scratch/bootstrap/cargo" "$scratch/bootstrap/cargo/bin/rustup" \
        toolchain install "$rust" --profile minimal -c rustfmt -c clippy
    toolchain=$RUSTUP_HOME/toolchains/$rust-$host
fi
export PATH="$toolchain/bin:$PATH"
if ! rustc -Vv | awk -v host="$host" -v rust="$rust" '
    $1 == "host:" { matched_host = ($2 == host) }
    $1 == "release:" { matched_rust = ($2 == rust) }
    END { exit !(matched_host && matched_rust) }
'; then
    echo "The SDK requires rustc $rust for $host" >&2
    exit 1
fi

cd -- "$project"
find src -name '*.rs' -print0 | xargs -0 rustfmt --check --edition 2024
cargo clippy --all-targets --locked -- -D warnings

tool=$scratch/tool/cargo-sunrise-mod
jq -nc --arg dir "$toolchain" --arg host "$host" --arg cargo "${CARGO_HOME:-$HOME/.cargo}" \
    --arg s "$scratch" --arg project "$project" '{
      contract: 2,
      work: ($s + "/work"),
      toolchain: {dir: $dir, target: $host, cargo_home: $cargo, cargo_config: [], c_bin: null},
      package: {runtime: ($s + "/runtime"), api: ($s + "/api"), engine: ($s + "/engine")},
      mods: $project,
      target: "client",
      registry: false,
    }' | "$tool" native-build | tee "$scratch/native-build.jsonl"
tail -n 1 "$scratch/native-build.jsonl" | jq -e '.event == "built" and .route == "prebuilt"' >/dev/null
"$tool" sunrise-mod build
package=target/sunrise-mod/splitscreen.smod
"$tool" sunrise-mod check "$package" --kind native-mod
mkdir -p "$root/target/$tag"
cp -- "$package" "$root/target/$tag/"
(cd -- "$root/target/$tag" && sha256sum splitscreen.smod > SHA256SUMS)
printf 'Unsigned %s\n' "target/$tag/splitscreen.smod"
if [[ -n ${GITHUB_OUTPUT:-} ]]; then echo "sdk_tag=$tag" >> "$GITHUB_OUTPUT"; fi
