# netctl · Firewall al estilo UniFi (fase 2.5)

Amplía §7 del spec principal (`2026-11-23-netctl-panel-design.md`). Referencia: UniFi OS Server 5.1.42 / Network 10.5 instalado en local (capturas en el scratchpad de la sesión del 2026-11-28; ver memoria `unifi-os-referencia`). Decidido con el usuario el 2026-11-28.

## 1. Dónde vive

- El firewall deja el menú principal y pasa a **Ajustes › Motor de políticas** (UniFi: *Settings › Policy Engine*). Ajustes tiene un **segundo menú lateral** (como UniFi): primero las secciones del núcleo (Contraseña, Verificación en dos pasos, Acceso) y después los grupos que aportan los módulos.
- Los módulos declaran entradas de Ajustes en `manifest.toml` con `[[ajustes]] grupo, titulo, ruta, icono`. Las entradas del mismo `grupo` se muestran como **pestañas con icono** en una fila (UniFi: iconos bajo «Policy Engine»).
- Motor de políticas del Firewall: **Tabla de políticas**, **Zonas**, **Grupos**, **Registro**.
- Un módulo sin `[[menu]]` pero con `[[ajustes]]` no aparece en el menú principal.

## 2. Aspecto (copiado de UniFi, tema oscuro)

| Elemento | Valor |
|---|---|
| Fondo de página y paneles | `#131416` |
| Barra superior | `#282b2f` |
| Bloques (tarjetas, secciones del panel, cabeceras de matriz) | `#1c1e21` |
| Campos | `#242629`, sin borde |
| Azul (radio activo, botón primario) | `#4797ff` |
| Enlaces y acciones | `#418ae8` |
| Casilla «Permitir todo» | fondo y borde `#123f20` |
| Casilla «Bloquear todo» | fondo y borde `#4a1a1d` |
| Letra base | 13 px; etiquetas 11.5 px gris; filas de tabla 32 px |

- **Panel lateral** de 420 px, título centrado y cruz a la derecha. Cada bloque (Nombre, Origen, Acción, Destino, Opciones) va en su propia caja `#1c1e21` con título en negrita y un «i» azul.
- **Opciones como radios en línea** («○ Cualquiera ○ Equipo ○ Red …») en lugar de desplegables o interruptores. Solo la zona es un desplegable.
- **Acciones como enlaces azules bajo la tabla** («Crear zona», «Crear política», «Reordenar»). El botón primario solo aparece en el pie del panel.
- Fuera: tarjetas de resumen, barra con título y subtítulo, y pestañas de texto en las páginas del firewall.
- Se mantienen las reglas §3b: cada campo explicado (el «i» de UniFi es nuestro «?»), sin secciones «Avanzado» y campos numéricos como texto con comprobador.

## 3. Zonas

Una sola página, de arriba abajo:

1. **Esquema plegable:** Internet → Gateway → Interna · VPN · DMZ · Hotspot, con las tarjetas de red de cada una.
2. **Tabla de zonas:** columnas Nombre (candado en las de serie, «i») y Tarjetas de red (chips). Debajo, el enlace «Crear zona», que abre un panel con nombre y tarjetas. Pulsar una zona abre ese mismo panel; en las de serie solo se pueden cambiar las tarjetas.
3. **Zona nueva de serie: Hotspot (invitados).**
   - Puede ir a Externa.
   - A Gateway solo DHCP y DNS (reglas fijas).
   - Todo lo demás, bloqueado.
4. **Zonas personalizadas:** pueden ir a Externa, a Gateway y a sí mismas; lo demás, bloqueado (UniFi).
5. **Matriz** «Destino» arriba y «Origen» en vertical.
   - Primera casilla: «Todas las políticas (n)».
   - Cada casilla dice «Permitir todo» o «Bloquear todo», con «(n)» si tiene reglas propias.
   - Pulsar una casilla **filtra la lista de debajo**; volver a pulsarla quita el filtro. Pulsar la cabecera abre el panel de la política de esa pareja.
   - Botón de pantalla completa.
6. **Lista de políticas:**
   - Casillas IPv4 / IPv6 y buscador.
   - Columnas a elegir: Nombre, Acción, Versión IP, Protocolo, Zona origen, Origen, Puerto origen, Zona destino, Destino, Puerto destino.
   - Las de serie van agrupadas por nombre, con candado y desplegables.
   - Cada fila tiene un interruptor para pausar o reanudar.
   - Debajo, los enlaces «Crear política» y «Reordenar» (subir y bajar).
