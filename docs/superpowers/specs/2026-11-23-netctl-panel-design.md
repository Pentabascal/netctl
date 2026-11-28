# netctl — panel web modular de red estilo UniFi

Fecha: 2026-11-23 · Repo: github.com/Pentabascal/netctl (público) · Estado: diseño

## 1. Intención

Un panel web en Rust, con el aspecto de la consola UniFi Network, para gestionar
la red de servidores Linux propios (Debian/Ubuntu, SUSE, Red Hat y derivados).
Uso privado: solo las máquinas del usuario, aunque el repo sea público.

Es un proyecto **totalmente separado de LinuxServerPanel** (no comparte código ni
aspecto con él).

El punto de partida es wg-configurador (GUI de escritorio egui ya hecha en este
mismo repo), que genera configuraciones WireGuard con permisos por cliente. El
panel lleva eso a la web y añade lo que la GUI no puede hacer: ver qué clientes
están conectados y cambiar cosas en caliente en el servidor real.

Lo que se instala depende de la máquina (un router con varias interfaces no
necesita lo mismo que un servidor de la LAN), por eso **el panel es modular y los
módulos se descargan e instalan desde las Releases del repo**.

## 2. Objetivos

- Núcleo mínimo: inicio de sesión, carcasa visual UniFi, gestor de módulos,
  capa de distro.
- Módulos descargables desde GitHub Releases de `netctl`, firmados, instalables,
  actualizables y desinstalables desde la página «Módulos» del panel.
- Módulo **WireGuard**: todo lo de wg-configurador + lista de clientes en vivo
  (conectado / no conectado) + edición en caliente.
- Módulo **Firewall** estilo UniFi (firewall por zonas con matriz zona→zona).
- Módulo **Router**: enrutamiento como tal (interfaces/VLAN, rutas, rutas por
  política, multi-WAN, OSPF/BGP/RIP, QoS). Sin servicios tipo DHCP/DNS.
- Módulo **DHCP** estilo UniFi OS: servidor DHCP por red (rango, puerta de
  enlace, DNS, concesión, IPs fijas, opciones), lista de clientes en vivo y
  relay. *Añadido por el usuario el 2026-11-27.*
- Módulo **Seguridad**: IDS/IPS y protección de amenazas.
- **VPN de configuración cero** basada en WireGuard, como Teleport de UniFi:
  el administrador manda una invitación (enlace o QR) y la app propia de
  netctl para **Android, Windows y Linux** se configura sola. Convive con el
  servidor WireGuard clásico. *Añadido por el usuario el 2026-11-27.*
- Vistas **Equipos** y **Topología** como las de UniFi: todos los equipos de
  la red (también los clientes VPN) y un mapa de cómo está distribuida.
  *Añadido por el usuario el 2026-11-27.*
- **Unirse a un dominio Active Directory o a un directorio OpenLDAP**: los
  usuarios del directorio entran al panel y usan la VPN con su cuenta de
  siempre. *Añadido por el usuario el 2026-11-27.*
- **Analizador de buenas prácticas (BPA)**, como el de Windows Server:
  revisa la configuración de todo el panel y de cada módulo y dice qué
  mejorar y cómo. *Añadido por el usuario el 2026-11-27.*
- **Varias WAN** con modo respaldo (failover) o balanceo de carga, y módulo
  **Alta disponibilidad** con «shadow mode» como UniFi: un servidor activo y
  otro en espera que toma el control si el activo cae. *Añadido por el
  usuario el 2026-11-27.*
- Funciona igual en Debian 12+, Ubuntu 22.04+, openSUSE Leap 15.6 / SLES 15,
  RHEL / Rocky / Alma 9+ y Fedora actual.

## 3. No objetivos

- Gestionar varios servidores desde un único panel (cada servidor tiene su
  propio panel). *Decisión de Claude: el usuario no lo ha pedido.* La
  excepción es la pareja de alta disponibilidad (§9f): son dos servidores
  que actúan como uno solo, y se configuran desde el panel del activo.
- Roles finos de permisos en el panel; distribución pulida para terceros;
  traducciones. (Con el directorio de §7.8 hay varios usuarios, pero todos
  los que entran son administradores; un rol «solo lectura» queda como
  posible mejora.)
- Integración con LinuxServerPanel.
- App de la VPN para iOS/macOS (requiere cuenta de desarrollador de Apple) y
  publicación en tiendas (Google Play, Microsoft Store): en la v1 las apps
  se descargan desde GitHub Releases. *Decisión de Claude: el usuario pidió
  Android, Windows y Linux.*
- Módulos de terceros (solo se aceptan módulos firmados con la clave del
  proyecto).
- Horarios en reglas de firewall y filtrado por aplicación/DPI.
- Servicios de red en el módulo Router: DNS, DNS dinámico, UPnP
  (decisión del usuario: «un router como tal»). El DHCP va en su propio
  módulo (§9e), no dentro del Router.

## 3b. Principios (lo más importante para el usuario)

El usuario pidió que **todo esté bien dividido para que nada se sobresature, que
sea bonito de ver y fácil de configurar**. Esto manda sobre cualquier otra
decisión de detalle:

- **Bien dividido en el sistema:** cada función es un módulo y un proceso aparte;
  un módulo pesado (Suricata) no frena a los demás. Cada unidad systemd de
  módulo lleva límites de recursos (`MemoryMax`, `CPUWeight`) y el núcleo
  muestra en Inicio el consumo de CPU y memoria de cada módulo.
- **Bien dividido en la interfaz:** ninguna pantalla intenta hacerlo todo. Cada
  módulo tiene pocas entradas en la barra lateral y cada página, pestañas
  claras. Lo más usado va primero; lo que se usa poco va al final de la
  página, en su propia sección con título (nunca escondido tras un modo o un
  «Avanzado»). Los detalles se editan en el panel lateral, no en páginas
  largas.
- **Fácil de configurar:** valores por defecto que funcionan sin tocar nada;
  asistentes para lo primero que hace cada módulo (primer servidor WireGuard,
  asignar interfaces a zonas, configurar WAN, activar IPS); validación en el
  formulario con mensajes en castellano llano; vista previa de lo que va a
  cambiar antes de aplicar.
- **Bonito:** estética UniFi Network cuidada (espaciado, iconos de línea,
  tarjetas con métricas, gráficas de tráfico, estados con color) y coherente
  en todos los módulos gracias al kit compartido.
- **Lo más fácil posible, estilo UniFi, también en lo complejo** (recordado
  por el usuario el 2026-11-27). Las funciones avanzadas añadidas al plan
  (varias WAN, shadow mode, directorio AD/LDAP, SNMP, VPN de configuración
  cero, OSPF/BGP, Suricata) no pueden convertir el panel en un formulario de
  expertos. Reglas que cada fase debe cumplir y comprobar en su revisión:
  - La tarea principal de cada página se hace en **3 pasos o menos** con los
    valores por defecto (p. ej. invitar a alguien a la VPN = «Invitar» →
    nombre → «Enviar»).
  - Cada función nueva entra con un **asistente** la primera vez y con
    valores por defecto que funcionan.
  - Se habla en **castellano llano** y se explica lo que pasará («Los equipos
    de la LAN no notarán el cambio»), no el nombre del protocolo.
  - Los avisos y errores dicen **cómo arreglarlo**, con botón directo si se
    puede (p. ej. «La hora está desfasada 8 min — Activar NTP»).
  - Si una función no aplica a la máquina (una sola tarjeta de red, sin
    controlador UniFi, sin directorio), **no se muestra** o se reduce, en
    vez de enseñar opciones vacías.
  - Antes de dar una fase por terminada se recorre la interfaz con capturas
    y se revisa con estas reglas.
