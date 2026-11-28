# Fase 2.5a · Firewall al estilo UniFi: aspecto, Ajustes › Motor de políticas, Zonas y Política completa

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans (el usuario quiere ejecución nativa). Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** el Firewall se ve y funciona como Zonas/Políticas de UniFi Network 10.5 y vive en Ajustes › Motor de políticas.

**Architecture:**
- El manifiesto gana `[[ajustes]]` y el shell pinta Ajustes con un segundo menú lateral que monta la interfaz del módulo, como ya hace `ModuloUI`.
- El modelo `Regla` del firewall gana los campos de UniFi, con `serde(default)` para que las configuraciones guardadas sigan cargando, y `nft.rs` los traduce.
- La interfaz del firewall se rehace con los componentes nuevos del kit.

**Tech Stack:** Rust 2024 (axum, serde), nftables, Svelte 5, `node --test`, Brave Origin `--headless` + CDP para la revisión visual.

**Spec:** `docs/superpowers/specs/2026-11-28-firewall-unifi.md` (§1–§4). Base: `docs/superpowers/specs/2026-11-23-netctl-panel-design.md` §7.

## Global Constraints

- Commits con autor `Pentabascal <185202188+Pentabascal@users.noreply.github.com>`. Nunca el correo de hotmail.
- Repo público. La clave privada de minisign nunca entra en el repo.
- No hacer push, publicar ni tocar srv-public sin preguntar.
- §3b:
  - cada campo con «?»/«i» y ejemplo;
  - sin secciones ni modos «Avanzado»;
  - números como texto con comprobador;
  - la tarea principal en ≤3 pasos.
- Revisión visual siempre sin ventanas (Brave `--headless` + CDP, como `scratchpad/visual/cdp.mjs`).
- Las configuraciones de la fase 2 ya guardadas (`confirmada.json`) deben seguir cargando sin perder nada.
- Versiones: firewall `0.2.0`, núcleo `0.3.0`, `nucleo_minimo` del firewall `0.3.0`.
- Colores y medidas: exactamente los de la tabla del spec §2.

## Review Focus

1. Un `confirmada.json` de la fase 2 (`politicas` con `"bloquear"`, reglas sin los campos nuevos) carga y genera el mismo nft que antes. Test: `fase2_sigue_cargando` (Task 3).
2. Una regla con estado «establecida» que bloquea el SSH de la red interna no corta el panel ni el SSH desde las redes privadas: las reglas fijas siguen antes. Test: `estado_establecida_no_pasa_de_las_fijas` (Task 4).
3. Una regla con horario «Una vez» cuyo fin ya pasó no rompe la carga de nft, sino que sale con aviso. Test: `horario_pasado_no_se_genera` (Task 4).
4. MAC en el destino, o una «Red» que no es de la zona elegida, se rechaza con un mensaje en español. Tests: `mac_solo_origen` y `red_de_otra_zona` (Task 3).
5. Una zona Hotspot sin tarjetas no genera cadenas vacías inválidas. Test: el de `nft_real` `hotspot_sin_tarjetas_carga` (Task 4).

---

### Task 1: Kit con el aspecto de UniFi

**Files:**
- Modify: `web/kit/src/tema.css` (valores de los tokens).
- Create:
  - `web/kit/src/Radios.svelte`
  - `web/kit/src/Bloque.svelte`
  - `web/kit/src/Enlace.svelte`
- Modify:
  - `web/kit/src/PanelLateral.svelte` (420 px, título centrado);
  - `web/kit/src/Matriz.svelte` (casillas UniFi);
  - `web/kit/src/index.js`.
- Test: `web/kit/src/validar.test.js` (sin cambios) y revisión visual.

