#!/bin/sh
# Instalador del núcleo de netctl.
#   curl -fsSL https://raw.githubusercontent.com/Pentabascal/netctl/master/install.sh | sudo sh
# Detecta la familia de la distribución, descarga el .deb o .rpm de la última Release
# «nucleo-v*» y lo instala. Muestra la contraseña inicial del usuario «admin».
set -eu

REPO=${NETCTL_REPO:-Pentabascal/netctl}
API=${NETCTL_API:-https://api.github.com/repos/$REPO/releases?per_page=100}

fallo() {
    echo "Error: $*" >&2
    exit 1
}

[ "$(id -u)" -eq 0 ] || fallo "hay que ejecutarlo como root (con sudo)"
[ "$(uname -m)" = x86_64 ] || fallo "netctl solo está disponible para x86_64"
[ -r /etc/os-release ] || fallo "no se encuentra /etc/os-release"

ID="" ID_LIKE=""
# shellcheck disable=SC1091
. /etc/os-release
FAMILIA=""
for d in $ID $ID_LIKE; do
    case "$d" in
    debian | ubuntu) FAMILIA=debian ;;
    suse | opensuse | opensuse-leap | opensuse-tumbleweed | sles | sled) FAMILIA=suse ;;
    rhel | centos | fedora | rocky | almalinux) FAMILIA=rhel ;;
    esac
    [ -z "$FAMILIA" ] || break
done
[ -n "$FAMILIA" ] || fallo "distribución no soportada ($ID). Sirven Debian/Ubuntu, openSUSE/SLES y RHEL/Rocky/Alma/Fedora"

descargar() { # descargar <url> <destino>
    if command -v curl >/dev/null 2>&1; then
        curl -fsSL -o "$2" "$1"
    elif command -v wget >/dev/null 2>&1; then
        wget -q -O "$2" "$1"
    else
        fallo "hace falta curl o wget"
    fi
}

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT INT TERM

echo "Buscando la última versión del núcleo de netctl…"
descargar "$API" "$TMP/releases.json" || fallo "no se pudo consultar GitHub ($API)"
case "$FAMILIA" in
debian) PATRON='netctl_[0-9][^"/]*_amd64\.deb' ;;
*) PATRON='netctl-[0-9][^"/]*\.x86_64\.rpm' ;;
esac
# La API devuelve las Releases de la más nueva a la más antigua: la primera URL que
# corresponda a una etiqueta nucleo-v* es la última.
URL=$(tr ',' '\n' <"$TMP/releases.json" |
    sed -n 's/.*"browser_download_url": *"\([^"]*\)".*/\1/p' |
    grep "/releases/download/nucleo-v[^/]*/$PATRON\$" | head -n 1 || true)
[ -n "$URL" ] || fallo "no hay ninguna Release nucleo-v* con paquete para $FAMILIA"
PAQUETE="$TMP/$(basename "$URL")"
echo "Descargando $(basename "$URL")…"
descargar "$URL" "$PAQUETE" || fallo "no se pudo descargar $URL"

SALIDA="$TMP/instalacion.log"
echo "Instalando…"
set +e
case "$FAMILIA" in
debian)
    apt-get update -qq >/dev/null 2>&1
    DEBIAN_FRONTEND=noninteractive apt-get install -y "$PAQUETE" 2>&1 | tee "$SALIDA"
    ;;
suse) zypper --non-interactive install --allow-unsigned-rpm "$PAQUETE" 2>&1 | tee "$SALIDA" ;;
rhel)
    if command -v dnf >/dev/null 2>&1; then
        dnf install -y "$PAQUETE" 2>&1 | tee "$SALIDA"
    else
        yum install -y "$PAQUETE" 2>&1 | tee "$SALIDA"
    fi
    ;;
esac
set -e
command -v netctl-core >/dev/null 2>&1 || [ -x /usr/bin/netctl-core ] || fallo "la instalación no terminó bien (mira los mensajes de arriba)"

IP=$(hostname -I 2>/dev/null | cut -d' ' -f1 || true)
echo
echo "netctl instalado. Panel: https://${IP:-<IP-del-servidor>}:8443"
if grep -q 'Contraseña del usuario «admin» puesta' "$SALIDA"; then
    echo "Usuario: admin"
    echo "Contraseña: root"
    echo "Cámbiala en Ajustes nada más entrar."
else
    echo "Ya había contraseña de administrador (actualización); se conserva."
    echo "Si la olvidaste: sudo netctl-core admin password-defecto  (vuelve a «root»)"
fi