- **Sin modo avanzado, pero completo y explicado** (decisión del usuario el
  2026-11-27, que sustituye a la idea de un modo avanzado). No hay modos ni
  secciones «Avanzado» escondidas: todo lo que hace falta configurar está en
  su página, ordenado de lo más usado a lo menos, y **cada cosa se explica**.
  - **Completo:** el plan de cada fase incluye una **lista de comprobación de
    opciones**: todo lo que el programa de debajo necesita o que un
    administrador espera poder cambiar (p. ej. en DHCP: rango, puerta de
    enlace, DNS, concesión, dominio, NTP, PXE, WINS, opciones por número,
    reservas, relay) tiene su campo en la interfaz. Nada imprescindible
    obliga a editar ficheros a mano.
  - **Cada campo explicado:** etiqueta en castellano llano + una línea debajo
    que dice **para qué sirve** y **qué pasa si se cambia**; si existe un
    término técnico, va entre paréntesis («Pasar el control al otro servidor
    (VRRP)»). Un icono «?» abre una explicación más larga con un **ejemplo**
    y el **valor recomendado**.
  - **Cada página y cada sección** empieza con una frase que explica qué se
    hace ahí.
  - **Valores por defecto marcados** como «(recomendado)», para que se pueda
    dejar todo como está sin miedo.
  - Las opciones que dependen de otra (p. ej. los campos de PPPoE solo si la
    WAN es PPPoE) aparecen solo cuando aplican, con su explicación.
  - Cada módulo enseña en su página de ajustes, en solo lectura, la
    **configuración que genera** y un **registro** de lo que ha aplicado, para
    entender qué hace por debajo.

## 4. Anti-objetivos (fallo aunque «funcione»)

- Pantallas saturadas de opciones o un módulo que haga demasiadas cosas.

- **Quedarse fuera del servidor** por aplicar una regla de firewall mala.
- **Desconectar a todos los clientes VPN** por editar uno.
- Que una descarga manipulada ejecute código como root (sin verificar firma).
- Que no parezca UniFi (formularios grises genéricos).

## 5. Restricciones

- Rust en el backend. Repo público: nada de secretos en el repo; commits con el
  email noreply de GitHub.
- Los módulos tocan nftables, wg y systemd → necesitan root.
- Un único binario por módulo que corra en las tres familias de distro.
- Solo x86_64 en la v1 (aarch64 queda abierto).

## 6. Enfoques considerados

| | Enfoque | Pros | Contras |
|-|-|-|-|
| **A (elegido)** | Cada módulo es un programa aparte, descargado como `.tar.gz` firmado desde Releases; el núcleo lo arranca como servicio systemd y le hace de proxy por socket Unix | Instalar/actualizar/quitar sin tocar el núcleo; un módulo que falla no tumba el panel; Releases sirven tal cual | Hay que diseñar el contrato núcleo↔módulo y la carga de la UI de cada módulo |
| B | Módulos como paquetes .deb/.rpm desde repos apt/dnf/zypper propios | Firmas y dependencias las da la distro | Tres tipos de repositorio que mantener; Releases no valen |
| C | Un solo binario con todo dentro; «instalar» = activar | Lo más simple | No hay descarga real de módulos; contradice el requisito |

Descartado: plugins WebAssembly (necesitan root para nft/wg; complejidad sin
ganancia). El usuario eligió **A**.

## 7. Arquitectura

### 7.1 Estructura del repo (workspace Cargo)

```
netctl/
  crates/
    netctl-core/       binario del panel (núcleo)
    netctl-sdk/        librería común: contrato con el núcleo, capa de distro,
                       aplicar-con-confirmación, tipos de manifiesto
    wg-modelo/         modelo + claves extraídos de wg-configurador (modelo.rs,
                       claves.rs), compartido por la GUI y el módulo
  modules/
    wireguard/         binario netctl-mod-wireguard + ui/
    firewall/          binario netctl-mod-firewall + ui/
  web/
    shell/             carcasa web del núcleo
    kit/               kit visual UniFi compartido (CSS + componentes)
  apps/
    wg-configurador/   la GUI egui actual, ahora usando wg-modelo
  tools/               scripts de empaquetado, firma y publicación
```

*Decisión de Claude:* la GUI de escritorio se conserva en `apps/` y pasa a usar
`wg-modelo`, para no mantener dos copias de la lógica.

### 7.2 Núcleo (`netctl-core`)

- axum, HTTPS en el puerto 8443. Certificado autofirmado generado en el primer
  arranque; se puede sustituir por uno propio.
- **Alcance de acceso configurable** (decisión del usuario), en Ajustes del panel
  y en `/etc/netctl/core.toml`:
  - **Solo LAN/VPN** (por defecto): escucha solo en las IPs de interfaces
    privadas (RFC1918, ULA) y en las interfaces WireGuard, y rechaza
    conexiones con origen público.
  - **LAN e internet**: escucha en todas las interfaces. Para activarlo hay
    que tener configurado el **doble factor TOTP**; desde ese momento el TOTP
    es obligatorio en cada inicio de sesión. El bloqueo por intentos se
    endurece (por IP, con espera creciente).
  - El cambio de alcance usa el mismo «aplicar con confirmación» de 60 s, para
    no perder el acceso.
- Corre como **root** bajo systemd (`netctl.service`) con el endurecimiento que
  permita systemd. *Decisión de Claude:* necesita instalar módulos y crear
  unidades systemd; separar en proceso web sin privilegios + ayudante root
  (como LinuxServerPanel) duplica trabajo para un uso privado.
- **Acceso:** (desde fase 2d también usuarios de AD/OpenLDAP, §7.8) un
  único usuario administrador local creado al instalar
  (`netctl-core admin set-password`), contraseña con argon2id, sesión por
  cookie `HttpOnly; Secure; SameSite=Strict`, CSRF por cabecera, bloqueo
  temporal tras 5 fallos.
- Estado en `/var/lib/netctl/` (permisos 700).
- Distribución del núcleo: `.deb` y `.rpm` en Releases (cargo-deb /
  cargo-generate-rpm) y un `install.sh` que detecta la distro y descarga el
  correcto.

### 7.3 Contrato núcleo ↔ módulo

- Cada módulo es un servicio systemd `netctl-mod-<id>.service` (root) que
  escucha HTTP en el socket Unix `/run/netctl/mod-<id>.sock` (solo root).
- El núcleo reenvía `/api/m/<id>/*` al socket del módulo tras validar la
  sesión, añadiendo cabeceras con el usuario. El módulo no ve el exterior.
- Endpoints obligatorios de todo módulo: `GET /salud`, `GET /resumen`
  (tarjetas para el panel de inicio del núcleo).
- Endpoint opcional `GET /equipos`: los equipos que conoce el módulo (ver
  §7.7). Lo ofrecen WireGuard, DHCP, Router y Alta disponibilidad.
- Endpoints del BPA (§7.9), obligatorios para todo módulo a partir de la
  fase 2e:
  `GET /bpa` (resultado de sus comprobaciones) y `POST /bpa/arreglar/<id>`
  (aplica el arreglo automático de una comprobación, si lo tiene).
- Los módulos se descubren entre sí a través del núcleo
  (`GET /api/modulos` interno) — p. ej. el firewall pregunta si existe el
  módulo WireGuard para crear la zona VPN.
- `netctl-sdk` implementa el servidor del socket, la lectura de cabeceras, la
  capa de distro y el «aplicar con confirmación», para que un módulo nuevo
  sea casi solo su lógica.

### 7.7 Equipos y Topología (núcleo, fase 2c)

Dos páginas del **núcleo** (no de un módulo), porque juntan lo que sabe cada
módulo. Funcionan desde el primer día con lo mínimo y van ganando detalle a
medida que se instalan módulos.

**De dónde salen los equipos** (el núcleo los une por MAC, o por IP si no
hay MAC):

| Fuente | Qué aporta | Disponible |
|---|---|---|
| Tabla de vecinos (`ip neigh`) | IP + MAC de los equipos de cada red local | Siempre |
| Base OUI (en `netctl-sdk`) | Fabricante a partir de la MAC | Siempre |
| mDNS / NetBIOS / DNS inverso | Nombre del equipo | Siempre |
| Escaneo ARP activo | Equipos que no han hablado últimamente | Opcional, desactivado por defecto |
| LLDP y CDP (`lldpd`) | Switches y APs conectados directamente al servidor, y en qué puerto | Siempre (si `lldpd` está instalado; el núcleo lo ofrece) |
| SNMP (v2c / v3) contra switches gestionados | Cadena switch→switch (LLDP-MIB) y en qué puerto está cada equipo (tabla de MACs, BRIDGE-MIB / Q-BRIDGE-MIB) | Opcional: el usuario añade cada switch con sus credenciales |
| API de UniFi Network | APs y switches UniFi, qué equipos van por Wi-Fi, a qué AP, banda, señal y SSID | Opcional: solo si hay un controlador UniFi en la red |
| Colocación manual | Switches no gestionados y APs sin API que el usuario dibuja | Siempre |
| Módulo DHCP | Nombre del equipo, concesión, IP fija | Con DHCP |
| Módulo WireGuard / netctl VPN | Clientes VPN: usuario, dispositivo, IP pública de origen, último saludo, tráfico | Con WireGuard |
| Router / Firewall | Tráfico en vivo por equipo (contadores de nftables) | Si esta máquina enruta |
| Alta disponibilidad | El otro servidor de la pareja | Con HA |

