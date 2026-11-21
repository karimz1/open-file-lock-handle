#!/usr/bin/env bash
# Build the desktop app from a local checkout without packaging an installer.
# Usage: scripts/build-dev.sh [--skip-npm-ci] [--run]
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"

run=0
skip_npm_ci=0
while [ "$#" -gt 0 ]; do
  case "$1" in
    --run) run=1 ;;
    --skip-npm-ci) skip_npm_ci=1 ;;
    -h|--help)
      sed -n '2,3p' "$0"
      echo "Options:"
      echo "  --skip-npm-ci  Skip npm ci when node_modules is already installed"
      echo "  --run          Start the built executable when the build finishes"
      exit 0
      ;;
    *)
      echo "unknown option: $1" >&2
      echo "usage: scripts/build-dev.sh [--skip-npm-ci] [--run]" >&2
      exit 2
      ;;
  esac
  shift
done

missing=()
for cmd in node npm cargo; do
  if ! command -v "$cmd" >/dev/null 2>&1; then
    missing+=("$cmd")
  fi
done
if [ "${#missing[@]}" -ne 0 ]; then
  echo "missing required tools: ${missing[*]}" >&2
  echo "install Node.js, npm, and the Rust toolchain from docs/development.md, then retry." >&2
  exit 1
fi

commit=${OFLH_BUILD_COMMIT:-}
if [ -z "$commit" ] && command -v git >/dev/null 2>&1 && git -C "$root" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  commit=$(git -C "$root" rev-parse --short HEAD)
  if [ -n "$(git -C "$root" status --porcelain)" ]; then
    commit="$commit-dirty"
  fi
fi
if [ -n "$commit" ]; then
  export OFLH_BUILD_COMMIT=$commit
  echo "building OFLH desktop at commit $commit"
else
  echo "building OFLH desktop (commit hash unavailable)"
fi

if [ "$skip_npm_ci" -eq 0 ]; then
  npm --prefix crates/oflh-desktop/ui ci
elif [ ! -d crates/oflh-desktop/ui/node_modules ]; then
  echo "node_modules is missing; run without --skip-npm-ci" >&2
  exit 1
fi

npm --prefix crates/oflh-desktop/ui run tauri -- build --no-bundle -- --locked

exe=target/release/oflh-desktop
if [ ! -x "$exe" ]; then
  echo "expected $exe after a successful build" >&2
  exit 1
fi
echo "built $exe"
if [ "$run" -eq 1 ]; then
  exec "$exe"
fi
