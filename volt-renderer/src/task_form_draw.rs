use super::{BgVertex, GlyphVertex, Renderer, Theme};
use crate::task_form::{TaskFormLayout, TaskFormView};
use volt_ui_kit::Colors;

pub(super) struct FormGeometry {
    view: TaskFormView,
    layout: TaskFormLayout,
    colors: Colors,
    bg: Vec<BgVertex>,
    glyphs: Vec<GlyphVertex>,
}

impl Renderer {
    pub fn task_form_layout(&self) -> Option<TaskFormLayout> {
        self.task_form.as_ref()?;
        TaskFormLayout::new(
            self.config.width as f32,
            self.config.height as f32,
            self.workspace_card_top_offset(),
            self.scale_factor,
        )
    }

    pub(super) fn draw_task_form(
        &mut self,
        bg: &mut Vec<BgVertex>,
        glyphs: &mut Vec<GlyphVertex>,
        view: &TaskFormView,
        theme: &Theme,
    ) {
        let Some(layout) = self.task_form_layout() else {
            return;
        };
        let colors = Colors::from_theme(&super::ui_kit_theme(theme));
        let cached = self
            .task_form_cache
            .as_ref()
            .is_some_and(|c| c.view == *view && c.layout == layout && c.colors == colors);
        if !cached {
            let mut b = vec![];
            let mut g = vec![];
            let error = theme.ansi[1].to_f32();
            let ok = theme.ansi[2].to_f32();
            volt_ui_kit::form_paint::FormPaint::build_task_form(
                self,
                &mut b,
                &mut g,
                view,
                &layout,
                &colors,
                (error, ok),
            );
            self.task_form_cache = Some(FormGeometry {
                view: view.clone(),
                layout,
                colors,
                bg: b,
                glyphs: g,
            });
        }
        if let Some(c) = &self.task_form_cache {
            bg.extend_from_slice(&c.bg);
            glyphs.extend_from_slice(&c.glyphs);
        }
    }
}
