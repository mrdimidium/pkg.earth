#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Nikolay Govorov
# SPDX-License-Identifier: MPL-2.0

# Starts pkg-earth in a temporary directory, runs smoke tests, cleans up.
# Expects a prebuilt binary at target/release/pkg-earth (run `cargo build --release` first).
#
# Usage:
#   ./tests/smoke/run-local.sh          # default port 2025
#   PKG_EARTH_PORT=9999 ./tests/smoke/run-local.sh

set -euo pipefail

BIN="target/release/pkg-earth"
PORT="${PKG_EARTH_PORT:-2025}"
BASE_URL="http://127.0.0.1:${PORT}"

TMPDIR="$(mktemp -d)"
LOGFILE="$TMPDIR/pkg-earth.log"
trap 'kill "$PID" 2>/dev/null; wait "$PID" 2>/dev/null; rm -rf "$TMPDIR"' EXIT

# Write minimal config
cat > "$TMPDIR/pkg-earth.toml" <<EOF
appname = "smoke"
dirname = "$TMPDIR/state"

[[listen]]
addr = "127.0.0.1:${PORT}"
hostnames = []

[server]
shutdown_timeout = "5s"
request_timeout = "600s"
max_body_size = "64 MB"
max_concurrent_requests = 64
rate_limit_period = "1s"
rate_limit_burst_size = 200

[log]
enabled = true
level = "info"
EOF

mkdir -p "$TMPDIR/state"

# Start pkg-earth
echo "Starting pkg-earth on ${BASE_URL}..."
echo "Server log: ${LOGFILE}"
"$BIN" --config="$TMPDIR/pkg-earth.toml" >"$LOGFILE" 2>&1 &
PID=$!

# Wait for index to load (log shows "index refreshed" for each backend)
echo "Waiting for index to load..."
for i in $(seq 1 120); do
    if grep -q "index refreshed" "$LOGFILE" 2>/dev/null; then
        echo "Index loaded after ${i}s"
        break
    fi
    if ! kill -0 "$PID" 2>/dev/null; then
        echo "pkg-earth exited unexpectedly. Server log:"
        cat "$LOGFILE"
        exit 1
    fi
    sleep 1
done

if ! grep -q "index refreshed" "$LOGFILE" 2>/dev/null; then
    echo "Timed out waiting for index. Server log:"
    cat "$LOGFILE"
    exit 1
fi

# Run smoke tests
PKG_EARTH_URL="${BASE_URL}" ./tests/smoke.sh
