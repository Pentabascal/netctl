#![cfg_attr(windows, windows_subsystem = "windows")]

mod claves;
mod modelo;
mod ui_kit;

use std::path::{Path, PathBuf};

use eframe::egui::{self, Align, Color32, Layout, RichText, Stroke, vec2};
use modelo::{Proyecto, nombre_seguro};
use ui_kit::*;

#[derive(PartialEq, Clone, Copy)]
enum Pagina {
    Resumen,
    Servidor,
    Intranet,
    Clientes,
    ConfServidor,
}

impl Pagina {
    fn titulo(self) -> &'static str {
        match self {
            Pagina::Resumen => "Resumen",
            Pagina::Servidor => "Servidor",
            Pagina::Intranet => "Intranet",
            Pagina::Clientes => "Clientes",
            Pagina::ConfServidor => "Configuración del servidor",
        }
    }
}

struct App {
    p: Proyecto,
    pagina: Pagina,
    sel: Option<usize>,
    ruta: Option<PathBuf>,
    estado: String,
    qr: Option<(String, egui::TextureHandle)>,
    ver_privadas: bool,
    logo: egui::TextureHandle,
}

impl App {
    fn new(logo: egui::TextureHandle) -> Self {
        let mut p = Proyecto::nuevo();
        p.anadir_cliente();
        Self {
            p,
            pagina: Pagina::Resumen,
            sel: None,
            ruta: None,
            estado: String::new(),
            qr: None,
            ver_privadas: false,
            logo,
        }
    }

    // ------------------------------------------------------------ ficheros

    fn guardar(&mut self, como: bool) {
        let ruta = match (&self.ruta, como) {
            (Some(r), false) => Some(r.clone()),
            _ => rfd::FileDialog::new()
                .set_title("Guardar proyecto")
                .add_filter("Proyecto WireGuard", &["json"])
                .set_file_name("vpn.wgproj.json")
                .save_file(),
        };
        let Some(ruta) = ruta else { return };
        let r = serde_json::to_string_pretty(&self.p)
            .map_err(|e| e.to_string())
            .and_then(|j| escribir_privado(&ruta, &j).map_err(|e| e.to_string()));
        self.estado = match r {
            Ok(()) => {
                self.ruta = Some(ruta.clone());
                format!("Proyecto guardado en {} (contiene claves privadas)", ruta.display())
            }
            Err(e) => format!("Error al guardar: {e}"),
        };
    }

    fn abrir(&mut self) {
        let Some(ruta) = rfd::FileDialog::new()
            .set_title("Abrir proyecto")
            .add_filter("Proyecto WireGuard", &["json"])
            .pick_file()
        else {
            return;
        };
        match std::fs::read_to_string(&ruta)
            .map_err(|e| e.to_string())
            .and_then(|t| serde_json::from_str::<Proyecto>(&t).map_err(|e| e.to_string()))
        {
            Ok(p) => {
                self.p = p;
                self.sel = None;
                self.estado = format!("Abierto {}", ruta.display());
                self.ruta = Some(ruta);
            }
            Err(e) => self.estado = format!("Error al abrir: {e}"),
        }
    }

    fn exportar(&mut self) {
        let errores = self.p.validar();
        if !errores.is_empty() {
            self.estado = format!("No se puede exportar: hay {} errores", errores.len());
            return;
        }
        let Some(dir) = rfd::FileDialog::new().set_title("Carpeta de destino").pick_folder() else {
            return;
        };
        let r = (|| -> Result<usize, String> {
            let srv = self.p.config_servidor()?;
            let nombre = format!("{}.conf", nombre_seguro(&self.p.servidor.interfaz));
            escribir_privado(&dir.join(&nombre), &srv).map_err(|e| e.to_string())?;
            let cdir = dir.join("clientes");
            std::fs::create_dir_all(&cdir).map_err(|e| e.to_string())?;
            for c in &self.p.clientes {
                let conf = self.p.config_cliente(c)?;
                escribir_privado(&cdir.join(format!("{}.conf", nombre_seguro(&c.nombre))), &conf)
                    .map_err(|e| e.to_string())?;
            }
            Ok(self.p.clientes.len())
        })();
        self.estado = match r {
            Ok(n) => format!("Exportado servidor + {n} clientes en {}", dir.display()),
            Err(e) => format!("Error al exportar: {e}"),
        };
    }

