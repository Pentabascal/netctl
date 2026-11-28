# Prueba de la fase 2: Firewall (Debian, openSUSE, Rocky)

Prueba corta. Si algo sale distinto de lo que pone aquí, apunta el paso y lo que viste.

## 0. Publicar (una vez, en tu PC)

La clave de firma anterior se perdió, así que hay una nueva y **el núcleo hay que
reinstalarlo** en cada servidor (los núcleos 0.1.x no aceptan módulos firmados con la
clave nueva).

1. Herramientas: `rustup target add x86_64-unknown-linux-musl`, `minisign`, `gh`,
   `cargo-deb`, `cargo-generate-rpm` y npm.
2. Clave nueva (la privada **nunca** va al repo):

   ```sh
   mkdir -p ~/.minisign
   minisign -G -W -p crates/netctl-core/clave_publica.txt -s ~/.minisign/netctl.key
   git add crates/netctl-core/clave_publica.txt && git commit -m "core: clave pública de firma nueva"
   ```

   Guarda una copia de `~/.minisign/netctl.key` fuera de este PC.
3. Publica:

   ```sh
   PATH=~/.cargo/bin:$PATH tools/publicar.sh nucleo
   PATH=~/.cargo/bin:$PATH tools/publicar.sh wireguard
   PATH=~/.cargo/bin:$PATH tools/publicar.sh firewall
   ```

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
