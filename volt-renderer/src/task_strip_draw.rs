use super::{BgVertex, GlyphVertex, Renderer, Theme};
use crate::task_strip::{TaskStripLayout, TaskStripView};

pub(super) struct StripGeometry {
    view: TaskStripView,
    size: (u32, u32),
    scale: f32,
    top: f32,
    theme: Theme,
    font: String,
    bg: Vec<BgVertex>,
    glyphs: Vec<GlyphVertex>,
}

impl Renderer {
    pub fn task_strip_layout(&self) -> Option<TaskStripLayout> {
        let view = self.task_strip.as_ref()?;
        TaskStripLayout::new(
            view,
            self.config.width as f32,
            self.workspace_card_top_offset(),
            self.scale_factor,
        )
    }

    pub(super) fn draw_task_strip(
        &mut self,
        bg: &mut Vec<BgVertex>,
        glyphs: &mut Vec<GlyphVertex>,
        view: &TaskStripView,
        theme: &Theme,
    ) {
        let size = (self.config.width, self.config.height);
        let top = self.workspace_card_top_offset();
        let hit = self.task_strip_cache.as_ref().is_some_and(|cache| {
            cache.view == *view
                && cache.size == size
                && cache.scale == self.scale_factor
                && cache.top == top
                && cache.theme == *theme
                && cache.font == self.font_family
        });
        if !hit {
            let mut strip_bg = Vec::new();
            let mut strip_glyphs = Vec::new();
            volt_ui_kit::strip_paint::StripPaint::build_task_strip(
                self,
                &mut strip_bg,
                &mut strip_glyphs,
                view,
                &super::ui_kit_theme(theme),
            );
            self.task_strip_cache = Some(StripGeometry {
                view: view.clone(),
                size,
                scale: self.scale_factor,
                top,
                theme: theme.clone(),
                font: self.font_family.clone(),
                bg: strip_bg,
                glyphs: strip_glyphs,
            });
        }
        if let Some(cache) = &self.task_strip_cache {
            bg.extend_from_slice(&cache.bg);
            glyphs.extend_from_slice(&cache.glyphs);
        }
    }
}
