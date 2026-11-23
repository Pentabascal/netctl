# netctl fase 1 — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Núcleo del panel netctl (login, carcasa UniFi, gestor de módulos firmados) + módulo WireGuard con clientes en vivo, publicables desde GitHub Releases.

**Architecture:** Workspace Cargo. `netctl-core` (axum, HTTPS, root) sirve la carcasa Svelte incrustada y hace de proxy autenticado a cada módulo, que es un binario aparte (`netctl-mod-<id>`) escuchando HTTP en `/run/netctl/mod-<id>.sock`. Los módulos se descargan de Releases como `.tar.gz` firmados con minisign y se instalan como servicios systemd.

**Tech Stack:** Rust 2024 (axum 0.8, tokio, hyper-util, axum-server+rustls, rcgen, argon2, totp-rs, minisign-verify, sha2, reqwest rustls, rust-embed, serde/toml), Svelte 5 + Vite, nftables, wireguard-tools, cargo-deb, cargo-generate-rpm, minisign (CLI, solo en la máquina del usuario).

**Spec:** `docs/superpowers/specs/2026-11-23-netctl-panel-design.md`

## Global Constraints

- Textos de UI y errores en castellano llano. Tema oscuro UniFi; referencia de colores/componentes: `src/ui_kit.rs` actual de wg-configurador.
- Distros: Debian 12+, Ubuntu 22.04+, openSUSE Leap 15.6 / SLES 15, RHEL/Rocky/Alma 9+, Fedora. Familias `debian` / `suse` / `rhel`.
- Binarios de módulo estáticos `x86_64-unknown-linux-musl`. Solo x86_64.
- Puerto del panel 8443 HTTPS, certificado autofirmado en el primer arranque (`/etc/netctl/tls/`).
- Estado en `/var/lib/netctl/` (700); ficheros con claves 600. Módulos en `/usr/lib/netctl/modules/<id>/`.
- Socket de módulo `/run/netctl/mod-<id>.sock` (0600 root). Unidad `netctl-mod-<id>.service`.
- Cabeceras núcleo→módulo: `X-Netctl-Usuario`. Endpoints obligatorios de módulo: `GET /salud`, `GET /resumen`, `POST /evento`.
- Release por módulo `<id>-v<versión>`, asset `netctl-mod-<id>-<versión>-x86_64.tar.gz` + `.minisig`; catálogo `index.json` (+ `.minisig`) en la Release `catalogo`. URL base: `https://github.com/Pentabascal/netctl/releases/download/`.
- Clave pública minisign compilada en el núcleo (`crates/netctl-core/clave_publica.txt`); la privada NUNCA en el repo.
- Conectado = último handshake hace < 180 s. Sondeo `wg show <iface> dump` cada 5 s.
- Login: argon2id, cookie `netctl_sesion` `HttpOnly; Secure; SameSite=Strict`, CSRF por cabecera `X-Netctl-Csrf`, bloqueo 5 fallos. Modo `internet` exige TOTP activado.
- Commits sin el email personal (el repo ya usa el noreply). Mensajes de commit en castellano.

## Review Focus

1. `wg show` falla (interfaz caída o aún no creada) → la lista muestra todos «Desconectado» y un aviso, nunca un 500. Test en Tarea 6.
2. Handshake `0` (nunca conectado) → «Nunca», no «hace 56 años». Test en Tarea 6.
3. GitHub inaccesible o catálogo con firma mala → la página Módulos muestra el error y los módulos instalados siguen funcionando. Test en Tarea 4.
4. Proyecto JSON antiguo de wg-configurador (puerto/keepalive como número) se importa sin pérdida. Test en Tarea 1.
5. Nombre de cliente con espacios/acentos → `.conf` descargable y comentarios nft válidos (`nombre_seguro`). Test en Tarea 1.

---

### Task 1: Workspace y `wg-modelo` con reglas nftables

