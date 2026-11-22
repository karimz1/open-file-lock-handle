#!/usr/bin/env bash
# Run the suite on Linux inside a virtual X display with its own session bus, e.g.
#   xvfb-run -a -s '-screen 0 1600x1000x24' dbus-run-session -- ./run-linux.sh [wdio args]
# A window manager is needed for focus and maximize behavior to match a real desktop.
set -euo pipefail
cd "$(dirname "$0")"
openbox >/dev/null 2>&1 &
manager=$!
trap 'kill "$manager" 2>/dev/null || true' EXIT
sleep 1
npm test -- "$@"
