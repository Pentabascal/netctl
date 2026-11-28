# Fase 2.5c · Destinos por Dominio, País y App (DPI)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans (ejecución nativa). Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** las políticas pueden tener como destino nombres de dominio, países o aplicaciones detectadas con nDPI.

**Architecture:**
- **Dominio:** tarea periódica del módulo que resuelve nombres y añade IPs con caducidad a conjuntos nft. Se hace con `nft add element`, sin recargar la tabla.
- **País:** `geo.rs` descarga y analiza el CSV mensual de db-ip y genera conjuntos por país dentro del script atómico.
- **App:**
  - `dpi/` carga `libndpi` con `libloading` y lee los primeros paquetes de cada conexión desde NFQUEUE;
  - devuelve la app en la marca del paquete, que nft copia a `ct mark`;
  - las reglas comparan `ct mark`.
- Todo pasa por el trait `Sistema` para poder probarlo con `Falso`.

**Tech Stack:** más crates: `ureq` (con rustls), `flate2`, `libloading`, `nfq`.

**Spec:** `docs/superpowers/specs/2026-11-28-firewall-unifi.md` §6. Requiere la 2.5a; la 2.5b no es necesaria.

## Global Constraints

Las de la 2.5a, más:
- Firewall pasa a `0.4.0`.
- El binario sigue siendo musl estático: nada de enlazar nDPI al compilar.
- Atribución visible exacta: «IP Geolocation by DB-IP» con enlace a https://db-ip.com.
- Si falta libndpi o la base GeoIP, nada se rompe: la opción sale desactivada o con su aviso.

## Review Focus

1. Un dominio que no resuelve (NXDOMAIN, o sin red al arrancar) no bloquea la carga nft: el conjunto queda vacío y sale un aviso. Test: `dominio_sin_resolver` (Task 1).
2. Sin internet la primera vez que se usa un país, la regla se guarda con el aviso «Descargando la base de países…» y se aplica al terminar. Test: `pais_sin_base_aviso` (Task 2).
3. Si el clasificador DPI muere, el tráfico no se corta: la cola lleva `bypass`. Test: `cola_con_bypass` (Task 3).
4. Una versión de libndpi con otra ABI se detecta al cargarla (comprobación de versión) y desactiva App en lugar de caerse. Test: `version_ndpi_no_soportada` (Task 3).
5. Un CSV de db-ip corrupto o a medias no sustituye al anterior. Test: `csv_roto_conserva_anterior` (Task 2).

---

### Task 1: Destino por Dominio

**Files:**
- Modify:
  - `modules/firewall/src/modelo.rs`;
  - `sistema.rs` (`resolver`);
  - `nft.rs`;
  - `validar.rs`;
  - `api.rs` (`Estado::arrancar` lanza la tarea).
- Create: `modules/firewall/src/dominios.rs`.
- Test: `dominios.rs` y `nft.rs` (módulos `tests`), más `nft_real`.

**Interfaces:**
- Produces:
  - `Direccion::Dominio { nombres: Vec<String> }`, solo como destino.
  - `Sistema::resolver(&self, nombre: &str) -> Vec<IpAddr>`. `Falso` la implementa con `responder_dns(nombre, ips)`.
  - En el script, los conjuntos `dom{regla_id}_4` y `dom{regla_id}_6` (`flags timeout, interval`).
  - `dominios::refrescar(c: &Config, s: &dyn Sistema) -> Result<Vec<String>>`, que devuelve los avisos. Escribe `add element inet netctl dom{id}_4 { ip timeout 1h }` por `nft -f -` y se ejecuta cada 300 s.
  - Mensaje: «El dominio «{n}» no es válido.» (validación de etiquetas RFC 1123, máximo 253 caracteres).
  - Aviso: «No se pudo resolver «{n}»; se reintenta en 5 minutos.».

- [x] **Step 1: Tests que fallan.**
  - `dominio_genera_conjuntos`;
  - `refrescar_anade_elementos`: con `Falso`, se comprueba la orden nft;
  - `dominio_sin_resolver`;
  - `dominio_invalido`;
  - en `nft_real`, `dominio_carga`.
- [x] **Step 2: Ver que fallan.**

  Run: `cargo test -p netctl-firewall dominio 2>&1 | tail -8`
  Expected: FAIL.
- [x] **Step 3: Implementar.**
- [x] **Step 4: Pasan.**

  Run: `cargo test -p netctl-firewall 2>&1 | tail -5`
  Expected: verde.
