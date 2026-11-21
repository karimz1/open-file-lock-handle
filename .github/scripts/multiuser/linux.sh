#!/usr/bin/env bash
# Multi-user E2E fixture for GitHub-hosted Ubuntu runners (systemd + logind).
# Stages: users | owner | session | nonadmin | diagnostics | cleanup
set -euo pipefail
STAGE=${1:?stage}
ROOT=/tmp/oflhci
RUN=$ROOT/run
U2=oflh_u2
HERE=$(cd "$(dirname "$0")" && pwd)

passwd_u2() { openssl rand -base64 18 | tr '/+' 'xy'; }

stage_users() {
  mkdir -p "$RUN"
  chmod 1777 "$ROOT" "$RUN" 2>/dev/null || true
  PW=$(passwd_u2)
  echo "::add-mask::$PW"
  if ! id -u "$U2" >/dev/null 2>&1; then
    sudo useradd -m -s /bin/bash "$U2"
  fi
  echo "$U2:$PW" | sudo chpasswd
  echo "PW_OFLH_U2=$PW" >> "$GITHUB_ENV"
  echo "OFLH_E2E_U2=$U2" >> "$GITHUB_ENV"
}

# Hold a file open until killed; print the PID then sleep.
locker() {
  local path=$1
  python3 - "$path" <<'PY' &
import os, sys, time
path = sys.argv[1]
fd = os.open(path, os.O_RDWR | os.O_CREAT, 0o644)
os.write(fd, b"held\n")
print(os.getpid(), flush=True)
try:
    time.sleep(1800)
finally:
    os.close(fd)
PY
  echo $!
}

stage_owner() {
  local file=$ROOT/owner.txt
  echo "held by second user" | sudo tee "$file" >/dev/null
  sudo chown "$U2:$U2" "$file"
  local pidfile=$RUN/owner.pid
  rm -f "$pidfile"
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
  [[ -f $pidfile ]] || { echo "owner locker did not start"; return 1; }
  local pid
  pid=$(cat "$pidfile")
  echo "owner locker pid=$pid"
  {
    echo "OFLH_E2E_USER=$U2"
    echo "OFLH_E2E_FILE=$file"
    echo "OFLH_E2E_PID=$pid"
  } >> "$GITHUB_ENV"
}

# Prefer systemd-run with PAMName=login so logind registers a session;
# fall back to ssh localhost with a fresh key.
start_session() {
  local file=$ROOT/session.txt
  echo "held inside logind session" | sudo tee "$file" >/dev/null
  sudo chown "$U2:$U2" "$file"
  local pidfile=$RUN/session.pid
  rm -f "$pidfile"

  if sudo systemd-run --uid="$U2" -p PAMName=login -p WorkingDirectory=/tmp \
      --unit=oflh-session.service --remain-after-exit \
      /bin/bash -c "python3 -c \"
import os, time
fd = os.open('$file', os.O_RDWR)
open('$pidfile', 'w').write(str(os.getpid()))
time.sleep(1800)
\"" 2>"$RUN/systemd-run.err"; then
    echo "started via systemd-run PAMName=login"
    echo "OFLH_E2E_SESSION_VIA=systemd-run" >> "$GITHUB_ENV"
  else
    echo "systemd-run failed; falling back to ssh localhost"
    cat "$RUN/systemd-run.err" || true
    local key=$RUN/id_ed25519
    ssh-keygen -t ed25519 -N '' -f "$key" -q
    sudo mkdir -p "/home/$U2/.ssh"
    sudo cp "${key}.pub" "/home/$U2/.ssh/authorized_keys"
    sudo chown -R "$U2:$U2" "/home/$U2/.ssh"
    sudo chmod 700 "/home/$U2/.ssh"
    sudo chmod 600 "/home/$U2/.ssh/authorized_keys"
    sudo systemctl start ssh || sudo service ssh start || true
    # Keep an interactive SSH session alive; the locker runs inside it.
    ssh -i "$key" -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null \
      "$U2@127.0.0.1" "python3 -c \"
import os, time
fd = os.open('$file', os.O_RDWR)
open('$pidfile', 'w').write(str(os.getpid()))
time.sleep(1800)
\"" >"$RUN/ssh.out" 2>"$RUN/ssh.err" &
    echo $! > "$RUN/ssh.pid"
    echo "OFLH_E2E_SESSION_VIA=ssh" >> "$GITHUB_ENV"
  fi

  for _ in $(seq 1 60); do
    [[ -f $pidfile ]] && break
    sleep 0.5
  done
  [[ -f $pidfile ]] || { echo "session locker did not start"; return 1; }
  local pid
  pid=$(cat "$pidfile")
  echo "session locker pid=$pid"
  # Confirm logind sees a session for U2.
  if command -v loginctl >/dev/null; then
    loginctl list-sessions --no-legend || true
    local sid
    sid=$(loginctl list-sessions --no-legend 2>/dev/null | awk -v u="$U2" '$3==u {print $1; exit}')
    if [[ -n ${sid:-} ]]; then
      echo "logind session $sid for $U2"
      echo "OFLH_E2E_SESSION_ID=$sid" >> "$GITHUB_ENV"
      loginctl show-session "$sid" -p Name -p User -p Type -p Class -p State || true
    else
      echo "::warning::logind has no session for $U2 yet (PAMName= may need a TTY on this image)"
    fi
  fi
  {
    echo "OFLH_E2E_SESSION_USER=$U2"
    echo "OFLH_E2E_SESSION_FILE=$file"
    echo "OFLH_E2E_SESSION_PID=$pid"
  } >> "$GITHUB_ENV"
}

