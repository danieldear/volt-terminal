use super::{BgVertex, GlyphVertex, Renderer, Theme};
use crate::workspace_card::{CardLayout, WorkspaceCard};

pub(super) struct CardGeometry {
    card: WorkspaceCard,
    size: (u32, u32),
    scale: f32,
    top: f32,
    colors: ([f32; 4], [f32; 4]),
    font: String,
    bg: Vec<BgVertex>,
    glyphs: Vec<GlyphVertex>,
}
impl Renderer {
    pub(super) fn draw_workspace_card(
        &mut self,
        bg: &mut Vec<BgVertex>,
        glyphs: &mut Vec<GlyphVertex>,
        card: &WorkspaceCard,
        theme: &Theme,
        top: f32,
    ) {
        let colors = (theme.background.to_f32(), theme.foreground.to_f32());
        let size = (self.config.width, self.config.height);
        let hit = self.workspace_card_cache.as_ref().is_some_and(|cache| {
            cache.card == *card
                && cache.size == size
                && cache.scale == self.scale_factor
                && cache.top == top
                && cache.colors == colors
                && cache.font == self.font_family
        });
        if !hit {
            let mut card_bg = Vec::new();
            let mut card_glyphs = Vec::new();
            volt_ui_kit::card_paint::CardPaint::build_workspace_card(
                self,
                &mut card_bg,
                &mut card_glyphs,
                card,
                &super::ui_kit_theme(theme),
                top,
            );
            self.workspace_card_cache = Some(CardGeometry {
                card: card.clone(),
                size,
                scale: self.scale_factor,
                top,
                colors,
                font: self.font_family.clone(),
                bg: card_bg,
                glyphs: card_glyphs,
            });
        }
        if let Some(cache) = &self.workspace_card_cache {
            bg.extend_from_slice(&cache.bg);
            glyphs.extend_from_slice(&cache.glyphs);
        }
    }

    pub(super) fn card_text(
        &mut self,
        glyphs: &mut Vec<GlyphVertex>,
        text: &str,
        x: f32,
        y: f32,
        size: f32,
        color: [f32; 4],
    ) {
        self.draw_text_with_line_height(glyphs, text, x, y, size, 1.2, color);
    }

    pub fn workspace_card_layout(&self) -> Option<CardLayout> {
        let card = self.workspace_card.as_ref()?;
        CardLayout::for_card(
            self.config.width as f32,
            self.config.height as f32,
            self.workspace_card_top_offset(),
            self.scale_factor,
            card,
        )
    }
    pub fn workspace_card_reserved_width(&self) -> f32 {
        if self.workspace_card.as_ref().is_some_and(|c| !c.floating)
            && self.workspace_card_layout().is_some()
        {
            340.0 * self.scale_factor
        } else {
            0.0
        }
    }
    pub fn workspace_card_top_offset(&self) -> f32 {
        self.top_offset_for_tab_count(1) + self.alert_bar_height()
    }
    pub(super) fn card_round(
        &self,
        bg: &mut Vec<BgVertex>,
        rect: [f32; 4],
        radius: f32,
        color: [f32; 4],
    ) {
        volt_ui_kit::paint::Painter::card_round(self, bg, rect, radius, color);
    }
}
