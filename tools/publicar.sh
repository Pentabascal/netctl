#!/bin/sh
# Uso: tools/publicar.sh <id>     empaqueta y publica el módulo y actualiza el catálogo
#      tools/publicar.sh nucleo   empaqueta y publica el núcleo (.deb y .rpm) y el catálogo
#      tools/publicar.sh catalogo solo regenera y sube el catálogo
# Con DRY_RUN=1 no llama a gh: muestra lo que haría.
set -eu
. "$(dirname "$0")/comun.sh"

gh_() {
    if [ -n "${DRY_RUN:-}" ]; then
        echo "+ gh $*"
    else
        gh "$@"
    fi
}

# release <etiqueta> <título> <ficheros…>: crea la Release o sustituye sus ficheros.
release() {
    TAG=$1 TITULO=$2
    shift 2
    if [ -z "${DRY_RUN:-}" ] && gh release view "$TAG" >/dev/null 2>&1; then
        gh release upload "$TAG" "$@" --clobber
    else
        gh_ release create "$TAG" "$@" --title "$TITULO" --notes "$TITULO" --latest=false
    fi
}

catalogo() {
    "$RAIZ/tools/generar-catalogo.sh"
    if [ -z "${DRY_RUN:-}" ] && ! gh release view catalogo >/dev/null 2>&1; then
        gh release create catalogo --title "Catálogo de módulos" \
            --notes "index.json firmado con los módulos de netctl (lo actualiza tools/publicar.sh)." --latest=false
    fi
    gh_ release upload catalogo "$DIST/index.json" "$DIST/index.json.minisig" --clobber
}

[ -n "${DRY_RUN:-}" ] || command -v gh >/dev/null || fallo "falta gh (GitHub CLI)"
QUE=${1:-}
case "$QUE" in
"")
    fallo "uso: $0 <id>|nucleo|catalogo"
    ;;
catalogo) ;;
nucleo)
    VER=$(version_nucleo)
    "$RAIZ/tools/empaquetar-nucleo.sh"
    release "nucleo-v$VER" "netctl núcleo $VER" \
        "$DIST"/netctl_"$VER"-1_amd64.deb "$DIST"/netctl-"$VER"-1.x86_64.rpm
    ;;
*)
    "$RAIZ/tools/empaquetar-modulo.sh" "$QUE"
    VER=$(valor_toml "$RAIZ/modules/$QUE/manifest.toml" version)
    T="$DIST/netctl-mod-$QUE-$VER-x86_64.tar.gz"
    release "$QUE-v$VER" "Módulo $QUE $VER" "$T" "$T.minisig"
    ;;
esac
catalogo
echo "Publicado."
