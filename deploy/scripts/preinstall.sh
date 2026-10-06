#!/bin/sh
# SPDX-FileCopyrightText: 2026 Nikolay Govorov
# SPDX-License-Identifier: MPL-2.0

set -e

PKG_EARTH_USER=${PKG_EARTH_USER:-pkg-earth}
PKG_EARTH_GROUP=${PKG_EARTH_GROUP:-${PKG_EARTH_USER}}

nologin=/usr/sbin/nologin
[ -x "$nologin" ] || nologin=/sbin/nologin
[ -x "$nologin" ] || nologin=/bin/false

if ! getent group "$PKG_EARTH_GROUP" >/dev/null; then
  if command -v groupadd >/dev/null; then
    groupadd --system "$PKG_EARTH_GROUP"
  else
    addgroup -S "$PKG_EARTH_GROUP"
  fi
fi

if ! getent passwd "$PKG_EARTH_USER" >/dev/null; then
  if command -v useradd >/dev/null; then
    useradd --system --gid "$PKG_EARTH_GROUP" --no-create-home --shell "$nologin" "$PKG_EARTH_USER"
  else
    adduser -S -H -G "$PKG_EARTH_GROUP" -s "$nologin" "$PKG_EARTH_USER"
  fi
fi