**Página «Equipos»** (como «Client Devices» de UniFi):
- Tarjetas arriba: **Equipos** (una sola tarjeta que une conectados y
  nuevos: «24 conectados de 31 · 3 nuevos esta semana»), por red, clientes
  VPN, desactivados.
- **Una sola lista con todos los equipos conocidos**, estén conectados o no.
  Los nuevos no van aparte: llevan una insignia «Nuevo» durante sus primeros
  7 días y se pueden filtrar.
- Columnas: icono del tipo de equipo, nombre, IP, MAC, fabricante, red/zona,
  **conexión** (cable/Wi-Fi de la LAN, o **VPN** con insignia propia),
  **fecha de unión** (primera vez que se vio en la red; para un dispositivo
  netctl VPN, el día que aceptó la invitación), **última vez visto**, tráfico
  ↓/↑ y **estado**:
  - ● **Conectado** (verde).
  - ○ **Desconectado** (gris): conocido pero sin actividad ahora.
  - ⊘ **Desactivado** (rojo apagado): bloqueado en el Firewall o dispositivo
    VPN revocado/pausado. Se sigue viendo en la lista, con el motivo, y desde
    el panel lateral se puede volver a activar.
- La lista se ordena por cualquier columna (por defecto: conectados primero,
  luego por fecha de unión, más recientes arriba).
- Filtros rápidos: Todos · Conectados · Desconectados · Desactivados ·
  Nuevos · LAN · VPN; búsqueda; por red. Por defecto «Todos», así que los
  desconectados y desactivados se ven siempre.
- Los equipos conocidos se guardan **hasta que el usuario los olvida**
  (botón «Olvidar», también en bloque). Opción para olvidar solos los
  desconectados tras N días, **apagada por defecto**.
- **Panel lateral** al pulsar un equipo, como el de cliente de wg-configurador:
  nombre e icono editables, notas, IP/MAC/fabricante, red, historial de
  conexión y gráfica de tráfico. Acciones según los módulos instalados:
  «Fijar IP» (DHCP), «Bloquear» (Firewall: lo añade a un grupo bloqueado),
  «Revocar» (dispositivo VPN), «Ver en la topología».
- **Clientes VPN** en la misma tabla: se ven con su usuario, dispositivo
  (Android/Windows/Linux para netctl VPN), IP dentro de la VPN, IP pública
  desde la que conectan y país.
- Aviso opcional cuando aparece un equipo nuevo en la red.
- El tipo de equipo (móvil, portátil, impresora, TV, cámara, switch…) se
  adivina por fabricante y nombre; el usuario lo puede corregir y se
  recuerda.

**Página «Topología»** (como el mapa de UniFi):
- Diagrama en árbol, de arriba abajo: **Internet** → cada **WAN** (con su
  estado de §9c) → **este servidor** (o la pareja activo/espera de §9f) →
  cada **red** (LAN, VLAN y una rama **VPN**) → sus **equipos**.
- **Switches y puntos de acceso**, en cuatro niveles que se suman:
  1. **LLDP / CDP:** el switch o AP conectado directamente al servidor y el
     puerto.
  2. **SNMP:** página «Dispositivos de red» donde se añaden los switches
     gestionados (IP + comunidad v2c o usuario/clave v3, solo lectura). Con
     eso se dibuja la cadena entera de switches y cada equipo cuelga de su
     **puerto real** (p. ej. «Switch planta 1 · puerto 12»). Sondeo cada
     5 min; credenciales guardadas cifradas (600).
  3. **UniFi Network:** si en la red hay un controlador UniFi (UDM, Cloud
     Key o software autoalojado), se conecta con un usuario local de solo
     lectura o una clave de API. Se leen sus APs y switches con sus enlaces, y de cada
     cliente Wi-Fi: AP, banda, SSID y señal. En la tabla «Equipos» la
     columna conexión pasa a decir «Wi-Fi · AP salón · 5 GHz · -58 dBm».
     Omada y OpenWrt quedan como integraciones posibles a futuro,
     con el mismo diseño de «conector».
  4. **Manual:** en modo edición del mapa se añade un nodo «switch no
     gestionado» o «AP» y se arrastran equipos debajo. Se recuerda, y si un
     equipo colocado a mano aparece por SNMP/UniFi en otro sitio, el panel
     avisa en vez de pisar lo manual.
- Lo que no se pueda situar con ningún método cuelga directamente de su red,
  con un aviso discreto «ubicación desconocida».
- **Límites que el panel explica:** los switches no gestionados no son
  detectables por ningún protocolo; distinguir cable de Wi-Fi solo es
  posible con la integración del AP/controlador.
- Con una sola tarjeta de red el mapa se simplifica: Internet → router de la
  LAN → este servidor y los demás equipos al mismo nivel.
- Líneas de color según estado (verde activa, gris caída/desconectada,
  discontinua para VPN) y grosor según tráfico.
- Zoom, arrastrar, plegar una red con muchos equipos (se muestra «+38
  equipos»), y al pulsar un nodo se abre el mismo panel lateral que en
  «Equipos».
- Dibujado en el navegador con SVG, con el aspecto oscuro del kit; se
  actualiza en vivo.

**Privacidad y rendimiento:** el escaneo activo está apagado por defecto;
los equipos conocidos no se borran solos (ver «Olvidar» arriba); el historial
de conexiones de cada equipo se guarda 90 días (configurable);
sondeo de estado cada 10 s.

**Tests:** fecha de unión que no cambia aunque el equipo cambie de IP o
vuelva tras desconectarse; estados conectado/desconectado/desactivado e
insignia «Nuevo»; unión de equipos de varias fuentes (misma MAC con distinta IP,
cliente VPN sin MAC), búsqueda de fabricante por OUI, adivinar el tipo de
equipo, construcción del árbol de topología (con y sin LLDP, con una y
varias interfaces, con pareja HA), lectura de tablas SNMP de ejemplo
(LLDP-MIB, BRIDGE-MIB, Q-BRIDGE-MIB) y de respuestas de ejemplo de la API de
UniFi, y prioridad entre fuentes (manual > UniFi > SNMP > LLDP > red).

### 7.8 Directorio: Active Directory y OpenLDAP (núcleo, fase 2d)

Página **Ajustes › Identidad** con tres opciones: **Solo cuentas locales**
(por defecto, como en fase 1), **Active Directory** u **OpenLDAP**.

**Active Directory — unión real al dominio:**
- Se pide el dominio (`empresa.local`), un usuario con permiso para unir
  equipos (solo se usa en ese momento, no se guarda) y, opcionalmente, la UO
  donde crear la cuenta de equipo.
- El panel comprueba antes: DNS que resuelve los controladores de dominio
  (registros SRV), hora sincronizada (Kerberos falla con más de 5 min de
  diferencia; ofrece activar NTP) y puertos alcanzables.
- Une la máquina con `adcli` + `sssd` + Kerberos (paquetes en las tres
  familias; *decisión de Claude: no se usa `realmd` porque en openSUSE/SLES
  no es la vía habitual*). La cuenta de equipo queda creada en AD y netctl
  consulta el directorio con ella (Kerberos), sin guardar contraseñas de
  servicio.
- **No abre el sistema a los usuarios del dominio:** por defecto `sssd`
  queda configurado para que **solo netctl** los use; iniciar sesión por SSH
  o consola con cuentas del dominio es una casilla aparte, apagada, con
  aviso, y limitada a los grupos que se elijan.
- Botón **Salir del dominio** (borra la cuenta de equipo si se dan
  credenciales, o la deja y solo limpia la máquina).

