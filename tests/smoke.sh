#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Nikolay Govorov
# SPDX-License-Identifier: MPL-2.0

# Black-box smoke tests for a running pkg.earth instance.
# Usage: PKG_EARTH_URL=https://pkg.earth ./tests/smoke.sh

set -euo pipefail

: "${PKG_EARTH_URL:?Usage: PKG_EARTH_URL=https://pkg.earth ./tests/smoke.sh}"
base_url=${PKG_EARTH_URL%/}
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

CURL=(curl --fail --location --silent --show-error --max-time 600)

get() {
    "${CURL[@]}" --output "$2" "$1"
}

sha256_file() {
    local result
    result=$(sha256sum "$1")
    printf '%s' "${result%% *}"
}

check() {
    local name=$1
    local line
    shift
    printf '  %-68s' "$name"
    if "$@" 2>"$work/check-error"; then
        echo 'ok'
    else
        echo 'FAIL'
        while IFS= read -r line; do
            printf '       %s\n' "$line"
        done <"$work/check-error"
        exit 1
    fi
}

check_status() {
    local url=$1
    local expected=$2
    local status
    status=$(curl --location --silent --show-error --max-time 600 \
        --output /dev/null --write-out '%{http_code}' "$url") || return
    if [ "$status" != "$expected" ]; then
        echo "expected HTTP $expected, got $status for $url" >&2
        return 1
    fi
}

check_home() {
    local document="$work/index.html"
    local foundation application
    get "$base_url/" "$document" || return
    grep -q 'pkg\.earth' "$document" || return
    grep -q '/-/assets/manifest\.webmanifest' "$document" || return
    foundation=$(grep -Eom1 '/-/assets/foundation\.[0-9a-f]{16}\.css' "$document") || return
    application=$(grep -Eom1 '/-/assets/application\.[0-9a-f]{16}\.css' "$document") || return
    get "$base_url$foundation" "$work/foundation.css" || return
    get "$base_url$application" "$work/application.css" || return
    grep -q 'IBM Plex Sans' "$work/foundation.css" || return
    grep -q -- '--color-text' "$work/application.css"
}

check_page_contains() {
    get "$base_url$1" "$work/page" || return
    grep -q -- "$2" "$work/page"
}

check_go_file() {
    local file=$1
    local mirror="$work/go-mirror"
    local upstream="$work/go-upstream"
    local checksum="$work/go-checksum"
    local mirror_hash upstream_hash expected

    get "$base_url/go/$file" "$mirror" || return
    get "https://go.dev/dl/$file" "$upstream" || return
    get "$base_url/go/$file.sha256" "$checksum" || return

    mirror_hash=$(sha256_file "$mirror")
    upstream_hash=$(sha256_file "$upstream")
    expected=$(tr -d '[:space:]' <"$checksum")
    if [ "$mirror_hash" != "$upstream_hash" ] || [ "$mirror_hash" != "$expected" ]; then
        echo "checksum mismatch for $file" >&2
        return 1
    fi
}

check_zig_file() {
    local file=$1
    local version=$2
    local mirror="$work/zig-mirror"
    local upstream="$work/zig-upstream"
    local mirror_hash upstream_hash

    get "$base_url/zig/$file" "$mirror" || return
    get "https://ziglang.org/download/$version/$file" "$upstream" || return
    mirror_hash=$(sha256_file "$mirror")
    upstream_hash=$(sha256_file "$upstream")
    if [ "$mirror_hash" != "$upstream_hash" ]; then
        echo "checksum mismatch for $file" >&2
        return 1
    fi

    get "$base_url/zig/$file.minisig" "$mirror.minisig" || return
    get "https://ziglang.org/download/$version/$file.minisig" "$upstream.minisig" || return
    if ! cmp --silent "$mirror.minisig" "$upstream.minisig"; then
        echo "minisig mismatch for $file" >&2
        return 1
    fi
}

printf 'Smoke tests against %s\n\n' "$base_url"
check 'web: home page and styles' check_home
for asset in \
    /favicon.ico \
    /apple-touch-icon.png \
    /robots.txt \
    /-/assets/favicon.svg \
    /-/assets/manifest.webmanifest \
    /-/assets/icon-192.png \
    /-/assets/icon-512.png \
    /-/assets/fonts/ibm-plex-math-1.1.0-regular.woff2; do
    check "web: $asset" check_status "$base_url$asset" 200
done
check 'web: licenses page' check_page_contains /about/licenses MPL-2.0
check 'web: missing page' check_status "$base_url/does-not-exist.xyz" 404

for file in \
    go1.23.0.linux-amd64.tar.gz \
    go1.23.0.darwin-arm64.tar.gz \
    go1.21.0.windows-amd64.zip; do
    check "go: $file and checksum match upstream" check_go_file "$file"
done
check 'go: missing version' check_status \
    "$base_url/go/go99.99.99.linux-amd64.tar.gz" 404

while read -r file version; do
    check "zig: $file and minisig match upstream" check_zig_file "$file" "$version"
done <<'EOF'
zig-0.15.2.tar.xz 0.15.2
zig-x86_64-windows-0.15.2.zip 0.15.2
zig-aarch64-macos-0.15.2.tar.xz 0.15.2
zig-aarch64-netbsd-0.15.2.tar.xz 0.15.2
zig-powerpc64le-freebsd-0.15.2.tar.xz 0.15.2
zig-x86_64-linux-0.14.1.tar.xz 0.14.1
zig-aarch64-linux-0.14.1.tar.xz 0.14.1
zig-armv7a-linux-0.14.1.tar.xz 0.14.1
zig-riscv64-linux-0.14.1.tar.xz 0.14.1
zig-powerpc64le-linux-0.14.1.tar.xz 0.14.1
zig-x86-linux-0.14.1.tar.xz 0.14.1
zig-loongarch64-linux-0.14.1.tar.xz 0.14.1
zig-s390x-linux-0.14.1.tar.xz 0.14.1
zig-0.10.1.tar.xz 0.10.1
zig-bootstrap-0.10.1.tar.xz 0.10.1
zig-linux-i386-0.10.1.tar.xz 0.10.1
zig-macos-aarch64-0.10.1.tar.xz 0.10.1
zig-windows-x86_64-0.10.1.zip 0.10.1
zig-0.7.1.tar.xz 0.7.1
zig-linux-x86_64-0.7.1.tar.xz 0.7.1
zig-0.6.0.tar.xz 0.6.0
zig-linux-x86_64-0.6.0.tar.xz 0.6.0
zig-win64-0.1.1.zip 0.1.1
EOF
check 'zig: missing version' check_status \
    "$base_url/zig/zig-x86_64-linux-99.99.99.tar.xz" 404

echo
echo 'All smoke tests passed.'