**Interfaces:**
- Produces:
  - `<Radios etiqueta ayuda opciones={[{valor, texto, ayuda?, desactivado?}]} bind:valor />`: radios en línea que saltan de línea si no caben, con «i» por opción si tiene `ayuda`.
  - `<Bloque titulo ayuda>…</Bloque>`: caja `#1c1e21` con radio 8, relleno 16 y título de 13 px en negrita con su «i».
  - `<Enlace icono? onclick desactivado?>texto</Enlace>`: botón con aspecto de enlace `#418ae8`.
  - `Matriz` mantiene sus props y gana:
    - `contar(o, d) -> number`;
    - `todas` (número para la primera casilla);
    - `onfiltrar(o, d | null)`.
  - Las casillas dicen «Permitir todo» o «Bloquear todo», con «(n)» si hay reglas.

- [ ] **Step 1: Tokens.** En `tema.css` pon los valores del spec §2:
  - `--fondo` y `--fondo-lateral` `#131416`;
  - `--superficie` `#1c1e21`;
  - `--superficie-2` y `--campo` `#242629`;
  - nuevo `--cabecera` `#282b2f`;
  - `--azul` `#4797ff`;
  - nuevo `--enlace` `#418ae8`;
  - `--verde-tinte` `#123f20`;
  - `--rojo-tinte` `#4a1a1d`;
  - letra base 13 px;
  - nuevo `--alto-fila: 32px`;
  - `--alto-campo: 32px`.

  Los campos `input` y `select` van sin borde y con fondo `--campo`.
- [ ] **Step 2: Componentes.** Crea `Radios`, `Bloque` y `Enlace`, cambia `PanelLateral` (ancho por defecto 420 px, título centrado, cuerpo con `gap: 12px` entre bloques) y `Matriz`. Expórtalos todos en `index.js`.
- [ ] **Step 3: Comprobar.**

  Run: `cd web/kit && node --test src/ && cd ../.. && npm --prefix web run build 2>&1 | tail -3`
  Expected: tests en verde y build sin errores ni avisos de Svelte.
- [ ] **Step 4: Commit.** `feat(kit): aspecto UniFi (tokens, Radios, Bloque, Enlace, panel y matriz)`

### Task 2: Ajustes con segundo menú y entradas de módulo

**Files:**
- Modify:
  - `crates/netctl-sdk/src/manifiesto.rs` (`ajustes`);
  - `crates/netctl-core/src/modulos/mod.rs:166,256` (pasar `ajustes` al JSON);
  - `web/shell/src/ruta.svelte.js`;
  - `web/shell/src/Lateral.svelte`;
  - `web/shell/src/App.svelte`;
  - `web/shell/src/paginas/Ajustes.svelte`;
  - `web/shell/src/ModuloUI.svelte` (prop `base`).
- Create:
  - `web/shell/src/rutas.js` (`analizar` puro, sacado de `ruta.svelte.js`);
  - `web/shell/src/rutas.test.js`.

**Interfaces:**
- Produces:
  - `EntradaAjustes { grupo: String, titulo: String, ruta: String, #[serde(default)] icono: String }`.
  - `Manifiesto.ajustes: Vec<EntradaAjustes>` (`#[serde(default)]`).
  - JSON de módulo con `"ajustes": [{grupo,titulo,ruta,icono}]`.
  - `analizar('/ajustes/m/firewall/zonas')` → `{ pagina: 'ajustes', seccion: 'm', id: 'firewall', sub: 'zonas' }`.
  - `analizar('/ajustes/password')` → `{ pagina: 'ajustes', seccion: 'password' }`.

- [ ] **Step 1: Tests que fallan.**
  - En `manifiesto.rs`, `ajustes_se_leen`: un manifiesto con dos `[[ajustes]]` de `grupo = "Motor de políticas"` da `m.ajustes.len() == 2` y `m.ajustes[1].ruta == "zonas"`.
  - En `modulos/mod.rs`, extiende el test de la línea 807 con `assert_eq!(v["modulos"][0]["ajustes"], json!([]))`.
  - En `rutas.test.js`, los dos casos de `analizar` de arriba más los que ya existen (`/`, `/modulos`, `/modulo/x/y`).
- [ ] **Step 2: Ver que fallan.**

  Run: `cargo test -p netctl-sdk ajustes_se_leen; cargo test -p netctl-core modulos; node --test web/shell/src/`
  Expected: FAIL (campo y función inexistentes).