**Files:**
- Create: `Cargo.toml` (workspace: `crates/*`, `modules/*`, `apps/*`), `crates/wg-modelo/{Cargo.toml,src/lib.rs}`
- Move: `src/claves.rs`, `src/modelo.rs` → `crates/wg-modelo/src/`; `src/main.rs`, `src/ui_kit.rs`, `assets/` → `apps/wg-configurador/`
- Create: `crates/wg-modelo/src/nft.rs`
- Modify: `crates/wg-modelo/src/modelo.rs` (añadir `config_servidor_panel`)

**Interfaces:**
- Produces: `wg_modelo::{Proyecto, Cliente, Servidor, Subred, claves, nombre_seguro, validar_*}` (API actual sin cambios); `Proyecto::config_servidor_panel(&self) -> Result<String,String>` (igual que `config_servidor` pero sin PostUp/PostDown); `wg_modelo::nft::tabla_wg(p: &Proyecto, nat: bool) -> Result<String,String>` (script completo para `nft -f`, empieza por `table inet netctl_wg` + `delete table` idempotente).
- Añadir campo `Cliente.activo: bool` (`#[serde(default = "si")]`, true); clientes inactivos no salen en la config del servidor ni en las reglas.

- [ ] **Step 1:** Mover ficheros, workspace; `cargo build -p wg-configurador` y `cargo test -p wg-modelo` pasan (los tests existentes migran con el módulo).
- [ ] **Step 2: Tests que fallan** en `nft.rs`: `tabla_por_cliente` — proyecto con red 10.8.0.0/24, cliente 10.8.0.2 sin internet y subred 192.168.1.0/24 → contiene `chain forward { type filter hook forward priority filter - 5; policy accept;`, `iifname "wg0" ip saddr 10.8.0.2 ip daddr 192.168.1.0/24 accept`, y termina con `iifname "wg0" drop`; `nat_opcional` — `nat=true` contiene `masquerade` con `oifname "eth0"`, `nat=false` no; `inactivo_fuera` — cliente `activo=false` no aparece; `comentario_seguro` — cliente «Móvil de Ana» → `comment "M_vil_de_Ana"`; `servidor_panel_sin_postup` — `config_servidor_panel()` no contiene `PostUp`; `importa_json_antiguo` — JSON con `"puerto": 51820, "keepalive": 25` carga y `activo == true`.
- [ ] **Step 3:** Implementar replicando la semántica de `reglas()` (entre_clientes, DNS a :53, internet = `oifname <salida> accept` salvo destinos privados bloqueados).
- [ ] **Step 4:** `cargo test -p wg-modelo` PASS; `cargo build -p wg-configurador` OK.
- [ ] **Step 5:** Commit `workspace: extraer wg-modelo y reglas nftables`.

### Task 2: `netctl-sdk`

**Files:** Create `crates/netctl-sdk/src/{lib.rs,distro.rs,manifiesto.rs,servidor.rs,confirmar.rs}`

**Interfaces:**
- `distro::Familia { Debian, Suse, Rhel }`; `distro::detectar_de(os_release: &str) -> Option<Familia>`; `distro::detectar() -> Option<Familia>`; `distro::instalar_paquetes(f: Familia, paquetes: &[String]) -> anyhow::Result<()>`.
- `manifiesto::Manifiesto { id, nombre, version: semver::Version, nucleo_minimo: semver::Version, icono, menu: Vec<EntradaMenu{titulo, ruta, icono}>, paquetes: Paquetes{debian, suse, rhel: Vec<String>}, conflictos: Vec<String> }`, `Manifiesto::leer(toml: &str) -> Result<Self>`.
- `servidor::servir(id: &str, router: axum::Router) -> anyhow::Result<()>` — crea el socket `/run/netctl/mod-<id>.sock` (0600), borra el anterior; ruta configurable por env `NETCTL_SOCKET` para tests. `servidor::usuario(headers) -> Option<String>`.
- `confirmar::Pendiente` — `aplicar(nuevo: impl FnOnce()->Result<()>, restaurar: impl FnOnce()+Send+'static, plazo: Duration) -> Result<IdCambio>`, `confirmar(id) -> bool`. Si no se confirma en `plazo`, ejecuta `restaurar`.

