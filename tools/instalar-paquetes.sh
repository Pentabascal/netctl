#!/bin/sh
# Se ejecuta como root en el servidor de pruebas, en la carpeta con los paquetes (la copia
# tools/probar-en-servidor.sh en /tmp/netctl-prueba). Instala el núcleo y los módulos igual que
# el panel: extrae, paquetes del sistema, unidad systemd, arranque y espera a /salud.
cd "$(dirname "$0")"
set -eu
. /etc/os-release
FAM=""
case " ${ID:-} ${ID_LIKE:-} " in
    *" debian "*|*" ubuntu "*) FAM=debian ;;
    *" suse "*|*" opensuse "*|*" sles "*) FAM=suse ;;
    *" rhel "*|*" fedora "*|*" centos "*) FAM=rhel ;;
esac
paquetes() {
    [ $# -gt 0 ] || return 0
    case "$FAM" in
        debian) DEBIAN_FRONTEND=noninteractive apt-get install -y "$@" ;;
        suse) zypper --non-interactive install "$@" ;;
        rhel) dnf install -y "$@" ;;
        *) echo "Distribución no reconocida; instala a mano: $*" >&2; return 1 ;;
    esac
}
echo "-- Núcleo"
case "$FAM" in
    debian) apt-get install -y --allow-downgrades ./netctl_*_amd64.deb ;;
    suse) zypper --non-interactive install --allow-unsigned-rpm ./netctl-*.x86_64.rpm ;;
    rhel) dnf install -y ./netctl-*.x86_64.rpm ;;
    *) echo "Distribución no reconocida" >&2; exit 1 ;;
esac

MODULOS=/usr/lib/netctl/modules
mkdir -p "$MODULOS"
for T in netctl-mod-*-x86_64.tar.gz; do
    [ -f "$T" ] || continue
    NUEVO=$(mktemp -d)
    tar -xzf "$T" -C "$NUEVO"
    MID=$(sed -n 's/^id *= *"\(.*\)"/\1/p' "$NUEVO/manifest.toml" | head -n1)
    NOMBRE=$(sed -n 's/^nombre *= *"\(.*\)"/\1/p' "$NUEVO/manifest.toml" | head -n1)
    MVER=$(sed -n 's/^version *= *"\(.*\)"/\1/p' "$NUEVO/manifest.toml" | head -n1)
    echo "-- Módulo $MID $MVER"
    # [paquetes] de su familia: debian = ["nftables"]
    PAQ=$(awk -v f="$FAM" '/^\[/{s=($0=="[paquetes]")} s && $1==f {sub(/^[^=]*= *\[/,""); sub(/\].*$/,""); gsub(/[",]/," "); print}' "$NUEVO/manifest.toml")
    # shellcheck disable=SC2086
    paquetes $PAQ
    DIR="$MODULOS/$MID"
    rm -rf "$DIR.anterior"
    [ -d "$DIR" ] && mv "$DIR" "$DIR.anterior"
    mv "$NUEVO" "$DIR"
    chmod 755 "$DIR" "$DIR/bin" "$DIR"/bin/*
    command -v restorecon >/dev/null 2>&1 && restorecon -R "$DIR/bin" || true
    if grep -q '^arranque_temprano *= *true' "$DIR/manifest.toml"; then
        ORDEN="DefaultDependencies=no
Before=network-pre.target
Wants=network-pre.target
After=local-fs.target
Before=shutdown.target
Conflicts=shutdown.target"
    else
        ORDEN="After=network-online.target
Wants=network-online.target"
    fi
    cat > "/etc/systemd/system/netctl-mod-$MID.service" <<UNIDAD
[Unit]
Description=netctl: módulo $NOMBRE
$ORDEN

[Service]
ExecStart=$DIR/bin/netctl-mod-$MID
Environment=NETCTL_SOCKET=/run/netctl/mod-$MID.sock
Restart=on-failure
RestartSec=2
MemoryMax=512M
CPUWeight=50
RuntimeDirectory=netctl
RuntimeDirectoryPreserve=yes
StateDirectory=netctl/$MID
StateDirectoryMode=0700

[Install]
WantedBy=multi-user.target
UNIDAD
    systemctl daemon-reload
    systemctl enable "netctl-mod-$MID.service" >/dev/null 2>&1
    systemctl restart "netctl-mod-$MID.service"
    OK=""
    for _ in $(seq 1 40); do
        if curl -fsS --unix-socket "/run/netctl/mod-$MID.sock" http://m/salud >/dev/null 2>&1; then OK=1; break; fi
        sleep 0.5
    done
    if [ -n "$OK" ]; then
        echo "   $MID responde"
    else
        echo "   $MID NO responde; vuelvo a la versión anterior" >&2
        journalctl -u "netctl-mod-$MID" -n 15 --no-pager >&2 || true
        rm -rf "$DIR"
        if [ -d "$DIR.anterior" ]; then mv "$DIR.anterior" "$DIR"; systemctl restart "netctl-mod-$MID.service"; fi
    fi
done
systemctl restart netctl.service
sleep 1
systemctl is-active netctl.service >/dev/null && echo "-- netctl activo" || { echo "-- netctl NO arrancó" >&2; journalctl -u netctl -n 20 --no-pager >&2; exit 1; }
