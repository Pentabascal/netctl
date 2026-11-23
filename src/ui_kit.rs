//! Tema oscuro de estilo "consola de red" (tarjetas blancas, acento azul) y widgets reutilizables.

use eframe::egui::{
    self, Align, Align2, Color32, CornerRadius, CursorIcon, FontFamily, FontId, Label, Layout, Rect,
    Response, RichText, Sense, Shape, Stroke, TextStyle, Ui, pos2, vec2,
};

use crate::modelo::{Cliente, Proyecto};

pub const AZUL: Color32 = Color32::from_rgb(64, 150, 255);
pub const AZUL_TINTE: Color32 = Color32::from_rgb(22, 46, 82);
pub const MORADO: Color32 = Color32::from_rgb(175, 145, 255);
pub const MORADO_TINTE: Color32 = Color32::from_rgb(46, 36, 82);
pub const FONDO: Color32 = Color32::from_rgb(20, 22, 25);
pub const HOVER: Color32 = Color32::from_rgb(38, 42, 48);
pub const BORDE: Color32 = Color32::from_rgb(46, 50, 56);
pub const BORDE_FUERTE: Color32 = Color32::from_rgb(70, 76, 84);
pub const TEXTO: Color32 = Color32::from_rgb(228, 231, 235);
pub const TEXTO2: Color32 = Color32::from_rgb(140, 148, 160);
pub const VERDE: Color32 = Color32::from_rgb(70, 205, 125);
pub const VERDE_TINTE: Color32 = Color32::from_rgb(20, 54, 36);
pub const ROJO: Color32 = Color32::from_rgb(255, 95, 100);
pub const ROJO_TINTE: Color32 = Color32::from_rgb(72, 26, 30);
pub const SUPERFICIE: Color32 = Color32::from_rgb(29, 32, 36);
pub const CAMPO: Color32 = Color32::from_rgb(22, 24, 28);
pub const CODIGO: Color32 = Color32::from_rgb(16, 18, 21);

pub const ALTO_CAMPO: f32 = 32.0;
pub const ANCHO_CAMPO: f32 = 360.0;
pub const ANCHO_ETIQ: f32 = 260.0;
const ALTO_FILA: f32 = 46.0;

pub fn aplicar_tema(ctx: &egui::Context) {
    ctx.set_theme(egui::ThemePreference::Dark);
    ctx.all_styles_mut(|s| {
        s.visuals = egui::Visuals::dark();
        s.text_styles = [
            (TextStyle::Heading, FontId::new(20.0, FontFamily::Proportional)),
            (TextStyle::Body, FontId::new(14.0, FontFamily::Proportional)),
            (TextStyle::Button, FontId::new(14.0, FontFamily::Proportional)),
            (TextStyle::Small, FontId::new(12.0, FontFamily::Proportional)),
            (TextStyle::Monospace, FontId::new(13.0, FontFamily::Monospace)),
        ]
        .into();
        s.spacing.item_spacing = vec2(8.0, 8.0);
        s.spacing.button_padding = vec2(14.0, 6.0);
        s.spacing.interact_size.y = 30.0;
        s.spacing.icon_width = 18.0;

        let v = &mut s.visuals;
        v.panel_fill = FONDO;
        v.window_fill = SUPERFICIE;
        v.extreme_bg_color = CAMPO;
        v.faint_bg_color = FONDO;
        v.hyperlink_color = AZUL;
        v.selection.bg_fill = AZUL_TINTE;
        v.selection.stroke = Stroke::new(1.0, AZUL);
        v.window_corner_radius = CornerRadius::same(8);
        v.menu_corner_radius = CornerRadius::same(8);
        v.window_stroke = Stroke::new(1.0, BORDE);

        let w = &mut v.widgets;
        for x in [&mut w.noninteractive, &mut w.inactive, &mut w.hovered, &mut w.active, &mut w.open] {
            x.corner_radius = CornerRadius::same(6);
            x.expansion = 0.0;
        }
        w.noninteractive.bg_stroke = Stroke::new(1.0, BORDE);
        w.noninteractive.fg_stroke = Stroke::new(1.0, TEXTO);
        w.inactive.bg_fill = CAMPO;
        w.inactive.weak_bg_fill = SUPERFICIE;
        w.inactive.bg_stroke = Stroke::new(1.0, BORDE_FUERTE);
        w.inactive.fg_stroke = Stroke::new(1.0, TEXTO);
        w.hovered.bg_fill = HOVER;
        w.hovered.weak_bg_fill = HOVER;
        w.hovered.bg_stroke = Stroke::new(1.0, TEXTO2);
        w.hovered.fg_stroke = Stroke::new(1.5, TEXTO);
        w.active.bg_fill = AZUL_TINTE;
        w.active.weak_bg_fill = AZUL_TINTE;
        w.active.bg_stroke = Stroke::new(1.0, AZUL);
        w.active.fg_stroke = Stroke::new(1.5, AZUL);
        w.open.weak_bg_fill = AZUL_TINTE;
        w.open.bg_stroke = Stroke::new(1.0, AZUL);
    });
}

