//! Orthogonal routing that goes **around** the boxes.
//!
//! `X-004`. The elbow router turned one corner between two centres and avoided
//! nothing; `check::after_layout` grew a diagnostic for it, which made a wrong
//! drawing say so but left it wrong. This is the router itself, and it replaces
//! `ortho` rather than standing beside it under a second name — nobody should
//! have to learn a second word to get a drawing that is not wrong, and `D-015`
//! refuses two spellings of one idea everywhere else.
//!
//! The method is the ordinary one and the reasons are worth stating.
//!
//! **A grid, not a free search.** An orthogonal path only ever turns on a line
//! that matters: a channel just outside a box, or the centre line of the two
//! boxes being joined. Every other coordinate is a translation of one of those,
//! so the candidate lines are enumerable and the search space is a few hundred
//! points rather than a plane.
//!
//! **A\* with a price on corners.** Shortest alone produces a staircase — every
//! path of the same length is equally good to it, and the one it happens to
//! find is usually the worst-looking. A fixed cost per direction change makes
//! the fewest-elbow path win among the short ones, which is what a person means
//! by a tidy line. The cost is low enough that it will not take a long detour to
//! save a corner.
//!
//! **Integer costs, and a tie broken by index.** The golden manifest pins a
//! hash of every archetype's SVG, and this project has already shipped a
//! diagram that rendered differently run to run (`W-065`, the venn's
//! `HashMap`). So there is no float
//! in the priority order and no hash container anywhere in here: costs are
//! thousandths of a point as `i64`, and equal costs are settled by the state's
//! own index. The same input gives the same path, always.
//!
//! **It can decline.** A box wholly enclosed by others has no path. The caller
//! keeps a visible fallback and `check::after_layout` reports its crossings.
//! Large processes first try local channels while checking every obstacle;
//! their obstacle count does not disable avoidance.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use crate::layout::Rect;

/// Clearance kept between a routed line and a box it passes. Also the width of
/// the channel the grid puts either side of every box.
///
/// Chosen by looking at the result rather than by taste. At 8 a detour ran
/// close enough over a box that the arrow head of the edge *terminating* on it
/// had nowhere to sit — a line that clears a box by less than an arrow head is
/// technically not touching it and reads as if it were.
const CLEARANCE: f64 = 16.0;

/// What a corner costs, in points of length. High enough that the path with
/// fewest elbows wins among equally short ones, low enough that the router will
/// not walk an extra screen to save one.
const TURN_COST: f64 = 30.0;

/// Coordinates are compared at this resolution, so a line that is a rounding
/// error away from another is the same line and the grid stays small.
const GRID: f64 = 1000.0;

/// Axis-aligned segments that touch are not crossing: an edge leaves its own
/// box at the boundary, and a channel line sits exactly `CLEARANCE` outside.
const EPS: f64 = 1e-6;

/// An axis-aligned run some earlier line already occupies.
pub type Used = ((f64, f64), (f64, f64));

/// How much a shared run costs, per point of overlap.
///
/// A price and not a wall (`X-018`). Blocking a used run outright would make a
/// line that has no alternative simply fail, and a missing edge is worse than a
/// doubled one — the same reasoning that made the router decline rather than
/// draw through a box. At this weight a detour is preferred wherever one
/// exists, and where none does the lines still overlap and the drawing is no
/// worse than it was.
const SHARE_COST: f64 = 2.5;

/// How far a lane sits from the run it is avoiding.
///
/// Contention creates its own grid lines rather than the grid being enriched
/// everywhere: parallel channels are only wanted where two lines actually want
/// the same one, and doubling the grid for every diagram would cost every
/// diagram.
const LANE: f64 = 11.0;

/// A path from `a` to `b` that enters no box in `obstacles`, or `None` when
/// there is none to find.
///
/// The returned points start on `a`'s boundary and end on `b`'s, with the
/// elbows in between — the same shape the old router returned, so nothing
/// downstream learns that this exists.
pub fn route(a: Rect, b: Rect, obstacles: &[Rect], used: &[Used]) -> Option<Vec<(f64, f64)>> {
    let (xs, ys) = lines(a, b, obstacles, used);
    let nx = xs.len();
    let at = |v: &[f64], want: f64| v.binary_search_by(|p| order(*p, want)).ok();
    let start = at(&xs, snap(a.cx()))? + at(&ys, snap(a.cy()))? * nx;
    let goal = at(&xs, snap(b.cx()))? + at(&ys, snap(b.cy()))? * nx;

    let path = search(start, goal, &xs, &ys, obstacles, used, CLEARANCE - 0.5)?;
    let points: Vec<(f64, f64)> = path.iter().map(|p| (xs[p % nx], ys[p / nx])).collect();

    clip(straighten(points), a, b)
}

