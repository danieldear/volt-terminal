use super::{BgVertex, GlyphVertex, Renderer, Theme};
use crate::search_palette::{SearchLayout, SearchView};
use volt_ui_kit::Colors;

pub(super) struct SearchGeometry {
    view: SearchView,
    layout: SearchLayout,
    colors: Colors,
    bg: Vec<BgVertex>,
    glyphs: Vec<GlyphVertex>,
}

impl Renderer {
    pub fn search_layout(&self) -> Option<SearchLayout> {
        let view = self.search_palette.as_ref()?;
        SearchLayout::new(
            self.config.width as f32,
            self.config.height as f32,
            self.workspace_card_top_offset(),
            self.scale_factor,
            view.rows.len(),
        )
    }

    pub(super) fn draw_search_palette(
        &mut self,
        bg: &mut Vec<BgVertex>,
        glyphs: &mut Vec<GlyphVertex>,
        view: &SearchView,
        theme: &Theme,
    ) {
        let colors = Colors::from_theme(&super::ui_kit_theme(theme));
        // Same layout function the click hit-test uses, so they can't drift.
        let Some(layout) = self.search_layout() else {
            return;
        };
        let cached = self
            .search_palette_cache
            .as_ref()
            .is_some_and(|c| c.view == *view && c.layout == layout && c.colors == colors);
        if !cached {
            let mut b = vec![];
            let mut g = vec![];
            volt_ui_kit::search_paint::SearchPaint::build_search_palette(
                self, &mut b, &mut g, view, &layout, &colors,
            );
            self.search_palette_cache = Some(SearchGeometry {
                view: view.clone(),
                layout,
                colors,
                bg: b,
                glyphs: g,
            });
        }
        if let Some(c) = &self.search_palette_cache {
            bg.extend_from_slice(&c.bg);
            glyphs.extend_from_slice(&c.glyphs);
        }
    }
}
