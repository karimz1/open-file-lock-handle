#!/usr/bin/env bash
# Prefetch the NSIS files tauri-bundler (from @tauri-apps/cli) would otherwise
# download from GitHub on every `tauri bundle --bundles nsis`. An unauthenticated
# fetch can fail with HTTP 403 under rate limiting.
#
# These URLs and hashes are from tauri-cli v2.12.0:
#   crates/tauri-bundler/src/bundle/windows/nsis/mod.rs
# If package-lock.json's @tauri-apps/cli version changes, update this script
# from that tag before the next CI run.
set -euo pipefail

EXPECTED_CLI_VERSION="2.12.0"
NSIS_ZIP_URL="https://github.com/tauri-apps/binary-releases/releases/download/nsis-3.11/nsis-3.11.zip"
NSIS_ZIP_SHA1="EF7FF767E5CBD9EDD22ADD3A32C9B8F4500BB10D"
UTILS_URL="https://github.com/tauri-apps/nsis-tauri-utils/releases/download/nsis_tauri_utils-v0.5.3/nsis_tauri_utils.dll"
UTILS_SHA1="75197FEE3C6A814FE035788D1C34EAD39349B860"

root="${GITHUB_WORKSPACE:-}"
if [[ -z "$root" || ! -f "$root/crates/oflh-desktop/ui/package-lock.json" ]]; then
  root=$(cd "$(dirname "$0")/../.." && pwd)
fi
lock="$root/crates/oflh-desktop/ui/package-lock.json"
cli_version=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["packages"]["node_modules/@tauri-apps/cli"]["version"])' "$lock")
if [[ "$cli_version" != "$EXPECTED_CLI_VERSION" ]]; then
  echo "prefetch-tauri-nsis: @tauri-apps/cli is $cli_version, but this script still lists the NSIS URLs for $EXPECTED_CLI_VERSION." >&2
  echo "Update the URLs and SHA1s from tauri-bundler at tag tauri-cli-v${cli_version} (crates/tauri-bundler/src/bundle/windows/nsis/mod.rs)." >&2
  exit 1
fi

if [[ "${OS:-}" == "Windows_NT" ]]; then
  if [[ -z "${LOCALAPPDATA:-}" ]]; then
    echo "prefetch-tauri-nsis: LOCALAPPDATA is not set." >&2
    exit 1
  fi
  tauri_cache="${LOCALAPPDATA}/tauri"
  need_nsis_zip=1
else
  tauri_cache="${XDG_CACHE_HOME:-$HOME/.cache}/tauri"
  need_nsis_zip=0
fi

nsis_dir="$tauri_cache/NSIS"
dll="$nsis_dir/Plugins/x86-unicode/additional/nsis_tauri_utils.dll"

sha1_of() {
  # Print the SHA-1 in uppercase, matching tauri-bundler's constants.
  python3 -c 'import hashlib,sys; print(hashlib.sha1(open(sys.argv[1],"rb").read()).hexdigest().upper())' "$1"
}

file_has_sha1() {
  local path="$1" expected="$2"
  [[ -f "$path" ]] && [[ "$(sha1_of "$path")" == "$expected" ]]
}

# Authenticated GitHub downloads avoid the unauthenticated 403 that failed
# the frontend NSIS preflight. The token is only passed to curl as a header.
download() {
  local url="$1" dest="$2"
  local tmp="${dest}.tmp"
  mkdir -p "$(dirname "$dest")"
  local args=(-fsSL --retry 5 --retry-delay 2 --retry-all-errors -o "$tmp" -- "$url")
  local token="${GITHUB_TOKEN:-${GH_TOKEN:-}}"
  if [[ -n "$token" ]]; then
    args=(-fsSL --retry 5 --retry-delay 2 --retry-all-errors -H "Authorization: Bearer ${token}" -H "Accept: application/octet-stream" -o "$tmp" -- "$url")
  fi
  if ! curl "${args[@]}"; then
    rm -f "$tmp"
    echo "prefetch-tauri-nsis: download failed for $url" >&2
    exit 1
  fi
  mv "$tmp" "$dest"
}

download_and_verify() {
  local url="$1" dest="$2" expected="$3"
  download "$url" "$dest"
  local actual
  actual=$(sha1_of "$dest")
  if [[ "$actual" != "$expected" ]]; then
    rm -f "$dest"
    echo "prefetch-tauri-nsis: SHA-1 mismatch for $url (got $actual, want $expected)" >&2
    exit 1
  fi
}

if [[ "$need_nsis_zip" == 1 ]]; then
  # The SHA-1 is of the zip archive, not of makensis.exe. Recreate the tree
  # when the expected Windows NSIS binaries are missing.
  if [[ ! -f "$nsis_dir/makensis.exe" || ! -f "$nsis_dir/Bin/makensis.exe" ]]; then
    zip="$tauri_cache/nsis-3.11.zip"
    download_and_verify "$NSIS_ZIP_URL" "$zip" "$NSIS_ZIP_SHA1"
    rm -rf "$nsis_dir" "$tauri_cache/nsis-3.11"
    python3 - "$zip" "$tauri_cache" <<'PY'
import sys, zipfile
zip_path, dest = sys.argv[1], sys.argv[2]
with zipfile.ZipFile(zip_path) as archive:
    archive.extractall(dest)
PY
    mv "$tauri_cache/nsis-3.11" "$nsis_dir"
    rm -f "$zip"
  fi
fi

if ! file_has_sha1 "$dll" "$UTILS_SHA1"; then
  download_and_verify "$UTILS_URL" "$dll" "$UTILS_SHA1"
fi

echo "prefetch-tauri-nsis: ready at $nsis_dir"