- [ ] **Step 3: Implementar.**
  - `ruta.svelte.js` reexporta `analizar` de `rutas.js`.
  - `Lateral.svelte` no muestra un módulo con `menu` vacío y `ajustes` no vacío.
  - En `Ajustes.svelte`:
    - columna izquierda de 210 px con buscador «Buscar ajustes» (filtra por título);
    - después las secciones del núcleo;
    - después, por cada `grupo` de los módulos, un rótulo pequeño y una fila de iconos (`Icono` + `title`) que enlazan a `/ajustes/m/<id>/<ruta>`;
    - a la derecha, `ModuloUI {id} sub={ruta}` cuando `seccion === 'm'`.
  - `ModuloUI` acepta `base` (por defecto `/modulo/<id>`). Desde Ajustes se pasa `/ajustes/m/<id>`, para que `ctx.navegar` se quede dentro de Ajustes.
- [ ] **Step 4: Pasan.** Mismos comandos.

  Expected: PASS, y `cargo clippy --workspace -- -D warnings` limpio salvo el aviso viejo de `wg-modelo`.
- [ ] **Step 5: Commit.** `feat(shell): Ajustes con segundo menú y entradas [[ajustes]] de los módulos`

### Task 3: Modelo de política completo y zona Hotspot

**Files:**
- Modify:
  - `modules/firewall/src/modelo.rs`;
  - `modules/firewall/src/validar.rs`;
  - `modules/firewall/src/plantillas.rs` (solo si construye `Regla` a mano).
- Test: `modules/firewall/src/modelo.rs` y `validar.rs` (módulos `tests`), más `modules/firewall/tests/datos/fase2.json` (copia de un `confirmada.json` real hecha con `examples/demo.rs`).

**Interfaces:**
- Produces (todo con `#[serde(default)]` y `rename_all = "snake_case"`):
  - `TipoZona::Hotspot`.
  - `Protocolo::Otro`, más el campo `Regla.numero_protocolo: u8`.
  - `Direccion::Interfaz { iface: String }` y `Direccion::Mac { mac: String }`.
  - `Regla` gana:
    - `puertos_origen: Puertos`;
    - `version_ip: VersionIp`;
    - `estado: EstadoConexion`;
    - `horario: Horario`;
    - `descripcion: String` con `#[serde(alias = "notas")]` (sustituye a `notas`).
  - `enum VersionIp { #[default] Ambas, V4, V6 }`.
  - `enum EstadoConexion { #[default] Todas, SoloNuevas, Elegir(Vec<Estado>) }` con `#[serde(tag="tipo", content="valor")]`.
  - `enum Estado { Nueva, Establecida, Relacionada, Invalida }`.
  - `enum Horario { #[default] Siempre, Diario(Franja), Semanal { dias: Vec<u8>, franja: Franja }, UnaVez { inicio: u64, fin: u64 }, AMedida(Vec<Tramo>) }` con `#[serde(tag="tipo", content="valor")]`.
  - `struct Franja { desde: String, hasta: String }`, con horas `"HH:MM"`.
  - `struct Tramo { dias: Vec<u8>, franja: Franja }`. Los días van de 0 (lunes) a 6.
  - `Regla::corta_establecidas(&self) -> bool`: true si `estado` es `Elegir` con `Establecida` o `Relacionada`.
  - En `politica_de_serie(o, d)`:
    - `Hotspot` → `Externa` = Permitir; el resto, Bloquear;
    - `Personalizada` → `Externa`, `Gateway` o ella misma = Permitir.
  - Nuevos mensajes de `validar` (exactos):
    - «La MAC solo se puede usar como origen.»
    - «La tarjeta {i} no es de la zona {z}.»
    - «La hora «{h}» no es válida: usa HH:MM, por ejemplo 08:30.»
    - «Elige al menos un día.»
    - «El fin tiene que ser después del inicio.»
    - «Con el protocolo {p} no se pueden elegir puertos.»
    - «El número de protocolo va del 0 al 255.»
    - «Una IPv6 no casa con «Solo IPv4».» (y al revés con «Solo IPv6»)

