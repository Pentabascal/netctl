# Fase 2.5b · Tabla de políticas unificada, NAT propio y Reenvío al estilo UniFi

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans (ejecución nativa). Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** una sola Tabla de políticas (Firewall, NAT y Reenvío) con filtros, reglas NAT propias y reenvíos con IP WAN, avisos de NAT/CGNAT/IP dinámica y conflictos al reanudar.

**Architecture:**
- `Config` gana `nat: Vec<ReglaNat>`; ya existe un `nat` para el masquerade por zona, así que el campo nuevo se llama `nat_propio`.
- `nft.rs` escribe las reglas NAT propias antes que las automáticas.
- Una ruta `/tabla` junta los tres tipos en un mismo formato de fila.
- La interfaz es una página nueva con filtros a la izquierda y un panel «Crear política» con selector de tipo.

**Tech Stack:** como en la 2.5a.

**Spec:** `docs/superpowers/specs/2026-11-28-firewall-unifi.md` §5. Requiere la 2.5a fusionada.

## Global Constraints

Las de la 2.5a, más:
- Firewall pasa a `0.3.0`.
- El asistente «Abrir un puerto» sigue en ≤3 pasos.

## Review Focus

1. Un DNAT propio que cae en el mismo puerto y protocolo que un reenvío activo se rechaza con el nombre del otro. Test: `dnat_choca_con_reenvio` (Task 1).
2. Si la WAN no tiene IP (cable quitado), el reenvío no se cae: no hay avisos y `wan_ip = Todas` sigue funcionando. Test: `sin_ip_wan_sin_aviso` (Task 2).
3. Una IP WAN guardada que desaparece de la tarjeta da un aviso y la regla no se genera, sin error al cargar nft. Test: `ip_wan_desaparecida` (Task 2).
4. Los contadores de filtros cuadran con la lista cuando hay reglas desactivadas. Test: `contadores_incluyen_pausadas` (Task 4).
5. Masquerade propio sobre una zona que ya tiene NAT automático no duplica la regla. Test: `masquerade_no_duplica` (Task 1).

---

### Task 1: Reglas NAT propias

**Files:**
- Modify:
  - `modules/firewall/src/modelo.rs`;
  - `validar.rs`;
  - `nft.rs` (cadenas `prerouting` dstnat y `postrouting` srcnat);
  - `api.rs`.
- Test: módulos `tests` de esos ficheros, `tests/api.rs` y `tests/nft_real.rs`.

**Interfaces:**
- Produces:
  - `ReglaNat`:
    - `id: u64`, `nombre: String`, `tipo: TipoNat`;
    - `iface: Option<String>` (salida para SNAT y Masquerade, entrada para DNAT);
    - `origen: Direccion`, `destino: Direccion`;
    - `protocolo: Protocolo`, `puertos: Puertos`;
    - `activa: bool`, `descripcion: String`.
  - `TipoNat` (con `tag="tipo"`), una de:
    - `Masquerade`;
    - `Snat { ip: IpAddr }`;
    - `Dnat { ip: IpAddr, puerto: Option<u16> }`.
  - `Config.nat_propio: Vec<ReglaNat>` con `#[serde(default)]`.
  - Rutas (todas responden 202 con `pendiente`):
    - `POST /nat`;
    - `PUT /nat/{id}`;
    - `DELETE /nat/{id}`;
    - `POST /nat/{id}/{activar|desactivar}`.
  - Mensajes:
    - «Ya hay un reenvío («{n}») en el puerto {p}/{proto}.»;
    - «Para DNAT con puerto elige TCP o UDP.».

- [x] **Step 1: Tests que fallan.**
  - `snat_antes_que_masquerade_automatico`: en `postrouting`, la posición de `snat ip to 203.0.113.5` es menor que la del `masquerade` de zona.
  - `dnat_con_puerto`: contiene `dnat ip to 192.168.1.10:8080`.
  - `dnat_choca_con_reenvio`.
  - `masquerade_no_duplica`.
  - En la API, `crear_nat_202`.
  - En `nft_real`, `nat_propio_carga`.
- [x] **Step 2: Ver que fallan.**

  Run: `cargo test -p netctl-firewall nat 2>&1 | tail -10`
  Expected: FAIL.
- [x] **Step 3: Implementar.**
- [x] **Step 4: Pasan.**

  Run: `cargo test -p netctl-firewall 2>&1 | tail -5`
  Expected: verde.
- [x] **Step 5: Commit.** `feat(firewall): reglas NAT propias (masquerade, SNAT, DNAT)`

### Task 2: Reenvío con IP WAN, avisos y conflictos al reanudar

**Files:**
- Modify:
  - `modules/firewall/src/modelo.rs` (`Reenvio.wan_ip: Option<IpAddr>`);
  - `leer.rs`, donde `parse_ip_addr` también devuelve `dinamica: bool`, que es true si la dirección tiene `dynamic`, viene de DHCP o la tarjeta es `ppp*`;
  - `validar.rs` (`avisos_reenvio`);
  - `nft.rs` (`ip daddr {wan_ip}` en el DNAT);
  - `api.rs` (`/reenvios/{id}/activar` devuelve 409 si choca).
- Test: módulos `tests` y `tests/api.rs`.

