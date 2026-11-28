# netctl fase 2 — Firewall con reenvío de puertos

> **Para quien lo implemente:** usar superpowers:executing-plans (un único implementador, revisión ligera al final). Pasos con casillas (`- [ ]`).

**Objetivo:** módulo `firewall` instalable desde el panel: zonas y matriz zona→zona en nftables (tabla `inet netctl`), vuelta atrás automática a los 60 s, reglas fijas para no quedarse fuera, reenvío de puertos con asistente de 3 pasos, y las piezas comunes del kit que piden las reglas de interfaz (§3b): selector de equipos, ayuda «?», campos numéricos con comprobador, configuración generada y registro.

**Arquitectura:** binario `netctl-mod-firewall` (mismo patrón que `modules/wireguard`: lógica pura + trait `Sistema` + `Rutas` inyectables). La generación de reglas es una función pura `modelo → texto nft` con tests. El núcleo sube a **0.2.0** (arranque temprano de módulos, aviso antes de desinstalar, `/api/equipos`, clave minisign nueva).

**Spec:** `docs/superpowers/specs/2026-11-23-netctl-panel-design.md` §3b, §7.6, §9, §9b.

## Restricciones globales

- Todo lo de la fase 1 sigue valiendo (castellano llano, tema oscuro, musl, rutas `/var/lib/netctl/firewall/` 700, socket `/run/netctl/mod-firewall.sock`).
- Tabla nftables `inet netctl` (IPv4 + IPv6), cadenas `input`/`forward` con prioridad `filter` y `output` con política accept; NAT (DNAT y masquerade) en la misma tabla `inet netctl` (cadenas `prerouting` dstnat y `postrouting` srcnat), como dice el spec. Carga **atómica** con `nft -f` (el script empieza con `table inet netctl {}` + `delete table inet netctl`). Nunca se toca `netctl_wg` ni tablas ajenas.
- Siempre antes de cualquier regla: `ct state established,related accept`, `ct state invalid drop`, `iif lo accept`.
- **Reglas fijas** (visibles, no borrables, con candado): puerto del panel desde donde permita el alcance del núcleo (SoloLan → zonas Interna y VPN; Internet → todas), SSH desde Interna (y VPN), puerto UDP de WireGuard desde Externa si existe el módulo, DHCPv6/ICMPv6 necesarios para que IPv6 funcione.
- **Todo cambio** del Firewall (políticas, reglas, zonas, reenvíos, grupos) se aplica con `netctl_sdk::confirmar::Pendiente` y 60 s, como pide el spec. Solo la desactivación automática por caducidad se aplica sin confirmar.
- Reenvío de puertos solo IPv4 en esta fase (es lo habitual); IPv6 se abre con reglas normales.
- Cada página y cada sección empieza con una frase que explica qué se hace ahí; las opciones que dependen de otra solo aparecen cuando aplican (§3b).
- Lo que depende de módulos o fases futuras **no se muestra** todavía: «fijar IP» (DHCP), países (Seguridad), grupos de usuarios (Directorio), dominio y varias WAN de Router; las comprobaciones BPA del Firewall se añaden en la fase 2e.
- Datos del sistema que el firewall **lee** (no hay llamadas entre módulos): interfaces (`ip -j addr`, `/sys/class/net`), ruta por defecto (`/proc/net/route`), puerto del panel y alcance (`/etc/netctl/core.toml`), puerto SSH (`sshd -T` → `port`, si falla 22), interfaces y `ListenPort` de WireGuard (`/etc/wireguard/*.conf`, solo lectura).
- Los países en «desde dónde» llegan en la fase 4 (GeoIP); en esta fase: «Desde cualquier sitio de internet» o «Solo desde estas IPs».
- Commits en castellano, autor noreply.

## Review Focus

1. **No quedarse fuera:** regla mala + sin confirmar → a los 60 s vuelve el ruleset anterior (test con `Sistema` doble y plazo corto). Las reglas fijas aparecen en el texto nft generado **antes** que cualquier regla del usuario, en todas las combinaciones (test por propiedad sobre modelos aleatorios).
2. Si el módulo se reinicia con un cambio sin confirmar, al arrancar carga la **última configuración confirmada**, no la pendiente.
3. Nombres, comentarios e IPs del usuario nunca se interpolan sin validar en el texto nft (inyección): nombres → `nombre_seguro`, direcciones → parseadas con `ipnet`, puertos → números/rangos validados.
4. Choque de reenvío con otra regla, el panel, SSH o WireGuard → rechazado ofreciendo el siguiente puerto libre.
5. Instalar el firewall con firewalld/ufw activos: se guarda su estado, se desactivan y el WireGuard UDP y el panel siguen accesibles (reglas fijas). Desinstalar con «restaurar» → firewalld/ufw vuelven como estaban.
6. Caducidad de un reenvío: al vencer se desactiva solo (no se borra) y queda en el registro.