- [ ] **Step 1: Tests que fallan.** Cada uno con la aserción indicada:
  - `fase2_sigue_cargando`: `serde_json::from_str::<Config>(include_str!("../tests/datos/fase2.json"))` es Ok y la primera regla tiene `descripcion` igual a su antiguo `notas`.
  - `hotspot_solo_internet`: `politica_de_serie(Hotspot, Externa) == Permitir` y `(Hotspot, Interna) == Bloquear`.
  - `personalizada_a_gateway`: `politica_de_serie(Personalizada, Gateway) == Permitir`.
  - `mac_solo_origen`: una regla con destino `Mac` da exactamente el mensaje de arriba.
  - `red_de_otra_zona`: origen `Interfaz{iface:"eth1"}` con `eth1` en la zona `externa` y `origen_zona = "interna"` da el mensaje «La tarjeta eth1 no es de la zona Interna.».
  - `hora_invalida`: `Diario(Franja{desde:"25:00",..})` da su mensaje.
  - `version_ip_choca`: `version_ip = V4` con origen `Ip{red:"fd00::/64"}` da su mensaje.
  - `protocolo_otro_rango`: `Otro` con `numero_protocolo` 0 y 255 es válido (el tipo `u8` ya impide salirse).
- [ ] **Step 2: Ver que fallan.**

  Run: `cargo test -p netctl-firewall --lib 2>&1 | tail -15`
  Expected: errores de compilación por los tipos nuevos.
- [ ] **Step 3: Implementar.** Tipos, `politica_de_serie`, `corta_establecidas` y las validaciones de arriba en `validar::validar`.
- [ ] **Step 4: Pasan.**

  Run: `cargo test -p netctl-firewall 2>&1 | tail -5`
  Expected: todos en verde, incluidos los 24 de `tests/api.rs`.
- [ ] **Step 5: Commit.** `feat(firewall): política completa (origen por red/MAC, puertos de origen, IPv4/6, estado, horario) y zona Hotspot`

### Task 4: nftables para los campos nuevos

**Files:**
- Modify:
  - `modules/firewall/src/nft.rs` (`reglas_usuario` en la línea 113, cadenas `input` y `forward` en las líneas 179 y 223);
  - `modules/firewall/src/aplicar.rs` (avisos de los horarios pasados).
- Test: `modules/firewall/src/nft.rs` (`tests`) y `modules/firewall/tests/nft_real.rs`.

**Interfaces:**
- Consumes: los tipos de la Task 3.
- Produces:
  - `generar(c, f, ahora)`: misma firma.
  - `pub fn avisos(c: &Config, ahora: u64) -> Vec<String>`: reglas activas con horario `UnaVez` ya pasado, con el texto «La regla «{nombre}» ya terminó su horario y no se aplica.».
  - Traducción de cada campo (todo dentro de la misma línea de la regla):

    | Campo | nftables |
    |---|---|
    | `version_ip` V4 / V6 | `meta nfproto ipv4` / `meta nfproto ipv6` |
    | `puertos_origen` | `th sport { … }` con TCP o UDP; si es un grupo, `th sport @gp{id}` |
    | `Protocolo::Otro` | `meta l4proto {n}` |
    | Origen `Interfaz` | `iifname "{iface}"` |
    | Destino `Interfaz` | `oifname "{iface}"` |
    | Origen `Mac` | `ether saddr {mac}` |
    | `SoloNuevas` | `ct state new` |
    | `Elegir(v)` | `ct state { new, established, related, invalid }`, solo los elegidos |
    | `Diario` | `meta hour "08:00"-"17:00"` |
    | `Semanal` | lo mismo más `meta day { "Monday", … }` |
    | `UnaVez` | `meta time "AAAA-MM-DD HH:MM:SS"-"…"` en UTC |
    | `AMedida` | una línea nft por tramo |
  - Las reglas con `corta_establecidas()` se escriben en `input` y `forward` **después** de las reglas fijas y **antes** de `ct state established,related accept`. El resto, en su sitio de siempre.