**Interfaces:**
- Produces:
  - `avisos_reenvio(c, r, ips_wan: &[(String, IpAddr, bool)]) -> Vec<String>`. Recibe las IPs WAN como (iface, ip, dinámica) y añade, por IP, estos textos exactos:
    - IP privada (RFC1918): «{iface} está detrás de otro router (IP privada {ip}). Abre también el mismo puerto en ese router.»
    - 100.64.0.0/10: «{iface} está detrás de CGNAT ({ip}): tu proveedor tiene que abrir el puerto.»
    - Dinámica: «{iface} tiene una IP que puede cambiar. Usa un DNS dinámico para llegar por nombre.»
  - `GET /config.ips_externas`: ahora `[{iface, ip, dinamica}]`.
  - 409 de reanudar: `{error: "No se puede reanudar: choca con «{otro}».", con: id}`.

- [x] **Step 1: Tests que fallan.**
  - `aviso_ip_privada`;
  - `aviso_cgnat`;
  - `aviso_dinamica`;
  - `sin_ip_wan_sin_aviso`;
  - `ip_wan_desaparecida`;
  - `dnat_con_wan_ip`, que contiene `ip daddr 203.0.113.7`;
  - en la API, `reanudar_con_choque_409`.
- [x] **Step 2: Ver que fallan.**

  Run: `cargo test -p netctl-firewall reenvio 2>&1 | tail -10`
  Expected: FAIL.
- [x] **Step 3: Implementar.**
- [x] **Step 4: Pasan.**

  Run: `cargo test -p netctl-firewall 2>&1 | tail -5`
  Expected: verde.
- [x] **Step 5: Commit.** `feat(firewall): reenvío con IP WAN, avisos NAT/CGNAT/IP dinámica y choque al reanudar`

### Task 3: API de la tabla unificada

**Files:**
- Modify: `modules/firewall/src/api.rs`.
- Test: `tests/api.rs`.

**Interfaces:**
- Produces: `GET /tabla` devuelve `[FilaTabla]`, con `FilaTabla` así:
  - `tipo`: `"firewall"`, `"nat"` o `"reenvio"`;
  - `id`;
  - `nombre`;
  - `accion`: el texto Permitir, Bloquear o Rechazar; en NAT, Masquerade, SNAT o DNAT; en reenvío, «Reenviar»;
  - `protocolo`: texto;
  - `zona_origen` y `zona_destino`: nombres o `null`;
  - `origen` y `destino`: texto;
  - `puerto_destino`: texto;
  - `interfaz`: texto o `null`;
  - `activa`;
  - `de_serie: bool`.

  Las políticas de serie salen agrupadas, como en `/politicas`.

- [x] **Step 1: Test que falla.** `tabla_junta_los_tres_tipos`: con una regla, un NAT y un reenvío, `tipo` cubre los tres valores y los ids no se repiten entre tipos.
- [x] **Step 2: Ver que falla.**

  Run: `cargo test -p netctl-firewall --test api tabla`
  Expected: 404.
- [x] **Step 3: Implementar.**
- [x] **Step 4: Pasa.** Mismo comando.

  Expected: PASS.
- [x] **Step 5: Commit.** `feat(firewall): /tabla con firewall, NAT y reenvíos`

### Task 4: Página Tabla de políticas y paneles de NAT y Reenvío

**Files:**
- Create:
  - `modules/firewall/ui/src/TablaPoliticas.svelte`;
  - `tabla.js` y `tabla.test.js`;
  - `FormNat.svelte`.
- Modify:
  - `PanelReenvio.svelte` (campos UniFi: interfaz WAN y su IP como `Radios`, «Elegir equipo», origen Cualquiera/Limitado);
  - `FormPolitica.svelte`, que gana un primer bloque «Tipo» con los `Radios` Firewall / NAT / Reenvío de puertos. Al elegir Reenvío se abre `PanelReenvio` en blanco, con el enlace «¿Primera vez? Usa el asistente» que lleva a `AsistenteReenvio`;
  - `App.svelte`;
  - `manifest.toml`, donde la entrada «Reenvío de puertos» se sustituye por «Tabla de políticas» (icono `lista`) como primera entrada;
  - versión `0.3.0`.
- Delete: `Reenvios.svelte`. Sus avisos «no responde» y la caducidad pasan a columnas y chips de la tabla.

**Interfaces:**
- Consumes: `/tabla`, `/nat`, `/reenvios` y `/config.ips_externas` (Tasks 1–3).
- Produces: en `tabla.js`:
  - `contadores(filas) -> {tipo:{firewall,nat,reenvio}, accion:{…}, protocolo:{…}, origen:{…}, destino:{…}}`, contando también las pausadas;
  - `aplicarFiltros(filas, filtros) -> filas`.

- [x] **Step 1: Tests que fallan.**
  - `contadores_incluyen_pausadas`;
  - `filtrar_por_tipo_y_accion`;
  - `limpiar_filtros`.
- [x] **Step 2: Ver que fallan.**

  Run: `node --test modules/firewall/ui/src/tabla.test.js`
  Expected: FAIL.
- [x] **Step 3: Implementar** la página:
  - filtros a la izquierda con su contador;
  - enlace «Ver las de serie (n)»;
  - tabla con columnas a elegir;
  - debajo, los enlaces «Reordenar» y «Crear política».
- [x] **Step 4: Pasan, compila y revisión visual.** Una captura de la tabla y otra del panel de reenvío, comparadas con `scratchpad/uni/capturas/30-tabla-politicas.png`.

  Run: `node --test modules/firewall/ui/src/ && npm --prefix modules/firewall/ui run build 2>&1 | tail -3`
  Expected: PASS.
- [x] **Step 5: Commit.** `feat(firewall-ui): Tabla de políticas unificada con NAT y reenvío al estilo UniFi`
- [x] **Step 6: Revisión final** con haiku (≤15 líneas) y correcciones con TDD.