---

## Lista de comprobación de opciones (para no dejarse nada)

Cada opción: dónde está, valor por defecto, y si es (recomendado).

**Interfaces y zonas**
- [ ] Zona de cada interfaz (Externa / Interna / VPN / DMZ / personalizada). Por defecto: la de la ruta por defecto → Externa (recomendado); interfaces con IP privada → Interna; `wg*` → VPN; el resto → Interna.
- [ ] Crear, renombrar y borrar zonas personalizadas (no se pueden borrar las de serie; una zona con interfaces no se borra).
- [ ] Interfaces que aparecen después (USB, VLAN): van a la zona «Sin asignar» = bloqueado entrante, con aviso en Inicio.

**Matriz zona→zona**
- [ ] Política por defecto de cada casilla: Permitir / Bloquear (recomendados de serie, tabla abajo).
- [ ] Gateway (la propia máquina) como zona de destino: es la cadena `input`.
- [ ] Modo «una sola interfaz»: si solo hay una interfaz que no es loopback ni wg, la matriz se oculta y se ve «Qué puede entrar a esta máquina».

**Reglas** (por casilla)
- [ ] Nombre · Acción (Permitir / Bloquear (drop) / Rechazar (reject)) · Protocolo (Cualquiera / TCP / UDP / TCP y UDP / ICMP) · Puertos de destino (uno, lista, rango, o grupo de puertos) · Origen (selector de equipos o grupo de IPs, «Cualquiera») · Destino (ídem) · Registrar coincidencias (log, desactivado) · Activa · Orden (arrastrar) · Contador de aciertos (solo lectura) · Notas.
- [ ] Puertos de origen: no (rara vez útil; fuera de alcance).

**Grupos**
- [ ] Grupos de IPs (equipos, IPs, redes, rangos) y grupos de puertos; usados en reglas y reenvíos; no se borra un grupo en uso (dice dónde se usa).

**Reenvío de puertos**
- [ ] En el asistente: Equipo (selector, vinculado por MAC) · Servicio (plantilla u «Otro» con puerto o rango, p. ej. `27015-27030`, y TCP / UDP / los dos) · Puerto de fuera (opcional, por defecto el mismo) · Desde dónde («Desde cualquier sitio de internet» (por defecto) / «Solo desde estas IPs») · Caduca (Nunca (por defecto) / 1 hora / 1 día / 1 semana).
- [ ] Solo en el panel lateral (no se preguntan, cada uno explicado): WAN de entrada (todas, por defecto) · Acceso desde dentro con la IP pública (NAT loopback / hairpin, activado) · Registro de conexiones (desactivado) · Nombre (equipo + servicio) · Activo.
- [ ] Plantillas: Web (80/443), SSH (22, delicado), Escritorio remoto (3389, delicado), Minecraft (25565), Servidor de juegos (27015 TCP+UDP), Plex (32400), Jellyfin (8096), Cámaras RTSP (554), FTP (21), Correo (25, 465, 587, 993), Bases de datos MySQL/PostgreSQL (3306/5432, delicado), Otro.
- [ ] Avisos: puerto de fuera ya usado por otra regla, el panel, SSH o WireGuard → se ofrece el siguiente libre · servicio delicado abierto a todo internet → recomienda «Solo desde estas IPs» o la VPN, con botón directo · destino en zona VPN o Gateway → explica que no hace falta reenvío.
- [ ] Gestión: estados ● Funcionando / ⚠ El equipo está desconectado / ⚠ El equipo no responde en ese puerto (el servidor prueba a conectar desde dentro) / ⏱ Caduca en … / ⊘ Desactivada · «Última conexión» y número de conexiones · editar, **duplicar** y borrar en el panel lateral · filtros Todas / Activas / Desactivadas / Con problemas / Temporales · búsqueda por equipo, servicio o puerto.
- [ ] Una sola tarjeta de red: no hay reenvío; la página explica cómo abrir el puerto en el router de la red y ofrece la regla equivalente Externa→Gateway.

