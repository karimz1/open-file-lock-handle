#!/usr/bin/env bash
# Open real multi-window X11 apps under Xvfb with a window manager, then
# compare the windows the termination preview reports for each app's process
# with the windows that are on screen.
set -u
probe=$(realpath "$1")
out=$(realpath -m "$2")
mkdir -p "$out"
work=$(mktemp -d)
failures=0

check() { # case ok message
    if [ "$2" = 1 ]; then echo "PASS $1: $3"; else echo "FAIL $1: $3"; failures=$((failures + 1)); fi
}
screen() { import -window root "$out/$1.png" 2>/dev/null || xwd -root -silent > "$out/$1.xwd" || true; }

# Scan the running executable of $pid, keep the report for that process.
report() { # case pid
    local exe
    exe=$(readlink "/proc/$2/exe")
    "$probe" "$exe" > "$out/$1.json"
    echo "== $1 (pid $2, $exe)"
    cat "$out/$1.json"
    python3 - "$out/$1.json" "$2" > "$out/$1.titles" <<'PY'
import json, sys
rows = [row for row in json.load(open(sys.argv[1])) if row["pid"] == int(sys.argv[2])]
for row in rows:
    for title in row["windows"] or []:
        print(title)
if not rows or rows[0]["windows"] is None:
    print("<windows unavailable>")
PY
    echo "-- preview windows for pid $2:"; cat "$out/$1.titles"
}
has() { grep -qF -- "$2" "$out/$1.titles" && echo 1 || echo 0; }
count() { grep -vc '^<windows unavailable>$' "$out/$1.titles"; }

openbox >/dev/null 2>&1 &
sleep 3
xprop -root _NET_SUPPORTING_WM_CHECK || true

# xterm: one process, one window; its menus and IME helpers must not count.
xterm -T OflhTerm &
term=$!
sleep 3
screen xterm
report xterm "$term"
check xterm "$( [ "$(count xterm)" = 1 ] && has xterm OflhTerm || echo 0)" "exactly the one xterm window"
kill "$term" 2>/dev/null

# Mousepad: one process, one window per document.
for name in alpha bravo charlie; do echo "$name" > "$work/$name.txt"; done
mousepad "$work/alpha.txt" >/dev/null 2>&1 &
sleep 5
mousepad --opening-mode=window "$work/bravo.txt" >/dev/null 2>&1
sleep 2
mousepad --opening-mode=window "$work/charlie.txt" >/dev/null 2>&1
sleep 4
screen mousepad
pid=$(pgrep -o -x mousepad)
if [ -n "$pid" ]; then
    report mousepad "$pid"
    for name in alpha bravo charlie; do check mousepad "$(has mousepad "$name.txt")" "preview lists the window of $name.txt"; done
    check mousepad "$( [ "$(count mousepad)" -ge 3 ] && echo 1 || echo 0)" "preview counts at least 3 windows (counted $(count mousepad))"
    [ "$(pgrep -c -x mousepad)" = 1 ] && echo "mousepad runs as a single process"
else
    check mousepad 0 "mousepad did not start"
fi
pkill -x mousepad

# Firefox: several windows of one browser process.
if command -v firefox >/dev/null; then
    profile=$(mktemp -d)
    for name in OflhOne OflhTwo OflhThree; do echo "<title>$name</title>$name" > "$work/$name.html"; done
    MOZ_ENABLE_WAYLAND=0 firefox --no-remote --profile "$profile" "file://$work/OflhOne.html" >/dev/null 2>&1 &
    sleep 20
    for name in OflhTwo OflhThree; do
        MOZ_ENABLE_WAYLAND=0 firefox --profile "$profile" --new-window "file://$work/$name.html" >/dev/null 2>&1
        sleep 6
    done
    sleep 5
    screen firefox
    pid=$(xprop -id "$(xdotool search --name OflhOne | head -n 1)" _NET_WM_PID 2>/dev/null | awk '{print $3}')
    if [ -n "$pid" ]; then
        report firefox "$pid"
        for name in OflhOne OflhTwo OflhThree; do check firefox "$(has firefox "$name")" "preview lists the window $name"; done
    else
        check firefox 0 "no Firefox window with a _NET_WM_PID"
    fi
    pkill -f "$profile"
else
    echo "Firefox not installed; skipped"
fi

if [ "$failures" -gt 0 ]; then echo "$failures check(s) failed"; exit 1; fi
echo "All real-app window checks passed."