- [ ] **Step 1: Tests que fallan:** `detecta_familias` con os-release de Debian 12, Ubuntu 24.04 (`ID_LIKE=debian`), openSUSE Leap (`ID_LIKE="suse opensuse"`), Rocky 9 (`ID_LIKE="rhel centos fedora"`), Fedora (`ID=fedora`) y Arch → `None`; `manifiesto_ejemplo` parsea un TOML de muestra; `confirmar_revierte` (plazo 50 ms, sin confirmar → restaurar llamado) y `confirmar_a_tiempo` (no llamado).
- [ ] **Step 2:** `cargo test -p netctl-sdk` FAIL.
- [ ] **Step 3:** Implementar. Comandos: `apt-get install -y`, `zypper --non-interactive install`, `dnf install -y` (con `DEBIAN_FRONTEND=noninteractive`).
- [ ] **Step 4:** PASS.
- [ ] **Step 5:** Commit `sdk: distro, manifiesto, socket y aplicar con confirmación`.

### Task 3: Núcleo — config, TLS, acceso

**Files:** Create `crates/netctl-core/src/{main.rs,config.rs,tls.rs,auth.rs,alcance.rs}`

**Interfaces:**
- `config::Config { puerto: u16 (8443), alcance: Alcance, datos: PathBuf }` en `/etc/netctl/core.toml`; `Alcance { SoloLan, Internet }`.
- `auth::Almacen` en `/var/lib/netctl/admin.json`: `fijar_password(&str)`, `login(pass, totp: Option<&str>, ip) -> Result<Sesion, ErrorLogin>`, `ErrorLogin { Credenciales, TotpRequerido, Bloqueado{segundos} }`, `activar_totp() -> (secreto, uri_otpauth)`, `confirmar_totp(codigo) -> bool`.
- `alcance::permitida(alcance, origen: IpAddr, ifaces_wg: &[IpNet]) -> bool` — SoloLan: RFC1918, 127/8, fc00::/7, fe80::/10, redes de interfaces wg.
- CLI: `netctl-core serve`, `netctl-core admin set-password`, `netctl-core admin reset-password` (genera aleatoria y la imprime una vez).
- Rutas: `POST /api/login`, `POST /api/logout`, `GET /api/yo`, `GET/PUT /api/ajustes/acceso` (cambio de alcance vía `confirmar::Pendiente` 60 s + `POST /api/ajustes/acceso/confirmar`), `POST /api/ajustes/totp`, `POST /api/ajustes/totp/confirmar`.

- [ ] **Step 1: Tests que fallan:** `bloqueo_tras_5` (5 fallos → `Bloqueado`, espera crece por IP); `totp_obligatorio_si_activo`; `internet_exige_totp` (PUT alcance=Internet sin TOTP → 409 «Activa primero el doble factor»); `alcance_solo_lan` (8.8.8.8 → false, 192.168.1.5 → true, 10.8.0.7 en wg → true); `csrf` (POST sin cabecera → 403); `sesion_cookie_flags`.
- [ ] **Step 2:** FAIL. **Step 3:** Implementar; en SoloLan además de filtrar por origen, enlazar solo a IPs de interfaces privadas/wg (si no hay ninguna, a 127.0.0.1 y avisar en el log). TLS con rcgen si faltan `cert.pem/key.pem`.
- [ ] **Step 4:** `cargo test -p netctl-core` PASS. **Step 5:** Commit `core: config, TLS, login, TOTP y alcance`.

### Task 4: Núcleo — gestor de módulos y proxy

**Files:** Create `crates/netctl-core/src/{modulos/mod.rs,modulos/catalogo.rs,modulos/instalar.rs,proxy.rs}`, `crates/netctl-core/clave_publica.txt` (placeholder de test; el usuario pone la real en Tarea 8)

