#!/bin/sh
# Uso: tools/empaquetar-modulo.sh <id> [--sin-web]
# Compila el módulo (musl) y su UI y crea dist/netctl-mod-<id>-<versión>-x86_64.tar.gz + .minisig
set -eu
. "$(dirname "$0")/comun.sh"

ID=${1:-}
[ -n "$ID" ] || fallo "uso: $0 <id> [--sin-web]"
MOD="$RAIZ/modules/$ID"
[ -f "$MOD/manifest.toml" ] || fallo "no existe modules/$ID/manifest.toml"
VER=$(valor_toml "$MOD/manifest.toml" version)
[ -n "$VER" ] || fallo "manifest.toml sin versión"
[ "$(valor_toml "$MOD/manifest.toml" id)" = "$ID" ] || fallo "el id de manifest.toml no es «$ID»"

echo "== Módulo $ID $VER"
(cd "$RAIZ" && cargo build --release --target "$OBJETIVO" -p "netctl-mod-$ID") || fallo "no compiló el módulo"
[ "${2:-}" = --sin-web ] || compilar_web
BIN="$RAIZ/target/$OBJETIVO/release/netctl-mod-$ID"
[ -x "$BIN" ] || fallo "falta $BIN"
[ -f "$MOD/ui/dist/index.js" ] || fallo "falta modules/$ID/ui/dist/index.js"

# Firewall: el ayudante netctl-dpi se compila con glibc 2.31 (Debian 11) en un contenedor para
# que cargue la libndpi de cualquier distro de 2021 en adelante (el módulo es musl y no puede).
DPI=""
if [ -d "$MOD/dpi" ]; then
    command -v podman >/dev/null 2>&1 || fallo "hace falta podman para compilar netctl-dpi"
    podman run --rm -v "$RAIZ:/src:Z" -w /src docker.io/library/rust:1-bullseye \
        cargo build --release -p netctl-dpi --target-dir target/glibc || fallo "no compiló netctl-dpi"
    DPI="$RAIZ/target/glibc/release/netctl-dpi"
    [ -x "$DPI" ] || fallo "falta $DPI"
fi

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
mkdir -p "$TMP/bin" "$TMP/ui"
cp "$MOD/manifest.toml" "$TMP/"
cp "$BIN" "$TMP/bin/"
[ -z "$DPI" ] || { cp "$DPI" "$TMP/bin/"; chmod 755 "$TMP/bin/netctl-dpi"; }
cp -R "$MOD/ui/dist/." "$TMP/ui/"
chmod 755 "$TMP/bin/netctl-mod-$ID"
find "$TMP/ui" -type d -exec chmod 755 {} + -o -type f -exec chmod 644 {} +
chmod 644 "$TMP/manifest.toml"

mkdir -p "$DIST"
TAR="$DIST/netctl-mod-$ID-$VER-x86_64.tar.gz"
(cd "$TMP" && tar --sort=name --owner=0 --group=0 --numeric-owner --mtime=@0 \
    -czf "$TAR" manifest.toml bin ui) || fallo "no se pudo crear el tar"
firmar "$TAR" "netctl-mod-$ID $VER"
echo "Listo: $TAR ($(du -h "$TAR" | cut -f1))$([ -f "$TAR.minisig" ] && echo " y $(basename "$TAR").minisig")"
