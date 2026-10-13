//! Task approval presentation. Volt owns permissions; this module only lays out
//! and paints a read-only, scrollable review with shared toolkit components.
use unicode_width::UnicodeWidthChar;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReviewMode {
    RunOnce,
    Confirm,
    TrustFile,
}
impl ReviewMode {
    pub fn title(self) -> &'static str {
        match self {
            Self::RunOnce => "Review task",
            Self::Confirm => "Run task?",
            Self::TrustFile => "Review task permissions",
        }
    }
    pub fn action(self) -> &'static str {
        match self {
            Self::RunOnce => "Run once",
            Self::Confirm => "Run task",
            Self::TrustFile => "Trust tasks",
        }
    }
    pub fn reason(self) -> &'static str {
        match self {
            Self::RunOnce => "This file is not trusted. Approval applies to this run only.",
            Self::Confirm => "This task has “Ask before running” enabled.",
            Self::TrustFile => "Trust this exact file. Edits require review. No task runs.",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskReview {
    pub mode: ReviewMode,
    pub name: String,
    pub directory: String,
    /// The exact configured command, executed in `directory`.
    pub command: String,
    pub scroll: usize,
    /// Trust-file approval starts on Cancel. Tab switches button focus.
    pub primary_focus: bool,
}
#[derive(Clone, Debug, PartialEq)]
pub struct ReviewLayout {
    pub panel: [f32; 4],
    pub body: [f32; 4],
    pub cancel: [f32; 4],
    pub primary: [f32; 4],
    pub scale: f32,
    pub lines: Vec<String>,
    pub first: usize,
    pub count: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReviewHit {
    Cancel,
    Primary,
    Body,
    Outside,
}
impl ReviewLayout {
    pub fn new(width: f32, height: f32, scale: f32, view: &TaskReview) -> Option<Self> {
        if ![width, height, scale].into_iter().all(f32::is_finite) || scale <= 0. {
            return None;
        }
        let w = (560. * scale).min(width - 24. * scale);
        let available_h = height - 24. * scale;
        if w < 300. * scale || available_h < 260. * scale {
            return None;
        }
        let cols = ((w - 64. * scale) / (13. * scale * 0.72)).floor().max(1.) as usize;
        let mut lines = wrap(&format!("Directory: {}", view.directory), cols);
        lines.push(String::new());
        lines.push("Command".into());
        lines.extend(wrap(&view.command, cols));
        // Short commands get a compact dialog; long ones scroll instead of
        // consuming the entire window. No command bytes are discarded.
        let h = ((198. + lines.len() as f32 * 19.).max(260.) * scale)
            .min(460. * scale)
            .min(available_h);
        let x = (width - w) * 0.5;
        let y = (height - h) * 0.5;
        let body = [
            x + 20. * scale,
            y + 112. * scale,
            w - 40. * scale,
            h - 178. * scale,
        ];
        let count = ((body[3] - 20. * scale) / (19. * scale)).floor().max(1.) as usize;
        let first = view.scroll.min(lines.len().saturating_sub(count));
        Some(Self {
            panel: [x, y, w, h],
            body,
            cancel: [
                x + w - 254. * scale,
                y + h - 50. * scale,
                100. * scale,
                32. * scale,
            ],
            primary: [
                x + w - 144. * scale,
                y + h - 50. * scale,
                124. * scale,
                32. * scale,
            ],
            scale,
            lines,
            first,
            count,
        })
    }
    pub fn hit(&self, x: f32, y: f32) -> ReviewHit {
        let contains = |r: [f32; 4]| {
            x.is_finite()
                && y.is_finite()
                && x >= r[0]
                && y >= r[1]
                && x < r[0] + r[2]
                && y < r[1] + r[3]
        };
        if contains(self.cancel) {
            ReviewHit::Cancel
        } else if contains(self.primary) {
            ReviewHit::Primary
        } else if contains(self.panel) {
            ReviewHit::Body
        } else {
            ReviewHit::Outside
        }
    }
}
pub(crate) fn wrap(text: &str, cols: usize) -> Vec<String> {
    let mut out = vec![];
    let mut line = String::new();
    let mut width = 0;
    for ch in text.chars() {
        if ch == '\n' {
            out.push(std::mem::take(&mut line));
            width = 0;
            continue;
        }
        let visible = if ch.is_control()
            || matches!(ch, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}' | '\u{200e}' | '\u{200f}')
        {
            ch.escape_default().to_string()
        } else {
            ch.to_string()
        };
        for c in visible.chars() {
            let cw = c.width().unwrap_or(0);
            if width + cw > cols && !line.is_empty() {
                out.push(std::mem::take(&mut line));
                width = 0;
            }
            line.push(c);
            width += cw;
        }
    }
    out.push(line);
    out
}
#[cfg(test)]
mod tests {
    use super::*;
    fn view() -> TaskReview {
        TaskReview {
            mode: ReviewMode::RunOnce,
            name: "Build".into(),
            directory: "/project".into(),
            command: format!("echo {}SAFE_END", "日本é".repeat(330)),
            scroll: usize::MAX,
            primary_focus: false,
        }
    }
    #[test]
    fn full_command_remains_accessible_and_buttons_share_geometry() {
        for scale in [1., 1.5, 2., 3.] {
            let l = ReviewLayout::new(900. * scale, 600. * scale, scale, &view()).unwrap();
            assert!(l.lines.join("").ends_with("SAFE_END"));
            assert_eq!(l.first + l.count, l.lines.len());
            assert_eq!(l.hit(l.primary[0], l.primary[1]), ReviewHit::Primary);
            assert_eq!(l.hit(l.cancel[0], l.cancel[1]), ReviewHit::Cancel);
            assert_eq!(
                l.hit(l.primary[0] + l.primary[2], l.primary[1]),
                ReviewHit::Body
            );
            assert_eq!(l.hit(f32::NAN, 0.), ReviewHit::Outside);
        }
    }
    #[test]
    fn invalid_sizes_fail_closed_and_escapes_are_visible() {
        assert!(ReviewLayout::new(200., 200., 1., &view()).is_none());
        assert!(ReviewLayout::new(900., 600., f32::NAN, &view()).is_none());
        assert_eq!(wrap("echo\x1b[31m", 80).join(""), "echo\\u{1b}[31m");
    }
}
