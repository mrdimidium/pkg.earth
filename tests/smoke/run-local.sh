#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Nikolay Govorov
# SPDX-License-Identifier: AGPL-3.0-or-later

# Starts tesor in a temporary directory, runs smoke tests, cleans up.
# Expects a prebuilt binary at target/release/tesor (run `cargo build --release` first).
#
# Usage:
#   ./tests/smoke/run-local.sh          # default port 2025
#   TESOR_PORT=9999 ./tests/smoke/run-local.sh

set -euo pipefail

BIN="target/release/tesor"
PORT="${TESOR_PORT:-2025}"
BASE_URL="http://127.0.0.1:${PORT}"

TMPDIR="$(mktemp -d)"
LOGFILE="$TMPDIR/tesor.log"
trap 'kill "$PID" 2>/dev/null; wait "$PID" 2>/dev/null; rm -rf "$TMPDIR"' EXIT

# Write minimal config
cat > "$TMPDIR/tesor.toml" <<EOF
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

[telemetry.stdout]
enabled = true
log_level = "info"
log_format = "pretty"
EOF

mkdir -p "$TMPDIR/state"

# Start tesor
echo "Starting tesor on ${BASE_URL}..."
echo "Server log: ${LOGFILE}"
"$BIN" --config="$TMPDIR/tesor.toml" >"$LOGFILE" 2>&1 &
PID=$!

# Wait for index to load (log shows "index refreshed" for each backend)
echo "Waiting for index to load..."
for i in $(seq 1 120); do
    if grep -q "index refreshed" "$LOGFILE" 2>/dev/null; then
        echo "Index loaded after ${i}s"
        break
    fi
    if ! kill -0 "$PID" 2>/dev/null; then
        echo "tesor exited unexpectedly. Server log:"
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
TESOR_URL="${BASE_URL}" cargo run -p smoke
