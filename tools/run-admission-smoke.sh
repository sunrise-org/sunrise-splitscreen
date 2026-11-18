#!/usr/bin/env bash
# Called by build-sdk.sh for an opt-in test on a machine with private prepared resources.
set -euo pipefail
project=$1
result=$2
scratch=$(dirname -- "$result")
mkdir -p "$scratch"/{install,resources}
cp -a -- "${SPLITSCREEN_TEST_RESOURCES%/}/." "$scratch/resources/"
jq -er '.binary, .libraries[]' "$result" | xargs -d '\n' cp -L -t "$scratch/install"
jq '{libraries: .records}' "$result" > "$scratch/install/native-mods.json"
mod=$scratch/install/sunrise-native-mods/splitscreen
mkdir -p "$mod"
cp -a -- "$project/"{Cargo.toml,Cargo.lock,sunrise-mod.toml,src} "$mod/"

for mode in loading direct; do
    root=$scratch/$mode
    mkdir -p "$root"/{home,launcher,data,config,game}
    flags=()
    if [[ "$mode" == direct ]]; then flags+=(--frames 10000); fi
    (
        cd -- "$root/game"
        export SUNRISE_HOME="$root/home" SUNRISE_LAUNCHER_HOME="$root/launcher"
        export XDG_DATA_HOME="$root/data" XDG_CONFIG_HOME="$root/config"
        export SUNRISE_NO_AUDIO=1 SUNRISE_LAUNCHER_NO_SELF_UPDATE=1
        timeout --kill-after=5s 180s xvfb-run -a "$scratch/install/sunrise-client" \
            --game-dir "$root/game" --resources "$scratch/resources" --developer-single-player \
            --world Admission --seed 123 --creative --width 520 --height 340 "${flags[@]}"
    ) > "$root/run.log" 2>&1 || {
        echo "The $mode admission fixture failed to exit normally" >&2
        grep '^admission fixture:' "$root/run.log" >&2 || true
        exit 1
    }
    grep -Fxq 'admission fixture: wrong pipe and duplicate name refused' "$root/run.log"
    grep -Fxq 'admission fixture: both players online, no guest operator grant' "$root/run.log"
    grep -Fxq 'admission fixture: PASS (owner stayed; unreserved replay refused)' "$root/run.log"
    printf '%s admission smoke passed\n' "$mode"
done
