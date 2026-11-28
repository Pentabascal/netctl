#!/bin/sh
# Uso: tools/probar-en-servidor.sh <usuario@host> [opciones de ssh…]
#   p. ej.: tools/probar-en-servidor.sh local@172.16.121.151 -i ~/.ssh/homelab
#
# Instala en un servidor de pruebas el núcleo y los módulos que haya en dist/ (los de
# `SIN_FIRMA=1 tools/empaquetar-nucleo.sh` y `tools/empaquetar-modulo.sh <id>`), sin publicar
# nada en GitHub. Hace lo mismo que el panel al instalar un módulo: lo extrae en
# /usr/lib/netctl/modules/<id>, instala sus paquetes, escribe la unidad systemd, lo arranca y
# espera a que responda en su socket. Si un módulo no responde, vuelve a la versión anterior.
# El usuario remoto necesita sudo.
set -eu
. "$(dirname "$0")/comun.sh"

DESTINO=${1:-}
[ -n "$DESTINO" ] || fallo "uso: $0 <usuario@host> [opciones de ssh…]"
shift

VER=$(valor_toml "$RAIZ/crates/netctl-core/Cargo.toml" version package)
DEB=$(ls "$DIST"/netctl_"$VER"-*_amd64.deb 2>/dev/null | head -n1)
RPM=$(ls "$DIST"/netctl-"$VER"-*.x86_64.rpm 2>/dev/null | head -n1)
[ -n "$DEB" ] && [ -n "$RPM" ] || fallo "faltan el .deb y el .rpm del núcleo $VER en dist/ (SIN_FIRMA=1 tools/empaquetar-nucleo.sh)"

MODS=""
for M in "$RAIZ"/modules/*/manifest.toml; do
    ID=$(valor_toml "$M" id)
    V=$(valor_toml "$M" version)
    T="$DIST/netctl-mod-$ID-$V-x86_64.tar.gz"
    if [ -f "$T" ]; then MODS="$MODS $T"; else echo "Aviso: no hay $ID $V en dist/; no se instala" >&2; fi
done


echo "== Copiando a $DESTINO"
ssh "$@" "$DESTINO" 'rm -rf /tmp/netctl-prueba && mkdir -p /tmp/netctl-prueba'
# shellcheck disable=SC2086
scp "$@" "$DEB" "$RPM" $MODS "$RAIZ/tools/instalar-paquetes.sh" "$DESTINO:/tmp/netctl-prueba/"

echo "== Instalando (sudo puede pedir la contraseña)"
ssh -t "$@" "$DESTINO" 'sudo sh /tmp/netctl-prueba/instalar-paquetes.sh'
echo "Listo: abre https://$(echo "$DESTINO" | sed 's/.*@//'):8443"
