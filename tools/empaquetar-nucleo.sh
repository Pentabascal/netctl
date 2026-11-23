#!/bin/sh
# Uso: tools/empaquetar-nucleo.sh [--sin-web]
# Compila la web y el núcleo (musl) y crea dist/netctl_<v>-1_amd64.deb y dist/netctl-<v>-1.x86_64.rpm
set -eu
. "$(dirname "$0")/comun.sh"

command -v cargo-deb >/dev/null || fallo "falta cargo-deb (cargo install cargo-deb)"
command -v cargo-generate-rpm >/dev/null || fallo "falta cargo-generate-rpm (cargo install cargo-generate-rpm)"
VER=$(version_nucleo)
echo "== Núcleo $VER"
[ "${1:-}" = --sin-web ] || compilar_web
mkdir -p "$DIST"
cd "$RAIZ"
cargo deb -p netctl-core --target "$OBJETIVO" -o "$DIST/" || fallo "cargo deb falló"
# gzip: el rpm de openSUSE Leap 15 / RHEL 9 lo abre seguro.
cargo generate-rpm -p crates/netctl-core --target "$OBJETIVO" --payload-compress gzip -o "$DIST/" ||
    fallo "cargo generate-rpm falló"
ls -l "$DIST"/netctl_"$VER"-*_amd64.deb "$DIST"/netctl-"$VER"-*.x86_64.rpm