**Interfaces:**
- `catalogo::Catalogo { modulos: Vec<EntradaCatalogo{id, nombre, descripcion, version, url, sha256, nucleo_minimo}>, nucleo: Option<semver::Version> }`; `catalogo::obtener(cliente, base_url, clave) -> Result<Catalogo, ErrorCatalogo>` (descarga `index.json` + `.minisig`, verifica).
- `instalar::instalar(entrada, clave, familia) -> Result<()>`: descargar → minisig → sha256 → `nucleo_minimo <= CARGO_PKG_VERSION` (si no, `ErrorInstalar::NucleoAntiguo`) → paquetes del manifiesto → extraer a `/usr/lib/netctl/modules/<id>.nuevo/` → mover el actual a `<id>.anterior/` → unidad systemd → `systemctl daemon-reload && enable --now` → esperar `/salud` 15 s, si falla restaurar `.anterior`. `desinstalar(id, borrar_datos: bool)`.
- Raíces de rutas inyectables (`Rutas { modulos, unidades, run, datos }`) para tests en tempdir; systemctl detrás de un trait `Systemd` con doble en tests.
- `proxy`: `ANY /api/m/{id}/{*resto}` → socket del módulo (hyper sobre `UnixStream`), añade `X-Netctl-Usuario`, soporta respuestas en streaming (SSE). `GET /m/{id}/ui/{*f}` sirve `ui/` del módulo. `GET /api/modulos` (instalados + estado + CPU/memoria vía `systemctl show -p CPUUsageNSec,MemoryCurrent`), `GET /api/catalogo`, `POST /api/modulos/{id}/instalar|actualizar|desinstalar`. Al cambiar módulos: `POST /evento {"tipo":"modulos_cambiados","modulos":[ids]}` a cada módulo.
- Unidad generada: `ExecStart=/usr/lib/netctl/modules/<id>/bin/netctl-mod-<id>`, `Restart=on-failure`, `MemoryMax=512M`, `CPUWeight=50`, `RuntimeDirectory=netctl`, `StateDirectory=netctl/<id>`.

- [ ] **Step 1: Tests que fallan** (claves minisign generadas en el test con el crate `minisign` como dev-dependency): `firma_mala_rechaza`; `sha_distinto_rechaza`; `nucleo_antiguo_bloquea`; `salud_falla_restaura_anterior`; `catalogo_inaccesible` (servidor HTTP de test caído → `ErrorCatalogo::Red` y `GET /api/modulos` sigue respondiendo 200); `proxy_reenvia` (módulo falso en socket temporal recibe la ruta y `X-Netctl-Usuario`); `proxy_sin_sesion_401`.
- [ ] **Step 2:** FAIL. **Step 3:** Implementar. **Step 4:** PASS.
- [ ] **Step 5:** Commit `core: catálogo firmado, instalación de módulos y proxy`.

### Task 5: Web — kit UniFi y carcasa

**Files:** Create `web/package.json` (workspaces `kit`, `shell`, `../modules/*/ui`), `web/kit/src/{tema.css,Barra.svelte,Tarjeta.svelte,Tabla.svelte,Interruptor.svelte,PanelLateral.svelte,Pestanas.svelte,Chip.svelte,Boton.svelte,Campo.svelte,Aviso.svelte,Confirmacion60.svelte,api.js}`, `web/shell/src/{App.svelte,paginas/{Login,Inicio,Modulos,Ajustes}.svelte}`; Modify `crates/netctl-core` (rust-embed de `web/shell/dist`, fallback SPA).

**Interfaces:**
- `api.js`: `crearApi(base) -> { get, post, put, del, sse(ruta, cb) }` con cookie + `X-Netctl-Csrf` (leído de `GET /api/yo`).
- Contrato de UI de módulo: `ui/index.js` exporta `mount(el: HTMLElement, api) -> () => void` (desmontar). La carcasa carga con `import('/m/<id>/ui/index.js')` al pulsar la entrada de menú del manifiesto.
- `Confirmacion60`: barra fija con cuenta atrás y botón «Confirmar cambios»; la usa Ajustes→Acceso (y el Firewall en fase 2).
- Páginas: Login (usuario fijo «admin», contraseña, TOTP si se pide), Inicio (tarjetas `/resumen` de cada módulo + CPU/memoria por módulo), Módulos (Instalados / Disponibles en pestañas; tarjeta por módulo con versión, estado, botón; aviso «Hay una versión nueva del núcleo» si `catalogo.nucleo` > versión actual, con el comando de actualización), Ajustes (Contraseña, Doble factor con QR, Acceso: SoloLan/Internet).