**NAT**
- [ ] Enmascaramiento (masquerade) por zona de salida Externa: activado para Interna, VPN y DMZ (recomendado). Interruptor por zona de origen.

**Ajustes generales**
- [ ] Responder a ping desde Externa (activado, recomendado: ayuda a diagnosticar).
- [ ] Al desinstalar: restaurar firewalld/ufw guardado (pregunta en el diálogo).
- [ ] Ver configuración generada (solo lectura, copiar) y Registro de cambios (fecha, quién, qué, resultado, confirmado/revertido).

**Políticas por defecto de la matriz (origen ↓ → destino →)**

| | Externa | Interna | Gateway | VPN | DMZ | Personalizada |
|---|---|---|---|---|---|---|
| Externa | — | Bloquear | Bloquear | Bloquear | Bloquear | Bloquear |
| Interna | Permitir | Permitir | Permitir | Permitir | Permitir | Permitir |
| VPN | Permitir | Permitir¹ | Permitir | Permitir | Permitir | Bloquear |
| DMZ | Permitir | Bloquear | Bloquear² | Bloquear | Permitir | Bloquear |
| Personalizada | Permitir | Bloquear | Bloquear² | Bloquear | Bloquear | Permitir |

¹ El módulo WireGuard sigue decidiendo por cliente (ambas tablas deben aceptar). ² Salvo DNS/DHCP si los ofrece la máquina (regla fija creada sola cuando esté el módulo DHCP; en esta fase no hay).

---

## Pantallas y textos de ayuda

Entradas de la barra lateral del módulo: **Zonas**, **Reenvío de puertos**, **Grupos**, **Registro**. Cada campo lleva línea de ayuda + «?» (ejemplo y recomendado).

**Asistente (primera vez, 3 pasos)**
1. «¿Por dónde llega internet?» — lista de interfaces con IP y la sugerida marcada «(recomendado)». Ayuda: «Es la conexión de tu router o proveedor. Todo lo que entre por ella se bloquea salvo lo que permitas.» «?»: «Ejemplo: `ens18 · 203.0.113.4` — si solo tienes una tarjeta de red, elige esa.»
2. «¿Qué es cada una de las demás?» — Interna / DMZ / VPN por interfaz. Ayuda: «Interna: tus equipos de confianza. DMZ (zona desmilitarizada): servidores expuestos que no deben ver tu red interna.»
3. «Revisa y aplica» — resumen en castellano («Se bloqueará todo lo que entre desde internet salvo: panel (8443) desde tu red, SSH (22) desde tu red, WireGuard (51820/udp)…») + aviso de firewalld/ufw si están activos («Se desactivará firewalld y se guardará su configuración para poder volver a ella»). Botón «Aplicar» → barra de 60 s.

**Zonas** — arriba tarjetas (interfaces, zonas, reglas activas, paquetes bloqueados hoy). Debajo la **matriz**: casilla verde «Permitir» / roja «Bloquear» + número de reglas propias. Clic en casilla → panel lateral «Interna → Externa»: política por defecto (selector Permitir/Bloquear, ayuda «Lo que pasa con el tráfico que no coincide con ninguna regla de abajo») y lista de reglas ordenable con botón «+ Regla». En VPN→Interna: aviso «Además, cada cliente VPN tiene sus propios permisos en WireGuard → Clientes» con enlace. Pestaña **Interfaces** (lista con zona editable y chip de estado) en la misma página.
- Regla, ayudas: Nombre «Para reconocerla en la lista»; Acción «Bloquear descarta en silencio (drop); Rechazar avisa al otro lado (reject), útil dentro de tu red»; Protocolo «Si no lo sabes, deja TCP y UDP»; Puertos «Ej.: `443`, `80,443`, `8000-8100` o un grupo»; Origen/Destino «Elige un equipo, escribe una IP o red (`192.168.1.0/24`) o un rango (`192.168.1.10-192.168.1.20`)»; Registrar «Guarda en el registro del sistema cada paquete que coincida; úsalo solo para investigar, genera mucho texto».

**Una sola interfaz** — «Qué puede entrar a esta máquina»: lista de servicios abiertos (fijos con candado + los tuyos), botón «+ Abrir un puerto» (asistente de 2 pasos: servicio/plantilla → desde dónde).