- [x] **Step 5: Commit.** `feat(firewall): destino por dominio con conjuntos nft que se refrescan`

### Task 2: Destino por País (db-ip lite)

**Files:**
- Create: `modules/firewall/src/geo.rs`.
- Modify:
  - `modelo.rs` (`Direccion::Pais { codigos: Vec<String> }`, ISO 3166 alfa-2 en mayúsculas);
  - `sistema.rs` (`descargar(url, destino) -> Result<()>`);
  - `nft.rs`;
  - `api.rs` (`GET /geo`, que devuelve `{estado: "falta"|"descargando"|"lista", fecha, paises: [{codigo, nombre}]}`);
  - `Cargo.toml` (`ureq` con rustls y `flate2`).
- Test: `geo.rs` (`tests`) con un CSV pequeño en `tests/datos/dbip-mini.csv`.

**Interfaces:**
- Produces:
  - `geo::url(año, mes) -> String`, con la forma `https://download.db-ip.com/free/dbip-country-lite-{AAAA}-{MM}.csv.gz`.
  - `geo::analizar(csv: &str) -> Result<BTreeMap<String, (Vec<String>, Vec<String>)>>`: rangos v4 y v6 por país, ya unidos los contiguos.
  - `geo::actualizar(rutas, s) -> Result<()>`: descarga en `datos/geo/nuevo.csv.gz`, lo analiza entero y solo entonces lo renombra a `actual.csv.gz`. Si falla en el mes actual, prueba el anterior. Se ejecuta el día 2 de cada mes y al usar un país por primera vez.
  - En el script, los conjuntos `pais_{cc}_4` y `pais_{cc}_6` (`flags interval`), solo para los países usados.
  - Aviso: «Descargando la base de países…; la regla se aplicará al terminar.».

- [x] **Step 1: Tests que fallan.**
  - `analiza_y_une_rangos`;
  - `url_del_mes`;
  - `csv_roto_conserva_anterior`;
  - `pais_sin_base_aviso`;
  - `pais_genera_conjunto`.
- [x] **Step 2: Ver que fallan.**

  Run: `cargo test -p netctl-firewall geo 2>&1 | tail -8`
  Expected: FAIL.
- [x] **Step 3: Implementar.**
- [x] **Step 4: Pasan**, más una prueba real a mano (sin commit) que descarga el mes actual y comprueba que `ES` tiene rangos.

  Run: `cargo test -p netctl-firewall 2>&1 | tail -5`
  Expected: verde.
- [x] **Step 5: Commit.** `feat(firewall): destino por país con la base libre de db-ip`

### Task 3: Destino por App (nDPI + NFQUEUE)

**Files:**
- Create:
  - `modules/firewall/src/dpi/mod.rs` (trait `Clasificador`, tabla de apps);
  - `dpi/ndpi.rs` (FFI con `libloading`);
  - `dpi/cola.rs` (NFQUEUE con `nfq`).
- Modify:
  - `modelo.rs` (`Direccion::App { apps: Vec<u16> }`, que son ids de protocolo nDPI);
  - `nft.rs`;
  - `api.rs` (`GET /apps`, que devuelve `{disponible, motivo?, version?, apps: [{id, nombre, categoria}]}`);
  - `main.rs` (lanza el hilo de la cola si hay reglas App);
  - `manifest.toml` (`[paquetes]`: debian `libndpi4.6` o el que toque, suse `libndpi4`, rhel `ndpi`; se comprueba en un contenedor de cada familia antes de fijarlo).
- Test: `dpi/mod.rs` con `ClasificadorFalso`, y `nft.rs`.

**Interfaces:**
- Produces:
  - `trait Clasificador: Send { fn paquete(&mut self, flujo: u64, datos: &[u8], ahora_ms: u64) -> Option<u16>; }`: devuelve la app cuando está decidida.
  - `ndpi::Ndpi::cargar() -> Result<Ndpi, String>`. Prueba `libndpi.so.4`, `libndpi.so.5` y `libndpi.so`. Comprueba `ndpi_revision()` contra las versiones soportadas (4.x y 5.x). El `Err` es el motivo exacto que se muestra al usuario, por ejemplo «No está instalada la librería nDPI (paquete libndpi).».
  - Funciones FFI que se usan:
    - `ndpi_init_detection_module`;
    - `ndpi_finalize_initialization`;
    - `ndpi_detection_get_sizeof_ndpi_flow_struct`;
    - `ndpi_detection_process_packet`;
    - `ndpi_get_proto_name`;
    - `ndpi_get_num_supported_protocols`;
    - `ndpi_revision`.
  - nft:
    - cadena `dpi` en `forward` con `priority filter - 5`;
    - solo para las parejas con reglas App;
    - la regla es `ct mark 0 meta l4proto { tcp, udp } ct original packets 1-12 queue num 100 bypass`;
    - en `postrouting` y `forward` va `ct mark set meta mark` cuando `meta mark != 0`;
    - las reglas App comparan `ct mark { ids }`;
    - la cola marca `meta mark = 0x10000 | app`, y las reglas comparan con ese desplazamiento.