- [ ] **Step 1: Tests que fallan.** Todos usan `generado(...)`. Comprobaciones de texto en `nft.rs`:
  - `v4_solo`: contiene `meta nfproto ipv4`.
  - `puerto_origen`: contiene `th sport 1024-65535`.
  - `protocolo_otro`: contiene `meta l4proto 47`.
  - `origen_red_y_mac`: contiene `iifname "eth1"` y `ether saddr aa:bb:cc:dd:ee:ff`.
  - `horario_semanal`: contiene `meta day { "Monday", "Friday" } meta hour "08:00"-"17:00"`.
  - `estado_establecida_no_pasa_de_las_fijas`: en `input`, la posición de la regla es mayor que la de la regla fija del SSH y menor que la de `ct state established,related accept`.
  - `horario_pasado_no_se_genera`: una regla `UnaVez{fin < ahora}` no aparece y `avisos` la nombra.

  En `nft_real.rs`, con `unshare -rn nft -f`:
  - `campos_nuevos_cargan`: una config con todos los campos nuevos carga en el kernel;
  - `hotspot_sin_tarjetas_carga`.
- [ ] **Step 2: Ver que fallan.**

  Run: `cargo test -p netctl-firewall nft 2>&1 | tail -15`
  Expected: FAIL.
- [ ] **Step 3: Implementar** en `reglas_usuario` y en la construcción de `input` y `forward`. Sustituye `notas` por `descripcion` en el comentario nft si se usa.
- [ ] **Step 4: Pasan.**

  Run: `cargo test -p netctl-firewall 2>&1 | tail -5`
  Expected: todo en verde (los de `nft_real` se saltan solos si no hay `unshare`; aquí sí lo hay).
- [ ] **Step 5: Commit.** `feat(firewall): nftables para versión IP, puertos de origen, protocolo, red/MAC, estado y horario`

### Task 5: API para la página de Zonas de UniFi

**Files:**
- Modify: `modules/firewall/src/api.rs`.
- Test: `modules/firewall/tests/api.rs`.

**Interfaces:**
- Consumes: `Config` y `Regla` de la Task 3, y `nft::avisos` de la Task 4.
- Produces:
  - `GET /config`: lo que ya devuelve, y además:
    - `matriz[i].reglas: usize`;
    - `avisos: Vec<String>`;
    - `tarjetas_de_zona: {zona_id: [iface]}`.
  - `GET /politicas`:
    - `{ de_serie: [{nombre, accion, parejas: [{origen, destino}]}], propias: [Regla + {zona_origen_nombre, zona_destino_nombre, texto_origen, texto_destino, texto_puertos, texto_puertos_origen}] }`;
    - `de_serie` agrupa las casillas por «Permitir todo» y «Bloquear todo»;
    - los textos usan el mismo formato que `textoDireccion` del kit.
  - `POST /reglas/{id}/duplicar`: copia desactivada con el nombre «{nombre} (copia)». Responde 202, como los demás cambios.
  - `POST /zonas` `{nombre, tarjetas: [iface], revision}`: crea una zona `Personalizada`.
  - `PUT /zonas/{id}` `{nombre?, tarjetas, revision}`: en las de serie solo cambia las tarjetas.
  - `DELETE /zonas/{id}`: solo las personalizadas; 409 con `usos` si hay reglas que la usan.

- [ ] **Step 1: Tests que fallan** en `tests/api.rs`:
  - `politicas_agrupa_las_de_serie`: el número total de parejas es zonas².
  - `duplicar_regla`.
  - `crear_zona_con_tarjetas`: la tarjeta pasa de su zona anterior a la nueva.
  - `zona_de_serie_no_se_borra`: 403.
  - `zona_en_uso_409`.