    // ------------------------------------------------------------ marco

    fn barra_lateral(&mut self, ui: &mut egui::Ui) {
        ui.add_space(16.0);
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.add(egui::Image::new((self.logo.id(), vec2(34.0, 34.0))));
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                ui.label(RichText::new("WireGuard").strong().color(TEXTO));
                ui.label(RichText::new("Configurador").small().color(TEXTO2));
            });
        });
        ui.add_space(20.0);

        let n_cli = self.p.clientes.len();
        let n_sub = self.p.intranet.len();
        let items = [
            (Pagina::Resumen, "▦", None),
            (Pagina::Servidor, "🖧", None),
            (Pagina::Intranet, "🏢", Some(n_sub)),
            (Pagina::Clientes, "👤", Some(n_cli)),
            (Pagina::ConfServidor, "📄", None),
        ];
        for (pag, icono, n) in items {
            let titulo = if pag == Pagina::ConfServidor { "Config. servidor" } else { pag.titulo() };
            if item_nav(ui, icono, titulo, self.pagina == pag, n).clicked() {
                self.pagina = pag;
            }
        }

        ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
            ui.add_space(12.0);
            if !self.estado.is_empty() {
                ui.add(egui::Label::new(RichText::new(&self.estado).small().color(TEXTO2)).wrap());
                ui.add_space(4.0);
            }
            let nombre = self
                .ruta
                .as_ref()
                .and_then(|r| r.file_name())
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "Proyecto sin guardar".into());
            ui.label(RichText::new(nombre).small().color(TEXTO2));
            ui.separator();
        });
    }

    fn cabecera(&mut self, ui: &mut egui::Ui, errores: &[String]) {
        ui.horizontal_centered(|ui| {
            ui.add_space(8.0);
            ui.label(RichText::new(self.pagina.titulo()).size(19.0).strong().color(TEXTO));
            ui.add_space(8.0);
            if errores.is_empty() {
                insignia(ui, "● Configuración válida", VERDE, VERDE_TINTE);
            } else {
                let texto = if errores.len() == 1 { "1 error".into() } else { format!("{} errores", errores.len()) };
                insignia(ui, &format!("● {texto}"), ROJO, ROJO_TINTE).on_hover_ui(|ui| {
                    for e in errores {
                        ui.label(RichText::new(e).color(ROJO));
                    }
                });
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(8.0);
                if boton_primario(ui, "⬇  Exportar configs").clicked() {
                    self.exportar();
                }
                if boton(ui, "Guardar").clicked() {
                    self.guardar(false);
                }
                if boton(ui, "Abrir…").clicked() {
                    self.abrir();
                }
                ui.menu_button(RichText::new("⋯").size(16.0), |ui| {
                    if ui.button("Nuevo proyecto").clicked() {
                        *self = App::new(self.logo.clone());
                    }
                    if ui.button("Guardar como…").clicked() {
                        self.guardar(true);
                    }
                });
            });
        });
    }

    // ------------------------------------------------------------ páginas

    fn pag_resumen(&mut self, ui: &mut egui::Ui, errores: &[String]) {
        let s = &self.p.servidor;
        let con_internet = self.p.clientes.iter().filter(|c| c.internet).count();
        ui.columns(4, |cols| {
            tarjeta_dato(&mut cols[0], "Clientes", &self.p.clientes.len().to_string(), &format!("{con_internet} con internet"));
            tarjeta_dato(&mut cols[1], "Subredes de intranet", &self.p.intranet.len().to_string(), "accesibles por VPN");
            tarjeta_dato(&mut cols[2], "Red VPN", &s.red_vpn, &format!("servidor {}", s.ip_servidor));
            tarjeta_dato(&mut cols[3], "Endpoint", &format!(":{}", s.puerto), &s.endpoint);
        });
        ui.add_space(16.0);

        tarjeta(ui, "Estado", Some("Comprobación de la configuración actual"), |ui| {
            if errores.is_empty() {
                ui.label(RichText::new("✔  Todo correcto, listo para exportar.").color(VERDE));
            } else {
                for e in errores {
                    ui.label(RichText::new(format!("✖  {e}")).color(ROJO));
                }
            }
        });
        ui.add_space(16.0);

        let mut ir = None;
        tarjeta(ui, "Clientes", None, |ui| {
            if self.p.clientes.is_empty() {
                ui.label(RichText::new("Todavía no hay clientes.").color(TEXTO2));
            }
            for (i, c) in self.p.clientes.iter().enumerate() {
                if fila_cliente_mini(ui, i, c, &self.p).clicked() {
                    ir = Some(i);
                }
            }
        });
        if let Some(i) = ir {
            self.pagina = Pagina::Clientes;
            self.sel = Some(i);
        }
        ui.add_space(16.0);

        let iface = nombre_seguro(&s.interfaz);
        tarjeta(ui, "Puesta en marcha", None, |ui| {
            for (n, t) in [
                format!("Exporta las configs y copia {iface}.conf a /etc/wireguard/ del servidor (chmod 600)."),
                format!("Actívalo con: systemctl enable --now wg-quick@{iface}"),
                format!("Abre/redirige el puerto UDP {} del router hacia el servidor.", s.puerto),
                "Entrega a cada usuario su .conf o que escanee el QR con la app WireGuard.".into(),
            ]
            .iter()
            .enumerate()
            {
                ui.horizontal(|ui| {
                    insignia(ui, &(n + 1).to_string(), AZUL, AZUL_TINTE);
                    ui.label(RichText::new(t).color(TEXTO));
                });
            }
        });
    }

    fn pag_servidor(&mut self, ui: &mut egui::Ui) {
        let s = &mut self.p.servidor;
        tarjeta(ui, "General", Some("Cómo se conectan los clientes al servidor"), |ui| {
            fila(ui, "Endpoint público", Some("IP pública o dominio"), |ui| campo(ui, &mut s.endpoint, ""));
            fila(ui, "Puerto UDP", None, |ui| {
                campo(ui, &mut s.puerto, "51820");
                aviso(ui, modelo::validar_puerto(&s.puerto));
            });
            fila(ui, "Interfaz WireGuard", None, |ui| campo(ui, &mut s.interfaz, "wg0"));
            fila(ui, "DNS para clientes", Some("Separados por comas; puede ser uno de la intranet"), |ui| {
                campo(ui, &mut s.dns, "1.1.1.1")
            });
            fila(ui, "MTU", Some("Opcional, entre 576 y 9000"), |ui| {
                campo(ui, &mut s.mtu, "automático");
                aviso(ui, modelo::validar_mtu(&s.mtu));
            });
        });
        ui.add_space(16.0);

        tarjeta(ui, "Red", None, |ui| {
            fila(ui, "Red VPN", Some("Rango de IPs del túnel"), |ui| campo(ui, &mut s.red_vpn, "10.8.0.0/24"));
            fila(ui, "IP del servidor", Some("Dentro de la red VPN"), |ui| campo(ui, &mut s.ip_servidor, "10.8.0.1"));
            fila(ui, "Interfaz de salida", Some("Hacia la LAN e internet: eth0, ens18…"), |ui| {
                campo(ui, &mut s.iface_salida, "eth0")
            });
        });
        ui.add_space(16.0);

        tarjeta(ui, "Seguridad", None, |ui| {
            fila(ui, "NAT (masquerade)", Some("Necesario para salir a internet; evita rutas en el router"), |ui| {
                interruptor(ui, &mut s.nat);
            });
            fila(
                ui,
                "Aplicar permisos en el servidor",
                Some("Reglas iptables por cliente; sin esto un cliente podría editar su .conf"),
                |ui| {
                    interruptor(ui, &mut s.firewall);
                },
            );
            fila(ui, "Tráfico entre clientes", Some("Permitir que los clientes VPN se vean entre sí"), |ui| {
                ui.add_enabled_ui(s.firewall, |ui| interruptor(ui, &mut s.entre_clientes));
            });
        });
        ui.add_space(16.0);

        let mut regenerado = false;
        tarjeta(ui, "Claves", Some("Puedes pegar la clave privada de un servidor existente"), |ui| {
            fila(ui, "Clave privada", None, |ui| {
                let r = ui.add_sized(
                    [ANCHO_CAMPO, ALTO_CAMPO],
                    egui::TextEdit::singleline(&mut s.clave_privada)
                        .password(!self.ver_privadas)
                        .margin(vec2(8.0, 4.0)).align(egui::Align2::LEFT_CENTER),
                );
                if r.changed()
                    && let Some(p) = claves::publica_de(&s.clave_privada)
                {
                    s.clave_publica = p;
                }
                ui.checkbox(&mut self.ver_privadas, "Mostrar");
            });
            fila(ui, "Clave pública", None, |ui| {
                ui.add_sized(
                    [ANCHO_CAMPO, ALTO_CAMPO],
                    egui::Label::new(RichText::new(&s.clave_publica).monospace().color(TEXTO)).truncate(),
                );
                if boton(ui, "Copiar").clicked() {
                    ui.ctx().copy_text(s.clave_publica.clone());
                }
            });
            fila(ui, "", None, |ui| {
                if boton(ui, "🔑  Regenerar claves").clicked() {
                    (s.clave_privada, s.clave_publica) = claves::generar_par();
                    regenerado = true;
                }
            });
        });
        if regenerado {
            self.estado = "Claves del servidor regeneradas: hay que redistribuir todas las configs de cliente".into();
        }
    }

    fn pag_intranet(&mut self, ui: &mut egui::Ui) {
        let mut borrar = None;
        let mut anadir = false;
        let clientes = self.p.clientes.clone();
        tarjeta_con_accion(
            ui,
            "Redes internas",
            Some("Redes que la VPN puede alcanzar. En cada cliente eliges a cuáles tiene acceso."),
            "+  Añadir subred",
            &mut anadir,
            |ui| {
                cabecera_tabla(ui, 0.0, &[("NOMBRE", 230.0), ("RED(ES) CIDR", 330.0), ("CLIENTES CON ACCESO", 170.0)]);
                for (i, s) in self.p.intranet.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        celda(ui, 230.0, |ui| {
                            ui.add_sized(
                                [218.0, ALTO_CAMPO],
                                egui::TextEdit::singleline(&mut s.nombre).margin(vec2(8.0, 4.0)).align(egui::Align2::LEFT_CENTER),
                            );
                        });
                        celda(ui, 330.0, |ui| {
                            ui.add_sized(
                                [318.0, ALTO_CAMPO],
                                egui::TextEdit::singleline(&mut s.cidr)
                                    .hint_text("192.168.1.0/24")
                                    .margin(vec2(8.0, 4.0)).align(egui::Align2::LEFT_CENTER),
                            );
                        });
                        let n = clientes.iter().filter(|c| c.toda_intranet || c.subredes.contains(&s.id)).count();
                        celda(ui, 170.0, |ui| {
                            insignia(ui, &n.to_string(), TEXTO2, FONDO);
                        });
                        if boton_icono(ui, "🗑").on_hover_text("Eliminar").clicked() {
                            borrar = Some(i);
                        }
                    });
                    separador(ui);
                }
                if self.p.intranet.is_empty() {
                    ui.label(RichText::new("No hay subredes definidas.").color(TEXTO2));
                }
                ui.add_space(4.0);
                ui.label(
                    RichText::new("Una fila puede tener varias redes separadas por comas (p. ej. una VLAN y su red de gestión).")
                        .small()
                        .color(TEXTO2),
                );
            },
        );
        if anadir {
            self.p.anadir_subred("Nueva subred", "");
        }
        if let Some(i) = borrar {
            let id = self.p.intranet.remove(i).id;
            for c in &mut self.p.clientes {
                c.subredes.retain(|x| *x != id);
            }
        }
    }

    fn pag_clientes(&mut self, ui: &mut egui::Ui) {
        let mut anadir = false;
        let mut elegido = None;
        tarjeta_con_accion(ui, "Clientes", Some("Pulsa un cliente para editarlo"), "+  Añadir cliente", &mut anadir, |ui| {
            cabecera_tabla(ui, 6.0, &[("NOMBRE", 220.0), ("IP VPN", 120.0), ("ACCESO", 360.0), ("SEGURIDAD", 100.0)]);
            for (i, c) in self.p.clientes.iter().enumerate() {
                let resp = fila_clicable(ui, ("cli", i), self.sel == Some(i), |ui| {
                    celda(ui, 220.0, |ui| {
                        avatar(ui, &c.nombre);
                        ui.label(RichText::new(&c.nombre).color(TEXTO).strong());
                    });
                    celda(ui, 120.0, |ui| {
                        ui.label(RichText::new(&c.ip).color(TEXTO));
                    });
                    celda(ui, 360.0, |ui| insignias_acceso(ui, c, &self.p));
                    celda(ui, 100.0, |ui| {
                        if c.usar_psk {
                            insignia(ui, "PSK", VERDE, VERDE_TINTE);
                        } else {
                            insignia(ui, "Sin PSK", TEXTO2, FONDO);
                        }
                    });
                });
                if resp.clicked() {
                    elegido = Some(i);
                }
            }
            if self.p.clientes.is_empty() {
                ui.label(RichText::new("Todavía no hay clientes.").color(TEXTO2));
            }
        });
        if anadir {
            self.sel = Some(self.p.anadir_cliente());
        }
        if let Some(i) = elegido {
            self.sel = if self.sel == Some(i) { None } else { Some(i) };
        }
    }

    /// Panel lateral derecho con el detalle del cliente seleccionado.
    fn detalle_cliente(&mut self, ui: &mut egui::Ui, i: usize) {
        let mut cerrar = false;
        let mut borrar = false;
        ui.add_space(16.0);
        ui.horizontal(|ui| {
            avatar(ui, &self.p.clientes[i].nombre);
            ui.label(RichText::new(&self.p.clientes[i].nombre).size(17.0).strong().color(TEXTO));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if boton_icono(ui, "✕").clicked() {
                    cerrar = true;
                }
            });
        });
        ui.add_space(8.0);
        separador(ui);

        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.add_space(8.0);
            let siguiente = self.p.siguiente_ip();
            let intranet = self.p.intranet.clone();
            let c = &mut self.p.clientes[i];

            seccion(ui, "GENERAL");
            etiqueta(ui, "Nombre");
            campo_ancho(ui, &mut c.nombre, "");
            etiqueta(ui, "IP en la VPN");
            ui.horizontal(|ui| {
                ui.add_sized(
                    [ui.available_width() - 70.0, ALTO_CAMPO],
                    egui::TextEdit::singleline(&mut c.ip).margin(vec2(8.0, 4.0)).align(egui::Align2::LEFT_CENTER),
                );
                if let Some(n) = siguiente
                    && boton(ui, "Auto").on_hover_text(format!("Usar {n}")).clicked()
                {
                    c.ip = n.to_string();
                }
            });
            etiqueta(ui, "Keepalive (segundos, 0 = desactivado)");
            ui.horizontal(|ui| {
                ui.add_sized(
                    [120.0, ALTO_CAMPO],
                    egui::TextEdit::singleline(&mut c.keepalive)
                        .hint_text("0")
                        .margin(vec2(8.0, 4.0))
                        .align(egui::Align2::LEFT_CENTER),
                );
                aviso(ui, modelo::validar_keepalive(&c.keepalive));
            });

            ui.add_space(12.0);
            seccion(ui, "ACCESO");
            opcion(ui, &mut c.internet, "Internet a través de la VPN", "Todo el tráfico sale por el servidor");
            opcion(ui, &mut c.toda_intranet, "Toda la intranet", "Todas las subredes definidas");
            if !c.toda_intranet {
                egui::Frame::new()
                    .fill(FONDO)
                    .corner_radius(6)
                    .inner_margin(egui::Margin::same(10))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(RichText::new("Subredes permitidas").small().color(TEXTO2));
                        if intranet.is_empty() {
                            ui.label(RichText::new("No hay subredes en la página Intranet").color(TEXTO2));
                        }
                        for s in &intranet {
                            let mut on = c.subredes.contains(&s.id);
                            let t = RichText::new(format!("{}   ", s.nombre)).color(TEXTO);
                            ui.horizontal(|ui| {
                                if ui.checkbox(&mut on, t).changed() {
                                    if on {
                                        c.subredes.push(s.id);
                                    } else {
                                        c.subredes.retain(|x| *x != s.id);
                                    }
                                }
                                ui.label(RichText::new(&s.cidr).small().color(TEXTO2));
                            });
                        }
                    });
                ui.add_space(6.0);
            }
            etiqueta(ui, "Redes o equipos adicionales");
            campo_ancho(ui, &mut c.extra, "192.168.5.20/32, 10.0.3.0/24");
            opcion(ui, &mut c.usar_dns, "Usar el DNS del servidor", "Sustituye el DNS del dispositivo mientras está conectado");

            ui.add_space(12.0);
            seccion(ui, "CLAVES");
            etiqueta(ui, "Clave pública");
            ui.add(egui::Label::new(RichText::new(&c.clave_publica).monospace().size(12.0).color(TEXTO)).truncate());
            opcion(ui, &mut c.usar_psk, "Clave precompartida (PSK)", "Capa extra de seguridad, recomendada");
            ui.horizontal(|ui| {
                if boton(ui, "Regenerar claves").clicked() {
                    (c.clave_privada, c.clave_publica) = claves::generar_par();
                }
                if c.usar_psk && boton(ui, "Regenerar PSK").clicked() {
                    c.psk = claves::generar_psk();
                }
            });

            ui.add_space(12.0);
            seccion(ui, "CONFIGURACIÓN");
            let c = &self.p.clientes[i];
            match self.p.config_cliente(c) {
                Ok(conf) => {
                    if let Some(tex) = qr_textura(ui.ctx(), &mut self.qr, &conf) {
                        ui.vertical_centered(|ui| {
                            egui::Frame::new()
                                .fill(Color32::WHITE)
                                .stroke(Stroke::new(1.0, BORDE))
                                .corner_radius(8)
                                .inner_margin(egui::Margin::same(8))
                                .show(ui, |ui| ui.image((tex.id(), vec2(230.0, 230.0))));
                            ui.label(RichText::new("Escanéalo con la app WireGuard").small().color(TEXTO2));
                        });
                    }
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if boton_primario(ui, "Copiar config").clicked() {
                            ui.ctx().copy_text(conf.clone());
                            self.estado = format!("Config de {} copiada", c.nombre);
                        }
                        if boton(ui, "Guardar .conf…").clicked()
                            && let Some(r) = rfd::FileDialog::new()
                                .set_file_name(format!("{}.conf", nombre_seguro(&c.nombre)))
                                .save_file()
                        {
                            self.estado = match escribir_privado(&r, &conf) {
                                Ok(()) => format!("Guardado {}", r.display()),
                                Err(e) => format!("Error: {e}"),
                            };
                        }
                    });
                    ui.add_space(6.0);
                    bloque_codigo(ui, &conf);
                }
                Err(e) => {
                    ui.label(RichText::new(format!("No se puede generar: {e}")).color(ROJO));
                }
            }

            ui.add_space(16.0);
            if boton_peligro(ui, "Eliminar cliente").clicked() {
                borrar = true;
            }
            ui.add_space(16.0);
        });

        if borrar {
            self.p.clientes.remove(i);
            self.sel = None;
        }
        if cerrar {
            self.sel = None;
        }
    }

    fn pag_conf_servidor(&mut self, ui: &mut egui::Ui) {
        let iface = nombre_seguro(&self.p.servidor.interfaz);
        match self.p.config_servidor() {
            Ok(conf) => {
                let mut copiar = false;
                tarjeta_con_accion(
                    ui,
                    &format!("{iface}.conf"),
                    Some(
                        "Requiere iptables. El filtrado afecta al tráfico reenviado (FORWARD); \
                         el acceso al propio servidor depende de su firewall de INPUT.",
                    ),
                    "Copiar",
                    &mut copiar,
                    |ui| bloque_codigo(ui, &conf),
                );
                if copiar {
                    ui.ctx().copy_text(conf);
                    self.estado = "Config del servidor copiada".into();
                }
            }
            Err(e) => {
                tarjeta(ui, "No se puede generar", None, |ui| {
                    ui.label(RichText::new(e).color(ROJO));
                });
            }
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let errores = self.p.validar();

        egui::Panel::left("nav")
            .exact_size(220.0)
            .resizable(false)
            .show_separator_line(false)
            .frame(
                egui::Frame::new()
                    .fill(SUPERFICIE)
                    .stroke(Stroke::new(1.0, BORDE))
                    .inner_margin(egui::Margin::symmetric(10, 0)),
            )
            .show(ui, |ui| self.barra_lateral(ui));

        egui::Panel::top("cabecera")
            .exact_size(60.0)
            .show_separator_line(false)
            .frame(
                egui::Frame::new()
                    .fill(SUPERFICIE)
                    .stroke(Stroke::new(1.0, BORDE))
                    .inner_margin(egui::Margin::symmetric(16, 0)),
            )
            .show(ui, |ui| self.cabecera(ui, &errores));

        if self.pagina == Pagina::Clientes
            && let Some(i) = self.sel.filter(|i| *i < self.p.clientes.len())
        {
            egui::Panel::right("detalle")
                .exact_size(440.0)
                .resizable(false)
                .show_separator_line(false)
                .frame(
                    egui::Frame::new()
                        .fill(SUPERFICIE)
                        .stroke(Stroke::new(1.0, BORDE))
                        .inner_margin(egui::Margin::symmetric(20, 0)),
                )
                .show(ui, |ui| self.detalle_cliente(ui, i));
        }

        egui::CentralPanel::no_frame().show(ui, |ui| {
            egui::Frame::new().fill(FONDO).inner_margin(egui::Margin::same(24)).show(ui, |ui| {
                ui.set_min_size(ui.available_size());
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    ui.set_max_width(1000.0);
                    match self.pagina {
                        Pagina::Resumen => self.pag_resumen(ui, &errores),
                        Pagina::Servidor => self.pag_servidor(ui),
                        Pagina::Intranet => self.pag_intranet(ui),
                        Pagina::Clientes => self.pag_clientes(ui),
                        Pagina::ConfServidor => self.pag_conf_servidor(ui),
                    }
                    ui.add_space(24.0);
                });
            });
        });
    }
}

