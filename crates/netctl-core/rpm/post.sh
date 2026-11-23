#!/bin/sh
# %post del .rpm de netctl. $1 = 1 en la primera instalación, 2 o más al actualizar.
if [ "$1" -gt 1 ]; then set -- actualizar; else set -- nueva; fi
install -d -m 755 /etc/netctl
install -d -m 700 /var/lib/netctl
chmod 700 /var/lib/netctl
# Solo si aún no hay contraseña: nunca se regenera al actualizar.
if ! grep -qs 'argon2' /var/lib/netctl/admin.json; then
    /usr/bin/netctl-core admin password-defecto
    echo "Entra en https://<IP-del-servidor>:8443 con el usuario «admin»."
fi
# Abre el puerto del panel si hay un firewall de la distro activo (firewalld / ufw).
PUERTO=$(sed -n 's/^[[:space:]]*puerto[[:space:]]*=[[:space:]]*\([0-9]*\).*/\1/p' /etc/netctl/core.toml 2>/dev/null | head -n 1)
PUERTO=${PUERTO:-8443}
if command -v firewall-cmd >/dev/null 2>&1 && firewall-cmd --state >/dev/null 2>&1; then
    firewall-cmd --permanent --add-port="$PUERTO/tcp" >/dev/null 2>&1 || true
    firewall-cmd --add-port="$PUERTO/tcp" >/dev/null 2>&1 || true
    echo "firewalld: abierto el puerto $PUERTO/tcp para el panel."
elif command -v ufw >/dev/null 2>&1 && ufw status 2>/dev/null | grep -q 'Status: active'; then
    ufw allow "$PUERTO/tcp" >/dev/null 2>&1 || true
    echo "ufw: abierto el puerto $PUERTO/tcp para el panel."
fi
if [ -d /run/systemd/system ]; then
    systemctl daemon-reload || true
    systemctl enable netctl.service >/dev/null 2>&1 || true
    if [ "$1" = actualizar ]; then
        systemctl try-restart netctl.service || true
    else
        systemctl start netctl.service || true
    fi
else
    systemctl enable netctl.service >/dev/null 2>&1 || true
fi
