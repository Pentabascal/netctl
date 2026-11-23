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
- Módulo **Seguridad**: IDS/IPS y protección de amenazas.
- Funciona igual en Debian 12+, Ubuntu 22.04+, openSUSE Leap 15.6 / SLES 15,
  RHEL / Rocky / Alma 9+ y Fedora actual.

## 3. No objetivos

- Gestionar varios servidores desde un único panel (cada servidor tiene su
  propio panel). *Decisión de Claude: el usuario no lo ha pedido.*
- Multiusuario con roles; distribución pulida para terceros; traducciones.
- Integración con LinuxServerPanel.
- Módulos de terceros (solo se aceptan módulos firmados con la clave del
  proyecto).
- Horarios en reglas de firewall y filtrado por aplicación/DPI.
- Servicios de red en el módulo Router: DHCP, DNS, DNS dinámico, UPnP
  (decisión del usuario: «un router como tal»).

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
  claras. Lo habitual va a la vista; lo avanzado, plegado en «Avanzado». Los
  detalles se editan en el panel lateral, no en páginas largas.
- **Fácil de configurar:** valores por defecto que funcionan sin tocar nada;
  asistentes para lo primero que hace cada módulo (primer servidor WireGuard,
  asignar interfaces a zonas, configurar WAN, activar IPS); validación en el
  formulario con mensajes en castellano llano; vista previa de lo que va a
  cambiar antes de aplicar.
- **Bonito:** estética UniFi Network cuidada (espaciado, iconos de línea,
  tarjetas con métricas, gráficas de tráfico, estados con color) y coherente
  en todos los módulos gracias al kit compartido.

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
- **Acceso:** un único usuario administrador creado al instalar
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
- Los módulos se descubren entre sí a través del núcleo
  (`GET /api/modulos` interno) — p. ej. el firewall pregunta si existe el
  módulo WireGuard para crear la zona VPN.
- `netctl-sdk` implementa el servidor del socket, la lectura de cabeceras, la
  capa de distro y el «aplicar con confirmación», para que un módulo nuevo
  sea casi solo su lógica.

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

## 9. Módulo Firewall (fase 2)

- Tabla propia `inet netctl` en nftables; carga atómica con `nft -f`; se
  reaplica al arrancar desde su servicio.
- **Zonas** como UniFi: Externa, Interna, Gateway (la propia máquina), VPN
  (aparece sola si está el módulo WireGuard; agrupa sus interfaces wg), DMZ y
  zonas personalizadas. Cada interfaz de red se asigna a una zona.
- **Matriz zona→zona** con política por defecto (permitir / bloquear) y, al
  pulsar una casilla, sus reglas: acción (permitir/bloquear/rechazar),
  protocolo, puertos, IP/red origen y destino, listas reutilizables (grupos de
  IPs, grupos de puertos), orden, activar/desactivar, contador de aciertos.
- **Se adapta a la máquina:** con una sola interfaz la vista se reduce a
  «qué puede entrar a esta máquina» (Externa→Gateway); con varias, matriz
  completa + NAT (masquerade) + reenvío de puertos (DNAT).
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

Enrutamiento «como tal»; sin DHCP/DNS/DDNS/UPnP.

- **Interfaces:** WAN (DHCP cliente, IP fija, PPPoE), LAN y VLAN; cada una se
  asigna a una zona del Firewall. Las interfaces que gestiona el módulo se
  configuran directamente con iproute2 desde su servicio y se marcan como no
  gestionadas en el gestor de red de la distro (NetworkManager, ifupdown /
  netplan, wicked); las demás no se tocan. *Decisión de Claude, para que
  funcione igual en las tres familias.*
- **Tabla de rutas** en vivo y **rutas estáticas**.
- **Rutas por política** (origen, destino, puerto → conexión o VPN) con tablas
  de rutas + marcas de nftables.
- **Multi-WAN:** conmutación por caída (sondeo de salud por WAN) o reparto de
  carga.
- **Enrutamiento dinámico** OSPF, BGP y RIP con FRR: vecinos, estado de
  sesiones, rutas aprendidas. Pestaña propia, plegada por defecto.
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

## 10. Fases

1. **Fase 1:** workspace, `wg-modelo` extraído, núcleo (login, carcasa, kit,
   gestor de módulos, catálogo firmado), herramientas de publicación, módulo
   WireGuard completo.
2. **Fase 2:** módulo Firewall.
3. **Fase 3:** módulo Router.
4. **Fase 4:** módulo Seguridad.

Cada fase tiene su propio plan; las fases 3 y 4 se detallan al llegar a ellas.

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