**Reenvío de puertos** — frase de cabecera: «Deja que desde internet se llegue a un servicio de un equipo de tu red.» Lista agrupada por equipo («Servidor-casa · 3 puertos»), cada fila con servicio, puerto de fuera → dentro, protocolo, desde dónde, estado en llano, última conexión y nº de conexiones, interruptor activar/desactivar. Filtros y búsqueda arriba. Botón «+ Abrir puerto» → asistente:
1. **¿A qué equipo?** Selector («Busca por nombre, IP o MAC, o escribe la IP. Si eliges un equipo, la regla le sigue aunque cambie de IP»).
2. **¿Qué servicio?** Tarjetas de plantillas + «Otro» (Puerto o rango «Ej.: `25565` o `27015-27030`», Protocolo «Si no lo sabes, deja TCP y UDP»). Campo opcional «Puerto de fuera» («Normalmente el mismo; cámbialo si ya está usado»). Error en el momento: «El 8443 lo usa el panel de netctl. [Usar 8444]».
3. **¿Desde dónde?** «Desde cualquier sitio de internet» (por defecto) / «Solo desde estas IPs» (selector, varias); «Caduca» (Nunca por defecto). Aviso si delicado: «Abrir el escritorio remoto a todo internet es peligroso: lo atacan robots a diario. Mejor limitarlo a tus IPs o entrar por la VPN.» [Solo desde mi IP actual] [Ir a la VPN].
Al pulsar «Crear»: barra de 60 s y, al confirmar, «Desde fuera, usa 85.12.34.56:25565» [Copiar] (IP pública de cada WAN averiguada por la interfaz Externa; si es privada —detrás de otro router— se avisa de que hay que abrir también el puerto en ese router).

**Grupos** — dos pestañas (IPs, Puertos), lista + panel lateral, «Se usa en: …».

**Registro** — tabla de cambios (fecha, usuario, resumen, estado: Aplicado / Confirmado / Revertido a los 60 s / Error) y pestaña **Configuración generada** (texto nft en solo lectura con botón Copiar).

---

### Task 1: Modelo y generación nft (puro, con tests)

**Files:** Create `modules/firewall/{Cargo.toml,manifest.toml,src/{lib.rs,main.rs,modelo.rs,nft.rs,validar.rs,plantillas.rs}}`; workspace ya incluye `modules/*`.

**Interfaces:**
- `modelo::Config { revision: u64, zonas: Vec<Zona{id, nombre, tipo: TipoZona{Externa,Interna,Gateway,Vpn,Dmz,Personalizada}}>, interfaces: BTreeMap<String /*iface*/, String /*zona id*/>, politicas: BTreeMap<(String,String), Politica{Permitir,Bloquear}>, reglas: Vec<Regla>, grupos_ip, grupos_puertos, reenvios: Vec<Reenvio>, nat: BTreeMap<String /*zona origen*/, bool>, ping_externo: bool }`.
- `Regla { id, nombre, origen_zona, destino_zona, accion: Accion{Permitir,Bloquear,Rechazar}, protocolo, puertos: Puertos, origen: Direccion, destino: Direccion, log, activa, notas }`. `Direccion { Cualquiera, Equipo{mac, ip}, Ip(IpNet), Rango(IpAddr,IpAddr), Grupo(id) }`.
- `Reenvio { id, nombre, equipo: Direccion, protocolo, externo: Puertos, interno: Option<u16>, wan: Option<String>, desde: Vec<Direccion>, caduca: Option<u64>, loopback: bool, activo, log }`.
- `Fijas { panel: u16, alcance_internet: bool, ssh: u16, wg: Vec<(String /*iface*/, u16)> }`.
- `nft::generar(c: &Config, f: &Fijas, ahora: u64) -> Result<String, Vec<String>>` → script para `nft -f` (tablas `inet netctl` + `ip netctl_nat`). `Config::por_defecto(ifaces: &[Iface], ruta_defecto: Option<&str>) -> Config` con la tabla de políticas de arriba.
- `validar::validar(c, f) -> Vec<String>` (errores en castellano con solución).

