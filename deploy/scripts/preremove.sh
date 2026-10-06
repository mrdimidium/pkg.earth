#!/bin/sh
# SPDX-FileCopyrightText: 2026 Nikolay Govorov
# SPDX-License-Identifier: MPL-2.0

set -e

if [ -x "/bin/systemctl" ] && [ -d /run/systemd/system ] && [ -f /usr/lib/systemd/system/pkg-earth.service ]; then
  /bin/systemctl stop pkg-earth.service || true
  /bin/systemctl disable pkg-earth.service || true
fi

if command -v rc-service >/dev/null && [ -f /etc/init.d/pkg-earth ]; then
  rc-service pkg-earth stop || true
  rc-update del pkg-earth || true
fi