// ---------------------------------------------------------------- navegación

pub fn item_nav(ui: &mut Ui, icono: &str, texto: &str, sel: bool, contador: Option<usize>) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 38.0), Sense::click());
    let fondo = if sel {
        AZUL_TINTE
    } else if resp.hovered() {
        HOVER
    } else {
        Color32::TRANSPARENT
    };
    let p = ui.painter();
    p.rect_filled(rect, 6, fondo);
    let color = if sel { AZUL } else { TEXTO };
    let icol = if sel { AZUL } else { TEXTO2 };
    p.text(rect.left_center() + vec2(14.0, 0.0), Align2::LEFT_CENTER, icono, FontId::proportional(16.0), icol);
    p.text(rect.left_center() + vec2(42.0, 0.0), Align2::LEFT_CENTER, texto, FontId::proportional(14.0), color);
    if let Some(n) = contador {
        p.text(
            rect.right_center() - vec2(12.0, 0.0),
            Align2::RIGHT_CENTER,
            n.to_string(),
            FontId::proportional(12.0),
            if sel { AZUL } else { TEXTO2 },
        );
    }
    resp.on_hover_cursor(CursorIcon::PointingHand)
}

// ---------------------------------------------------------------- botones

pub fn boton_primario(ui: &mut Ui, texto: &str) -> Response {
    ui.add(
        egui::Button::new(RichText::new(texto).color(Color32::WHITE).strong())
            .fill(AZUL)
            .stroke(Stroke::NONE)
            .min_size(vec2(0.0, ALTO_CAMPO)),
    )
    .on_hover_cursor(CursorIcon::PointingHand)
}

pub fn boton(ui: &mut Ui, texto: &str) -> Response {
    ui.add(egui::Button::new(RichText::new(texto).color(TEXTO)).min_size(vec2(0.0, ALTO_CAMPO)))
        .on_hover_cursor(CursorIcon::PointingHand)
}

pub fn boton_icono(ui: &mut Ui, icono: &str) -> Response {
    ui.add(
        egui::Button::new(RichText::new(icono).size(15.0).color(TEXTO2))
            .frame_when_inactive(false)
            .min_size(vec2(30.0, 30.0)),
    )
    .on_hover_cursor(CursorIcon::PointingHand)
}

pub fn boton_peligro(ui: &mut Ui, texto: &str) -> Response {
    ui.add(
        egui::Button::new(RichText::new(texto).color(ROJO))
            .fill(SUPERFICIE)
            .stroke(Stroke::new(1.0, ROJO))
            .min_size(vec2(0.0, ALTO_CAMPO)),
    )
    .on_hover_cursor(CursorIcon::PointingHand)
}

// ---------------------------------------------------------------- tarjetas

fn marco_tarjeta() -> egui::Frame {
    egui::Frame::new()
        .fill(SUPERFICIE)
        .stroke(Stroke::new(1.0, BORDE))
        .corner_radius(8)
        .inner_margin(egui::Margin::same(20))
}

fn titulo_tarjeta(ui: &mut Ui, titulo: &str, sub: Option<&str>) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 2.0;
        ui.label(RichText::new(titulo).size(16.0).strong().color(TEXTO));
        if let Some(s) = sub {
            ui.label(RichText::new(s).small().color(TEXTO2));
        }
    });
}

pub fn tarjeta<R>(ui: &mut Ui, titulo: &str, sub: Option<&str>, f: impl FnOnce(&mut Ui) -> R) -> R {
    marco_tarjeta()
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            titulo_tarjeta(ui, titulo, sub);
            ui.add_space(10.0);
            f(ui)
        })
        .inner
}

/// Tarjeta con un botón primario a la derecha del título.
pub fn tarjeta_con_accion<R>(
    ui: &mut Ui,
    titulo: &str,
    sub: Option<&str>,
    accion: &str,
    pulsado: &mut bool,
    f: impl FnOnce(&mut Ui) -> R,
) -> R {
    marco_tarjeta()
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                titulo_tarjeta(ui, titulo, sub);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if boton_primario(ui, accion).clicked() {
                        *pulsado = true;
                    }
                });
            });
            ui.add_space(10.0);
            f(ui)
        })
        .inner
}