- [ ] **Step 1: Tests que fallan:** `fijas_primero` (las reglas del panel/SSH/WG están antes que cualquier regla del usuario, también con política Bloquear en todo); `fijas_siempre` (propiedad con `proptest`: para cualquier Config válida, el texto contiene la regla del panel); `matriz_por_defecto` (Externa→Gateway drop, Interna→Externa accept, VPN→Interna accept); `una_interfaz` (`por_defecto` con una sola iface → Externa); `nombre_inyeccion` (nombre `"x\"; flush ruleset; \""` → comentario saneado); `rango_ip_valido` / `rango_invertido_error`; `reenvio_dnat` (`dnat to 192.168.1.20:3389` + regla forward que acepta); `reenvio_rango` (`27015-27030`); `reenvio_puerto_distinto` (fuera 2222 → dentro 22); `reenvio_loopback` (snat/masquerade de la LAN hacia el equipo); `reenvio_caducado` (con `ahora` > `caduca` no sale); `choque_panel` (reenvío externo 8443 → error con sugerencia 8444); `siguiente_libre` (salta los ocupados); `choque_entre_reenvios`; `grupo_en_uso_no_borrable`; `nat_masquerade_por_zona`; `ipv6_icmp_necesario` (`icmpv6 type { nd-neighbor-solicit, nd-router-advert, … } accept`).
- [ ] **Step 2:** `cargo test -p netctl-mod-firewall` FAIL. **Step 3:** Implementar (sets con nombre para grupos, `counter` en cada regla con `comment "r<id>"` para leer contadores). **Step 4:** PASS.
- [ ] **Step 5:** Commit `firewall: modelo y generación de reglas nftables`.

### Task 2: Sistema, aplicar con confirmación y traspaso de firewalld/ufw

**Files:** Create `modules/firewall/src/{sistema.rs,aplicar.rs,estado.rs,leer.rs,registro.rs}`.

**Interfaces:**
- `Sistema` (trait, doble en tests): `ejecutar(cmd, args, stdin)`; `Rutas { datos, etc_netctl, wireguard, sys_net, proc_route }`.
- `leer::fijas(s, r) -> Fijas` (core.toml, `sshd -T`, `/etc/wireguard/*.conf`), `leer::interfaces(s) -> Vec<Iface{nombre, ips, mac, activa}>`.
- `estado::Almacen`: `confirmada.json` (última buena) y `pendiente.json`; al arrancar se aplica `confirmada`.
- `aplicar::Aplicador`: `aplicar(nueva, peligroso: bool) -> Result<Option<IdCambio>>` — guarda `nft list table inet netctl` + `ip netctl_nat` actuales, `nft -f` nuevo; si `peligroso`, `Pendiente::aplicar` con 60 s que restaura y escribe «Revertido» en el registro; `confirmar(id)` pasa pendiente → confirmada.
- `aplicar::traspaso_inicial(s)`: si `firewalld` o `ufw` activos → guarda (`firewall-cmd --list-all-zones` + copia de `/etc/firewalld`, o `ufw status verbose` + `/etc/ufw`) en `datos/anterior/`, `systemctl disable --now` (ufw: `ufw disable`). `restaurar_anterior(s)`.
- `registro::Registro` (`registro.jsonl`, rotado a 1000 entradas).
- Estado «no responde»: cada 60 s, conexión TCP de prueba (timeout 2 s) desde el servidor al equipo y puerto interno de cada reenvío activo TCP; UDP no se prueba (se muestra solo conectado/desconectado).
- Caducidades: tarea tokio cada 30 s, desactiva reenvíos vencidos (aplica sin confirmación) y registra. Vinculación por MAC: misma tarea, si `ip neigh` da otra IP para una MAC usada → regenerar y registrar «IP de <equipo> cambió de A a B».

- [ ] **Step 1: Tests que fallan:** `sin_confirmar_revierte` (plazo 50 ms → el doble registra el `nft -f` del ruleset anterior y el registro dice Revertido); `confirmado_no_revierte`; `arranque_usa_confirmada` (existe pendiente.json → se aplica confirmada.json); `nft_falla_no_cambia` (error de `nft -f` → 422 con el mensaje de nft traducido y estado sin tocar); `traspaso_firewalld` (activo → guardado + `disable --now`); `restaurar_firewalld`; `caducidad_desactiva`; `mac_cambia_ip_regenera`.
- [ ] **Step 2–4:** FAIL → implementar → PASS.
- [ ] **Step 5:** Commit `firewall: aplicar con confirmación de 60 s y traspaso de firewalld/ufw`.

### Task 3: API del módulo

**Files:** Create `modules/firewall/src/api.rs`, `modules/firewall/tests/api.rs`, `modules/firewall/examples/demo.rs` (datos falsos, como el de WireGuard).

