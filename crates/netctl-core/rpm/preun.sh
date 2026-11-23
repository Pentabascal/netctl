#!/bin/sh
# %preun del .rpm de netctl: $1 = 0 al desinstalar (no al actualizar).
if [ "$1" -eq 0 ] && [ -d /run/systemd/system ]; then
    systemctl disable --now netctl.service >/dev/null 2>&1 || true
fi