stage_session() { start_session; }

stage_nonadmin() {
  # GitHub-hosted Ubuntu runners are not root; geteuid()!=0 is the non-admin pass.
  local exe=${OFLH_E2E_TEST_EXE:?}
  if [[ $(id -u) -eq 0 ]]; then
    echo "::error::non-admin pass must not run as root"; exit 1
  fi
  env \
    OFLH_MULTIUSER_E2E=1 \
    OFLH_E2E_ELEVATED=0 \
    OFLH_E2E_USER="${OFLH_E2E_USER:-}" \
    OFLH_E2E_FILE="${OFLH_E2E_FILE:-}" \
    OFLH_E2E_PID="${OFLH_E2E_PID:-}" \
    OFLH_E2E_SESSION_USER="${OFLH_E2E_SESSION_USER:-}" \
    OFLH_E2E_SESSION_FILE="${OFLH_E2E_SESSION_FILE:-}" \
    OFLH_E2E_SESSION_PID="${OFLH_E2E_SESSION_PID:-}" \
    "$exe" --ignored --exact non_admin_cross_user_actions_are_refused --nocapture --test-threads=1
}

stage_diagnostics() {
  local dir=${RUNNER_TEMP:-/tmp}/oflh-multiuser-diag
  mkdir -p "$dir"
  {
    whoami; id
    command -v loginctl && loginctl list-sessions --no-legend || true
    command -v loginctl && loginctl list-users --no-legend || true
    ps -eo user,pid,sid,tty,cmd --forest | head -200
    ls -la "$ROOT" "$RUN" 2>/dev/null || true
    cat "$RUN"/*.err "$RUN"/*.out 2>/dev/null || true
  } > "$dir/linux.txt" 2>&1 || true
}

stage_cleanup() {
  if command -v loginctl >/dev/null; then
    for sid in $(loginctl list-sessions --no-legend 2>/dev/null | awk -v u="$U2" '$3==u {print $1}'); do
      sudo loginctl terminate-session "$sid" || true
    done
  fi
  sudo systemctl stop oflh-session.service 2>/dev/null || true
  sudo systemctl reset-failed oflh-session.service 2>/dev/null || true
  if [[ -f $RUN/ssh.pid ]]; then kill "$(cat "$RUN/ssh.pid")" 2>/dev/null || true; fi
  for pidfile in "$RUN"/*.pid; do
    [[ -f $pidfile ]] || continue
    kill "$(cat "$pidfile")" 2>/dev/null || true
  done
  sudo userdel -r "$U2" 2>/dev/null || true
  sudo rm -rf "$ROOT"
}

case $STAGE in
  users) stage_users ;;
  owner) stage_owner ;;
  session) stage_session ;;
  nonadmin) stage_nonadmin ;;
  diagnostics) stage_diagnostics ;;
  cleanup) stage_cleanup ;;
  *) echo "unknown stage $STAGE"; exit 1 ;;
esac
