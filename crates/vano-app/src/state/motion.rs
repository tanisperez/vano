//! Spring animation for sets of rectangles: the treemap tiles, the ranking rows.
//!
//! Every item has a target rectangle and opacity. When the targets change (a zoom, a live scan
//! update, an item moved to the Trash) each item keeps its current position and velocity and
//! springs toward its new target, so a change in the middle of another one never jumps. New
//! items start from a rectangle the caller chooses (inside the tile that was clicked, for a
//! zoom) and fade in; items that are gone fly to a rectangle the caller chooses and fade out.
//!
//! Plain Rust with gpui's analytic [`SpringConfig`], stepped by the real time between frames,
//! so the motion is the same at 60 and 144 Hz.

use std::collections::HashMap;
use std::hash::Hash;

use gpui_kit::{SpringConfig, SpringState};
use vano_core::treemap::Rect;

/// Critically damped: quick, no bounce, the eye sees it arrive in about 350 ms.
pub const SPRING: SpringConfig = SpringConfig::new(260.0, 32.0, 1.0);
/// A little softer, for the live scan, where targets change ten times a second.
pub const LIVE_SPRING: SpringConfig = SpringConfig::new(140.0, 24.0, 1.0);

/// Below this many pixels (and pixels per second) a position counts as arrived. A quarter of a
/// pixel is invisible, and stopping there saves the frames a spring spends creeping in.
const EPSILON: f32 = 0.25;
/// The same for opacity.
const OPACITY_EPSILON: f32 = 0.01;
/// The longest step taken at once: after a stall, the motion resumes instead of jumping.
const MAX_STEP: f32 = 1.0 / 30.0;

#[derive(Debug, Clone, Copy)]
struct Axis {
    state: SpringState,
    target: f32,
}

impl Axis {
    fn at(value: f32) -> Self {
        Self {
            state: SpringState {
                position: value,
                velocity: 0.0,
            },
            target: value,
        }
    }

    fn step(&mut self, spring: &SpringConfig, dt: f32, epsilon: f32) -> bool {
        self.state = spring.step(self.state, self.target, dt);
        let settled = (self.state.position - self.target).abs() < epsilon
            && self.state.velocity.abs() < epsilon * 10.0;
        if settled {
            self.state.position = self.target;
            self.state.velocity = 0.0;
        }
        !settled
    }

    fn snap(&mut self) {
        self.state.position = self.target;
        self.state.velocity = 0.0;
    }
}

#[derive(Debug, Clone)]
struct Item<T> {
    axes: [Axis; 5],
    exiting: bool,
    data: T,
}

impl<T> Item<T> {
    fn rect(&self) -> Rect {
        Rect::new(
            self.axes[0].state.position,
            self.axes[1].state.position,
            self.axes[2].state.position.max(0.0),
            self.axes[3].state.position.max(0.0),
        )
    }

    fn opacity(&self) -> f32 {
        self.axes[4].state.position.clamp(0.0, 1.0)
    }

    fn retarget(&mut self, rect: Rect, opacity: f32) {
        let values = [rect.x, rect.y, rect.w, rect.h, opacity];
        for (axis, value) in self.axes.iter_mut().zip(values) {
            axis.target = value;
        }
    }
}

fn axes(rect: Rect, opacity: f32) -> [Axis; 5] {
    [
        Axis::at(rect.x),
        Axis::at(rect.y),
        Axis::at(rect.w),
        Axis::at(rect.h),
        Axis::at(opacity),
    ]
}

/// One animated item as it should be drawn now.
#[derive(Debug, Clone, Copy)]
pub struct Frame<'a, K, T> {
    pub key: &'a K,
    pub data: &'a T,
    pub rect: Rect,
    pub opacity: f32,
    /// The item is leaving and should not react to the mouse.
    pub exiting: bool,
}

/// A set of springs, one item per key, in drawing order.
#[derive(Debug, Clone)]
pub struct Motion<K, T> {
    order: Vec<K>,
    items: HashMap<K, Item<T>>,
    spring: SpringConfig,
}

impl<K: Clone + Eq + Hash, T> Default for Motion<K, T> {
    fn default() -> Self {
        Self::new(SPRING)
    }
}

impl<K: Clone + Eq + Hash, T> Motion<K, T> {
    pub fn new(spring: SpringConfig) -> Self {
        Self {
            order: Vec::new(),
            items: HashMap::new(),
            spring,
        }
    }

    /// Changes the spring for the motion that follows.
    pub fn set_spring(&mut self, spring: SpringConfig) {
        self.spring = spring;
    }