/// Muestra junto a un campo el motivo por el que no es válido.
fn aviso<T>(ui: &mut egui::Ui, r: Result<T, String>) {
    if let Err(e) = r {
        let motivo = e.rsplit(": ").next().unwrap_or(&e).to_string();
        ui.label(RichText::new(format!("✖ {motivo}")).small().color(ROJO));
    }
}

/// Escribe un fichero con permisos 600 (contiene claves privadas).
fn escribir_privado(ruta: &Path, contenido: &str) -> std::io::Result<()> {
    std::fs::write(ruta, contenido)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(ruta, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

/// Genera (con caché) la textura del QR para el texto dado.
fn qr_textura<'a>(
    ctx: &egui::Context,
    cache: &'a mut Option<(String, egui::TextureHandle)>,
    texto: &str,
) -> Option<&'a egui::TextureHandle> {
    if cache.as_ref().is_none_or(|(t, _)| t != texto) {
        let qr = qrcode::QrCode::new(texto.as_bytes()).ok()?;
        let w = qr.width();
        let borde = 2;
        let lado = w + borde * 2;
        let colores = qr.to_colors();
        let mut img = egui::ColorImage::filled([lado, lado], Color32::WHITE);
        for y in 0..w {
            for x in 0..w {
                if colores[y * w + x] == qrcode::Color::Dark {
                    img[(x + borde, y + borde)] = Color32::BLACK;
                }
            }
        }
        let tex = ctx.load_texture("qr", img, egui::TextureOptions::NEAREST);
        *cache = Some((texto.to_string(), tex));
    }
    cache.as_ref().map(|(_, t)| t)
}

/// Emblema de WireGuard, 128×128 RGBA sin cabecera (generado desde el logo oficial).
const LOGO_RGBA: &[u8] = include_bytes!("../assets/wireguard-128.rgba");
const LOGO_LADO: usize = 128;

fn main() -> eframe::Result<()> {
    let opciones = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Configurador WireGuard")
            .with_inner_size([1280.0, 820.0])
            .with_min_inner_size([980.0, 600.0])
            .with_icon(egui::IconData { rgba: LOGO_RGBA.to_vec(), width: LOGO_LADO as u32, height: LOGO_LADO as u32 }),
        ..Default::default()
    };
    eframe::run_native(
        "wg-configurador",
        opciones,
        Box::new(|cc| {
            aplicar_tema(&cc.egui_ctx);
            let img = egui::ColorImage::from_rgba_unmultiplied([LOGO_LADO, LOGO_LADO], LOGO_RGBA);
            let logo = cc.egui_ctx.load_texture("logo", img, egui::TextureOptions::LINEAR);
            Ok(Box::new(App::new(logo)))
        }),
    )
}
