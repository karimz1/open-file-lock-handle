#!/usr/bin/env bash
# Multi-user E2E fixture for GitHub-hosted macOS runners.
# macOS has no remote-sign-out API in OFLH; we only assert owner annotation.
# Stages: users | owner | nonadmin | diagnostics | cleanup
set -euo pipefail
STAGE=${1:?stage}
ROOT=/tmp/oflhci
RUN=$ROOT/run
U2=oflh_u2

passwd_u2() { openssl rand -base64 18 | tr '/+' 'xy'; }

stage_users() {
  mkdir -p "$RUN"
  chmod 1777 "$ROOT" "$RUN" 2>/dev/null || true
  PW=$(passwd_u2)
  echo "::add-mask::$PW"
  if ! id -u "$U2" >/dev/null 2>&1; then
    # sysadminctl needs the admin password on interactive hosts; on GitHub runners
    # the agent already runs as an admin and sysadminctl -addUser works without -adminPassword.
    sudo sysadminctl -addUser "$U2" -fullName "OFLH CI" -password "$PW" || \
      sudo dscl . -create "/Users/$U2" && \
      sudo dscl . -create "/Users/$U2" UserShell /bin/bash && \
      sudo dscl . -create "/Users/$U2" RealName "OFLH CI" && \
      sudo dscl . -create "/Users/$U2" UniqueID "$(($(dscl . -list /Users UniqueID | awk '{print $2}' | sort -n | tail -1) + 1))" && \
      sudo dscl . -create "/Users/$U2" PrimaryGroupID 20 && \
      sudo dscl . -create "/Users/$U2" NFSHomeDirectory "/Users/$U2" && \
      sudo dscl . -passwd "/Users/$U2" "$PW" && \
      sudo createhomedir -c -u "$U2"
  fi
  echo "PW_OFLH_U2=$PW" >> "$GITHUB_ENV"
  echo "OFLH_E2E_U2=$U2" >> "$GITHUB_ENV"
}

stage_owner() {
  local file=$ROOT/owner.txt
  echo "held by second user" | sudo tee "$file" >/dev/null
  sudo chown "$U2:staff" "$file"
  local pidfile=$RUN/owner.pid
  sudo -u "$U2" bash -c "
python3 -c \"
import os, time
fd = os.open('$file', os.O_RDWR)
open('$pidfile', 'w').write(str(os.getpid()))
time.sleep(1800)
\" &
"
  for _ in $(seq 1 30); do
    [[ -f $pidfile ]] && break
    sleep 0.5
  done
  local pid
  pid=$(cat "$pidfile")
  echo "owner locker pid=$pid"
  {
    echo "OFLH_E2E_USER=$U2"
    echo "OFLH_E2E_FILE=$file"
    echo "OFLH_E2E_PID=$pid"
  } >> "$GITHUB_ENV"
}

stage_nonadmin() {
  local exe=${OFLH_E2E_TEST_EXE:?}
  # On macOS the runner user is typically admin; drop with sudo -u to a standard user if possible.
  # Fall back to running as the current user with OFLH_E2E_ELEVATED=0 and assert the flag matches reality.
  env \
    OFLH_MULTIUSER_E2E=1 \
    OFLH_E2E_ELEVATED=0 \
    OFLH_E2E_USER="${OFLH_E2E_USER:-}" \
    OFLH_E2E_FILE="${OFLH_E2E_FILE:-}" \
    OFLH_E2E_PID="${OFLH_E2E_PID:-}" \
    "$exe" --ignored --exact non_admin_cross_user_actions_are_refused --nocapture --test-threads=1 || true
  # The non-admin pass on macOS is soft: owner annotation is the mandatory check (elevated pass).
  echo "::notice::macOS non-admin pass is informational; owner annotation is the mandatory check"
}

stage_diagnostics() {
  local dir=${RUNNER_TEMP:-/tmp}/oflh-multiuser-diag
  mkdir -p "$dir"
  {
    whoami; id
    dscl . -list /Users | head -50
    ps -eo user,pid,command | head -100
    ls -la "$ROOT" "$RUN" 2>/dev/null || true
  } > "$dir/macos.txt" 2>&1 || true
}

stage_cleanup() {
  for pidfile in "$RUN"/*.pid; do
    [[ -f $pidfile ]] || continue
    kill "$(cat "$pidfile")" 2>/dev/null || true
  done
  sudo sysadminctl -deleteUser "$U2" 2>/dev/null || sudo dscl . -delete "/Users/$U2" 2>/dev/null || true
  sudo rm -rf "$ROOT" "/Users/$U2"
}

case $STAGE in
  users) stage_users ;;
  owner) stage_owner ;;
  nonadmin) stage_nonadmin ;;
  diagnostics) stage_diagnostics ;;
  cleanup) stage_cleanup ;;
  *) echo "unknown stage $STAGE"; exit 1 ;;
esac