    /// Sets the new targets, in drawing order. Items already shown move from where they are.
    /// A new item starts at `enter_from(key, target)`, transparent. An item that is not in
    /// `targets` any more moves to `exit_to(key, current rect)` and fades out; it is drawn under
    /// the others until it is gone.
    pub fn set_targets(
        &mut self,
        targets: impl IntoIterator<Item = (K, Rect, T)>,
        mut enter_from: impl FnMut(&K, Rect) -> Rect,
        mut exit_to: impl FnMut(&K, Rect) -> Rect,
    ) {
        let mut order = Vec::new();
        let mut seen = HashMap::new();
        for (key, rect, data) in targets {
            seen.insert(key.clone(), ());
            match self.items.get_mut(&key) {
                Some(item) => {
                    item.exiting = false;
                    item.data = data;
                    item.retarget(rect, 1.0);
                }
                None => {
                    let mut item = Item {
                        axes: axes(enter_from(&key, rect), 0.0),
                        exiting: false,
                        data,
                    };
                    item.retarget(rect, 1.0);
                    self.items.insert(key.clone(), item);
                }
            }
            order.push(key);
        }
        let mut leaving = Vec::new();
        for key in &self.order {
            if seen.contains_key(key) {
                continue;
            }
            if let Some(item) = self.items.get_mut(key) {
                if !item.exiting {
                    item.exiting = true;
                    let to = exit_to(key, item.rect());
                    item.retarget(to, 0.0);
                }
                leaving.push(key.clone());
            }
        }
        leaving.extend(order);
        self.order = leaving;
    }

    /// Advances every spring by `dt` seconds. Returns whether anything is still moving.
    pub fn step(&mut self, dt: f32) -> bool {
        let dt = dt.clamp(0.0, MAX_STEP);
        let spring = self.spring;
        let mut moving = false;
        for item in self.items.values_mut() {
            for (index, axis) in item.axes.iter_mut().enumerate() {
                let epsilon = if index == 4 { OPACITY_EPSILON } else { EPSILON };
                moving |= axis.step(&spring, dt, epsilon);
            }
        }
        self.drop_gone();
        moving
    }

    /// Jumps every item to its target, for the first layout and when motion is reduced.
    pub fn snap(&mut self) {
        for item in self.items.values_mut() {
            for axis in &mut item.axes {
                axis.snap();
            }
        }
        self.drop_gone();
    }

    /// Removes everything at once.
    pub fn clear(&mut self) {
        self.order.clear();
        self.items.clear();
    }

    /// The items to draw now, in order.
    pub fn frames(&self) -> impl Iterator<Item = Frame<'_, K, T>> {
        self.order.iter().filter_map(|key| {
            let item = self.items.get(key)?;
            Some(Frame {
                key,
                data: &item.data,
                rect: item.rect(),
                opacity: item.opacity(),
                exiting: item.exiting,
            })
        })
    }

    /// The current rectangle of `key`, if it is shown.
    pub fn rect_of(&self, key: &K) -> Option<Rect> {
        self.items.get(key).map(Item::rect)
    }

    fn drop_gone(&mut self) {
        let items = &mut self.items;
        self.order.retain(|key| {
            let gone = items
                .get(key)
                .is_none_or(|item| item.exiting && item.axes[4].state.position <= OPACITY_EPSILON);
            if gone {
                items.remove(key);
            }
            !gone
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(motion: &mut Motion<u32, ()>) {
        for _ in 0..600 {
            if !motion.step(1.0 / 60.0) {
                break;
            }
        }
    }

    #[test]
    fn items_spring_to_their_targets() {
        let mut motion = Motion::default();
        motion.set_targets(
            [(1, Rect::new(10.0, 10.0, 100.0, 50.0), ())],
            |_, r| r.collapsed(),
            |_, r| r,
        );
        assert_eq!(motion.frames().count(), 1);
        assert!(motion.step(1.0 / 60.0));
        run(&mut motion);
        let frame = motion.frames().next().map(|f| (f.rect, f.opacity));
        assert_eq!(frame, Some((Rect::new(10.0, 10.0, 100.0, 50.0), 1.0)));
        assert!(!motion.step(1.0 / 60.0));
    }

    #[test]
    fn missing_items_fade_out_and_are_dropped() {
        let mut motion = Motion::default();
        motion.set_targets(
            [(1, Rect::new(0.0, 0.0, 10.0, 10.0), ())],
            |_, r| r,
            |_, r| r,
        );
        motion.snap();
        motion.set_targets(
            [(2, Rect::new(0.0, 0.0, 10.0, 10.0), ())],
            |_, r| r,
            |_, r| r,
        );
        let keys: Vec<(u32, bool)> = motion.frames().map(|f| (*f.key, f.exiting)).collect();
        assert_eq!(keys, vec![(1, true), (2, false)]);
        run(&mut motion);
        let keys: Vec<u32> = motion.frames().map(|f| *f.key).collect();
        assert_eq!(keys, vec![2]);
    }

    #[test]
    fn retargeting_keeps_the_current_position() {
        let mut motion = Motion::default();
        motion.set_targets(
            [(1, Rect::new(0.0, 0.0, 10.0, 10.0), ())],
            |_, r| r,
            |_, r| r,
        );
        motion.snap();
        motion.set_targets(
            [(1, Rect::new(100.0, 0.0, 10.0, 10.0), ())],
            |_, r| r,
            |_, r| r,
        );
        motion.step(1.0 / 60.0);
        let x = motion.rect_of(&1).map(|r| r.x).unwrap_or_default();
        assert!(x > 0.0 && x < 100.0);
    }
}
