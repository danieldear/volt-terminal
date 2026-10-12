//! Small, host-rendered bridge to the new toolkit controls. No native toolkit
//! host/device/timer is enabled. Settings keeps its existing input transaction.
use super::{resolve_family, BgVertex, GlyphVertex, Renderer, Theme};
use cosmic_text::{Attrs, Buffer, Metrics, Shaping};
use volt_ui_kit::declarative::{
    Bindings, Document, DocumentPainter, Handlers, Runtime, TextMetrics,
};
use volt_ui_kit::ui::{prelude::*, Element};
use volt_ui_kit::{Painter, Viewport};

struct ControlPainter<'a> {
    renderer: &'a mut Renderer,
    origin: [f32; 2],
    viewport: Viewport,
}
impl Painter for ControlPainter<'_> {
    type Shape = BgVertex;
    type Glyph = GlyphVertex;
    fn viewport(&self) -> Viewport {
        self.viewport
    }
    fn draw_rect(&self, out: &mut Vec<BgVertex>, x: f32, y: f32, w: f32, h: f32, color: [f32; 4]) {
        self.renderer
            .draw_rect(out, x + self.origin[0], y + self.origin[1], w, h, color);
    }
    fn card_triangle(&self, out: &mut Vec<BgVertex>, points: [[f32; 2]; 3], color: [f32; 4]) {
        self.renderer.card_triangle(
            out,
            points.map(|p| [p[0] + self.origin[0], p[1] + self.origin[1]]),
            color,
        );
    }
    fn card_text(
        &mut self,
        out: &mut Vec<GlyphVertex>,
        text: &str,
        x: f32,
        y: f32,
        size: f32,
        color: [f32; 4],
    ) {
        Painter::card_text(
            self.renderer,
            out,
            text,
            x + self.origin[0],
            y + self.origin[1],
            size,
            color,
        );
    }
    fn draw_text_colored(
        &mut self,
        out: &mut Vec<GlyphVertex>,
        text: &str,
        x: f32,
        y: f32,
        size: f32,
        line_height: f32,
        color_at: &dyn Fn(usize) -> [f32; 4],
    ) {
        self.renderer.draw_text_colored(
            out,
            text,
            x + self.origin[0],
            y + self.origin[1],
            size,
            line_height,
            color_at,
        );
    }
}
impl TextMetrics for ControlPainter<'_> {
    fn measure(&mut self, text: &str, size: f32, max_width: f32) -> [f32; 2] {
        let family = self.renderer.font_family.clone();
        let mut buffer = Buffer::new(
            &mut self.renderer.font_system,
            Metrics::new(size, size * 1.2),
        );
        buffer.set_size(&mut self.renderer.font_system, max_width.max(1.), size * 2.);
        buffer.set_text(
            &mut self.renderer.font_system,
            text,
            Attrs::new().family(resolve_family(&family)),
            Shaping::Advanced,
        );
        buffer.shape_until_scroll(&mut self.renderer.font_system, false);
        [
            buffer.layout_runs().map(|r| r.line_w).fold(0., f32::max),
            size * 1.2,
        ]
    }
}
impl DocumentPainter for ControlPainter<'_> {
    fn ui_path(
        &mut self,
        out: &mut Vec<BgVertex>,
        path: &volt_ui_kit::graphics::VectorPath,
        color: [f32; 4],
        stroke: Option<f32>,
    ) {
        use volt_ui_kit::graphics::PathCommand;
        let Some(width) = stroke.filter(|v| v.is_finite() && *v > 0.) else {
            return;
        };
        // Closed select triggers use a small stroked polyline chevron. Filled
        // or curved paths still require a general tessellation adapter.
        if path
            .commands
            .iter()
            .any(|c| !matches!(c, PathCommand::Move(_) | PathCommand::Line(_)))
        {
            return;
        }
        let mut cursor = None;
        for command in &path.commands {
            match command {
                PathCommand::Move(p) => cursor = Some(*p),
                PathCommand::Line(p) => {
                    if let Some(from) = cursor {
                        if from.into_iter().chain(*p).all(f32::is_finite) {
                            self.card_line(out, from, *p, width, color);
                        }
                    }
                    cursor = Some(*p);
                }
                _ => unreachable!(),
            }
        }
    }
    fn ui_text(
        &mut self,
        out: &mut Vec<GlyphVertex>,
        text: &str,
        bounds: [f32; 4],
        size: f32,
        color: [f32; 4],
    ) {
        // This narrow bridge hosts bounded closed controls and short labels. It is not a
        // general image/path/effects/text-editor adapter; do not mount arbitrary
        // documents here without adding matching clipping/compositing services.
        self.card_text(out, text, bounds[0], bounds[1], size, color);
    }
}
impl Renderer {
    /// Paint only. Volt still owns events and the settings transaction.
    fn paint_toolkit_element(
        &mut self,
        shapes: &mut Vec<BgVertex>,
        glyphs: &mut Vec<GlyphVertex>,
        rect: [f32; 4],
        scale: f32,
        element: Element<()>,
        theme: &Theme,
    ) -> bool {
        let result = (|| {
            let (document, callbacks) = element.build()?.into_parts();
            let values = Bindings::default();
            let mut runtime = Runtime::new(document, &callbacks, &values)?;
            runtime.set_reduced_motion(true);
            let viewport = Viewport::new(rect[2], rect[3], scale)
                .ok_or_else(|| anyhow::anyhow!("invalid control viewport"))?;
            let mut painter = ControlPainter {
                renderer: self,
                origin: [rect[0], rect[1]],
                viewport,
            };
            runtime.layout(viewport, &values, &mut painter)?;
            for batch in runtime.paint(&mut painter, &super::ui_kit_theme(theme)) {
                shapes.extend(batch.shapes);
                glyphs.extend(batch.glyphs);
            }
            Ok::<_, anyhow::Error>(())
        })();
        result.is_ok()
    }
    pub(super) fn draw_toolkit_select(
        &mut self,
        shapes: &mut Vec<BgVertex>,
        glyphs: &mut Vec<GlyphVertex>,
        rect: [f32; 4],
        scale: f32,
        label: &str,
        style: (bool, &Theme),
    ) -> bool {
        let (focused, theme) = style;
        let c = volt_ui_kit::style::Colors::from_theme(&super::ui_kit_theme(theme));
        // Only the closed trigger is mounted. The host's list supports all
        // installed fonts, custom values and its own scrolling/input rules.
        let element = select::<(), _>([select_item(0u8, label)])
            .key("settings-select")
            .selected(0)
            .width(rect[2] / scale)
            .height(rect[3] / scale)
            .font_family("monospace")
            .font_size(12.)
            .background(hex(c.panel))
            .color(hex(c.text))
            .border_color(hex(if focused { c.accent } else { c.border }))
            .border_width(1.)
            .rounded(6.)
            .on_select(|_, _| {})
            .on_open_change(|_, _| {})
            .into();
        self.paint_toolkit_element(shapes, glyphs, rect, scale, element, theme)
    }
    pub(super) fn draw_toolkit_slider(
        &mut self,
        shapes: &mut Vec<BgVertex>,
        glyphs: &mut Vec<GlyphVertex>,
        rect: [f32; 4],
        scale: f32,
        range: [f32; 3],
        theme: &Theme,
    ) -> bool {
        let c = volt_ui_kit::style::Colors::from_theme(&super::ui_kit_theme(theme));
        let element = slider::<()>(range[0])
            .range(range[1]..=range[2])
            .step(0.01)
            .key("settings-slider")
            .width(rect[2] / scale)
            .height(rect[3] / scale)
            .background(hex(c.border))
            .color(hex(c.text))
            .accent_color(hex(c.accent))
            .on_change(|_, _| {})
            .into();
        self.paint_toolkit_element(shapes, glyphs, rect, scale, element, theme)
    }
    pub(super) fn draw_toolkit_toggle(
        &mut self,
        shapes: &mut Vec<BgVertex>,
        glyphs: &mut Vec<GlyphVertex>,
        origin: [f32; 2],
        scale: f32,
        checked: bool,
        theme: &Theme,
    ) {
        let mut handlers = Handlers::<()>::default();
        // The toolkit paints; Volt's existing row hit/keyboard handlers change
        // the real draft. No renderer callback writes configuration or inputs.
        let result = (|| {
            handlers.on_toggle_changed("toggle", |_, _| {})?;
            let document = Document::parse(&format!(
                r#"{{"schema_version":1,"root":{{"type":"toggle","id":"switch","label":"","checked":{checked},"on_toggle_changed":"toggle","style":{{"font_size":13,"font_family":"monospace"}}}}}}"#
            ))?;
            let values = Bindings::default();
            let mut runtime = Runtime::new(document, &handlers, &values)?;
            runtime.set_reduced_motion(true);
            let viewport = Viewport::new(44. * scale, 22. * scale, scale)
                .ok_or_else(|| anyhow::anyhow!("invalid control viewport"))?;
            let mut painter = ControlPainter {
                renderer: self,
                origin,
                viewport,
            };
            runtime.layout(viewport, &values, &mut painter)?;
            for batch in runtime.paint(&mut painter, &super::ui_kit_theme(theme)) {
                shapes.extend(batch.shapes);
                glyphs.extend(batch.glyphs);
            }
            Ok::<_, anyhow::Error>(())
        })();
        if result.is_err() {
            // Unsupported extreme backing scales keep a usable legacy value.
            self.card_text(
                glyphs,
                if checked { "On" } else { "Off" },
                origin[0],
                origin[1],
                13. * scale,
                theme.foreground.to_f32(),
            );
        }
    }
}

fn hex(color: [f32; 4]) -> String {
    let c = color.map(|v| (v.clamp(0., 1.) * 255.).round() as u8);
    format!("#{:02x}{:02x}{:02x}{:02x}", c[0], c[1], c[2], c[3])
}