**OpenLDAP (y otros LDAP: 389 DS, FreeIPA en modo LDAP):**
- Servidor(es) `ldaps://` o `ldap://` con StartTLS (**sin cifrar no se
  permite**), certificado de la CA que se sube o se fija por huella.
- Base DN, cuenta de servicio de solo lectura (DN + contraseña, guardada
  cifrada, 600), filtro de usuarios y de grupos con valores típicos ya
  rellenos (`posixAccount`/`inetOrgPerson`, `groupOfNames`/`posixGroup`,
  `memberOf` si está).

**Común a los dos:**
- **Probar:** botón que comprueba la conexión, busca un usuario de prueba y
  enseña sus grupos, antes de guardar nada.
- **Quién entra al panel:** se elige un grupo del directorio (p. ej.
  «netctl-admins»); sus miembros entran como administradores. El resto no.
- **Cuenta local de emergencia:** el administrador local de fase 1 sigue
  existiendo siempre, para entrar si el directorio no responde o se rompe la
  unión. Se avisa si no tiene TOTP.
- **Doble factor:** cuando el panel está abierto a internet, el TOTP sigue
  siendo obligatorio también para los usuarios del directorio; cada uno lo
  configura en su primer inicio de sesión.
- **Usuarios desactivados o borrados** en el directorio no pueden entrar;
  pertenencia a grupos en caché 5 min, con sincronización cada 15 min.
- Los inicios de sesión quedan registrados con el usuario del directorio.

**Uso en los módulos:**
- **netctl VPN (§8b):**
  - Los usuarios de la VPN pueden ser usuarios del directorio. Además de las
    invitaciones, la app permite **«Iniciar sesión con usuario del
    dominio»**: dirección del servidor + usuario y contraseña → se inscribe
    el dispositivo sin invitación.
  - **Grupos → permisos:** se asigna a cada grupo un perfil de acceso (p. ej.
    «VPN-Intranet» → toda la intranet; «VPN-Contabilidad» → subred de
    contabilidad + internet). Si un usuario está en varios, se suman.
  - Si un usuario se desactiva en el directorio o sale del grupo, sus
    dispositivos pasan solos a **Desactivado** (se ven así en «Equipos»,
    §7.7, con el motivo «usuario desactivado en el dominio») y su túnel se
    corta en la siguiente sincronización.
- **Servidor WireGuard clásico:** funciona por claves, sin usuario; no
  cambia. Se puede anotar a qué usuario del directorio pertenece cada
  cliente, solo como dato.
- **Firewall (§9):** grupos de origen dinámicos «usuarios VPN del grupo X»
  (las IPs de sus dispositivos conectados), para reglas como «solo el grupo
  Informática puede llegar a la red de servidores».
- **Equipos (§7.7):** los clientes VPN muestran el nombre completo del
  usuario del directorio.

**Tests:** generación de la config de `sssd`/Kerberos, filtros LDAP con
caracteres especiales (escape correcto, sin inyección), resolución de grupos
anidados de AD, suma de perfiles de varios grupos, desactivar dispositivos
al salir del grupo, y acceso con la cuenta local cuando el directorio no
responde. La prueba real contra un AD y un OpenLDAP la hace el usuario con
una guía paso a paso.

### 7.9 Analizador de buenas prácticas, BPA (núcleo, fase 2e)

Página **«Buenas prácticas»** en la barra lateral del núcleo, como el Best
Practices Analyzer de Windows Server pero con el aspecto y la sencillez del
resto del panel.

**Cómo funciona:**
- El núcleo tiene sus propias comprobaciones y pide las suyas a cada módulo
  instalado (`GET /bpa`). Cada módulo es responsable de las comprobaciones de
  lo que configura.
- Se ejecuta **al pulsar «Analizar ahora»**, **cada día** solo (hora
  configurable) y **después de cada cambio aplicado** (solo las del módulo
  que cambió).
- Tarjeta en **Inicio** con el resumen («2 errores · 5 avisos · 31
  correctas») y una insignia en la barra lateral si hay errores.

**Cada resultado muestra** (siguiendo el principio «completo y explicado»):
- **Gravedad:** ✖ Error (riesgo real, p. ej. seguridad), ⚠ Aviso (conviene
  cambiarlo), ℹ Información (sugerencia), ✔ Correcto.
- **Qué pasa**, en castellano llano: «El panel sigue con la contraseña de
  fábrica».
- **Por qué importa:** «Cualquiera de la red que la conozca puede entrar».
- **Cómo arreglarlo**, paso a paso, con enlace a la página exacta.
- Botón **«Arreglar»** cuando se puede hacer solo: enseña antes lo que va a
  cambiar y lo aplica con las mismas validaciones (y con la confirmación de
  60 s si toca red o firewall).
- **«Ignorar»** con un motivo opcional; lo ignorado no desaparece, pasa a la
  pestaña «Ignorados» y se puede volver a activar.

**Vista:** filtros por gravedad y por módulo, búsqueda, lista de
«Correctas» también visible (para ver qué se comprobó), **historial** de
análisis con la evolución, y **exportar el informe** (HTML imprimible / PDF).

**Comprobaciones previstas** (cada fase añade las de su módulo y no se da
por terminada sin ellas):

| Módulo | Ejemplos |
|---|---|
| Núcleo / sistema | Contraseña de fábrica sin cambiar; panel abierto a internet; cuenta local de emergencia sin TOTP; certificado autofirmado o a punto de caducar; hora sin sincronizar (NTP); actualizaciones del sistema o del núcleo pendientes; poco espacio en disco; versiones de módulos desfasadas |
| WireGuard / netctl VPN | Clientes sin clave precompartida; clientes sin conectar en más de 90 días; clientes con internet + toda la intranet (permisos más amplios de lo necesario); invitaciones sin usar caducadas acumuladas; varias WAN pero el servidor clásico con un Endpoint fijo; dispositivos de usuarios desactivados en el directorio |
| Firewall | Regla que lo permite todo; reglas que nunca se usan (0 aciertos en 30 días); SSH o el panel abiertos desde Externa; reenvíos de puertos a equipos que ya no existen; reglas duplicadas o tapadas por otra anterior |
| Router | Una sola WAN sin respaldo; WAN sin comprobación de salud; MTU distinto entre extremos; rutas estáticas hacia una puerta de enlace que no responde |
| DHCP | Rango casi agotado (>90 %); otro servidor DHCP detectado en la red; IPs fijas dentro del rango; puerta de enlace o DNS que no responden |
| Seguridad | Firmas sin actualizar en más de 7 días; IPS solo en modo detección; CPU al límite con el tráfico actual |
| Alta disponibilidad | Última sincronización antigua; versiones distintas en los dos servidores; enlace de sincronización sin redundancia |
| Directorio | Hora desfasada con el dominio; cuenta de servicio con más permisos de los necesarios; certificado del servidor LDAP a punto de caducar |
| Equipos | Equipos nuevos sin identificar; nombres repetidos |

**Tests:** cada comprobación con estados de ejemplo que la hacen pasar y
fallar; que «Arreglar» deja la comprobación en verde; que «Ignorar» se
recuerda; y que un módulo que no responde sale como aviso, sin romper el
análisis de los demás.

### 7.4 Paquete de módulo y catálogo

- Release por módulo con etiqueta `<id>-v<versión>` (p. ej. `wireguard-v0.1.0`).
  Asset `netctl-mod-<id>-<versión>-x86_64.tar.gz` + `.minisig`.
- Contenido del tar:
  - `manifest.toml`: id, nombre, versión, versión mínima del núcleo, icono,
    entradas de la barra lateral, paquetes del sistema necesarios por familia
    (`debian`/`suse`/`rhel`), servicios que conflictúan.
  - `bin/netctl-mod-<id>`: binario estático (musl) → corre en las tres
    familias.
  - `ui/`: bundle web ya compilado del módulo.
- **Catálogo:** `index.json` en una Release fija con etiqueta `catalogo`
  (lista de módulos, versiones, URL, sha256). Lo regenera y sube
  `tools/publicar.sh`.
- **Firma:** minisign (Ed25519). La clave pública va compilada en el núcleo;
  la privada solo en la máquina del usuario (nunca en el repo). Se verifica la
  firma del tar **y** del `index.json` antes de usarlos.