**Rutas:** `GET /salud`, `GET /resumen` (zonas, reglas, reenvíos activos, bloqueados hoy), `POST /evento` (`modulos_cambiados` → releer fijas; `desinstalando {restaurar: bool}` → quitar tablas y, si toca, restaurar), `GET /equipos` (vecinos `ip -j neigh` con MAC, para el núcleo), `GET /asistente` (sugerencia) y `POST /asistente`, `GET/PUT /config` (revisión → 409 si vieja), `GET /interfaces`, `PUT /interfaces/{if}`, `GET/PUT /politicas/{origen}/{destino}`, `GET/POST /reglas`, `PUT/DELETE /reglas/{id}`, `POST /reglas/orden`, `GET/POST /reenvios`, `PUT/DELETE /reenvios/{id}`, `POST /reenvios/{id}/activar|desactivar|duplicar`, `POST /reenvios/comprobar` (avisos y choques sin guardar, para el asistente), `GET /plantillas`, `GET/POST /grupos`, `PUT/DELETE /grupos/{id}`, `GET /pendiente` (id + segundos restantes) y `POST /confirmar/{id}`, `GET /contadores` (de `nft -j list table`), `GET /generada`, `GET /registro`. Respuesta de cambios peligrosos: `202 {pendiente: id, segundos: 60}`.

- [ ] **Step 1: Tests que fallan:** `politica_devuelve_pendiente`; `revision_vieja_409`; `comprobar_avisa_rdp`; `comprobar_destino_vpn_explica`; `reenvio_pide_confirmacion`; `regla_fija_no_borrable` (DELETE → 403 «Esta regla protege tu acceso al panel»); `contadores_parsea_json`.
- [ ] **Step 2–4:** FAIL → implementar → PASS. **Step 5:** Commit `firewall: API`.

### Task 4: Núcleo 0.2.0 y WireGuard 0.2.0

**Files:** Modify `crates/netctl-sdk/src/manifiesto.rs` (campo `arranque_temprano: bool`, `#[serde(default)]`), `crates/netctl-core/src/modulos/{mod.rs,instalar.rs}`, `crates/netctl-core/src/api.rs`, `modules/wireguard/src/api.rs`, versiones en `Cargo.toml`.

- Unidad con `arranque_temprano` → `DefaultDependencies=no`, `Before=network-pre.target`, `Wants=network-pre.target`, `After=local-fs.target`.
- `desinstalar(id, borrar_datos, restaurar)`: antes de parar, `POST /evento {"tipo":"desinstalando","restaurar":bool}` (espera 10 s). El diálogo de Módulos muestra la casilla «Restaurar el firewall que había antes» solo para `firewall`.
- `GET /api/equipos`: junta `GET /equipos` de cada módulo instalado (404 se ignora) y `ip -j neigh` del núcleo; deduplica por MAC; campos `{nombre, ip, mac, fabricante?, origen, conectado}`. WireGuard aporta sus clientes (`origen: "vpn"`, sin MAC → clave pública como id).
- Clave minisign nueva (la anterior se perdió): el usuario genera `minisign -G -W -p crates/netctl-core/clave_publica.txt -s ~/.minisign/netctl.key`; se commitea solo la pública. Los núcleos 0.1.x no podrán instalar módulos nuevos → en srv-public se actualiza el núcleo con `install.sh` (lo hace el usuario).

- [ ] **Step 1: Tests que fallan:** `unidad_temprana`; `desinstalar_avisa_modulo` (módulo falso recibe `desinstalando`); `equipos_junta_y_deduplica`; WireGuard `equipos_lista_clientes`.
- [ ] **Step 2–4:** FAIL → implementar → PASS. **Step 5:** Commit `core 0.2.0: arranque temprano, aviso al desinstalar y equipos`.

### Task 5: Kit — piezas comunes del §3b

**Files:** Create `web/kit/src/{SelectorEquipo.svelte,CampoNumero.svelte,Ayuda.svelte,ConfigGenerada.svelte,Registro.svelte,Asistente.svelte,Matriz.svelte}`; Modify `Campo.svelte` (prop `ayudaExtra` → «?»), `index.js`; migrar `Opcion.svelte` de WireGuard al kit.