/// Route exact ports and waypoints, without moving endpoints after avoidance.
pub(crate) fn route_points(
    start: (f64, f64),
    end: (f64, f64),
    obstacles: &[Rect],
    used: &[Used],
) -> Option<Vec<(f64, f64)>> {
    if start == end {
        return Some(vec![start]);
    }
    // A blocked endpoint has no incident step; reject it before exploring the whole grid.
    if obstacles.iter().any(|rect| {
        enters(start, start, rect, CLEARANCE - 0.5) || enters(end, end, rect, CLEARANCE - 0.5)
    }) {
        return None;
    }
    let clear = |path: &[(f64, f64)]| {
        path.windows(2).all(|pair| {
            !obstacles
                .iter()
                .any(|rect| enters(pair[0], pair[1], rect, CLEARANCE - 0.5))
                && overlap(pair[0], pair[1], used) < EPS
        })
    };
    let middle = ((start.0 + end.0) / 2.0, (start.1 + end.1) / 2.0);
    for path in [
        vec![start, (end.0, start.1), end],
        vec![start, (start.0, end.1), end],
        vec![start, (middle.0, start.1), (middle.0, end.1), end],
        vec![start, (start.0, middle.1), (end.0, middle.1), end],
    ] {
        if clear(&path) {
            return Some(straighten(path));
        }
    }
    let dot = |p: (f64, f64)| Rect {
        x: p.0,
        y: p.1,
        w: 0.0,
        h: 0.0,
    };
    // Local channels keep large processes cheap; every segment is still checked
    // against all obstacles. Retry with all channels when blocked or sharing a run.
    let near = Rect {
        x: start.0.min(end.0) - 64.0,
        y: start.1.min(end.1) - 64.0,
        w: (start.0 - end.0).abs() + 128.0,
        h: (start.1 - end.1).abs() + 128.0,
    };
    let local: Vec<_> = obstacles
        .iter()
        .copied()
        .filter(|r| {
            r.x < near.right() && r.right() > near.x && r.y < near.bottom() && r.bottom() > near.y
        })
        .collect();
    let mut fallback = None;
    for channels in [&local[..], obstacles] {
        let (mut xs, mut ys) = lines(dot(start), dot(end), channels, used);
        xs.extend([start.0, end.0]);
        ys.extend([start.1, end.1]);
        xs.sort_by(|a, b| order(*a, *b));
        xs.dedup();
        ys.sort_by(|a, b| order(*a, *b));
        ys.dedup();
        let at = |v: &[f64], want: f64| v.binary_search_by(|p| order(*p, want)).ok();
        let begin = at(&xs, start.0)? + at(&ys, start.1)? * xs.len();
        let finish = at(&xs, end.0)? + at(&ys, end.1)? * xs.len();
        if let Some(path) = search(begin, finish, &xs, &ys, obstacles, used, CLEARANCE - 0.5) {
            let path = straighten(
                path.iter()
                    .map(|p| (xs[p % xs.len()], ys[p / xs.len()]))
                    .collect(),
            );
            // A remote box can supply the only clear lane beside a shared local run.
            if channels.len() == obstacles.len()
                || path
                    .windows(2)
                    .all(|pair| overlap(pair[0], pair[1], used) < EPS)
            {
                return Some(path);
            }
            fallback = Some(path);
        }
        if local.len() == obstacles.len() {
            break;
        }
    }
    fallback
}

#[cfg(test)]
#[test]
fn exact_ports_on_clearance_are_reachable_but_ports_inside_it_are_not() {
    let obstacle = Rect {
        x: 10.0,
        y: 10.0,
        w: 20.0,
        h: 20.0,
    };
    let clear = (-100.0, 20.0);
    let boundary = (obstacle.x - (CLEARANCE - 0.5), 20.0);
    let inside = (boundary.0 + 0.08, boundary.1);
    for reverse in [false, true] {
        let endpoints = |point| {
            if reverse {
                (point, clear)
            } else {
                (clear, point)
            }
        };
        let (start, end) = endpoints(inside);
        assert!(route_points(start, end, &[obstacle], &[]).is_none());
        let (start, end) = endpoints(boundary);
        assert_eq!(
            route_points(start, end, &[obstacle], &[]),
            Some(vec![start, end])
        );
    }
}