- **Instalar:** descargar → verificar firma y sha256 → instalar paquetes del
  sistema que pida el manifiesto (apt/zypper/dnf) → extraer a
  `/usr/lib/netctl/modules/<id>/` → crear y arrancar la unidad systemd.
- **Actualizar:** igual, conservando la versión anterior para volver atrás si
  el nuevo no responde a `/salud` en 15 s.
- **Desinstalar:** parar, quitar la unidad y los ficheros; el estado en
  `/var/lib/netctl/<id>/` se conserva salvo que se marque «borrar datos».
- Los repos sin red/GitHub caído: el panel sigue funcionando; solo falla el
  catálogo, con aviso.

### 7.5 Capa de distro (`netctl-sdk`)

- Detección por `/etc/os-release` (`ID`, `ID_LIKE`) → familia `debian`,
  `suse` o `rhel`.
- Instalar paquetes: `apt-get install -y` / `zypper --non-interactive install`
  / `dnf install -y`. Mapeo de nombres de paquete por familia en el manifiesto.
- Servicios: systemd en todas (mismo código).
- Firewalls ajenos: detectar y desactivar `ufw` / `firewalld` cuando el módulo
  Firewall lo pida, guardando antes el ruleset actual.

### 7.6 Web y aspecto

- Carcasa y kit en **Svelte + Vite**, compilados e incrustados en el binario del
  núcleo. *Decisión de Claude.*
- Aspecto UniFi Network, **tema oscuro** como el de wg-configurador
  (`src/ui_kit.rs` sirve de referencia de colores y componentes): barra lateral
  con iconos, cabecera, tarjetas, tablas con filtros, interruptores, panel
  lateral deslizante al abrir un elemento, chips de estado verde/gris.
- El kit se publica como CSS con variables + componentes Svelte; cada módulo
  compila su `ui/` contra el mismo kit.
- Carga de la UI de un módulo: el núcleo sirve `/m/<id>/ui/*`; el bundle exporta
  `mount(elemento, api)` y la carcasa lo monta en el área principal al pulsar
  su entrada de la barra lateral. `api` da `fetch` ya autenticado contra
  `/api/m/<id>/`.
- Página **Inicio**: tarjetas `/resumen` de cada módulo. Página **Módulos**:
  catálogo, instalados, botones instalar/actualizar/quitar.
- **Selector de equipos** (componente del kit, pedido por el usuario el
  2026-11-27). **En todo el panel**, cualquier campo donde se elige un
  equipo o una IP usa este mismo selector, como los de UniFi: reenvío de
  puertos, reglas del Firewall (origen/destino), IPs fijas del DHCP, rutas
  por política, límites de QoS, grupos de IPs, acciones del BPA…
  - **Lista desplegable con buscador**: cada equipo con su icono, nombre,
    IP, MAC (en pequeño) y estado (● conectado / ○ desconectado / ⊘
    desactivado). Agrupada por red (LAN, VLAN, VPN) y con los conectados
    primero. Se busca por nombre, IP, MAC o fabricante.
  - **IP a mano en el mismo campo:** si lo escrito es una IP válida que no
    está en la lista, aparece la opción «Usar 192.168.1.50 (escrita a
    mano)». Se valida al momento (formato, que esté en una red conocida, que
    no sea la de la propia máquina) con el aviso en castellano llano.
  - Donde el campo lo admite (Firewall, grupos de IPs, QoS) también se
    puede elegir **una red entera** de la lista o escribir una **red CIDR o
    un rango** (`192.168.1.0/24`, `192.168.1.10-192.168.1.20`), y elegir
    **varios** a la vez (se muestran como fichas que se quitan con ×).
  - Lo elegido se muestra como una ficha con icono + nombre + IP; si es una
    IP a mano, con un icono de lápiz.
  - **Vinculado al equipo, no solo a la IP:** si se eligió un equipo de la
    lista y cambia de IP, las reglas que lo usan se actualizan solas y se
    avisa. Si se escribió una IP a mano y luego aparece un equipo con esa
    IP, se ofrece vincularlo.
  - Si un equipo elegido se olvida en «Equipos», las reglas que lo usan se
    muestran antes de olvidarlo y se pide qué hacer con ellas.

```
┌────┬──────────────────────────────────────────────────────┐
│ ⌂  │ WireGuard › Clientes                    [+ Cliente]  │
│ ⇄  │ ┌──────────┐ ┌──────────┐ ┌──────────┐               │
│ ▦  │ │ 4 conect.│ │ 9 total  │ │ 2.1 GB ↓ │               │
│ ⚙  │ └──────────┘ └──────────┘ └──────────┘               │
│    │ Nombre    IP         Estado     Último saludo  Tráfico│
│    │ portatil  10.8.0.2   ● Conect.  hace 12 s     1.2 GB │
│    │ movil     10.8.0.3   ○ Descon.  hace 3 h      40 MB  │
└────┴──────────────────────────────────────────────────────┘
```

## 8. Módulo WireGuard (fase 1)

- Reutiliza `wg-modelo`: servidor, subredes de intranet, clientes con permisos
  (internet, toda la intranet, subredes concretas, CIDR extra), PSK, DNS,
  keepalive, MTU.
- Estado: `/var/lib/netctl/wireguard/proyecto.json` (600). **Importar** un
  proyecto JSON de wg-configurador desde la UI.
- Escribe `/etc/wireguard/<iface>.conf` y habilita `wg-quick@<iface>`.
- **Cambios en caliente:** `wg syncconf <iface> <(wg-quick strip <iface>)` →
  añadir/editar/quitar un peer no corta a los demás. Solo cambios del propio
  servidor (puerto, red, clave) requieren reiniciar la interfaz, con aviso.
- **Permisos por cliente** en nftables (tabla propia `inet netctl_wg`, cadena
  forward), sustituyendo las reglas iptables de PostUp de wg-configurador. Se
  regeneran y cargan atómicamente (`nft -f`) en cada cambio. NAT/masquerade
  también aquí si el módulo Firewall no está instalado.
- **Estado en vivo:** `wg show <iface> dump` cada 5 s. Conectado = último
  handshake hace < 180 s. Mostrar: estado, último saludo, endpoint real,
  bytes recibidos/enviados. La UI refresca por SSE.
- **Acciones por cliente:** crear, editar permisos, activar/desactivar (quita el
  peer pero conserva datos), regenerar claves, descargar `.conf`, ver QR,
  borrar.
- Ajustes del servidor en una pestaña aparte.

## 8b. VPN de configuración cero «netctl VPN» (fase 2b)

El módulo WireGuard pasa a tener **dos tipos de servidor**, que pueden
funcionar a la vez en interfaces y puertos distintos:

| | **Servidor WireGuard** (el de fase 1) | **netctl VPN** (configuración cero) |
|---|---|---|
| Cliente | Cualquier app oficial de WireGuard | App propia de netctl |
| Alta de un usuario | Se crea el cliente y se le pasa su `.conf` o QR | Se le manda una invitación; la app hace el resto |
| Claves | Las genera el servidor | La privada se genera en el dispositivo y nunca sale de él |
| Varias WAN / HA | Un solo Endpoint fijo | La app recibe todas las direcciones y prueba la que funcione |

Los permisos por cliente (internet, toda la intranet, subredes, CIDR extra)
son los mismos en los dos tipos; el motor por debajo es WireGuard igual.

**Nombre provisional:** «netctl VPN» (se puede cambiar).

### Servidor (dentro del módulo WireGuard)
- **Usuarios y dispositivos:** cada persona es un usuario con sus permisos;
  cada usuario puede tener varios dispositivos (móvil, portátil…), cada uno
  con su propia clave e IP. Se ve nombre, sistema, última conexión, tráfico y
  estado en vivo, y se puede **revocar** un dispositivo o un usuario entero.
- **Invitaciones:** botón «Invitar» → enlace `netctl://…` + QR + código corto
  para escribir a mano. Cada invitación es de **un solo uso**, caduca (24 h
  por defecto, configurable) y opcionalmente queda **pendiente de aprobación**
  del administrador antes de activarse. Se puede enviar por correo, copiar o
  enseñar en pantalla.
