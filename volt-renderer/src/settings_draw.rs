use super::{BgVertex, GlyphVertex, Renderer, Theme};
use crate::settings::{SettingsLayout, SettingsView};
pub(super) struct SettingsGeometry {
    view: SettingsView,
    layout: SettingsLayout,
    theme: Theme,
    bg: Vec<BgVertex>,
    glyphs: Vec<GlyphVertex>,
}
impl Renderer {
    pub fn installed_font_families(&self) -> Vec<String> {
        let mut names = vec!["monospace".to_string()];
        for f in self.font_system.db().faces() {
            if !f.monospaced {
                continue;
            }
            for (n, _) in &f.families {
                if !n.chars().any(char::is_control) {
                    names.push(n.clone());
                }
            }
        }
        names.sort();
        names.dedup();
        names
    }
    pub fn settings_layout(&self) -> Option<SettingsLayout> {
        self.settings.as_ref()?;
        SettingsLayout::new(
            self.config.width as f32,
            self.config.height as f32,
            self.workspace_card_top_offset(),
            self.scale_factor,
        )
    }
    pub(super) fn draw_settings(
        &mut self,
        bg: &mut Vec<BgVertex>,
        glyphs: &mut Vec<GlyphVertex>,
        view: &SettingsView,
        theme: &Theme,
    ) {
        let Some(l) = self.settings_layout() else {
            return;
        };
        if !self
            .settings_cache
            .as_ref()
            .is_some_and(|c| c.view == *view && c.layout == l && c.theme == *theme)
        {
            let mut b = vec![];
            let mut g = vec![];
            volt_ui_kit::settings_paint::SettingsPaint::build_settings(
                self,
                &mut b,
                &mut g,
                view,
                &l,
                &super::ui_kit_theme(theme),
            );
            self.settings_cache = Some(SettingsGeometry {
                view: view.clone(),
                layout: l,
                theme: theme.clone(),
                bg: b,
                glyphs: g,
            });
        }
        if let Some(c) = &self.settings_cache {
            bg.extend_from_slice(&c.bg);
            glyphs.extend_from_slice(&c.glyphs);
        }
    }
}