- [ ] **Step 1:** `npm run build` genera `web/shell/dist`; `cargo build -p netctl-core` lo incrusta.
- [ ] **Step 2:** Verificación visual con Xvfb `:99` + navegador headless (no abrir ventanas en el escritorio del usuario): capturas de Login, Inicio, Módulos, Ajustes en `/tmp/.../capturas/`; comparar con la estética de `apps/wg-configurador/src/ui_kit.rs` (fondo, tarjetas, acento azul UniFi).
- [ ] **Step 3:** Commit `web: kit UniFi y carcasa del panel`.

### Task 6: Módulo WireGuard — backend

**Files:** Create `modules/wireguard/{Cargo.toml,manifest.toml,src/{main.rs,estado.rs,aplicar.rs,vivo.rs,api.rs}}`

**Interfaces:**
- Consumes: `wg_modelo::*`, `netctl_sdk::servidor::servir("wireguard", router)`.
- `estado::Almacen` — `/var/lib/netctl/wireguard/proyecto.json` (600), `cargar()`, `guardar(&Proyecto)`, campo `revision: u64` para detectar ediciones simultáneas (PUT con revisión vieja → 409).
- `aplicar::aplicar(p: &Proyecto, cambio: Cambio, firewall_presente: bool) -> Result<()>` con `Cambio { Peers, Servidor }`: escribe `/etc/wireguard/<iface>.conf` (600, `config_servidor_panel`), `Peers` → `wg syncconf <iface> /dev/stdin` con la salida de `wg-quick strip`; `Servidor` → `systemctl restart wg-quick@<iface>`; siempre `nft -f` con `tabla_wg(p, !firewall_presente)`. Comandos detrás de un trait `Sistema` (doble en tests).
- `aplicar::adoptar(iface)` — si el `.conf` existente tiene PostUp, avisa y reinicia una vez.
- `vivo::parsear_dump(salida: &str, ahora: u64) -> Vec<EstadoPeer{clave_publica, endpoint: Option<String>, ultimo_handshake: Option<u64>, rx: u64, tx: u64, conectado: bool}>`; tarea tokio cada 5 s → `watch` channel.
- API: `GET /salud`, `GET /resumen` (conectados, total, tráfico), `POST /evento`, `GET/PUT /servidor`, `GET/POST /clientes`, `GET/PUT/DELETE /clientes/{i}`, `POST /clientes/{i}/activar|desactivar|regenerar-claves`, `GET /clientes/{i}/conf` (descarga `<nombre_seguro>.conf`), `GET /clientes/{i}/qr.svg`, `GET /vivo` (SSE), `POST /importar` (JSON de wg-configurador), `GET/POST/DELETE /subredes`. Errores de validación: 422 con la lista de `Proyecto::validar()`.
- `manifest.toml`: id `wireguard`, menú «Clientes», «Servidor», paquetes debian `[wireguard-tools, nftables]`, suse `[wireguard-tools, nftables]`, rhel `[wireguard-tools, nftables]`.

- [ ] **Step 1: Tests que fallan:** `dump_conectado` (handshake hace 30 s → conectado), `dump_viejo` (hace 200 s → no), `dump_nunca` (`0` → `ultimo_handshake: None`, no conectado); `wg_caido` (`wg show` devuelve error → `GET /clientes` 200 con todos desconectados y `aviso: "La interfaz wg0 no está activa"`); `editar_peer_usa_syncconf` (el doble de `Sistema` registra `wg syncconf` y NO `restart`); `cambiar_puerto_reinicia`; `nat_segun_firewall` (evento `modulos_cambiados` con `firewall` → `nft` sin masquerade); `revision_vieja_409`; `conf_600`.
- [ ] **Step 2:** FAIL. **Step 3:** Implementar. **Step 4:** `cargo test -p netctl-mod-wireguard` PASS.
- [ ] **Step 5:** Commit `wireguard: módulo backend con estado en vivo`.

### Task 7: Módulo WireGuard — UI