- **Inscripción:** la app genera su par de claves, manda la pública, el
  nombre del dispositivo y el código de la invitación; el servidor crea el
  dispositivo y devuelve su configuración. La invitación lleva la huella del
  certificado del servidor, así que la app no se fía de nadie más (no hace
  falta un certificado público ni dominio).
- **Servicio de inscripción aparte del panel:** escucha en su propio puerto
  TCP y solo ofrece tres cosas: inscribirse con una invitación, pedir la
  configuración actualizada (direcciones de las WAN, permisos, DNS) y
  comprobar que el dispositivo sigue autorizado. El panel de administración
  **no** tiene que estar abierto a internet. Límite de intentos por IP y
  registro de inscripciones.
- **Direcciones del servidor:** el módulo junta todas las formas de llegar
  (IPs de cada WAN, dominio si hay, IP virtual de la pareja HA) y la app las
  recibe cada vez que conecta. Así funcionan solos el cambio de WAN (§9c) y
  el shadow mode (§9f), sin DNS dinámico.

### Sin abrir puertos: relé opcional
Para cuando el servidor está detrás de un router que no se puede tocar
(como Teleport, que funciona sin redirigir puertos):
- **Relé** (`netctl-rele`): programa pequeño que el usuario instala en una
  máquina con IP pública (un VPS o un servidor propio). El servidor netctl
  mantiene una conexión saliente con el relé; las apps que no llegan directo
  se conectan al relé y este reenvía el tráfico.
- El relé **no puede leer nada**: solo pasa paquetes de WireGuard ya
  cifrados de extremo a extremo.
- Salida por UDP y, en redes que lo bloquean (hoteles, empresas), por
  TCP/443 como si fuera web.
- La app prueba primero conexión directa y usa el relé solo si hace falta.
- **Más adelante** (no en la v1): conexión directa atravesando NAT
  (*hole punching*) con el relé como mediador, para no gastar su ancho de
  banda.

### Apps (Android, Windows, Linux)
- **Núcleo compartido en Rust** (`crates/netctl-vpn-cliente`): protocolo de
  inscripción, elección de dirección (directa / relé), renovación de config,
  almacenamiento seguro de la clave. Lo usan las tres apps; en Android a
  través de `uniffi`.
- **Aspecto:** el mismo estilo oscuro UniFi del panel. Pantalla principal
  con un botón grande «Conectar», estado, IP, tiempo conectado y tráfico;
  lista de servidores (se pueden tener varios sitios); «Unirse» con enlace,
  QR o código.
- **Opciones:** conectar al arrancar, conectar solo en redes Wi-Fi no de
  confianza (Android), bloquear el tráfico si se cae la VPN (*kill switch*,
  Windows y Linux), notificaciones de conexión.
- **Android:** Kotlin + Jetpack Compose, túnel con la librería oficial
  `com.wireguard.android:tunnel` (VpnService). Lector de QR integrado. APK
  firmado con clave propia, publicado en GitHub Releases.
- **Windows:** app Rust (egui, como wg-configurador) + servicio de Windows
  que gestiona el túnel con `wireguard-nt` (driver oficial de WireGuard).
  Icono en la bandeja del sistema. Instalador `.exe` con NSIS. Sin
  certificado de firma de código, Windows SmartScreen mostrará un aviso la
  primera vez (se documenta cómo pasarlo).
- **Linux:** la misma app Rust (egui) + un pequeño servicio con permisos que
  crea la interfaz WireGuard del kernel (netlink). Icono en la bandeja.
  Paquetes `.deb`, `.rpm` y AppImage.
- **Actualizaciones:** la app avisa cuando hay versión nueva en GitHub
  Releases.

### Tests
Protocolo de inscripción (invitación caducada, reutilizada, pendiente de
aprobación, dispositivo revocado), elección de dirección y relé, generación
de la config del dispositivo con sus permisos, y el relé reenviando sin
poder leer. La prueba real en móvil, Windows y Linux la hace el usuario con
una guía paso a paso.

### Orden de construcción
1. Servidor: usuarios, dispositivos, invitaciones, servicio de inscripción.
2. App Linux (la más fácil de probar en este equipo).
3. App Windows.
4. App Android.
5. Relé.

## 9. Módulo Firewall (fase 2)

- Tabla propia `inet netctl` en nftables; carga atómica con `nft -f`; se
  reaplica al arrancar desde su servicio.
- **Zonas** como UniFi: Externa, Interna, Gateway (la propia máquina), VPN
  (aparece sola si está el módulo WireGuard; agrupa sus interfaces wg), DMZ y
  zonas personalizadas. Cada interfaz de red se asigna a una zona.
- **Matriz zona→zona** con política por defecto (permitir / bloquear) y, al
  pulsar una casilla, sus reglas: acción (permitir/bloquear/rechazar),
  protocolo, puertos, IP/red origen y destino, listas reutilizables (grupos de
  IPs, grupos de puertos y, con directorio, grupos de usuarios VPN §7.8),
  orden, activar/desactivar, contador de aciertos.
- **Se adapta a la máquina:** con una sola interfaz la vista se reduce a
  «qué puede entrar a esta máquina» (Externa→Gateway); con varias, matriz
  completa + NAT (masquerade) + reenvío de puertos (DNAT, ver «Reenvío de
  puertos» abajo).
- **Relación con WireGuard:** el firewall decide a nivel de zona (¿puede VPN
  hablar con Interna?), el módulo WireGuard decide por cliente. Un paquete
  debe ser aceptado por las dos tablas (en nftables un drop en cualquier tabla
  gana). La UI del firewall lo explica en la zona VPN y enlaza a WireGuard.
- **Anti-bloqueo:** al aplicar, se guarda el ruleset anterior y se arranca un
  temporizador de 60 s; si no se pulsa «Confirmar» en el panel, se restaura
  solo. El puerto del panel y SSH desde la zona Interna están siempre
  permitidos (regla fija, visible pero no borrable).
- Al instalarse: avisa, guarda el ruleset actual y desactiva `ufw`/`firewalld`.
  Al desinstalarse: ofrece restaurar lo guardado.

### Reenvío de puertos (dentro del Firewall)

Objetivo (pedido por el usuario el 2026-11-27): **crear un reenvío debe ser
tan fácil como en UniFi o más, y gestionarlos, igual de fácil.** Por debajo
son reglas DNAT de nftables en la tabla `netctl`, pero el usuario nunca
tiene que pensar en NAT ni en zonas.

**Crear: asistente de 3 pasos** (botón «+ Abrir puerto»):
1. **¿A qué equipo?** Selector de equipos (§7.6): lista con buscador como
   en UniFi, o escribir la IP a mano en el mismo campo. Si su IP la da el DHCP y no es fija, una casilla
   marcada por defecto la fija («para que la regla no se rompa si cambia de
   IP»).
2. **¿Qué servicio?** Tarjetas con servicios habituales que rellenan solos
   puerto y protocolo: Web (80/443), SSH, Escritorio remoto, Minecraft,
   servidor de juegos, Plex/Jellyfin, cámaras (RTSP), FTP, correo… y
   «Otro» para poner el puerto (o un rango, p. ej. `27015-27030`) y elegir
   TCP, UDP o los dos. Por defecto el puerto de fuera es el mismo que el de
   dentro; cambiarlo es un campo opcional en el mismo paso.
3. **¿Desde dónde?** «Desde cualquier sitio de internet» (por defecto),
   «Solo desde estas IPs» o, con el módulo Seguridad, «Solo desde estos
   países». Opcional: **caduca** en 1 hora / 1 día / 1 semana / nunca (por
   defecto nunca), útil para abrir algo un rato.

Al pulsar «Crear» se aplica (con la confirmación de 60 s del Firewall) y
se muestra **cómo conectarse**: «Desde fuera, usa 85.12.34.56:25565»
(con botón copiar; con varias WAN, una línea por WAN o el dominio si hay).

Valores por defecto que no se preguntan (se pueden cambiar luego en el
panel lateral, cada uno explicado): todas las WAN, acceso desde dentro con
la IP pública activado (*NAT loopback / hairpin*), registro de conexiones
desactivado.

**Gestionar:** página «Reenvío de puertos»:
- Lista **agrupada por equipo** («Servidor-casa · 3 puertos»), cada fila
  con servicio, puerto de fuera → dentro, protocolo, desde dónde, estado y
  un interruptor para **activar/desactivar** sin borrar.