pub fn tarjeta_dato(ui: &mut Ui, titulo: &str, valor: &str, sub: &str) {
    marco_tarjeta().inner_margin(egui::Margin::same(16)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing.y = 4.0;
        ui.label(RichText::new(titulo).small().color(TEXTO2));
        ui.add(Label::new(RichText::new(valor).size(22.0).strong().color(TEXTO)).truncate());
        ui.add(Label::new(RichText::new(sub).small().color(TEXTO2)).truncate());
    });
}

// ---------------------------------------------------------------- formularios

/// Fila de formulario: etiqueta (y ayuda) a la izquierda, control a la derecha.
pub fn fila<R>(ui: &mut Ui, titulo: &str, ayuda: Option<&str>, contenido: impl FnOnce(&mut Ui) -> R) {
    ui.horizontal(|ui| {
        ui.set_min_height(ALTO_CAMPO + 12.0);
        ui.allocate_ui_with_layout(vec2(ANCHO_ETIQ, ALTO_CAMPO), Layout::top_down(Align::Min), |ui| {
            ui.set_width(ANCHO_ETIQ);
            ui.spacing_mut().item_spacing.y = 1.0;
            ui.label(RichText::new(titulo).color(TEXTO));
            if let Some(a) = ayuda {
                ui.add(Label::new(RichText::new(a).small().color(TEXTO2)).wrap());
            }
        });
        contenido(ui);
    });
}

pub fn campo(ui: &mut Ui, texto: &mut String, pista: &str) -> Response {
    ui.add_sized(
        [ANCHO_CAMPO, ALTO_CAMPO],
        egui::TextEdit::singleline(texto).hint_text(pista).margin(vec2(8.0, 4.0)).align(egui::Align2::LEFT_CENTER),
    )
}

pub fn campo_ancho(ui: &mut Ui, texto: &mut String, pista: &str) -> Response {
    ui.add_sized(
        [ui.available_width(), ALTO_CAMPO],
        egui::TextEdit::singleline(texto).hint_text(pista).margin(vec2(8.0, 4.0)).align(egui::Align2::LEFT_CENTER),
    )
}

pub fn etiqueta(ui: &mut Ui, t: &str) {
    ui.add_space(2.0);
    ui.label(RichText::new(t).small().color(TEXTO2));
}

pub fn seccion(ui: &mut Ui, t: &str) {
    ui.label(RichText::new(t).size(11.5).strong().color(TEXTO2));
}

pub fn separador(ui: &mut Ui) {
    let r = ui.available_rect_before_wrap();
    ui.painter().hline(r.x_range(), r.top(), Stroke::new(1.0, BORDE));
    ui.add_space(1.0);
}

/// Interruptor deslizante.
pub fn interruptor(ui: &mut Ui, on: &mut bool) -> Response {
    let (rect, mut resp) = ui.allocate_exact_size(vec2(38.0, 22.0), Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    if ui.is_rect_visible(rect) {
        let t = ui.ctx().animate_bool(resp.id, *on);
        let activo = ui.is_enabled();
        let mut fondo = if *on { AZUL } else { BORDE_FUERTE };
        if !activo {
            fondo = fondo.gamma_multiply(0.4);
        }
        ui.painter().rect_filled(rect, 11, fondo);
        let x = egui::lerp((rect.left() + 11.0)..=(rect.right() - 11.0), t);
        ui.painter().circle_filled(pos2(x, rect.center().y), 8.5, Color32::WHITE);
    }
    resp.on_hover_cursor(CursorIcon::PointingHand)
}

/// Interruptor con título y descripción a su derecha.
pub fn opcion(ui: &mut Ui, on: &mut bool, titulo: &str, desc: &str) {
    ui.horizontal(|ui| {
        ui.set_min_height(42.0);
        interruptor(ui, on);
        ui.add_space(4.0);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 1.0;
            ui.label(RichText::new(titulo).color(TEXTO));
            ui.add(Label::new(RichText::new(desc).small().color(TEXTO2)).wrap());
        });
    });
}

pub fn insignia(ui: &mut Ui, texto: &str, color: Color32, fondo: Color32) -> Response {
    let galley = ui.painter().layout_no_wrap(texto.to_string(), FontId::proportional(12.0), color);
    let (rect, resp) = ui.allocate_exact_size(galley.size() + vec2(16.0, 6.0), Sense::hover());
    ui.painter().rect_filled(rect, 10, fondo);
    ui.painter().galley(rect.min + vec2(8.0, 3.0), galley, color);
    resp
}