**Files:** Create `modules/wireguard/ui/{package.json,vite.config.js,src/{index.js,Clientes.svelte,Servidor.svelte,PanelCliente.svelte,Asistente.svelte}}`

**Interfaces:** Consumes kit (Tarea 5) y la API de Tarea 6 vía `api` de `mount`.

- Clientes: tarjetas arriba (conectados, total, tráfico ↓↑), tabla con chip verde «Conectado» / gris «Desconectado» / «Nunca», último saludo relativo, endpoint, tráfico; filtro y búsqueda; actualización por SSE `/vivo`. Clic → `PanelCliente` (pestañas General / Acceso / Avanzado: PSK, keepalive, DNS, regenerar claves) con QR y descarga `.conf`; interruptor activo.
- Servidor: pestañas General (endpoint, puerto, red, interfaz de salida, DNS) / Intranet (subredes) / Avanzado (MTU, claves, entre clientes); aviso «Esto reinicia la VPN» antes de guardar cambios de servidor.
- `Asistente`: si no hay proyecto, 3 pasos (endpoint+puerto, red VPN + intranet, primer cliente) o «Importar de wg-configurador».

- [ ] **Step 1:** `npm run build` en `modules/wireguard/ui` → `dist/index.js`.
- [ ] **Step 2:** Capturas con Xvfb de Clientes, PanelCliente, Servidor y Asistente contra un módulo de prueba con datos falsos (doble de `Sistema` + dump simulado).
- [ ] **Step 3:** Commit `wireguard: interfaz web`.

### Task 8: Empaquetado, publicación e instalación

**Files:** Create `tools/{empaquetar-modulo.sh,publicar.sh,generar-catalogo.sh}`, `install.sh`, `crates/netctl-core/{debian postinst, rpm scripts}` (metadatos cargo-deb / cargo-generate-rpm en su `Cargo.toml`), `packaging/netctl.service`, `docs/prueba-fase1.md`

- `empaquetar-modulo.sh <id>`: `cargo build --release --target x86_64-unknown-linux-musl -p netctl-mod-<id>`, build de `ui/`, tar con `manifest.toml`, `bin/`, `ui/` → `dist/netctl-mod-<id>-<ver>-x86_64.tar.gz`, firma con `minisign -S -s ~/.minisign/netctl.key`.
- `generar-catalogo.sh`: `index.json` (id, nombre, descripcion, version, url, sha256, nucleo_minimo; `nucleo` = versión del core) + firma.
- `publicar.sh <id>`: `gh release create <id>-v<ver>` con assets; actualiza la Release `catalogo` (`gh release upload catalogo --clobber`).
- Núcleo: `.deb` y `.rpm` musl; postinst crea `/etc/netctl`, `/var/lib/netctl` (700), habilita `netctl.service` y, si no hay admin, ejecuta `netctl-core admin reset-password` (muestra la contraseña una vez).
- `install.sh`: detecta familia por `/etc/os-release`, descarga el `.deb`/`.rpm` de la última Release `nucleo-v*` y lo instala.
- `docs/prueba-fase1.md`: guía corta para el usuario (Debian, openSUSE, Rocky): instalar, entrar, instalar módulo WireGuard, crear cliente, conectar desde el móvil, ver «Conectado», editar otro cliente sin cortar al primero, cambiar alcance a Internet con TOTP.

- [ ] **Step 1:** `tools/empaquetar-modulo.sh wireguard` con una clave de prueba en tempdir → tar + `.minisig` válidos; `netctl-core` de test lo instala desde un servidor HTTP local (`python3 -m http.server` en dist/) en un tempdir con `Rutas` de prueba.
- [ ] **Step 2:** `cargo deb -p netctl-core` y `cargo generate-rpm -p crates/netctl-core` generan paquetes.
- [ ] **Step 3:** Commit `tools: empaquetado, firma, publicación e instalador`.
- [ ] **Step 4 (lo hace el usuario):** generar la clave real (`minisign -G -p crates/netctl-core/clave_publica.txt -s ~/.minisign/netctl.key`), commitear solo la pública y publicar. Claude no publica Releases sin que el usuario lo pida.