- [ ] **Step 2: Ver que fallan.**

  Run: `cargo test -p netctl-firewall --test api 2>&1 | tail -10`
  Expected: FAIL (404 en las rutas nuevas).
- [ ] **Step 3: Implementar** las rutas con el patrón `202 {pendiente, revision}` de siempre.
- [ ] **Step 4: Pasan.** Mismo comando más `cargo test -p netctl-firewall`.

  Expected: todo en verde.
- [ ] **Step 5: Commit.** `feat(firewall): API de políticas agrupadas, zonas y duplicar regla`

### Task 6: Página Zonas al estilo UniFi

**Files:**
- Modify:
  - `modules/firewall/ui/src/Zonas.svelte` (rehacer);
  - `modules/firewall/ui/src/App.svelte` (rutas y sin `Barra`).
- Create:
  - `modules/firewall/ui/src/EsquemaZonas.svelte`;
  - `TablaZonas.svelte`;
  - `PanelZona.svelte`;
  - `ListaPoliticas.svelte`;
  - `listaPoliticas.js` y `listaPoliticas.test.js`.
- Delete:
  - `modules/firewall/ui/src/Interfaces.svelte`, que pasa a ser `TablaZonas` + `PanelZona`;
  - la pestaña `Ajustes.svelte`. Su contenido (ping externo y NAT por zona) pasa a un enlace «Ajustes del firewall» bajo la tabla de zonas, que abre un `PanelLateral` con dos `Bloque`.

**Interfaces:**
- Consumes: `GET /config`, `GET /politicas`, `POST/PUT/DELETE /zonas` (Task 5); `Radios`, `Bloque`, `Enlace` y `Matriz` (Task 1).
- Produces:
  - `listaPoliticas.js`:
    - `filtrar(propias, { par: [o,d] | null, v4: bool, v6: bool, texto: string }) -> Regla[]`: v4/v6 miran `version_ip`, y `Ambas` pasa con cualquiera de las dos;
    - `COLUMNAS`: `[{id, titulo, porDefecto}]` con los diez del spec §3.6;
    - `mover(lista, id, delta) -> ids[]`.

- [ ] **Step 1: Tests que fallan** en `listaPoliticas.test.js`:
  - `filtra por pareja`;
  - `quita v6`: una regla `V6` desaparece con `v6:false`; una `Ambas` se queda;
  - `busca sin acentos` (reutiliza la idea de `buscarEquipos`);
  - `mover respeta los límites`.
- [ ] **Step 2: Ver que fallan.**

  Run: `node --test modules/firewall/ui/src/`
  Expected: FAIL (módulo inexistente).
- [ ] **Step 3: Implementar** las piezas en el orden del spec §3:
  - esquema plegable;
  - tabla de zonas con `Enlace` «Crear zona»;
  - matriz con `onfiltrar` y «Todas las políticas (n)»;
  - casillas IPv4/IPv6, buscador y elegir columnas (se recuerda en `localStorage` con try/catch);
  - filas de serie agrupadas con candado y desplegables;
  - interruptor de pausa;
  - enlaces «Crear política» y «Reordenar» (modo subir/bajar que guarda con `/reglas/orden`).

  `SoloEntrada` se queda para el caso de una sola tarjeta. Al pulsar una fila o «Crear política» se abre `FormPolitica` (Task 7).
- [ ] **Step 4: Pasan y compila.**

  Run: `node --test modules/firewall/ui/src/ && npm --prefix modules/firewall/ui run build 2>&1 | tail -3`
  Expected: PASS y build sin avisos.
- [ ] **Step 5: Commit.** `feat(firewall-ui): página Zonas como UniFi (esquema, tabla, matriz que filtra, lista de políticas)`

### Task 7: Formulario de política

**Files:**
- Create:
  - `modules/firewall/ui/src/FormPolitica.svelte`;
  - `modules/firewall/ui/src/politica.js`;
  - `modules/firewall/ui/src/politica.test.js`.