pub fn bloque_codigo(ui: &mut Ui, texto: &str) {
    egui::Frame::new()
        .fill(CODIGO)
        .stroke(Stroke::new(1.0, BORDE))
        .corner_radius(6)
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            egui::ScrollArea::horizontal().id_salt(ui.next_auto_id()).show(ui, |ui| {
                ui.add(
                    Label::new(RichText::new(texto).monospace().size(12.5).color(TEXTO))
                        .extend()
                        .selectable(true),
                );
            });
        });
}

// ---------------------------------------------------------------- tablas

pub fn celda<R>(ui: &mut Ui, ancho: f32, f: impl FnOnce(&mut Ui) -> R) -> R {
    ui.allocate_ui_with_layout(vec2(ancho, ALTO_FILA), Layout::left_to_right(Align::Center), |ui| {
        ui.set_width(ancho);
        ui.set_min_height(ALTO_FILA);
        f(ui)
    })
    .inner
}

/// `sangria`: 6.0 si las filas son `fila_clicable`, 0.0 si no.
pub fn cabecera_tabla(ui: &mut Ui, sangria: f32, columnas: &[(&str, f32)]) {
    ui.horizontal(|ui| {
        ui.add_space(sangria);
        for (t, w) in columnas {
            ui.allocate_ui_with_layout(vec2(*w, 22.0), Layout::left_to_right(Align::Center), |ui| {
                ui.set_width(*w);
                ui.label(RichText::new(*t).size(11.5).strong().color(TEXTO2));
            });
        }
    });
    separador(ui);
}

/// Fila de tabla clicable con resaltado al pasar el ratón / seleccionada.
pub fn fila_clicable(ui: &mut Ui, id: impl std::hash::Hash + std::fmt::Debug, sel: bool, f: impl FnOnce(&mut Ui)) -> Response {
    let fondo = ui.painter().add(Shape::Noop);
    let r = ui
        .horizontal(|ui| {
            ui.set_min_height(ALTO_FILA);
            ui.add_space(6.0);
            f(ui);
        })
        .response;
    let rect = Rect::from_x_y_ranges(ui.max_rect().x_range(), r.rect.y_range());
    let resp = ui.interact(rect, ui.scope_id().with(egui::IdSalt::new(id)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
    let color = if sel {
        AZUL_TINTE
    } else if resp.hovered() {
        HOVER
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().set(fondo, Shape::rect_filled(rect, 6, color));
    if !sel {
        ui.painter().hline(rect.x_range(), rect.bottom() + 2.0, Stroke::new(1.0, BORDE));
    }
    resp
}

pub fn avatar(ui: &mut Ui, nombre: &str) {
    let (r, _) = ui.allocate_exact_size(vec2(28.0, 28.0), Sense::hover());
    ui.painter().circle_filled(r.center(), 14.0, Color32::from_rgb(30, 62, 110));
    let ini: String = nombre.trim().chars().next().unwrap_or('?').to_uppercase().collect();
    ui.painter().text(r.center(), Align2::CENTER_CENTER, ini, FontId::proportional(13.0), AZUL);
}

pub fn insignias_acceso(ui: &mut Ui, c: &Cliente, p: &Proyecto) {
    let mut n = 0;
    if c.internet {
        insignia(ui, "🌐 Internet", AZUL, AZUL_TINTE);
        n += 1;
    }
    if c.toda_intranet {
        insignia(ui, "Toda la intranet", MORADO, MORADO_TINTE);
        n += 1;
    } else {
        let subs: Vec<_> = p.intranet.iter().filter(|s| c.subredes.contains(&s.id)).collect();
        for s in subs.iter().take(2) {
            insignia(ui, &s.nombre, TEXTO, FONDO);
            n += 1;
        }
        if subs.len() > 2 {
            insignia(ui, &format!("+{}", subs.len() - 2), TEXTO2, FONDO);
        }
    }
    if !c.extra.trim().is_empty() {
        insignia(ui, "+ extra", TEXTO2, FONDO);
        n += 1;
    }
    if n == 0 {
        insignia(ui, "Solo VPN", TEXTO2, FONDO);
    }
}

pub fn fila_cliente_mini(ui: &mut Ui, i: usize, c: &Cliente, p: &Proyecto) -> Response {
    fila_clicable(ui, ("mini", i), false, |ui| {
        celda(ui, 220.0, |ui| {
            avatar(ui, &c.nombre);
            ui.label(RichText::new(&c.nombre).color(TEXTO).strong());
        });
        celda(ui, 120.0, |ui| ui.label(RichText::new(&c.ip).color(TEXTO2)));
        celda(ui, 420.0, |ui| insignias_acceso(ui, c, p));
    })
}