/// The candidate lines a path may turn on.
///
/// A channel either side of every box, and the centre lines of the two being
/// joined — which is what lets an edge leave from the middle of a side instead
/// of from a corner. An obstacle's own centre lines are deliberately absent:
/// they buy no path that the channels do not already offer, and every extra
/// line costs a column of grid.
fn lines(a: Rect, b: Rect, obstacles: &[Rect], used: &[Used]) -> (Vec<f64>, Vec<f64>) {
    let mut xs = vec![snap(a.cx()), snap(b.cx())];
    let mut ys = vec![snap(a.cy()), snap(b.cy())];

    for r in obstacles.iter().chain([&a, &b]) {
        xs.push(snap(r.x - CLEARANCE));
        xs.push(snap(r.right() + CLEARANCE));
        ys.push(snap(r.y - CLEARANCE));
        ys.push(snap(r.bottom() + CLEARANCE));
    }

    // A lane either side of every run an earlier line took. Without these the
    // penalty below has nowhere to send anything: the only horizontal channel
    // over a tall box is the one that box defines, so two lines crossing it
    // both take it however dearly it is priced (`X-018`).
    for ((x0, y0), (x1, y1)) in used {
        if (y0 - y1).abs() < EPS {
            ys.push(snap(y0 - LANE));
            ys.push(snap(y0 + LANE));
        }
        if (x0 - x1).abs() < EPS {
            xs.push(snap(x0 - LANE));
            xs.push(snap(x0 + LANE));
        }
    }

    let tidy = |v: &mut Vec<f64>| {
        v.sort_by(|p, q| order(*p, *q));
        v.dedup();
    };
    tidy(&mut xs);
    tidy(&mut ys);
    (xs, ys)
}

fn snap(v: f64) -> f64 {
    (v * GRID).round() / GRID
}

/// A total order over coordinates that are known to be finite. `partial_cmp`
/// would be an `unwrap` in a sort, which is a panic waiting for a NaN.
fn order(p: f64, q: f64) -> std::cmp::Ordering {
    p.partial_cmp(&q).unwrap_or(std::cmp::Ordering::Equal)
}

/// Which axis the path was travelling on when it arrived — a corner is a change
/// of axis, and the start has no history to charge for.
const AXES: usize = 3;
const H: usize = 0;
const V: usize = 1;
const NONE: usize = 2;

/// A\* over the grid, returning the point indices of a least-cost path.
#[allow(clippy::too_many_arguments)]
fn search(
    start: usize,
    goal: usize,
    xs: &[f64],
    ys: &[f64],
    obstacles: &[Rect],
    used: &[Used],
    clearance: f64,
) -> Option<Vec<usize>> {
    let nx = xs.len();
    let point = |p: usize| (xs[p % nx], ys[p / nx]);
    let states = xs.len() * ys.len() * AXES;
    let turn = (TURN_COST * GRID) as i64;

    // Manhattan, in the same thousandths the costs use. Admissible: a corner
    // only ever adds, so this never overestimates.
    let (gx, gy) = point(goal);
    let heuristic = |p: usize| {
        let (x, y) = point(p);
        (((x - gx).abs() + (y - gy).abs()) * GRID) as i64
    };

    let mut best = vec![i64::MAX; states];
    let mut came: Vec<usize> = vec![usize::MAX; states];
    // `Reverse` makes the heap a min-heap; the state index in the key is what
    // settles equal costs, so the search cannot depend on insertion order.
    let mut open: BinaryHeap<Reverse<(i64, usize)>> = BinaryHeap::new();

    let from = start * AXES + NONE;
    best[from] = 0;
    open.push(Reverse((heuristic(start), from)));

    while let Some(Reverse((f, state))) = open.pop() {
        let (p, axis) = (state / AXES, state % AXES);
        // A state reached again more cheaply leaves its old entry behind; the
        // heap has no way to withdraw one, so it is recognised and dropped here.
        if best[state] == i64::MAX || f != best[state] + heuristic(p) {
            continue;
        }
        if p == goal {
            let mut path = vec![p];
            let mut at = state;
            while came[at] != usize::MAX {
                at = came[at];
                path.push(at / AXES);
            }
            path.reverse();
            return Some(path);
        }

        let here = best[state];
        let (x, y) = point(p);
        let (ix, iy) = (p % nx, p / nx);

        for (next, next_axis) in [
            (ix.checked_sub(1).map(|i| i + iy * nx), H),
            ((ix + 1 < nx).then(|| ix + 1 + iy * nx), H),
            (iy.checked_sub(1).map(|j| ix + j * nx), V),
            ((iy + 1 < ys.len()).then(|| ix + (iy + 1) * nx), V),
        ] {
            let Some(next) = next else { continue };
            let to = point(next);
            if obstacles.iter().any(|r| enters((x, y), to, r, clearance)) {
                continue;
            }
            let step = ((to.0 - x).abs() + (to.1 - y).abs()) * GRID;
            let corner = if axis != NONE && axis != next_axis {
                turn
            } else {
                0
            };
            // What this step would cost by running on top of a line already
            // drawn. Priced by how much of it is shared, so brushing past one
            // is cheap and running with it for a screen is not.
            let shared = (overlap((x, y), to, used) * SHARE_COST * GRID) as i64;
            let cost = here + step as i64 + corner + shared;
            let reached = next * AXES + next_axis;
            if cost < best[reached] {
                best[reached] = cost;
                came[reached] = state;
                open.push(Reverse((cost + heuristic(next), reached)));
            }
        }
    }

    None
}