- **Estado de cada regla**, en llano: ● Funcionando (el equipo está
  conectado y responde en ese puerto), ⚠ El equipo está desconectado, ⚠ El
  equipo no responde en ese puerto (el servidor prueba a conectar desde
  dentro), ⏱ Caduca en 3 h, ⊘ Desactivada.
- Columna **«Última conexión»** y número de conexiones (contadores de
  nftables), para saber qué reglas se usan.
- Panel lateral para editar cualquier cosa, **duplicar** (p. ej. el mismo
  servicio a otro equipo) y borrar.
- Filtros: Todas · Activas · Desactivadas · Con problemas · Temporales; y
  búsqueda por equipo, servicio o puerto.
- **Accesos directos** desde el panel lateral de cada equipo («Abrir un
  puerto hacia este equipo», con sus reglas listadas) y desde cada WAN en
  Router › Internet.

**Avisos al crear o editar** (con cómo arreglarlo):
- El puerto de fuera ya lo usa otra regla, el panel, SSH o WireGuard → se
  ofrece el siguiente libre.
- El servicio es delicado abierto a todo internet (Escritorio remoto, SSH,
  bases de datos) → se recomienda «Solo desde estas IPs», la VPN o netctl
  VPN, con botón directo.
- El equipo de destino está en la zona VPN o Gateway → se explica que no
  hace falta reenvío.

**BPA (§7.9):** reglas hacia equipos que ya no existen, servicios delicados
abiertos a todo internet, reglas sin uso en 90 días, reglas temporales
caducadas que siguen guardadas.

**Si no aplica:** con una sola tarjeta de red no hay reenvío posible; en su
lugar la página explica cómo abrir el puerto en el router de la red y
ofrece la regla equivalente Externa→Gateway («permitir entrar a esta
máquina por el puerto X»).

**Tests:** generación de DNAT + permiso de entrada + loopback para puerto
simple, rango y puerto distinto fuera/dentro; varias WAN; caducidad; detección
de choques de puertos; plantillas de servicios; estado «no responde».

## 9b. Decisiones tras la revisión (decisiones de Claude)

- **WireGuard sin confirmación de 60 s:** sus reglas solo viven en la cadena
  forward y solo afectan a tráfico que entra por la interfaz wg, así que no
  pueden cortar el panel ni SSH. El «aplicar con confirmación» del SDK es
  genérico, pero solo lo usa el Firewall.
- **Dos tablas nftables:** `netctl_wg` (forward, prioridad `filter - 5`,
  política accept, solo reglas `iifname <wg>`) y `netctl` (prioridad `filter`,
  políticas por zona). Ambas deben aceptar.
- **NAT:** lo pone el módulo WireGuard mientras no haya módulo Firewall. Al
  instalar o quitar el Firewall, el núcleo avisa a los módulos
  (`POST /evento` con `modulos_cambiados`) y WireGuard regenera su tabla sin
  NAT o con NAT, según toque.
- **Migración desde wg-configurador:** el `.conf` de servidor que genera el
  módulo ya no lleva PostUp/PostDown de iptables. Al adoptar una interfaz
  existente, el módulo avisa, reinicia una vez la interfaz (eso quita las
  reglas iptables antiguas) y carga su tabla nftables. Los `.conf` de los
  clientes no cambian.
- **Arranque:** el servicio del Firewall va con `Before=network-pre.target`.
  WireGuard funciona aunque la tabla `netctl` no exista.
- **Núcleo:** el panel avisa de que hay versión nueva del núcleo (está en el
  catálogo), pero se actualiza con `install.sh` / el gestor de paquetes. Si un
  módulo pide una «versión mínima del núcleo» mayor que la instalada, la
  instalación se bloquea.
- **Firma:** se verifica con el crate `minisign-verify`. Rotar o revocar la
  clave queda fuera de la v1.
- **Primer acceso:** `install.sh` / el postinstall genera una contraseña de
  administrador aleatoria y la muestra una vez. Se puede cambiar con
  `netctl-core admin set-password`.

## 9c. Módulo Router (fase 3)

Enrutamiento «como tal»; sin DNS/DDNS/UPnP. El servidor DHCP es el módulo
aparte de §9e; el Router le ofrece sus redes (LAN/VLAN) si está instalado.

- **Interfaces:** WAN (DHCP cliente, IP fija, PPPoE), LAN y VLAN; cada una se
  asigna a una zona del Firewall. Las interfaces que gestiona el módulo se
  configuran directamente con iproute2 desde su servicio y se marcan como no
  gestionadas en el gestor de red de la distro (NetworkManager, ifupdown /
  netplan, wicked); las demás no se tocan. *Decisión de Claude, para que
  funcione igual en las tres familias.*
- **Tabla de rutas** en vivo y **rutas estáticas**.
- **Rutas por política** (origen, destino, puerto → conexión o VPN) con tablas
  de rutas + marcas de nftables.
- **Varias WAN** (sin límite fijo; cada una puede ser DHCP, IP fija o
  PPPoE). Página «Internet» como la de UniFi, con una tarjeta por WAN:
  estado, IP pública, latencia, pérdida y tráfico en vivo.
  - **Modo respaldo (failover):** cada WAN tiene una prioridad; se usa la
    de mayor prioridad que esté sana y, si cae, pasa a la siguiente. Al
    volver la principal, regresa sola (se puede desactivar el regreso
    automático).
  - **Modo balanceo de carga:** cada WAN tiene un peso (p. ej. 70/30). El
    reparto es por conexión, no por paquete: una conexión se queda siempre
    en la misma WAN (marca de conntrack), así no se rompen sesiones web,
    juegos ni VPN. Si una WAN cae, sus conexiones nuevas van a las demás.
  - **Mixto:** se pueden combinar, p. ej. balancear entre WAN1 y WAN2 y dejar
    WAN3 (4G) solo de respaldo.
  - **Comprobación de salud** por WAN: ping a varios destinos configurables
    (por defecto 1.1.1.1 y 8.8.8.8) por esa salida concreta; se da por caída
    si fallan todos durante N comprobaciones seguidas y por recuperada tras M
    correctas (valores en campos de texto con comprobador).
  - **Reglas por WAN:** las rutas por política pueden forzar cierto tráfico
    (origen, destino, puerto) por una WAN concreta, con opción de usar o no
    las demás si esa cae.
  - Cada WAN tiene su propio NAT (masquerade por su interfaz), que pone el
    Firewall; el Router solo le dice qué interfaces son WAN.
  - **Historial:** registro de caídas y cambios de WAN, y aviso en el panel.
  - **Relación con WireGuard:** con la app netctl VPN (§8b) no hay que hacer
    nada: recibe todas las direcciones. Para el servidor WireGuard clásico,
    un cliente solo admite un Endpoint. El módulo WireGuard avisa de que, con varias WAN, conviene
    usar un nombre de dominio como Endpoint y apuntarlo a la WAN activa (el
    DNS dinámico sigue fuera de alcance). Las respuestas del servidor salen
    por la misma WAN por la que entró la conexión.
- **Enrutamiento dinámico** OSPF, BGP y RIP con FRR: vecinos, estado de
  sesiones, rutas aprendidas. Pestaña propia, con cada opción explicada.
- **QoS:** límites de ancho de banda por red/dispositivo y control de latencia
  (CAKE / fq_codel con `tc`).
- NAT y reenvío de puertos siguen en el Firewall.
- Cambios de red con el «aplicar con confirmación» de 60 s.

## 9d. Módulo Seguridad (fase 4)

- **IDS/IPS con Suricata**: modo detección (avisa) o prevención (bloquea),
  el tráfico entra por una cola de nftables (NFQUEUE) en las zonas que se
  elijan. En RHEL/Rocky/Alma requiere activar EPEL (el manifiesto lo
  indica).
- Categorías de amenazas activables, lista de excepciones, actualización
  automática de reglas.
- **Amenazas detectadas:** IP, país, regla; bloquear o ignorar con un clic.
- **Bloqueo por país (GeoIP)** y **listas de IPs maliciosas** que se actualizan
  solas (sets de nftables).
- Mostrar el impacto en CPU y avisar si la máquina no da para el tráfico.

