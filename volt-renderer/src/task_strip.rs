//! Task buttons shown where the workspace card sits while the card is
//! closed. Only drawn when the project has tasks of its own.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StripState {
    Idle,
    ReviewRequired,
    Running,
    Succeeded,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StripTask {
    pub name: String,
    pub state: StripState,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskStripView {
    /// At most `MAX_BUTTONS`; the rest are behind "⋯".
    pub tasks: Vec<StripTask>,
    /// More tasks than fit, reachable through "⋯" (opens the card).
    pub more: bool,
    /// One line under the strip, e.g. why the last run didn't happen.
    pub message: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StripHit {
    Task(usize),
    More,
    Add,
    Outside,
}

pub const MAX_BUTTONS: usize = 5;
// Logical units.
const MARGIN: f32 = 12.;
pub(crate) const H: f32 = 30.;
pub(crate) const TEXT: f32 = 12.5;
const GAP: f32 = 6.;
const MAX_NAME: usize = 18;
const ICON_W: f32 = 30.;

/// Button label: the name, shortened to keep the strip compact.
pub fn label(name: &str) -> String {
    if name.chars().count() <= MAX_NAME {
        name.to_string()
    } else {
        let mut s: String = name.chars().take(MAX_NAME - 1).collect();
        s.push('…');
        s
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TaskStripLayout {
    pub scale: f32,
    /// Physical rects: one per task, then "⋯" (if shown), then "+".
    pub buttons: Vec<[f32; 4]>,
    pub more: Option<[f32; 4]>,
    pub add: [f32; 4],
    /// Where the message line goes, right-aligned under the buttons.
    pub message_right: f32,
    pub message_y: f32,
}

impl TaskStripLayout {
    pub fn new(view: &TaskStripView, width: f32, top: f32, scale: f32) -> Option<Self> {
        let s = scale.max(0.1);
        let text_w = |t: &str| t.chars().count() as f32 * TEXT * 0.6;
        let mut widths: Vec<f32> = view
            .tasks
            .iter()
            .map(|t| text_w(&label(&t.name)) + 38.)
            .collect();
        let more_w = view.more.then_some(ICON_W);
        let add_w = if view.tasks.is_empty() {
            text_w("+ Add task") + 22.
        } else {
            ICON_W
        };
        let total = widths.iter().sum::<f32>()
            + more_w.unwrap_or(0.)
            + add_w
            + GAP * (widths.len() + usize::from(view.more)) as f32;
        // A window too narrow for the strip doesn't show it; the card still works.
        if (total + 2. * MARGIN) * s > width {
            return None;
        }
        let y = top + MARGIN * s;
        let mut x = width - MARGIN * s - total * s;
        let mut rect = |w: f32| {
            let r = [x, y, w * s, H * s];
            x += (w + GAP) * s;
            r
        };
        let buttons = widths.drain(..).map(&mut rect).collect();
        let more = more_w.map(&mut rect);
        let add = rect(add_w);
        Some(Self {
            scale: s,
            buttons,
            more,
            add,
            message_right: width - MARGIN * s,
            message_y: y + (H + 6.) * s,
        })
    }

    pub fn hit(&self, x: f32, y: f32) -> StripHit {
        let inside = |r: &[f32; 4]| x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3];
        if let Some(i) = self.buttons.iter().position(inside) {
            return StripHit::Task(i);
        }
        if self.more.as_ref().is_some_and(inside) {
            return StripHit::More;
        }
        if inside(&self.add) {
            return StripHit::Add;
        }
        StripHit::Outside
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(n: usize, more: bool) -> TaskStripView {
        TaskStripView {
            tasks: (0..n)
                .map(|i| StripTask {
                    name: format!("Task {i}"),
                    state: StripState::Idle,
                })
                .collect(),
            more,
            message: None,
        }
    }

    #[test]
    fn buttons_sit_right_aligned_in_order_and_hit_where_drawn() {
        for scale in [1., 2.] {
            let l =
                TaskStripLayout::new(&view(3, true), 1400. * scale, 30. * scale, scale).unwrap();
            assert_eq!(l.buttons.len(), 3);
            assert!(l.buttons[0][0] < l.buttons[1][0]);
            let add = l.add;
            assert!(
                (add[0] + add[2] - (1400. - 12.) * scale).abs() < 0.01,
                "flush right"
            );
            let c = |r: [f32; 4]| (r[0] + r[2] / 2., r[1] + r[3] / 2.);
            let (x, y) = c(l.buttons[2]);
            assert_eq!(l.hit(x, y), StripHit::Task(2));
            let (x, y) = c(l.more.unwrap());
            assert_eq!(l.hit(x, y), StripHit::More);
            let (x, y) = c(add);
            assert_eq!(l.hit(x, y), StripHit::Add);
            assert_eq!(l.hit(5., 5.), StripHit::Outside);
        }
    }

    #[test]
    fn a_project_with_no_tasks_has_a_visible_first_task_button() {
        let l = TaskStripLayout::new(&view(0, false), 800., 30., 1.).unwrap();
        assert!(l.buttons.is_empty());
        assert!(l.add[2] > ICON_W);
        assert_eq!(l.hit(l.add[0] + 10., l.add[1] + 10.), StripHit::Add);
    }

    #[test]
    fn long_names_shorten_and_narrow_windows_hide_the_strip() {
        assert_eq!(label("Deploy to the staging cluster"), "Deploy to the sta…");
        assert!(TaskStripLayout::new(&view(5, false), 300., 0., 1.).is_none());
    }
}
