//! Keyed paint-only interpolation; application callbacks and layout never tick.
use super::{
    effects::{Animation, Effects},
    Kind, LayoutBox,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    time::Duration,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ResolvedShadow {
    pub color: [f32; 4],
    pub blur: f32,
    pub spread: f32,
    pub offset: [f32; 2],
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Visual {
    pub color: [f32; 4],
    pub background: [f32; 4],
    pub border: [f32; 4],
    pub width: f32,
    pub opacity: f32,
    pub shadow: Option<ResolvedShadow>,
    pub glow: Option<ResolvedShadow>,
    /// Paint-time translation in physical pixels (hover lift, slide-ins).
    pub offset: [f32; 2],
}
fn mix(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
fn color(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    std::array::from_fn(|i| mix(a[i], b[i], t))
}
fn shadow(a: Option<ResolvedShadow>, b: Option<ResolvedShadow>, t: f32) -> Option<ResolvedShadow> {
    let geometry = b.or(a)?;
    let transparent = [geometry.color[0], geometry.color[1], geometry.color[2], 0.];
    // Geometry snaps to the new target; only color/alpha interpolate so cached
    // blur masks are reusable throughout an interaction animation.
    Some(ResolvedShadow {
        color: color(
            a.map_or(transparent, |v| v.color),
            b.map_or(transparent, |v| v.color),
            t,
        ),
        ..geometry
    })
}
impl Visual {
    fn interpolate(self, to: Self, t: f32) -> Self {
        if t >= 1. {
            return to;
        }
        Self {
            color: color(self.color, to.color, t),
            background: color(self.background, to.background, t),
            border: color(self.border, to.border, t),
            width: mix(self.width, to.width, t),
            opacity: mix(self.opacity, to.opacity, t),
            shadow: shadow(self.shadow, to.shadow, t),
            glow: shadow(self.glow, to.glow, t),
            offset: [
                mix(self.offset[0], to.offset[0], t),
                mix(self.offset[1], to.offset[1], t),
            ],
        }
    }
}
#[derive(Clone)]
struct Track {
    kind: Kind,
    from: Visual,
    to: Visual,
    flags: [bool; 3],
    start: Duration,
    animation: Animation,
}
impl Track {
    fn sample(&self, now: Duration) -> Visual {
        self.from.interpolate(
            self.to,
            self.animation
                .progress(now.saturating_sub(self.start).as_secs_f32() * 1000.),
        )
    }
    fn active(&self, now: Duration) -> bool {
        self.from != self.to
            && now.saturating_sub(self.start).as_millis() < u128::from(self.animation.duration)
    }
}
#[derive(Clone, Copy)]
struct Fade {
    start: Duration,
    animation: Animation,
    from: f32,
    /// Logical offset the element slides in from.
    slide: [f32; 2],
}
impl Fade {
    fn value(self, now: Duration) -> f32 {
        self.from + (1. - self.from) * self.progress(now)
    }
    fn progress(self, now: Duration) -> f32 {
        self.animation
            .progress(now.saturating_sub(self.start).as_secs_f32() * 1000.)
    }
}
pub(crate) struct Exit {
    pub boxes: Vec<LayoutBox>,
    fade: Fade,
    ancestors: Vec<(String, Kind, f32)>,
    before: Option<String>,
}
#[derive(Default)]
pub(crate) struct Motion {
    pub now: Duration,
    pub reduced: bool,
    /// Physical pixels per logical pixel, for slide offsets.
    pub scale: f32,
    tracks: BTreeMap<String, Track>,
    seen: BTreeMap<String, Kind>,
    mounts: BTreeMap<String, Fade>,
    pub exits: Vec<Exit>,
}
impl Motion {
    pub fn value(
        &mut self,
        id: &str,
        kind: Kind,
        target: Visual,
        flags: [bool; 3],
        effects: &Effects,
    ) -> Visual {
        let now = self.now;
        let track = self.tracks.entry(id.into()).or_insert(Track {
            kind,
            from: target,
            to: target,
            flags,
            start: now,
            animation: Animation::linear().duration_ms(0),
        });
        if track.kind != kind {
            *track = Track {
                kind,
                from: target,
                to: target,
                flags,
                start: now,
                animation: Animation::linear().duration_ms(0),
            };
        }
        if track.to != target {
            let animation = [2, 0, 1]
                .into_iter()
                .filter(|&i| flags[i] != track.flags[i])
                .filter_map(|i| effects.animations[i])
                .next_back()
                .or(effects.animation);
            track.from = track.sample(now);
            track.to = target;
            track.start = now;
            track.animation = if self.reduced {
                Animation::linear().duration_ms(0)
            } else {
                animation.unwrap_or(Animation::linear().duration_ms(0))
            };
        }
        track.flags = flags;
        let mut value = track.sample(now);
        if let Some(fade) = self.mounts.get(id) {
            let f = fade.value(now);
            value.opacity *= f;
            let s = self.scale.max(0.25);
            value.offset[0] += fade.slide[0] * s * (1. - f);
            value.offset[1] += fade.slide[1] * s * (1. - f);
        }
        value
    }
    pub fn exit_value(&self, b: &LayoutBox, mut target: Visual) -> Visual {
        if let Some(value) = b.detached_visual {
            return value;
        }
        target.opacity *= b.detached_mount;
        if let Some(t) = b.effects.transition {
            let s = self.scale.max(0.25);
            target.offset[0] += t.1[0] * s * (1. - b.detached_mount);
            target.offset[1] += t.1[1] * s * (1. - b.detached_mount);
        }
        target
    }
    fn sampled_opacity(&self, b: &LayoutBox) -> f32 {
        self.tracks
            .get(&b.id)
            .map_or(b.effects.base.opacity.unwrap_or(1.), |t| {
                t.sample(self.now).opacity
            })
            * self.mounts.get(&b.id).map_or(1., |f| f.value(self.now))
    }
    pub fn reconcile(&mut self, boxes: &[LayoutBox]) {
        let live: BTreeMap<_, _> = boxes.iter().map(|b| (b.id.clone(), b.kind)).collect();
        let mut restoring = BTreeMap::new();
        for e in &self.exits {
            if let Some(root) = e.boxes.first() {
                if live.get(&root.id) == Some(&root.kind) {
                    restoring.insert(
                        root.id.clone(),
                        root.detached_mount * (1. - e.fade.progress(self.now)),
                    );
                }
            }
        }
        self.exits
            .retain(|e| e.boxes.first().is_some_and(|b| !live.contains_key(&b.id)));
        for e in &mut self.exits {
            e.boxes.retain(|b| !live.contains_key(&b.id));
        }
        for b in boxes {
            let fresh = self.seen.get(&b.id) != Some(&b.kind);
            if (fresh || restoring.contains_key(&b.id)) && !self.reduced {
                if let Some(transition) = b.effects.transition {
                    self.mounts.insert(
                        b.id.clone(),
                        Fade {
                            start: self.now,
                            animation: transition.0,
                            from: restoring.get(&b.id).copied().unwrap_or(0.),
                            slide: transition.1,
                        },
                    );
                }
            }
        }
        self.mounts.retain(|id, _| live.contains_key(id));
        self.seen = live;
        self.prune();
    }
    pub fn remove(&mut self, old: &[LayoutBox], next: &BTreeSet<String>) {
        if !self.reduced {
            let previous_paint = self.paint_boxes(old);
            let mut i = 0;
            while i < old.len() {
                let b = &old[i];
                if !next.contains(&b.id) {
                    if let Some(t) = b.effects.transition {
                        let end = (i + 1..old.len())
                            .find(|&j| old[j].depth <= b.depth)
                            .unwrap_or(old.len());
                        let boxes = old[i..end]
                            .iter()
                            .filter(|b| !next.contains(&b.id))
                            .map(|b| {
                                let mut box_ = b.clone();
                                box_.departing = true;
                                box_.detached_state =
                                    self.tracks.get(&b.id).map(|t| super::ButtonState {
                                        hovered: t.flags[0],
                                        pressed: t.flags[1],
                                        focused: t.flags[2],
                                        disabled: !b.enabled,
                                    });
                                box_.detached_mount =
                                    self.mounts.get(&b.id).map_or(1., |f| f.value(self.now));
                                box_.detached_visual = self.tracks.get(&b.id).map(|t| {
                                    let mut v = t.sample(self.now);
                                    v.opacity *= box_.detached_mount;
                                    v
                                });
                                box_
                            })
                            .collect();
                        let mut ancestors = Vec::new();
                        let mut depth = b.depth;
                        for a in old[..i].iter().rev() {
                            if depth > 0 && a.depth == depth - 1 {
                                ancestors.push((a.id.clone(), a.kind, self.sampled_opacity(a)));
                                depth -= 1;
                            }
                        }
                        ancestors.reverse();
                        let before = if let Some(paint) = &previous_paint {
                            paint
                                .iter()
                                .position(|p| !p.departing && p.id == b.id)
                                .and_then(|index| {
                                    let end = (index + 1..paint.len())
                                        .find(|&j| paint[j].depth <= b.depth)
                                        .unwrap_or(paint.len());
                                    paint[end..]
                                        .iter()
                                        .find(|b| b.departing || next.contains(&b.id))
                                        .map(|b| b.id.clone())
                                })
                        } else {
                            old[end..]
                                .iter()
                                .find(|b| next.contains(&b.id))
                                .map(|b| b.id.clone())
                        };
                        self.exits.push(Exit {
                            boxes,
                            fade: Fade {
                                start: self.now,
                                animation: t.0,
                                from: 0.,
                                slide: t.1,
                            },
                            ancestors,
                            before,
                        });
                        i = end;
                        continue;
                    }
                }
                i += 1;
            }
        }
        while self.exits.iter().map(|e| e.boxes.len()).sum::<usize>() > super::document::MAX_NODES {
            self.exits.remove(0);
        }
        self.seen.retain(|id, _| next.contains(id));
        self.mounts.retain(|id, _| next.contains(id));
        self.prune();
    }
    /// Reinsert detached paint snapshots at their old sibling position, within
    /// surviving ancestor groups. Missing ancestors retain their sampled opacity.
    pub fn paint_boxes(&self, live: &[LayoutBox]) -> Option<Vec<LayoutBox>> {
        if self.exits.is_empty() {
            return None;
        }
        let mut boxes = live.to_vec();
        for e in &self.exits {
            let Some(root) = e.boxes.first() else {
                continue;
            };
            let parent = e
                .ancestors
                .iter()
                .enumerate()
                .rev()
                .find_map(|(a, (id, kind, _))| {
                    boxes
                        .iter()
                        .position(|b| !b.departing && b.id == *id && b.kind == *kind)
                        .map(|index| (a, index))
                });
            let (start, end, depth, missing) = if let Some((a, index)) = parent {
                let depth = boxes[index].depth;
                let end = (index + 1..boxes.len())
                    .find(|&j| boxes[j].depth <= depth)
                    .unwrap_or(boxes.len());
                (
                    index + 1,
                    end,
                    depth + 1,
                    e.ancestors[a + 1..]
                        .iter()
                        .map(|(_, _, o)| o)
                        .product::<f32>(),
                )
            } else {
                (
                    0,
                    boxes.len(),
                    0,
                    e.ancestors.iter().map(|(_, _, o)| o).product::<f32>(),
                )
            };
            let index = e
                .before
                .as_ref()
                .and_then(|id| {
                    boxes[start..end]
                        .iter()
                        .position(|b| b.id == *id)
                        .map(|i| i + start)
                })
                .unwrap_or(end);
            let mut detached = e.boxes.clone();
            for b in &mut detached {
                b.depth = depth + (b.depth - root.depth);
            }
            if let Some(b) = detached.first_mut() {
                let opacity = missing * (1. - e.fade.progress(self.now));
                if let Some(v) = &mut b.detached_visual {
                    v.opacity *= opacity;
                } else {
                    b.detached_mount *= opacity;
                }
            }
            boxes.splice(index..index, detached);
        }
        Some(boxes)
    }
    pub fn advance(&mut self, now: Duration) -> bool {
        let was = self.active();
        self.now = self.now.max(now);
        self.mounts.retain(|_, f| f.progress(self.now) < 1.);
        self.exits.retain(|e| e.fade.progress(self.now) < 1.);
        self.prune();
        was || self.active()
    }
    fn prune(&mut self) {
        let retained: BTreeSet<_> = self
            .exits
            .iter()
            .flat_map(|e| e.boxes.iter().map(|b| b.id.clone()))
            .collect();
        self.tracks
            .retain(|id, _| self.seen.contains_key(id) || retained.contains(id));
    }
    pub fn active(&self) -> bool {
        !self.reduced
            && (self.tracks.values().any(|t| t.active(self.now))
                || self.mounts.values().any(|f| f.progress(self.now) < 1.)
                || !self.exits.is_empty())
    }
    pub fn settle(&mut self) {
        self.mounts.clear();
        self.exits.clear();
        for t in self.tracks.values_mut() {
            t.from = t.to;
        }
        self.prune();
    }
}