7. Con una sola tarjeta de red sigue la vista «Qué puede entrar a esta máquina» (§7, fase 2).

## 4. Política (formulario completo)

| Bloque | Campos |
|---|---|
| Nombre | texto |
| Zona de origen | desplegable de zona · Cualquiera / Equipo / Red / IP / MAC · Puerto: Cualquiera / Concreto / Lista (grupo de puertos) |
| Acción | Bloquear / Permitir / Rechazar |
| Zona de destino | desplegable · Cualquiera / Equipo-IP / Red / Dominio / País / App (las tres últimas en 2.5c) · Puerto: Cualquiera / Concreto / Lista |
| Opciones | Versión IP (Ambas / IPv4 / IPv6) · Protocolo (Todos / TCP y UDP / TCP / UDP / Otro nº) · Estado de la conexión (Todas / Solo nuevas / A elegir: nueva, establecida, relacionada, inválida) · Registrar (log) · Horario (Siempre / Diario / Semanal / Una vez / A medida) · Descripción |

- **Red** es una tarjeta de red de la zona elegida.
- **MAC** solo se admite como origen.
- **Estado de la conexión:** las reglas cuyo estado incluye «establecida» o «relacionada» se ponen **antes** del `ct state established,related accept` general, para que puedan cortar conexiones ya abiertas.
- **Horario:** Diario es una franja horaria. Semanal es días más franja. Una vez es inicio y fin con fecha. A medida es varias franjas, cada una con sus días.

## 5. Tabla de políticas (2.5b)

- Una sola lista con Firewall, NAT y Reenvío de puertos.
- **Filtros a la izquierda**, cada opción con su contador:
  - Tipo;
  - Acción;
  - Protocolo;
  - Origen;
  - Destino.
- «Crear política» abre el panel con un primer bloque de radios **Firewall / NAT / Reenvío de puertos**.
- **Reglas NAT propias:** Masquerade, SNAT (cambiar el origen por una IP) y DNAT (cambiar el destino por IP y puerto). Cada una con interfaz, origen, destino, protocolo y puertos. Van **antes** que las automáticas.
- **Reenvío de puertos al estilo UniFi:**
  - Nombre;
  - Interfaz WAN;
  - **IP WAN** («Todas» o una de las IPs de esa interfaz);
  - Puerto WAN;
  - IP de destino con «Elegir equipo»;
  - Puerto de destino;
  - Protocolo;
  - Origen: Cualquiera o Limitado (IP, red o rango);
  - Registrar.

  Además mantiene lo nuestro: caducidad, NAT loopback y seguimiento por MAC.
- **Avisos del reenvío**, sacados de la IP de la WAN:
  - IP privada: «está detrás de otro router; abre el mismo puerto también allí»;
  - 100.64.0.0/10: «CGNAT; tu proveedor tiene que abrir el puerto»;
  - dirección obtenida por DHCP o PPPoE: «IP dinámica; usa un DNS dinámico».
- **Conflictos al reanudar:** si al reanudar un reenvío choca con otro activo, no se reanuda y se dice con cuál (409).
- La página «Reenvío de puertos» desaparece. El asistente «Abrir un puerto» (≤3 pasos) se abre desde «Crear política».

## 6. Destinos extra (2.5c)

- **Dominio:**
  - una lista de nombres;
  - el módulo los resuelve cada 5 min y añade las IPs a conjuntos de nftables con caducidad de 1 h;
  - no se recarga la tabla.
- **País:**
  - base libre **db-ip «IP to Country Lite»** (CC BY 4.0), CSV mensual;
  - se descarga al usarla por primera vez y después cada mes;
  - un conjunto por país usado;
  - en la interfaz se muestra «IP Geolocation by DB-IP».
- **App (DPI):**
  - la librería **nDPI del sistema** se carga en tiempo de ejecución, sin enlazarla, porque el binario es musl estático;
  - se le pasan por **NFQUEUE** (con `bypass`) solo los primeros 12 paquetes de cada conexión, y solo en las parejas de zonas que tienen reglas de App;
  - la app detectada se guarda en `ct mark` y las reglas comparan ese `ct mark`;
  - si no está libndpi, la opción App sale desactivada con su explicación;
  - el manifiesto pide el paquete de la distro.
