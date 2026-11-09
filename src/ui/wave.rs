//! The magnify wave, as pure arithmetic along the dock's long axis.
//!
//! The model is the macOS one: icons near the pointer grow, and the row is
//! stretched *around the pointer*, so the point under the cursor stays under
//! it. Concretely, every coordinate `x` on the resting row moves by
//! `G(x) - G(anchor)`, where `G(x)` is how much the row has grown between its
//! start and `x`. The anchor is the pointer.
//!
//! Every position comes from the scales as they are drawn *now*, never from
//! where they are heading, so icons can never overlap or drift apart however
//! fast the pointer moves: the layout is always one consistent stretch.

/// One item on the resting row: where it starts along the dock, and how long
/// it is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Slot {
    pub start: f64,
    pub extent: f64,
}

impl Slot {
    fn end(&self) -> f64 {
        self.start + self.extent
    }
    fn centre(&self) -> f64 {
        self.start + self.extent / 2.0
    }
}

/// The scale each item heads for with the pointer at `pointer`: full `zoom`
/// right under it, easing on a cosine to nothing `spread` slot pitches away.
/// Items that do not react (dividers) stay at 1.
pub fn targets(slots: &[Slot], reacts: &[bool], pointer: f64, zoom: f64, pitch: f64, spread: f64) -> Vec<f64> {
    let reach = (pitch * spread).max(1.0);
    slots
        .iter()
        .enumerate()
        .map(|(i, s)| {
            if !reacts.get(i).copied().unwrap_or(false) {
                return 1.0;
            }
            let d = ((s.centre() - pointer).abs() / reach).min(1.0);
            let f = (d * std::f64::consts::FRAC_PI_2).cos().powi(2);
            1.0 + (zoom - 1.0).max(0.0) * f
        })
        .collect()
}

/// How much item `i` has grown along the dock at `scale`.
fn growth(slot: &Slot, scale: f64) -> f64 {
    slot.extent * (scale - 1.0)
}

/// `G(x)`: how much the row has grown between its start and `x`.
fn grown_by(slots: &[Slot], scales: &[f64], x: f64) -> f64 {
    let mut g = 0.0;
    for (s, &k) in slots.iter().zip(scales) {
        if x >= s.end() {
            g += growth(s, k);
        } else {
            if x > s.start && s.extent > 0.0 {
                g += growth(s, k) * (x - s.start) / s.extent;
            }
            break;
        }
    }
    g
}

/// How far each item's centre moves along the dock, for items drawn at
/// `scales` and scaling about their own centres, with the row stretched
/// about `anchor`.
pub fn offsets(slots: &[Slot], scales: &[f64], anchor: f64) -> Vec<f64> {
    let at_anchor = grown_by(slots, scales, anchor);
    let mut before = 0.0;
    slots
        .iter()
        .zip(scales)
        .map(|(s, &k)| {
            let g = growth(s, k);
            let off = before + g / 2.0 - at_anchor;
            before += g;
            off
        })
        .collect()
}

/// How far the row reaches past its resting ends: `(before the first item,
/// after the last)`. The glass panel stretches by exactly this.
pub fn spill(slots: &[Slot], scales: &[f64], anchor: f64) -> (f64, f64) {
    let at_anchor = grown_by(slots, scales, anchor);
    let total: f64 = slots.iter().zip(scales).map(|(s, &k)| growth(s, k)).sum();
    (at_anchor.max(0.0), (total - at_anchor).max(0.0))
}

/// The most the row can reach past either end at rest under any pointer
/// position: the room the surface must keep at each end so nothing is cut.
pub fn max_spill(slots: &[Slot], reacts: &[bool], zoom: f64, pitch: f64, spread: f64) -> f64 {
    let (Some(first), Some(last)) = (slots.first(), slots.last()) else { return 0.0 };
    let (from, to) = (first.start - pitch, last.end() + pitch);
    let mut worst: f64 = 0.0;
    let mut p = from;
    while p <= to {
        let scales = targets(slots, reacts, p, zoom, pitch, spread);
        let (lo, hi) = spill(slots, &scales, p);
        worst = worst.max(lo).max(hi);
        p += 1.0;
    }
    worst
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(n: usize) -> Vec<Slot> {
        (0..n).map(|i| Slot { start: i as f64 * 40.0, extent: 32.0 }).collect()
    }

    #[test]
    fn the_point_under_the_pointer_stays_under_it() {
        let slots = row(10);
        let reacts = vec![true; 10];
        for p in [5.0, 16.0, 100.0, 211.0, 390.0] {
            let scales = targets(&slots, &reacts, p, 1.6, 40.0, 2.5);
            let offs = offsets(&slots, &scales, p);
            if let Some(i) = slots.iter().position(|s| p >= s.start && p <= s.end()) {
                let s = slots[i];
                let t = (p - s.start) / s.extent;
                let drawn_start = s.centre() + offs[i] - s.extent * scales[i] / 2.0;
                let at = drawn_start + t * s.extent * scales[i];
                assert!((at - p).abs() < 1e-9, "pointer {p}: drawn {at}");
            }
        }
    }

    #[test]
    fn items_never_overlap_at_any_drawn_scales() {
        let slots = row(8);
        // Arbitrary mid-animation scales, as springs leave them.
        let scales = [1.0, 1.31, 1.6, 1.58, 1.2, 1.02, 1.0, 1.44];
        for anchor in [0.0, 77.0, 150.0, 330.0] {
            let offs = offsets(&slots, &scales, anchor);
            for i in 1..slots.len() {
                let end_prev = slots[i - 1].centre() + offs[i - 1] + slots[i - 1].extent * scales[i - 1] / 2.0;
                let start = slots[i].centre() + offs[i] - slots[i].extent * scales[i] / 2.0;
                // The resting gap between them is kept exactly.
                assert!((start - end_prev - 8.0).abs() < 1e-9);
            }
        }
    }

    #[test]
    fn at_rest_nothing_moves_wherever_the_anchor_is() {
        let slots = row(5);
        let ones = vec![1.0; 5];
        assert!(offsets(&slots, &ones, 123.0).iter().all(|o| *o == 0.0));
        assert_eq!(spill(&slots, &ones, 50.0), (0.0, 0.0));
    }

    #[test]
    fn the_ends_reach_out_by_what_grew_on_each_side() {
        let slots = row(9);
        let reacts = vec![true; 9];
        let p = slots[4].centre();
        let scales = targets(&slots, &reacts, p, 1.6, 40.0, 2.5);
        let (lo, hi) = spill(&slots, &scales, p);
        let offs = offsets(&slots, &scales, p);
        assert!((lo - hi).abs() < 1e-9, "symmetric about the middle");
        let first_start = slots[0].centre() + offs[0] - slots[0].extent * scales[0] / 2.0;
        assert!((slots[0].start - first_start - lo).abs() < 1e-9);
        assert!(max_spill(&slots, &reacts, 1.6, 40.0, 2.5) >= lo);
    }

    #[test]
    fn dividers_do_not_swell() {
        let slots = row(3);
        let s = targets(&slots, &[true, false, true], slots[1].centre(), 1.6, 40.0, 2.5);
        assert_eq!(s[1], 1.0);
        assert!(s[0] > 1.0 && s[2] > 1.0);
    }
}
