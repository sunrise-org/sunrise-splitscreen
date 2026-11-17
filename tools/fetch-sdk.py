"""Fetch a Linux client SDK. No asset is unpacked or run before signature verification."""

import base64
import hashlib
import json
import re
import subprocess
import sys
import tomllib
from pathlib import Path

# Public SDK verification key, not a mod signing key. Update only with Sunrise's release key rotation.
SDK_KEY = "r53xb3cuafGxZSicLz06KC+xXp9NJdPH8snNmXo5pAc="
BASE = "https://github.com/sunrise-org/sunrise/releases/download"
HOST = "x86_64-unknown-linux-gnu"
SMALL = 16 << 20


def fetch(name, limit):
    path = scratch / name
    subprocess.run(
        ["curl", "--fail", "--silent", "--show-error", "--location", "--retry", "2",
         "--proto", "=https", "--max-filesize", str(limit), "-o", path, f"{BASE}/{tag}/{name}"],
        check=True,
    )
    return path


def verified_file(name, limit):
    path = fetch(name, limit)
    with path.open("rb") as file:
        sha = hashlib.file_digest(file, "sha256").hexdigest()
    if sha != sums.get(name):
        sys.exit(f"{tag}/{name} does not match the signed checksum")
    return path


project, out = map(Path, sys.argv[1:3])
manifest = (project / "sunrise-mod.toml").read_text()
pin = tomllib.loads(manifest)["native"]
tag = sys.argv[3] or f"sdk-{pin['sunrise']}"
if not re.fullmatch(r"sdk-[0-9a-f]{40}", tag):
    sys.exit("Expected sdk- followed by the full 40-character game commit")
commit = tag.removeprefix("sdk-")
scratch = out / "download"
scratch.mkdir()
checksums = fetch("SHA256SUMS", SMALL).read_bytes()
signatures = fetch("SHA256SUMS.sig", 8192).read_bytes().splitlines()
message = scratch / "message"
# The release signature domain is stable across game renames.
message.write_bytes(b"horizon release " + tag.encode("ascii") + b"\n" + checksums)
key = scratch / "sdk-public.der"
key.write_bytes(bytes.fromhex("302a300506032b6570032100") + base64.b64decode(SDK_KEY))
signature = scratch / "signature"
for encoded in signatures:
    if not re.fullmatch(rb"[A-Za-z0-9+/]{86}==", encoded):
        continue
    signature.write_bytes(base64.b64decode(encoded, validate=True))
    result = subprocess.run(
        ["openssl", "pkeyutl", "-verify", "-rawin", "-pubin", "-keyform", "DER",
         "-inkey", key, "-in", message, "-sigfile", signature],
        capture_output=True,
    )
    if result.returncode == 0:
        break
else:
    sys.exit(f"{tag} has no signature from the trusted Sunrise SDK key")

sums = {name: sha for sha, name in (line.split() for line in checksums.decode().splitlines())}
index = verified_file("sdk.json", SMALL)
sdk = json.loads(index.read_bytes())
if sdk["build"]["commit"] != commit:
    sys.exit(f"{tag} has an index for a different build")
if commit == pin["sunrise"]:
    if (sdk["build"]["game"], sdk["mixins"]["rev"]) != (pin["game"], pin["mixins"]):
        sys.exit(f"{tag} does not match the manifest's game and mixins pins")
else:
    # The build recipe always passes a scratch copy, never the checkout.
    before, native = manifest.split("[native]\n", 1)
    values = {"sunrise": commit, "game": sdk["build"]["game"], "mixins": sdk["mixins"]["rev"]}
    for field, value in values.items():
        native = re.sub(
            rf'^{field} = "[^"\n]*"$',
            lambda _: f"{field} = {json.dumps(value)}",
            native, count=1, flags=re.MULTILINE,
        )
    (project / "sunrise-mod.toml").write_text(before + "[native]\n" + native)

cache = project / ".sunrise" / "sdk"
cache.mkdir(parents=True)
for name, folder in {
    f"runtime-{HOST}.tar.xz": out / "runtime",
    f"api-{HOST}.tar.xz": out / "api",
    f"engine-{HOST}.tar.xz": out / "engine",
    f"tool-{HOST}.tar.xz": out / "tool",
    "api-source.tar.xz": cache / "api",
}.items():
    archive = verified_file(name, sdk["assets"][name]["size"])
    folder.mkdir(parents=True)
    subprocess.run(["tar", "-xJf", archive, "-C", folder, "--no-same-owner"], check=True)
    archive.unlink()
verified_file("surface.json", SMALL).replace(cache / "surface.json")
index.replace(cache / "sdk.json")
# Stamp the cache only after verifying all its inputs, like cargo-sunrise-mod does.
(cache / "build").write_text(commit)
rust = sdk["toolchain"]["rust"]
if not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+", rust):
    sys.exit(f"{tag} does not pin a Rust release")
print(rust, tag)