## 9e. Módulo DHCP (fase 3b)

Servidor DHCPv4 como el de UniFi OS: se configura dentro de cada red, no en
un fichero aparte.

- **Motor:** `dnsmasq` en modo solo DHCP (`port=0`, sin DNS), con su propia
  instancia y servicio (`netctl-dhcp`), sin tocar un dnsmasq que ya exista.
  *Decisión de Claude:* está en los repositorios base de Debian, openSUSE y
  RHEL (Kea necesita EPEL en algunos RHEL), es el motor que usa UniFi OS y el
  fichero de concesiones es fácil de leer en vivo.
- **Independiente del Router:** funciona sobre cualquier interfaz de la
  máquina (p. ej. un servidor con una sola tarjeta que da DHCP a la LAN de
  otro router). Si el Router está instalado, sus redes LAN/VLAN aparecen
  solas y cada red muestra su bloque «DHCP».
- **Por red** (igual que en UniFi OS):
  - Modo: **Servidor**, **Relay** (reenviar a otro servidor DHCP) o
    **Desactivado**.
  - Rango de IPs (inicio–fin), con sugerencia automática a partir de la red
    (p. ej. .6–.254) y aviso si choca con la IP de la puerta de enlace o con
    IPs fijas.
  - Puerta de enlace: automática (la IP de la máquina en esa red) o manual.
  - Servidores DNS: automáticos (la puerta de enlace) o hasta 4 manuales.
  - Tiempo de concesión (por defecto 86400 s, campo de texto con
    comprobador como el resto del panel).
  - Nombre de dominio.
  - Otras opciones (sección «Más opciones» al final del bloque, visible y
    explicada): NTP, arranque por red / PXE (opciones 66/67), WINS, y
    opciones DHCP personalizadas (número + valor).
- **IPs fijas (reservas):** MAC → IP con nombre. La IP puede estar dentro
  o fuera del rango; se avisa si está repetida o fuera de la red.
- **Clientes en vivo:** tabla con nombre de equipo, MAC, fabricante (por el
  OUI, con la base de datos compartida de `netctl-sdk`, §7.7), IP, red y tiempo restante
  de concesión. Botón «Fijar IP» en cada fila que crea la reserva con un
  clic; filtro por red y búsqueda.
- **Detección de servidores DHCP intrusos** (como el «DHCP Guarding» de
  UniFi, pero solo aviso): sondeo DHCPDISCOVER periódico en cada red; si
  responde otro servidor, el panel muestra su IP y MAC.
- **Firewall:** si está el módulo Firewall, el DHCP pide abrir UDP 67/68
  hacia Gateway desde las zonas con DHCP activo (regla fija, visible). Sin
  Firewall, abre los puertos en firewalld/ufw como hace WireGuard.
- **Aplicar:** genera la config, la valida con `dnsmasq --test` y recarga el
  servicio. Sin confirmación de 60 s: un cambio de DHCP no corta el panel ni
  SSH. Las concesiones se conservan entre reinicios.
- **Tests:** generación de la config de dnsmasq (rango, opciones, reservas,
  relay), validaciones (rango fuera de red, solapes, MAC mal escrita),
  lectura del fichero de concesiones y búsqueda de fabricante por OUI.

## 9f. Módulo Alta disponibilidad / «shadow mode» (fase 5)

Dos servidores netctl forman una pareja: uno **activo** y otro **en
espera** (shadow). Si el activo cae, el de espera toma el control en pocos
segundos sin que haya que tocar los clientes.

- **Solo parejas de dos.** No es un panel para gestionar muchos servidores:
  se configura todo en el activo y se copia al de espera.
- **Requisitos que el panel comprueba antes de activarlo:** mismas versiones
  del núcleo y de los módulos, mismas redes conectadas en los dos (cada LAN y
  cada WAN cableada a ambos, p. ej. a través de un switch) y un enlace entre
  ellos para la sincronización (dedicado recomendado, o una LAN).
- **Emparejado:** en el activo se genera un código de emparejamiento; se
  pega en el panel del otro. A partir de ahí se hablan por TLS con
  certificado mutuo.
- **IP virtual por red** con VRRP (`keepalived`): los equipos de la LAN usan
  como puerta de enlace una IP compartida que siempre tiene el activo. En
  las WAN con IP fija se comparte la IP pública; en las WAN por DHCP o PPPoE
  el de espera tiene la WAN levantada pero sin usar y la activa al tomar el
  control (se puede clonar la MAC para que el proveedor dé la misma IP).
- **Qué se sincroniza:** toda la configuración de netctl (núcleo y módulos)
  del activo al de espera, al aplicar cada cambio; el estado de las
  conexiones del firewall (`conntrackd`), para que las conexiones abiertas
  sigan vivas tras el cambio; y las concesiones DHCP, para que el de espera
  no reparta IPs repetidas. Las claves de WireGuard son las mismas en los
  dos, así que los clientes VPN reconectan solos.
- **Servicios en el de espera:** DHCP, WireGuard y las reglas de reenvío
  quedan preparados pero parados (o sin IP virtual) hasta tomar el control.
- **Panel:** tarjeta «Alta disponibilidad» con el estado de los dos (activo,
  en espera, sin conexión), última sincronización, historial de cambios de
  papel y botón «Pasar el control al otro» para mantenimiento. El panel del
  de espera solo muestra el estado y avisa de que se configura desde el
  activo.
- **Protección contra «cerebro dividido»:** si se pierde el enlace de
  sincronización pero los dos siguen vivos, el de espera comprueba también
  por las LAN antes de tomar el control; si no puede asegurarse, avisa y no
  lo toma.
- **Actualizar sin cortes:** se actualiza primero el de espera, se le pasa
  el control y luego se actualiza el otro.
- **Tests:** generación de la config de keepalived/conntrackd, máquina de
  estados del papel (activo / espera / fallo / regreso), comprobaciones
  previas y formato de la sincronización. La prueba real con dos VMs la hace
  el usuario, con una guía paso a paso.

## 10. Fases

1. **Fase 1:** workspace, `wg-modelo` extraído, núcleo (login, carcasa, kit,
   gestor de módulos, catálogo firmado), herramientas de publicación, módulo
   WireGuard completo.
2. **Fase 2:** módulo Firewall.
   - **Fase 2b:** VPN de configuración cero «netctl VPN» y sus apps (§8b).
   - **Fase 2c:** páginas Equipos y Topología del núcleo (§7.7). Salen con
     lo que haya (vecinos de la red + clientes VPN) y cada módulo posterior
     les añade su parte (DHCP nombres, Router tráfico, HA la pareja).
   - **Fase 2d:** unión a Active Directory / OpenLDAP (§7.8). netctl VPN y
     el Firewall funcionan sin ella; al llegar, les añade usuarios y grupos
     del directorio.
   - **Fase 2e:** analizador de buenas prácticas (§7.9) con las
     comprobaciones de todo lo hecho hasta ahí (núcleo, WireGuard, Firewall,
     netctl VPN, Equipos y Directorio). A partir de aquí, cada fase incluye
     las comprobaciones BPA de su módulo como parte de «terminado».
3. **Fase 3:** módulo Router.
   - **Fase 3b:** módulo DHCP (§9e). Puede ir antes del Router si se
     prefiere, porque no depende de él.
4. **Fase 4:** módulo Seguridad.
5. **Fase 5:** módulo Alta disponibilidad / shadow mode (§9f). Va al final
   porque tiene que sincronizar la config de todos los módulos anteriores.

Cada fase tiene su propio plan; las fases 3, 4 y 5 se detallan al llegar a
ellas.

Las pruebas reales en Debian, openSUSE y Rocky las hace **el usuario**. Quien
lo construya entrega tests automáticos (unitarios: generación de reglas
nftables, configs, manifiesto, firma, detección de distro con `os-release` de
ejemplo) y una guía corta de prueba manual por fase.

## 11. Lo que queda a quien lo construya

- Versiones exactas de crates y herramientas; cómo hacer el build musl.
- Diseño concreto de las APIs de cada módulo y de los componentes del kit.
- La guía de prueba manual para el usuario.
- Formato exacto de `manifest.toml` e `index.json`.
- Iconos (estilo UniFi, línea fina) y detalles visuales finos.
