# Prueba de la fase 2.5a: Firewall al estilo UniFi (Debian, openSUSE, Rocky)

Prueba corta. Si algo sale distinto de lo que pone aquí, apunta el paso y lo que viste.

## 0. Publicar (una vez, en tu PC)

El Firewall 0.2.0 necesita el núcleo 0.3.0: hay que publicar los dos y **reinstalar el
núcleo** en cada servidor.

```sh
PATH=~/.cargo/bin:$PATH tools/publicar.sh nucleo
PATH=~/.cargo/bin:$PATH tools/publicar.sh firewall
```

## A. Lo nuevo de la 2.5a (después de actualizar núcleo y Firewall)

1. El Firewall ya no está en el menú de la izquierda. Ve a **Ajustes**: abajo sale
   «Motor de políticas» con cuatro iconos (Zonas, Reenvío de puertos, Grupos, Registro).
2. **Zonas**: arriba el esquema (Internet → Esta máquina → tus zonas), que se pliega con la
   flecha y lo recuerda; debajo la tabla de zonas con candado en las de serie y una zona
   nueva, **Hotspot (invitados)**.
3. Pulsa una zona → marca una tarjeta → Guardar → confirma en 60 s. La tarjeta cambia de zona.
4. «Crear zona» → «Cámaras» con una tarjeta libre → en la matriz, Cámaras solo llega a
   Externa, a Esta máquina y a sí misma.
5. Matriz: pulsa una casilla → la lista de debajo solo enseña esa pareja; vuelve a pulsarla o
   pulsa «Todas las políticas» para quitar el filtro. El lápiz de la casilla elegida cambia
   «Permitir todo / Bloquear todo» y deja «Volver a la de serie».
6. «Crear política»: prueba origen **MAC** de un móvil → Bloquear → destino Externa →
   Horario **Semanal** L–V 08:00–17:00 → Guardar y confirmar. Desde ese móvil, en ese horario,
   no hay internet; fuera del horario, sí.
7. Otra política: origen **Red** (una tarjeta), Protocolo **Otro 47**, Estado **Solo nuevas**.
   Guarda: en «Ajustes del firewall» no hay errores y `nft list table inet netctl` la enseña.
8. Lista: desmarca IPv6 (desaparecen las de «Solo IPv6»), busca por nombre, quita columnas
   con el icono de columnas (lo recuerda al recargar), pausa una política con su interruptor y
   prueba «Reordenar» (subir/bajar y Guardar orden).
9. Conexión abierta cortada: SSH desde la red interna a un equipo de la DMZ, y crea
   Interna→DMZ «Bloquear», puerto 22, Estado «A elegir: Establecida». La sesión abierta se corta;
   **el panel y el SSH a esta máquina siguen funcionando**.

## 1. Actualizar el núcleo (en cada distro)

```sh
curl -fsSL https://raw.githubusercontent.com/Pentabascal/netctl/master/install.sh | sudo sh
```

En **Inicio** debe poner «Versión del núcleo 0.2.0». En **Módulos › Instalados**,
actualiza WireGuard a 0.2.0. Tus clientes VPN siguen igual.

## 2. Instalar el Firewall con el firewall de la distro activo

En Rocky (firewalld) y en Debian/Ubuntu con `sudo ufw enable` hecho antes:

1. **Módulos › Disponibles › Firewall › Instalar.**
2. Entra en **Firewall**: sale el asistente de 3 pasos. Comprueba:
   - Paso 1: la tarjeta de internet viene marcada «(recomendado)».
   - Paso 3: avisa de que se apagará firewalld/ufw y lista lo que queda siempre abierto
     (panel, SSH, WireGuard).
3. **Aplicar** → sale la barra de 60 s → **Confirmar cambios**.
4. Comprueba que **siguen funcionando**: el panel, una sesión SSH nueva y la VPN desde el
   móvil (internet e intranet).
5. `systemctl is-active firewalld` (o `ufw status`) → inactivo.
   `sudo nft list table inet netctl` muestra las reglas.

## 3. La vuelta atrás de 60 s

1. **Zonas › Matriz**, pulsa la casilla **Interna → Esta máquina** y ponla en **Bloquear**.
2. **No confirmes.** Si pierdes el panel, espera: a los 60 s vuelve solo.
3. Recarga: la casilla vuelve a **Permitir** y en **Registro** pone «Deshecho solo».

## 4. Reenvío de puertos (solo en una máquina que haga de router, con 2 tarjetas)

1. **Reenvío de puertos › Abrir puerto**: elige un equipo de la LAN, plantilla
   **Minecraft** (o «Otro» con el puerto de algo que tengas) y «Desde cualquier sitio».
2. Al crear, enseña «Desde fuera, usa IP:puerto». Confirma.
3. Desde el móvil **con datos (sin wifi)**, conéctate a esa dirección: funciona.
4. Desde un equipo de la LAN, usa la **misma dirección pública**: también funciona
   (NAT loopback).
5. Cambia la IP del equipo de destino (o renueva su DHCP). En menos de un minuto el
   reenvío le sigue y en **Registro** pone «La IP de … cambió».
6. Edita el reenvío y pon **Caduca: dentro de 1 hora**. Pasada la hora sale
   «Desactivada» (no se borra).
7. Prueba a abrir el **8443** desde fuera: no te deja y ofrece «Usar el 8444».
8. Elige **Escritorio remoto** con «Desde cualquier sitio»: sale el aviso de peligro.

Con una sola tarjeta de red, **Reenvío de puertos** explica que hay que abrirlo en el
router, y **Zonas** pasa a «Qué puede entrar a esta máquina»: prueba **Abrir un puerto**
(p. ej. Web) y comprueba que llegas desde otro equipo de la LAN.

## 5. Reinicio

`sudo reboot`. Al volver, sin entrar en el panel: SSH y la VPN funcionan y
`sudo nft list table inet netctl` ya tiene las reglas (el Firewall arranca antes que la
red).

## 6. Quitar el Firewall

1. **Módulos › Firewall › Quitar**, con «Volver al firewall que había antes
   (recomendado)» encendido.
2. `systemctl is-active firewalld` (o `ufw status`) → activo otra vez, con su
   configuración de antes. `sudo nft list table inet netctl` → no existe.
3. La VPN sigue funcionando (WireGuard vuelve a poner su propio NAT).
