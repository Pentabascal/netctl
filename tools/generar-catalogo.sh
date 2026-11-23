#!/bin/sh
# Uso: tools/generar-catalogo.sh
# Crea dist/index.json (+ .minisig) con la versión actual de cada módulo de modules/*.
# Usa el .tar.gz de dist/ y, si no está, el ya publicado en NETCTL_URL_BASE.
set -eu
. "$(dirname "$0")/comun.sh"

json() { printf '"%s"' "$(printf '%s' "$1" | sed 's/\\/\\\\/g; s/"/\\"/g')"; }

mkdir -p "$DIST"
SALIDA="$DIST/index.json"
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
ENTRADAS=""
for M in "$RAIZ"/modules/*/manifest.toml; do
    [ -f "$M" ] || continue
    DIR=$(dirname "$M")
    ID=$(valor_toml "$M" id)
    VER=$(valor_toml "$M" version)
    NOMBRE=$(valor_toml "$M" nombre)
    MINIMO=$(valor_toml "$M" nucleo_minimo)
    DESC=$(valor_toml "$DIR/Cargo.toml" description package)
    FICHERO="netctl-mod-$ID-$VER-x86_64.tar.gz"
    URL="$NETCTL_URL_BASE$ID-v$VER/$FICHERO"
    TAR="$DIST/$FICHERO"
    if [ ! -f "$TAR" ]; then
        if curl -fsSL -o "$TMP/$FICHERO" "$URL"; then
            TAR="$TMP/$FICHERO"
        else
            echo "Aviso: $ID $VER no está en dist/ ni publicado; no entra en el catálogo" >&2
            continue
        fi
    fi
    E=$(printf '    {"id": %s, "nombre": %s, "descripcion": %s, "version": %s, "url": %s, "sha256": %s, "nucleo_minimo": %s}' \
        "$(json "$ID")" "$(json "$NOMBRE")" "$(json "$DESC")" "$(json "$VER")" \
        "$(json "$URL")" "$(json "$(sha256 "$TAR")")" "$(json "$MINIMO")")
    if [ -n "$ENTRADAS" ]; then ENTRADAS="$ENTRADAS,
$E"; else ENTRADAS="$E"; fi
    echo "  $ID $VER"
done
[ -n "$ENTRADAS" ] || fallo "ningún módulo para el catálogo"
printf '{\n  "nucleo": %s,\n  "modulos": [\n%s\n  ]\n}\n' "$(json "$(version_nucleo)")" "$ENTRADAS" >"$SALIDA"
firmar "$SALIDA" "netctl catalogo $(date -u +%Y-%m-%dT%H:%M:%SZ)"
echo "Listo: $SALIDA y index.json.minisig"