- `SelectorEquipo` (spec §7.6): desplegable con buscador desde `/api/equipos`, cada equipo con icono, nombre, IP, MAC en pequeño y estado (● / ○ / ⊘), **agrupado por red** (LAN, VLAN, VPN) con los conectados primero; busca por nombre, IP, MAC o fabricante. Texto que es una IP válida no listada → opción «Usar 192.168.1.50 (escrita a mano)», validada al momento (formato, red conocida, no es la IP de la propia máquina). Prop `redes` para admitir CIDR, rango y redes enteras de la lista; prop `varios` → fichas con ×. Ficha elegida: icono + nombre + IP (lápiz si es a mano). Si una IP a mano coincide con un equipo que aparece, ofrece «Vincular a <equipo>». Teclado (↑↓ Intro Esc).
- `CampoNumero`: `type=text inputmode=numeric`, filtra no-dígitos, `min`/`max`, error «Entre 1 y 65535». Admite modo puertos (`80,443,8000-8100`).
- `Ayuda`: «?» con ventanita (ejemplo + «Recomendado: …»), accesible con teclado.
- `Asistente`: pasos numerados, Atrás/Siguiente, «Paso 2 de 3».
- `Matriz`: rejilla de zonas con casillas de color y contador; responsive.
- Sustituir en WireGuard los `tipo="number"` por `CampoNumero` y añadir «?» a sus campos (lista de comprobación §3b también para la fase 1).

- [ ] **Step 1:** `npm run build` en kit, shell y wireguard OK. **Step 2:** página de muestra en la demo del firewall con cada componente. **Step 3:** Commit `kit: selector de equipos, ayuda, campos numéricos, asistente y matriz`.

### Task 6: UI del módulo Firewall

**Files:** Create `modules/firewall/ui/{package.json,vite.config.js,svelte.config.js,src/{index.js,App.svelte,AsistenteInicial.svelte,Zonas.svelte,PanelCasilla.svelte,PanelRegla.svelte,Interfaces.svelte,SoloEntrada.svelte,Reenvios.svelte,AsistenteReenvio.svelte,Grupos.svelte,Registro.svelte}}`.

- Pantallas y textos exactamente como en «Pantallas y textos de ayuda». Tras un cambio peligroso, `Confirmacion60` del kit con `POST /confirmar/{id}`; si la página se recarga, `GET /pendiente` la vuelve a mostrar.
- Contadores y estado de equipos se refrescan cada 5 s.

- [ ] **Step 1:** `npm run build` → `dist/index.js`.
- [ ] **Step 2: Revisión visual** con la demo (`cargo run --example demo -p netctl-mod-firewall`) y el núcleo apuntando a ella, en Xvfb `:99` + navegador headless + `import`: asistente (3 pasos), matriz, panel de casilla, regla, una sola interfaz, lista de reenvíos, asistente de reenvío con aviso RDP, grupos, registro, configuración generada, barra de 60 s. Nunca en el escritorio del usuario. Corregir lo que no parezca UniFi.
- [ ] **Step 3:** Commit `firewall: interfaz web`.

### Task 7: Empaquetado, guía de prueba y revisión

**Files:** `modules/firewall/manifest.toml` (id `firewall`, icono `escudo`, `arranque_temprano = true`, menú Zonas / Reenvío de puertos / Grupos / Registro, paquetes `nftables` en las tres familias, conflictos `["firewalld","ufw"]` informativos), `tools/` (sin cambios salvo que haga falta), `docs/prueba-fase2.md`.

- `docs/prueba-fase2.md` (Debian, openSUSE, Rocky): actualizar núcleo con `install.sh`; instalar Firewall con firewalld/ufw activo; asistente; comprobar que el panel, SSH y la VPN siguen; poner Externa→Gateway en Permitir y **no** confirmar → a los 60 s vuelve; reenviar un puerto a un equipo de la LAN y probarlo desde el móvil por datos; cambiarle la IP al equipo y ver que el reenvío le sigue; caducidad de 1 h; desinstalar con «restaurar» y ver que firewalld vuelve.
- [ ] **Step 1:** `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, builds musl.
- [ ] **Step 2:** Revisión ligera de seguridad (un revisor haiku, informe ≤15 líneas) centrada en el Review Focus.
- [ ] **Step 3:** Fusionar `fase2` en `master` (con confirmación del usuario) y push.
- [ ] **Step 4 (con permiso del usuario):** clave nueva, `tools/publicar.sh nucleo`, `wireguard`, `firewall`.
