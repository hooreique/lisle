#!/usr/bin/env bash
set -euo pipefail

work_dir="$(mktemp -d)"
trap 'rm -rf "$work_dir"' EXIT

mkdir -p \
  "$work_dir/home" \
  "$work_dir/config" \
  "$work_dir/cache" \
  "$work_dir/runtime"
chmod 700 "$work_dir/runtime"

export HOME="$work_dir/home"
export XDG_CONFIG_HOME="$work_dir/config"
export XDG_CACHE_HOME="$work_dir/cache"
export XDG_RUNTIME_DIR="$work_dir/runtime"
export IBUS_ADDRESS_FILE="$work_dir/ibus-address"
unset DISPLAY IBUS_ADDRESS IBUS_USE_PORTAL WAYLAND_DISPLAY

timeout 30s dbus-run-session --config-file="$DBUS_SESSION_CONF" -- bash -euo pipefail -c '
  ibus_pid=
  cleanup() {
    if [[ -n "$ibus_pid" ]]; then
      kill "$ibus_pid" >/dev/null 2>&1 || true
      wait "$ibus_pid" >/dev/null 2>&1 || true
    fi
  }
  trap cleanup EXIT

  ibus-daemon --single --cache=none &
  ibus_pid=$!
  ready=false
  for _ in $(seq 1 100); do
    if ibus list-engine --name-only >/dev/null 2>&1; then
      ready=true
      break
    fi
    sleep 0.05
  done
  if [[ "$ready" != true ]]; then
    echo "isolated ibus-daemon did not become ready" >&2
    exit 1
  fi

  ibus list-engine --name-only | grep -Fx lisle

  # The CLI may fail its post-selection X11 setxkbmap step in this headless test.
  ibus engine lisle >/dev/null 2>&1 || true
  if [[ "$(ibus engine)" != lisle ]]; then
    echo "isolated ibus-daemon did not select Lisle" >&2
    exit 1
  fi

  address="$(ibus address)"
  owner="$(
    gdbus call \
      --address "$address" \
      --dest org.freedesktop.DBus \
      --object-path /org/freedesktop/DBus \
      --method org.freedesktop.DBus.NameHasOwner \
      org.freedesktop.IBus.Lisle
  )"
  if [[ "$owner" != "(true,)" ]]; then
    echo "Lisle component did not claim its IBus bus name: $owner" >&2
    exit 1
  fi

  engine_reply="$(
    gdbus call \
      --address "$address" \
      --dest org.freedesktop.IBus.Lisle \
      --object-path /org/freedesktop/IBus/Factory \
      --method org.freedesktop.IBus.Factory.CreateEngine \
      lisle
  )"
  if [[ "$engine_reply" =~ (/org/freedesktop/IBus/Engine/[0-9]+) ]]; then
    engine_path="${BASH_REMATCH[1]}"
  else
    echo "could not parse activated Lisle engine: $engine_reply" >&2
    exit 1
  fi

  result="$(
    gdbus call \
      --address "$address" \
      --dest org.freedesktop.IBus.Lisle \
      --object-path "$engine_path" \
      --method org.freedesktop.IBus.Engine.ProcessKeyEvent \
      $((0x66)) 18 0
  )"
  if [[ "$result" != "(false,)" ]]; then
    echo "activated Lisle did not pass through the Roman key: $result" >&2
    exit 1
  fi

  for event in \
    "$((0xffe2)) 54 1" \
    "$((0xffe2)) 54 $((1 | 1 << 30))" \
    "$((0x6b)) 37 0"
  do
    result="$(
      gdbus call \
        --address "$address" \
        --dest org.freedesktop.IBus.Lisle \
        --object-path "$engine_path" \
        --method org.freedesktop.IBus.Engine.ProcessKeyEvent \
        $event
    )"
    if [[ "$result" != "(true,)" ]]; then
      echo "activated Lisle did not handle the Hangul event: $result" >&2
      exit 1
    fi
  done
'