- [x] **Step 1: Tests que fallan.**
  - `cola_con_bypass`: el script contiene `queue num 100 bypass`;
  - `app_compara_ct_mark`;
  - `sin_reglas_app_sin_cola`;
  - `version_ndpi_no_soportada` (con una revisión simulada en `Ndpi::comprobar_version("3.4")`);
  - `clasificador_falso_marca`.
- [x] **Step 2: Ver que fallan.**

  Run: `cargo test -p netctl-firewall dpi 2>&1 | tail -8`
  Expected: FAIL.
- [x] **Step 3: Implementar.** Instala libndpi en este PC (Arch: `ndpi`; pide al usuario `! sudo pacman -S ndpi`) para la prueba manual de `Ndpi::cargar()` y de que clasifica una captura `.pcap` de ejemplo de nDPI (sin commit del pcap si tiene licencia distinta).
- [x] **Step 4: Pasan.**

  Run: `cargo test -p netctl-firewall 2>&1 | tail -5`
  Expected: verde.
- [x] **Step 5: Commit.** `feat(firewall): destino por app con nDPI cargado en tiempo de ejecución`

### Task 4: Interfaz de Dominio, País y App

**Files:**
- Modify:
  - `modules/firewall/ui/src/FormPolitica.svelte`;
  - `politica.js` y `politica.test.js`.
- Create:
  - `modules/firewall/ui/src/ElegirPais.svelte`;
  - `ElegirApp.svelte`.

**Interfaces:**
- Consumes: `GET /geo` y `GET /apps`.
- Produces: `opcionesDestino(zona, {geo, apps})` añade Dominio, País y App. Si `/apps` dice `disponible: false`, App va desactivada con el motivo en su «i». Solo se ofrecen con una zona de destino Externa.

- [x] **Step 1: Tests que fallan.**
  - `ida y vuelta con dominio, país y app`;
  - `app desactivada con motivo`;
  - `solo hacia externa`.
- [x] **Step 2: Ver que fallan.**

  Run: `node --test modules/firewall/ui/src/politica.test.js`
  Expected: FAIL.
- [x] **Step 3: Implementar.**
  - **Dominio:** chips con validación.
  - **País:** buscador con bandera emoji, nombre en español (`Intl.DisplayNames('es', {type:'region'})`) y la atribución de DB-IP.
  - **App:** buscador por nombre y categoría.
- [x] **Step 4: Pasan, compila y revisión visual** (una captura del panel).

  Run: `node --test modules/firewall/ui/src/ && npm --prefix modules/firewall/ui run build 2>&1 | tail -3`
  Expected: PASS.
- [x] **Step 5: Commit.** `feat(firewall-ui): destinos Dominio, País y App`
- [ ] **Step 6: Revisión final** con haiku (≤15 líneas) y correcciones con TDD. Firewall `0.4.0`.

## Cambios al ejecutar (2026-11-28)

- **App:** el módulo es musl estático y no puede hacer `dlopen`. Con el visto bueno del usuario, nDPI se carga en un ayudante aparte, `netctl-dpi` (crate `modules/firewall/dpi`), compilado con glibc 2.31 en un contenedor (`tools/empaquetar-modulo.sh`). El módulo lo arranca mientras haya reglas de App (`main.rs`, `vigilar_dpi`) y le pregunta las apps (`netctl-dpi apps`).
- ABI comprobada en las cabeceras de nDPI 4.14, 5.0 y 6.0. Debian 12/13 traen la 4.2 y openSUSE Leap la 4.0: ahí App sale desactivada con el motivo. Sin probar con una libndpi real (no está en Arch y no se pudo compilar aquí).
- **Dominio:** los conjuntos no llevan `timeout` de nft; la caducidad de 1 h la lleva el módulo y cada 30 s rehace el conjunto con `flush` + `add element` (así sobrevive a recargas y al «deshacer» de 60 s).
- `[paquetes]` del manifiesto sin libndpi: ninguna distro estable trae una versión soportada.
