use crate::layout::Rect;
use std::collections::BTreeSet;

type Interval = (f64, f64, usize);

fn ordered(rects: &[Rect], horizontal: bool) -> (usize, Vec<Interval>, Vec<Interval>) {
    let mut starts: Vec<_> = rects
        .iter()
        .enumerate()
        .filter_map(|(index, rect)| {
            if [rect.x, rect.y, rect.right(), rect.bottom()]
                .iter()
                .any(|value| value.is_nan())
            {
                return None;
            }
            let (start, end) = if horizontal {
                (rect.x, rect.right())
            } else {
                (rect.y, rect.bottom())
            };
            Some((start.min(end), start.max(end), index))
        })
        .collect();
    starts.sort_by(|left, right| left.0.total_cmp(&right.0).then(left.2.cmp(&right.2)));
    let mut ends = starts.clone();
    ends.sort_by(|left, right| left.1.total_cmp(&right.1).then(left.2.cmp(&right.2)));
    let mut expired = 0;
    let mut candidates = 0usize;
    for (seen, (start, _, _)) in starts.iter().enumerate() {
        while expired < ends.len() && ends[expired].1 < *start {
            expired += 1;
        }
        candidates = candidates.saturating_add(seen.saturating_sub(expired));
    }
    (candidates, starts, ends)
}

/// Exact strict rectangle intersections in the original declaration-pair order.
pub(super) fn pairs(rects: &[Rect]) -> Vec<(usize, usize)> {
    let x = ordered(rects, true);
    let y = ordered(rects, false);
    let (_, starts, ends) = if x.0 <= y.0 { x } else { y };
    let mut active = BTreeSet::new();
    let mut expired = 0;
    let mut result = Vec::new();
    for (start, _, index) in starts {
        // Keep touching intervals as candidates, including zero-sized public Rects.
        while expired < ends.len() && ends[expired].1 < start {
            active.remove(&ends[expired].2);
            expired += 1;
        }
        let rect = rects[index];
        for &other in &active {
            let other_rect: Rect = rects[other];
            if rect.x < other_rect.right()
                && rect.right() > other_rect.x
                && rect.y < other_rect.bottom()
                && rect.bottom() > other_rect.y
            {
                result.push((index.min(other), index.max(other)));
            }
        }
        active.insert(index);
    }
    result.sort_unstable();
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn oracle(rects: &[Rect]) -> Vec<(usize, usize)> {
        let mut pairs = Vec::new();
        for (left, a) in rects.iter().enumerate() {
            for (right, b) in rects.iter().enumerate().skip(left + 1) {
                if a.x < b.right() && a.right() > b.x && a.y < b.bottom() && a.bottom() > b.y {
                    pairs.push((left, right));
                }
            }
        }
        pairs
    }

    #[test]
    fn sweep_matches_pairwise_for_generated_geometry_and_degenerate_public_rects() {
        let mut random = 11u64;
        for _ in 0..64 {
            let mut next = || {
                random = (random * 1_664_525 + 1_013_904_223) % (1u64 << 32);
                (random % 101) as f64 / 4.0 - 12.0
            };
            let mut rects: Vec<_> = (0..100)
                .map(|_| Rect {
                    x: next(),
                    y: next(),
                    w: next(),
                    h: next(),
                })
                .collect();
            rects.extend([
                Rect {
                    x: 0.0,
                    y: 0.0,
                    w: 0.0,
                    h: 0.0,
                },
                Rect {
                    x: f64::NAN,
                    y: 0.0,
                    w: 2.0,
                    h: 2.0,
                },
                Rect {
                    x: 0.0,
                    y: 0.0,
                    w: f64::INFINITY,
                    h: 2.0,
                },
            ]);
            assert_eq!(pairs(&rects), oracle(&rects));
        }
    }

    #[test]
    fn vertical_and_horizontal_twenty_thousand_box_chains_have_no_pairs() {
        for horizontal in [true, false] {
            let rects: Vec<_> = (0..20_000)
                .map(|index| Rect {
                    x: if horizontal { (index * 12) as f64 } else { 0.0 },
                    y: if horizontal { 0.0 } else { (index * 12) as f64 },
                    w: 10.0,
                    h: 10.0,
                })
                .collect();
            assert!(pairs(&rects).is_empty());
        }
    }
}