/// How much of an axis-aligned segment lies on top of a run already taken.
///
/// Collinear overlap only: two lines that cross are two lines a reader can
/// follow, and charging for a crossing would make every route wander.
fn overlap(p: (f64, f64), q: (f64, f64), used: &[Used]) -> f64 {
    let mut total = 0.0;
    for ((x0, y0), (x1, y1)) in used {
        let along = |a: f64, b: f64, c: f64, d: f64| {
            (a.max(b).min(c.max(d)) - a.min(b).max(c.min(d))).max(0.0)
        };
        if (p.1 - q.1).abs() < EPS && (y0 - y1).abs() < EPS && (p.1 - y0).abs() < EPS {
            total += along(p.0, q.0, *x0, *x1);
        }
        if (p.0 - q.0).abs() < EPS && (x0 - x1).abs() < EPS && (p.0 - x0).abs() < EPS {
            total += along(p.1, q.1, *y0, *y1);
        }
    }
    total
}

/// Whether an axis-aligned segment comes closer to a box than the clearance.
///
/// The box is grown by `CLEARANCE` before the test, so this asks the question
/// the clearance was always meant to answer: *no run passes nearer than that*.
/// It used to ask only whether the run entered the box, which was the same
/// thing while every candidate line sat exactly a clearance out — and stopped
/// being the same thing the moment contention started creating lanes of its
/// own, one of which hugged an obstacle at five pixels (`X-018`).
///
/// A hair less than the clearance, so a line *on* a channel — which is exactly
/// a clearance away — is still allowed.
fn enters(p: (f64, f64), q: (f64, f64), r: &Rect, m: f64) -> bool {
    p.0.max(q.0) > r.x - m + EPS
        && p.0.min(q.0) < r.right() + m - EPS
        && p.1.max(q.1) > r.y - m + EPS
        && p.1.min(q.1) < r.bottom() + m - EPS
}

/// Drop the points that say nothing: a repeat, or a corner that does not turn.
fn straighten(points: Vec<(f64, f64)>) -> Vec<(f64, f64)> {
    let mut out: Vec<(f64, f64)> = Vec::with_capacity(points.len());
    for p in points {
        if out.last() == Some(&p) {
            continue;
        }
        if out.len() >= 2 {
            let (a, b) = (out[out.len() - 2], out[out.len() - 1]);
            let straight = (a.0 - b.0).abs() < EPS && (b.0 - p.0).abs() < EPS
                || (a.1 - b.1).abs() < EPS && (b.1 - p.1).abs() < EPS;
            if straight && (b.0 - a.0) * (p.0 - b.0) + (b.1 - a.1) * (p.1 - b.1) >= 0.0 {
                out.pop();
            }
        }
        out.push(p);
    }
    out
}

/// Cut the path back to the two boundaries it runs between.
///
/// The search starts and ends at a centre, because a centre is a grid point and
/// a side is four of them. Trimming afterwards is what turns that into an edge
/// that leaves one box and arrives at the other, and it costs nothing: an
/// axis-aligned segment meets a side at one coordinate.
fn clip(points: Vec<(f64, f64)>, a: Rect, b: Rect) -> Option<Vec<(f64, f64)>> {
    let inside = |r: &Rect, (x, y): (f64, f64)| {
        x > r.x + EPS && x < r.right() - EPS && y > r.y + EPS && y < r.bottom() - EPS
    };
    // Where the segment running from a point inside the box to one outside it
    // meets the side. Axis-aligned, so one coordinate is the side and the other
    // is unchanged — the direction of travel picks which side.
    let leave = |inner: (f64, f64), outer: (f64, f64), r: &Rect| {
        if (inner.1 - outer.1).abs() < EPS {
            (if outer.0 > inner.0 { r.right() } else { r.x }, inner.1)
        } else {
            (inner.0, if outer.1 > inner.1 { r.bottom() } else { r.y })
        }
    };

    let head = points.iter().position(|p| !inside(&a, *p))?;
    let tail = points.iter().rposition(|p| !inside(&b, *p))?;
    if head > tail {
        return None;
    }

    let mut out: Vec<(f64, f64)> = Vec::with_capacity(tail - head + 3);
    if head > 0 {
        out.push(leave(points[head - 1], points[head], &a));
    }
    out.extend_from_slice(&points[head..=tail]);
    if tail + 1 < points.len() {
        out.push(leave(points[tail + 1], points[tail], &b));
    }

    // A path of one point is not a line, and a caller that drew it would emit a
    // marker pointing nowhere.
    (out.len() >= 2).then(|| straighten(out))
}
