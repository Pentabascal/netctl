# Funciones comunes de tools/*.sh (se incluye con «. tools/comun.sh»).
# Variables que se pueden cambiar:
#   MINISIGN        orden compatible con minisign (por defecto «minisign»)
#   NETCTL_CLAVE    clave privada de firma (por defecto ~/.minisign/netctl.key)
#   NETCTL_URL_BASE URL base de las Releases (por defecto la de GitHub)
#   DRY_RUN=1       publicar.sh solo muestra las órdenes de gh

RAIZ=$(cd "$(dirname "$0")/.." && pwd)
DIST="$RAIZ/dist"
OBJETIVO=x86_64-unknown-linux-musl
MINISIGN=${MINISIGN:-minisign}
NETCTL_CLAVE=${NETCTL_CLAVE:-$HOME/.minisign/netctl.key}
NETCTL_URL_BASE=${NETCTL_URL_BASE:-https://github.com/Pentabascal/netctl/releases/download/}
case "$NETCTL_URL_BASE" in */) ;; *) NETCTL_URL_BASE="$NETCTL_URL_BASE/" ;; esac

fallo() {
    echo "Error: $*" >&2
    exit 1
}

# `ring` compila C: sin compilador musl se usa gcc (el resultado se enlaza estático igual).
if [ -z "${CC_x86_64_unknown_linux_musl:-}" ] && ! command -v x86_64-linux-musl-gcc >/dev/null 2>&1; then
    export CC_x86_64_unknown_linux_musl=gcc
fi

# valor_toml <fichero> <clave> [sección]: valor de `clave = "valor"` antes de la primera
# sección, o dentro de `[sección]` si se indica.
valor_toml() {
    awk -v k="$2" -v s="${3:-}" '
        /^\[/ { if (dentro) exit; dentro = ($0 == "[" s "]"); next }
        NR == 1 && s == "" { dentro = 1 }
        dentro && $1 == k { sub(/^[^=]*= *"/, ""); sub(/".*$/, ""); print; exit }
    ' "$1"
}

version_nucleo() {
    valor_toml "$RAIZ/crates/netctl-core/Cargo.toml" version package
}

# Compila la web (carcasa y UI de los módulos); hay que hacerlo desde web/.
compilar_web() {
    (cd "$RAIZ/web" && npm ci --no-audit --no-fund && npm run build) || fallo "no se pudo compilar la web"
}

# firmar <fichero> [comentario de confianza] → <fichero>.minisig
firmar() {
    [ -f "$NETCTL_CLAVE" ] || fallo "no existe la clave privada $NETCTL_CLAVE (ver docs/prueba-fase1.md)"
    rm -f "$1.minisig"
    "$MINISIGN" -S -s "$NETCTL_CLAVE" -m "$1" -t "${2:-netctl $(basename "$1")}" ||
        fallo "no se pudo firmar $1"
    [ -s "$1.minisig" ] || fallo "falta la firma $1.minisig"
}

sha256() {
    sha256sum "$1" | cut -d' ' -f1
}
