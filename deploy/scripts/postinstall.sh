#!/bin/sh
# SPDX-FileCopyrightText: 2026 Nikolay Govorov
# SPDX-License-Identifier: MPL-2.0

set -e

if [ -x "/bin/systemctl" ] && [ -d /run/systemd/system ] && [ -f /usr/lib/systemd/system/pkg-earth.service ]; then
  /bin/systemctl daemon-reload
  /bin/systemctl enable pkg-earth
fi

if command -v rc-update >/dev/null && [ -f /etc/init.d/pkg-earth ]; then
  rc-update add pkg-earth default
fi