- Delete:
  - `FormRegla.svelte`;
  - `PanelCasilla.svelte`. Lo de la política de la pareja pasa a la cabecera de la matriz y abre un panel pequeño con `Radios` Permitir todo / Bloquear todo y «Volver a la de serie».

**Interfaces:**
- Consumes: el modelo de la Task 3 (forma JSON) y `SelectorEquipo` y `CampoNumero` del kit.
- Produces:
  - `politica.js`:
    - `deRegla(r) -> estadoFormulario`;
    - `aRegla(f) -> Regla` (ida y vuelta sin pérdidas);
    - `opcionesOrigen(zona, tarjetas)` → radios Cualquiera/Equipo/Red/IP/MAC; «Red» solo si la zona tiene tarjetas;
    - `opcionesDestino(zona)` → Cualquiera/Equipo-IP/Red;
    - `textoHorario(h)`, por ejemplo «L–V 08:00–17:00».

- [ ] **Step 1: Tests que fallan** en `politica.test.js`:
  - `ida y vuelta`: para tres reglas de ejemplo, con origen MAC, horario semanal y estado elegir, `aRegla(deRegla(r))` es igual a `r`;
  - `sin Red si la zona no tiene tarjetas`;
  - `textoHorario semanal`, que da exactamente `L, V 08:00–17:00`.
- [ ] **Step 2: Ver que fallan.**

  Run: `node --test modules/firewall/ui/src/politica.test.js`
  Expected: FAIL.
- [ ] **Step 3: Implementar** `FormPolitica` con `PanelLateral` y los bloques del spec §4, en ese orden y con radios en línea. Cada campo lleva su «i» con ejemplo. El pie tiene Borrar, Duplicar, Cancelar y Guardar. Los errores de `/reglas` se muestran con `Aviso`.
- [ ] **Step 4: Pasan y compila.**

  Run: `node --test modules/firewall/ui/src/ && npm --prefix modules/firewall/ui run build 2>&1 | tail -3`
  Expected: PASS.
- [ ] **Step 5: Commit.** `feat(firewall-ui): formulario de política como UniFi`

### Task 8: Manifiesto, versiones y revisión visual contra UniFi

**Files:**
- Modify:
  - `modules/firewall/manifest.toml`: `menu` vacío, `[[ajustes]]` con grupo «Motor de políticas» y entradas Zonas (icono `cortafuegos`), Reenvío de puertos (`globo`; pasa a la Tabla en 2.5b), Grupos (`clientes`) y Registro (`reloj`); versión `0.2.0` y `nucleo_minimo` `0.3.0`;
  - `modules/firewall/Cargo.toml`, a `0.2.0`;
  - `crates/netctl-core/Cargo.toml`, a `0.3.0`;
  - `docs/prueba-fase2.md`, que pasa a llamarse `docs/prueba-fase25.md` con los pasos nuevos.
- Test: suite completa y capturas.

- [ ] **Step 1: Suite.**

  Run: `cargo test --workspace 2>&1 | grep -E "test result|FAILED" ; node --test web/kit/src/ web/shell/src/ modules/firewall/ui/src/; cargo clippy --workspace -- -D warnings 2>&1 | tail -3`
  Expected: todo en verde.
- [ ] **Step 2: Revisión visual.** Con `scratchpad/visual/arrancar.sh` (núcleo + `examples/demo.rs`) y un guion CDP, captura a 1600×1000:
  - `/ajustes/m/firewall/zonas`;
  - la matriz filtrada;
  - el panel «Crear política»;
  - el panel «Crear zona».

  Compáralas con `scratchpad/uni/capturas/16-zonas-oscuro.png`, `20-crear-politica-panel.png` y `21-crear-zona-panel.png`. Corrige lo que no case (colores, alturas, orden). Cuenta máximo 3 capturas leídas.
- [ ] **Step 3: Commit.** `chore: firewall 0.2.0 en Ajustes › Motor de políticas; núcleo 0.3.0`
- [ ] **Step 4: Revisión final** con un revisor haiku (informe de ≤15 líneas), como dice `executing-plans`. Corrige lo importante con TDD.
