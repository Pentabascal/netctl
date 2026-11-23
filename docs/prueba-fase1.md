# Prueba de la fase 1 (Debian, openSUSE, Rocky)

## 0. Publicar (una sola vez, en tu PC)

Herramientas: `minisign`, `gh` (con sesión iniciada), `cargo-deb`, `cargo-generate-rpm`,
el target `x86_64-unknown-linux-musl` (`rustup target add x86_64-unknown-linux-musl`) y npm.

1. Genera la clave de firma. La privada se queda en `~/.minisign/`; **nunca** al repo:

   ```sh
   mkdir -p ~/.minisign
   minisign -G -p crates/netctl-core/clave_publica.txt -s ~/.minisign/netctl.key
   git add crates/netctl-core/clave_publica.txt
   git commit -m "core: clave pública de firma"
   ```

   Sin esto el núcleo dice «no tiene configurada la clave pública de firma».
2. Sube la rama a `master` (el instalador se descarga de `master`).
3. Publica (pide la contraseña de la clave en cada firma):

   ```sh
   tools/publicar.sh nucleo      # Release nucleo-v0.1.0 con .deb y .rpm + catálogo
   tools/publicar.sh wireguard   # Release wireguard-v0.1.0 + catálogo
   ```

   `DRY_RUN=1` muestra las órdenes de `gh` sin publicar. Si usas otra herramienta de firma
   compatible, `MINISIGN=/ruta/orden`; otra clave, `NETCTL_CLAVE=/ruta/clave`.

## 1. Instalar el núcleo (en cada distro, como root)

```sh
curl -fsSL https://raw.githubusercontent.com/Pentabascal/netctl/master/install.sh | sudo sh
```

Al final muestra `Panel: https://<IP>:8443`, el usuario `admin` y la contraseña (solo esa
vez). Si la pierdes: `sudo netctl-core admin reset-password`. Al actualizar no se cambia.

Comprueba: `systemctl status netctl` activo. Si el cortafuegos de la distro bloquea el 8443:
openSUSE `firewall-cmd --add-port=8443/tcp --permanent && firewall-cmd --reload`
(igual en Rocky).

## 2. Lista de pruebas

1. Entrar en `https://<IP>:8443` (aceptar el certificado autofirmado) con `admin`.
2. **Módulos** → instalar **WireGuard**. Debe instalar `wireguard-tools` y `nftables` con
   apt/zypper/dnf y aparecer en la barra lateral.
3. **WireGuard → Servidor**: crear el servidor (puerto UDP abierto en el router y en el
   cortafuegos de la distro).
4. **Clientes**: crear un cliente, escanear el QR con la app WireGuard del móvil (con datos
   móviles, no la WiFi de casa) y conectar.
5. En unos segundos el cliente pasa a **Conectado**; un cliente nunca usado dice «Nunca».
6. Con el móvil conectado, editar **otro** cliente (nombre, permisos): el móvil no debe
   perder la conexión.
7. **Ajustes**: activar el doble factor (TOTP) y cambiar el alcance a **LAN e internet**;
   confirmar en 60 s. Cerrar sesión y entrar pidiendo el código. Si te quedas fuera:
   `sudo netctl-core admin quitar-totp`.

Apunta en cada distro qué paso falla y el mensaje (y `journalctl -u netctl -u
netctl-mod-wireguard -n 50`).

## Para desarrolladores: prueba local sin publicar

```sh
rsign generate -W -p /tmp/p/clave.pub -s /tmp/p/clave.key     # o minisign -G -W
MINISIGN=… NETCTL_CLAVE=/tmp/p/clave.key tools/empaquetar-modulo.sh wireguard
NETCTL_URL_BASE=http://127.0.0.1:8765/ NETCTL_CLAVE=/tmp/p/clave.key tools/generar-catalogo.sh
# servir dist/ como las Releases: catalogo/index.json*, wireguard-v0.1.0/netctl-mod-…
NETCTL_PRUEBA_URL=http://127.0.0.1:8765/ NETCTL_PRUEBA_CLAVE=/tmp/p/clave.pub \
  NETCTL_PRUEBA_MODULO=wireguard cargo test -p netctl-core -- --ignored paquete_real
```

`rsign` no acepta las opciones de minisign: hace falta un pequeño adaptador para `MINISIGN`
que traduzca `-S -s clave -m fichero -t comentario` a `rsign sign -W -s clave -x fichero.minisig
-t comentario fichero`.
