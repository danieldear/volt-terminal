//! Compatibility export; implementation lives in volt-ui-kit.
pub use volt_ui_kit::settings::*;

/// Host-side typed controls; no configuration or callbacks live in the renderer.
#[derive(Clone, Debug, PartialEq)]
pub enum SettingsControl {
    Switch(bool),
    Choice,
    Number,
    Text,
    Slider { value: f32, min: f32, max: f32 },
    Action(String),
    Shortcut,
    Info,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SettingsDropdown {
    pub row: usize,
    pub items: Vec<String>,
    pub selected: usize,
    pub current: usize,
    pub offset: usize,
}
pub const DROPDOWN_ROWS: usize = 6;

/// The same geometry is used by paint and pointer handling.
pub fn control_rect(l: &SettingsLayout, row: usize) -> [f32; 4] {
    let r = l.row(row);
    let w = (r[2] * 0.48).min(230. * l.scale);
    [
        r[0] + r[2] - w - 10. * l.scale,
        r[1] + (r[3] - 32. * l.scale) * 0.5,
        w,
        32. * l.scale,
    ]
}
pub fn slider_track(l: &SettingsLayout, row: usize) -> [f32; 4] {
    let r = control_rect(l, row);
    [
        r[0] + 8. * l.scale,
        r[1] + 14. * l.scale,
        r[2] - 65. * l.scale,
        4. * l.scale,
    ]
}
pub fn dropdown_rect(l: &SettingsLayout, d: &SettingsDropdown) -> [f32; 4] {
    let r = control_rect(l, d.row);
    let count = d.items.len().saturating_sub(d.offset).min(DROPDOWN_ROWS);
    let h = (count as f32 * 28. + 8.) * l.scale;
    let bottom = l.y + l.h - 90. * l.scale;
    let below = r[1] + r[3] + 4. * l.scale;
    let y = if below + h <= bottom {
        below
    } else {
        r[1] - h - 4. * l.scale
    };
    [r[0], y.clamp(l.y + 70. * l.scale, bottom - h), r[2], h]
}
pub fn inside(r: [f32; 4], x: f32, y: f32) -> bool {
    x.is_finite() && y.is_finite() && x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3]
}
