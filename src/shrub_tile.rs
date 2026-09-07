//! Procedural shrub-tile raster: open-space branch growth (doc02.02). Solid fill box with
//! a vine network in the complement tone; `lean_left` sets the trunk diagonal for mirror grains.

use std::collections::VecDeque;

use rand::Rng;
use rand::SeedableRng;
use rand::rngs::StdRng;

/// Tunables for one shrub tile.
pub struct ShrubTileParams {
    pub canvas: usize,
    pub margin: f32,
    /// `false` = bottom-left→top-right (tone A); `true` = bottom-right→top-left (tone B, mirror grain).
    pub lean_left: bool,
    /// Growth moves: 1=trunk, 2=+branches, 3=+recursion, 4=+void-fill.
    pub stage: usize,
    pub max_extrema: usize,
    pub wave_amp: f32,
    pub amp_jitter: f32,
    pub pos_jitter: f32,
    pub branch_spacing: f32,
    pub branch_curve_ref: f32,
    pub branch_len: f32,
    pub branch_curve_gain: f32,
    pub branch_momentum: f32,
    pub branch_round: f32,
    pub branch_jitter: f32,
    pub branch_fan: f32,
    /// Arc-length near the root over which parent ink is forgiven; past it even the parent truncates.
    pub branch_attach: f32,
    pub branch_min_gap: f32,
    pub branch_shrink: f32,
    pub branch_min_reach: f32,
    pub max_recursion: usize,
    pub void_max_fills: usize,
    pub void_min_dist: f32,
    pub thickness: f32,
    pub mark_blend: f32,
    pub fill_jitter: i32,
}

impl Default for ShrubTileParams {
    fn default() -> Self {
        Self {
            canvas: 48,
            margin: 5.0,
            lean_left: false,
            stage: 4,
            max_extrema: 3,
            wave_amp: 6.0,
            amp_jitter: 0.4,
            pos_jitter: 0.12,
            branch_spacing: 4.0,
            branch_curve_ref: 0.05,
            branch_len: 16.0,
            branch_curve_gain: 0.8,
            branch_momentum: 0.5,
            branch_round: 0.5,
            branch_jitter: 0.15,
            branch_fan: 2.6,
            branch_attach: 6.0,
            branch_min_gap: 7.0,
            branch_shrink: 0.62,
            branch_min_reach: 6.0,
            max_recursion: 2,
            void_max_fills: 8,
            void_min_dist: 5.0,
            thickness: 1.0,
            mark_blend: 0.4,
            fill_jitter: 6,
        }
    }
}

fn blend(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
    let t = t.clamp(0.0, 1.0);
    let mix = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round().clamp(0.0, 255.0) as u8;
    [mix(a[0], b[0]), mix(a[1], b[1]), mix(a[2], b[2])]
}

/// Stamp a filled disc; marks touched pixels in the occupancy `mask` for branch collision.
fn stamp_disc(buf: &mut [u8], n: usize, mask: &mut [bool], cx: f32, cy: f32, r: f32, col: [u8; 3]) {
    let r = r.max(0.5);
    let r2 = r * r;
    let x0 = (cx - r).floor() as i32;
    let x1 = (cx + r).ceil() as i32;
    let y0 = (cy - r).floor() as i32;
    let y1 = (cy + r).ceil() as i32;
    for y in y0..=y1 {
        for x in x0..=x1 {
            if x < 0 || y < 0 || x >= n as i32 || y >= n as i32 {
                continue;
            }
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            if dx * dx + dy * dy <= r2 {
                let cell = y as usize * n + x as usize;
                let idx = cell * 4;
                buf[idx] = col[0];
                buf[idx + 1] = col[1];
                buf[idx + 2] = col[2];
                buf[idx + 3] = 255;
                mask[cell] = true;
            }
        }
    }
}

fn disc_hits(mask: &[bool], n: usize, cx: f32, cy: f32, r: f32) -> bool {
    let r = r.max(0.5);
    let r2 = r * r;
    let x0 = (cx - r).floor() as i32;
    let x1 = (cx + r).ceil() as i32;
    let y0 = (cy - r).floor() as i32;
    let y1 = (cy + r).ceil() as i32;
    for y in y0..=y1 {
        for x in x0..=x1 {
            if x < 0 || y < 0 || x >= n as i32 || y >= n as i32 {
                continue;
            }
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            if dx * dx + dy * dy <= r2 && mask[y as usize * n + x as usize] {
                return true;
            }
        }
    }
    false
}

/// Vertex distance approximates curve distance because curves are sampled densely.
fn dist_to_poly(pt: (f32, f32), poly: &[(f32, f32)]) -> f32 {
    poly.iter()
        .map(|q| (q.0 - pt.0).hypot(q.1 - pt.1))
        .fold(f32::INFINITY, f32::min)
}

/// Like `disc_hits` but forgives ink within `parent_clear` of `parent` — only reports foreign collisions.
fn disc_collides(mask: &[bool], n: usize, cx: f32, cy: f32, r: f32, parent: &[(f32, f32)], parent_clear: f32) -> bool {
    let r = r.max(0.5);
    let r2 = r * r;
    let x0 = (cx - r).floor() as i32;
    let x1 = (cx + r).ceil() as i32;
    let y0 = (cy - r).floor() as i32;
    let y1 = (cy + r).ceil() as i32;
    for y in y0..=y1 {
        for x in x0..=x1 {
            if x < 0 || y < 0 || x >= n as i32 || y >= n as i32 {
                continue;
            }
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            if dx * dx + dy * dy <= r2
                && mask[y as usize * n + x as usize]
                && dist_to_poly((x as f32, y as f32), parent) > parent_clear
            {
                return true;
            }
        }
    }
    false
}

/// How far a ray runs before hitting the margin or foreign ink — used to find the most open aim direction.
#[allow(clippy::too_many_arguments)]
fn open_run(mask: &[bool], n: usize, from: (f32, f32), dir: (f32, f32), maxlen: f32, r: f32, parent: &[(f32, f32)], parent_clear: f32, margin: f32) -> f32 {
    let lo = margin;
    let hi = n as f32 - margin;
    let mut d = 1.0;
    while d <= maxlen {
        let x = from.0 + dir.0 * d;
        let y = from.1 + dir.1 * d;
        if x < lo || y < lo || x > hi || y > hi || disc_collides(mask, n, x, y, r, parent, parent_clear) {
            return (d - 1.0).max(0.0);
        }
        d += 1.0;
    }
    maxlen
}

fn catmull_rom(p0: (f32, f32), p1: (f32, f32), p2: (f32, f32), p3: (f32, f32), t: f32) -> (f32, f32) {
    let t2 = t * t;
    let t3 = t2 * t;
    let f = |a: f32, b: f32, c: f32, d: f32| {
        0.5 * ((2.0 * b)
            + (-a + c) * t
            + (2.0 * a - 5.0 * b + 4.0 * c - d) * t2
            + (-a + 3.0 * b - 3.0 * c + d) * t3)
    };
    (f(p0.0, p1.0, p2.0, p3.0), f(p0.1, p1.1, p2.1, p3.1))
}

fn spline_samples(way: &[(f32, f32)], steps_per_seg: usize) -> Vec<(f32, f32)> {
    if way.len() < 2 {
        return way.to_vec();
    }
    let mut out = Vec::new();
    for i in 0..way.len() - 1 {
        let p0 = if i == 0 { way[0] } else { way[i - 1] };
        let p1 = way[i];
        let p2 = way[i + 1];
        let p3 = if i + 2 < way.len() { way[i + 2] } else { way[way.len() - 1] };
        for s in 0..steps_per_seg {
            out.push(catmull_rom(p0, p1, p2, p3, s as f32 / steps_per_seg as f32));
        }
    }
    out.push(way[way.len() - 1]);
    out
}

fn cubic_bezier(p0: (f32, f32), c1: (f32, f32), c2: (f32, f32), p3: (f32, f32), t: f32) -> (f32, f32) {
    let u = 1.0 - t;
    let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
    (
        a * p0.0 + b * c1.0 + c * c2.0 + d * p3.0,
        a * p0.1 + b * c1.1 + c * c2.1 + d * p3.1,
    )
}

/// Unit tangent and signed curvature per vertex; neighbour window smooths the estimate.
fn differentials(pts: &[(f32, f32)]) -> Vec<((f32, f32), (f32, f32), f32)> {
    let n = pts.len();
    let w = 3;
    (0..n)
        .map(|i| {
            let a = pts[i.saturating_sub(w)];
            let b = pts[i];
            let c = pts[(i + w).min(n - 1)];
            let (tx, ty) = (c.0 - a.0, c.1 - a.1);
            let tl = (tx * tx + ty * ty).sqrt().max(1e-3);
            let tang = (tx / tl, ty / tl);
            let ab = (b.0 - a.0, b.1 - a.1);
            let bc = (c.0 - b.0, c.1 - b.1);
            let cross = ab.0 * bc.1 - ab.1 * bc.0;
            let dot = ab.0 * bc.0 + ab.1 * bc.1;
            let lab = (ab.0 * ab.0 + ab.1 * ab.1).sqrt();
            let lbc = (bc.0 * bc.0 + bc.1 * bc.1).sqrt();
            let len = ((lab + lbc) / 2.0).max(1e-3);
            let kappa = cross.atan2(dot) / len; // signed turn per length
            (b, tang, kappa)
        })
        .collect()
}

/// Stroke a round arc from `base` to `dest`, truncating at the first foreign-ink collision.
#[allow(clippy::too_many_arguments)]
fn sweep_to(buf: &mut [u8], n: usize, detail: [u8; 3], mask: &mut [bool], parent: &[(f32, f32)], p: &ShrubTileParams, base: (f32, f32), tang: (f32, f32), dest: (f32, f32), side: f32) -> Vec<(f32, f32)> {
    let parent_clear = p.thickness + 1.5;
    let chord = (dest.0 - base.0, dest.1 - base.1);
    let cd = (chord.0 * chord.0 + chord.1 * chord.1).sqrt().max(1e-3);
    let c1 = (base.0 + tang.0 * cd * p.branch_momentum, base.1 + tang.1 * cd * p.branch_momentum);
    let cperp = (-chord.1 / cd, chord.0 / cd);
    let mid = ((base.0 + dest.0) / 2.0, (base.1 + dest.1) / 2.0);
    let c2 = (mid.0 + cperp.0 * side * cd * p.branch_round, mid.1 + cperp.1 * side * cd * p.branch_round);

    let steps = (cd * 1.5).ceil().max(8.0) as usize;
    let pts: Vec<(f32, f32)> = (0..=steps)
        .map(|s| {
            let q = cubic_bezier(base, c1, c2, dest, s as f32 / steps as f32);
            (q.0.clamp(0.0, n as f32 - 1.0), q.1.clamp(0.0, n as f32 - 1.0))
        })
        .collect();

    // `mask` doesn't yet hold this curve's pixels, so the check never self-truncates.
    let mut cut = pts.len();
    let mut s = 0.0f32;
    for i in 0..pts.len() {
        if i > 0 {
            s += (pts[i].0 - pts[i - 1].0).hypot(pts[i].1 - pts[i - 1].1);
        }
        let q = pts[i];
        let collide = if s < p.branch_attach {
            disc_collides(mask, n, q.0, q.1, p.thickness, parent, parent_clear)
        } else {
            disc_hits(mask, n, q.0, q.1, p.thickness)
        };
        if i > 0 && collide {
            cut = i;
            break;
        }
    }
    let drawn = pts[..cut].to_vec();
    for q in &drawn {
        stamp_disc(buf, n, mask, q.0, q.1, p.thickness, detail);
    }
    drawn
}

/// Draw one branch toward the most open direction on the curve's convex side, probing a fan.
#[allow(clippy::too_many_arguments)]
fn draw_branch(buf: &mut [u8], n: usize, detail: [u8; 3], mask: &mut [bool], parent: &[(f32, f32)], rng: &mut StdRng, p: &ShrubTileParams, base: (f32, f32), tang: (f32, f32), kappa: f32, depth: usize) -> Vec<(f32, f32)> {
    // Convex side is opposite the turn direction.
    let side = if kappa >= 0.0 { -1.0 } else { 1.0 };
    let curv_w = (kappa.abs() / p.branch_curve_ref).min(1.0);
    let reach = p.branch_len * (1.0 + p.branch_curve_gain * curv_w) * p.branch_shrink.powi(depth as i32);
    if reach < p.branch_min_reach {
        return Vec::new();
    }
    let parent_clear = p.thickness + 1.5;

    let cand = 9usize;
    let a_min = 0.25f32;
    let a_max = p.branch_fan.max(a_min + 0.1);
    let mut best_dir = tang;
    let mut best_run = -1.0f32;
    for c in 0..cand {
        let frac = c as f32 / (cand - 1) as f32;
        let ang = (a_min + (a_max - a_min) * frac) * side;
        let (ca, sa) = (ang.cos(), ang.sin());
        let dir = (tang.0 * ca - tang.1 * sa, tang.0 * sa + tang.1 * ca);
        let run = open_run(mask, n, base, dir, reach, p.thickness, parent, parent_clear, p.margin);
        if run > best_run {
            best_run = run;
            best_dir = dir;
        }
    }
    if best_run < p.branch_min_reach {
        return Vec::new();
    }

    let jit = rng.random_range(-p.branch_jitter..=p.branch_jitter);
    let (cj, sj) = (jit.cos(), jit.sin());
    let dir = (best_dir.0 * cj - best_dir.1 * sj, best_dir.0 * sj + best_dir.1 * cj);
    let lo = p.margin;
    let hi = (n as f32 - p.margin).max(lo);
    let aim = (best_run * 0.95).min(reach);
    let dest = (
        (base.0 + dir.0 * aim).clamp(lo, hi),
        (base.1 + dir.1 * aim).clamp(lo, hi),
    );

    sweep_to(buf, n, detail, mask, parent, p, base, tang, dest, side)
}

/// Branch pass over a parent curve; at depth 0 (trunk) forces one branch on each sign of curvature.
fn branches_off(buf: &mut [u8], n: usize, detail: [u8; 3], mask: &mut [bool], rng: &mut StdRng, p: &ShrubTileParams, parent: &[(f32, f32)], depth: usize) -> Vec<Vec<(f32, f32)>> {
    let mut children = Vec::new();
    if parent.len() < 2 || p.branch_spacing <= 0.0 {
        return children;
    }
    let diff = differentials(parent);

    let mut arclen = vec![0.0f32; parent.len()];
    for i in 1..parent.len() {
        arclen[i] = arclen[i - 1] + (parent[i].0 - parent[i - 1].0).hypot(parent[i].1 - parent[i - 1].1);
    }
    let total = arclen[parent.len() - 1];
    let (lo_a, hi_a) = (total * 0.12, total * 0.88);
    let interior = |i: usize| arclen[i] >= lo_a && arclen[i] <= hi_a;

    let mut placed: Vec<f32> = Vec::new();

    // Forced both-sign: ensures both sides of the trunk always get a branch.
    if depth == 0 {
        let mut best_pos: Option<usize> = None;
        let mut best_neg: Option<usize> = None;
        for i in 0..parent.len() {
            if !interior(i) {
                continue;
            }
            let k = diff[i].2;
            if k > 0.0 && best_pos.is_none_or(|j| k > diff[j].2) {
                best_pos = Some(i);
            }
            if k < 0.0 && best_neg.is_none_or(|j| k < diff[j].2) {
                best_neg = Some(i);
            }
        }
        for opt in [best_pos, best_neg].into_iter().flatten() {
            let (base, tang, kappa) = diff[opt];
            let child = draw_branch(buf, n, detail, mask, parent, rng, p, base, tang, kappa, depth);
            if child.len() >= 2 {
                children.push(child);
            }
            placed.push(arclen[opt]);
        }
    }

    let mut next = p.branch_spacing;
    for i in 1..parent.len() {
        if arclen[i] < next {
            continue;
        }
        next += p.branch_spacing;
        if !interior(i) {
            continue;
        }
        let kappa = diff[i].2;
        let prob = (kappa.abs() / p.branch_curve_ref).clamp(0.0, 1.0);
        let clear = placed.iter().all(|q| (q - arclen[i]).abs() >= p.branch_min_gap);
        if clear && rng.random::<f32>() < prob {
            let (base, tang, kp) = diff[i];
            let child = draw_branch(buf, n, detail, mask, parent, rng, p, base, tang, kp, depth);
            if child.len() >= 2 {
                children.push(child);
            }
            placed.push(arclen[i]);
        }
    }
    children
}

fn trunk_waypoints(p: &ShrubTileParams, rng: &mut StdRng, start: (f32, f32), end: (f32, f32)) -> Vec<(f32, f32)> {
    let (dx, dy) = (end.0 - start.0, end.1 - start.1);
    let len = (dx * dx + dy * dy).sqrt().max(1e-3);
    let (perpx, perpy) = (-dy / len, dx / len);
    // Minimum 2 extrema so the trunk bends both ways and each side can branch.
    let k = rng.random_range(2..=p.max_extrema.max(2));

    let mut way = vec![start];
    for i in 1..=k {
        let base_t = i as f32 / (k + 1) as f32;
        let t = (base_t + rng.random_range(-p.pos_jitter..=p.pos_jitter)).clamp(0.05, 0.95);
        let side = if i % 2 == 1 { 1.0 } else { -1.0 };
        let amp = p.wave_amp * (1.0 + rng.random_range(-p.amp_jitter..=p.amp_jitter));
        way.push((
            start.0 + dx * t + perpx * side * amp,
            start.1 + dy * t + perpy * side * amp,
        ));
    }
    way.push(end);
    way
}

fn draw_trunk(buf: &mut [u8], n: usize, detail: [u8; 3], mask: &mut [bool], rng: &mut StdRng, p: &ShrubTileParams) -> Vec<(f32, f32)> {
    let lo = p.margin;
    let hi = (n as f32 - p.margin).max(lo);
    // y increases downward, so "bottom" is high y and "top" is low y.
    let (start, end) = if p.lean_left {
        ((hi, hi), (lo, lo)) // bottom-right → top-left (up and to the left)
    } else {
        ((lo, hi), (hi, lo)) // bottom-left → top-right (up and to the right)
    };
    let way = trunk_waypoints(p, rng, start, end);
    let pts = spline_samples(&way, 12);
    for q in &pts {
        let cx = q.0.clamp(0.0, n as f32 - 1.0);
        let cy = q.1.clamp(0.0, n as f32 - 1.0);
        stamp_disc(buf, n, mask, cx, cy, p.thickness, detail);
    }
    pts
}

/// Multi-source BFS distance from every inked pixel; high scores mark void centres.
fn distance_to_ink(mask: &[bool], n: usize) -> Vec<i32> {
    let mut dist = vec![i32::MAX; n * n];
    let mut q: VecDeque<usize> = VecDeque::new();
    for (i, &m) in mask.iter().enumerate() {
        if m {
            dist[i] = 0;
            q.push_back(i);
        }
    }
    while let Some(c) = q.pop_front() {
        let (cx, cy) = ((c % n) as i32, (c / n) as i32);
        for dy in -1..=1 {
            for dx in -1..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let (nx, ny) = (cx + dx, cy + dy);
                if nx < 0 || ny < 0 || nx >= n as i32 || ny >= n as i32 {
                    continue;
                }
                let nc = ny as usize * n + nx as usize;
                if dist[nc] > dist[c] + 1 {
                    dist[nc] = dist[c] + 1;
                    q.push_back(nc);
                }
            }
        }
    }
    dist
}

fn nearest_curve_point(curves: &[Vec<(f32, f32)>], target: (f32, f32)) -> Option<(usize, usize)> {
    let mut best = f32::INFINITY;
    let mut res = None;
    for (ci, c) in curves.iter().enumerate() {
        for (pi, q) in c.iter().enumerate() {
            let d = (q.0 - target.0).hypot(q.1 - target.1);
            if d < best {
                best = d;
                res = Some((ci, pi));
            }
        }
    }
    res
}

fn point_and_tangent(poly: &[(f32, f32)], i: usize) -> ((f32, f32), (f32, f32)) {
    let w = 3;
    let a = poly[i.saturating_sub(w)];
    let b = poly[i];
    let c = poly[(i + w).min(poly.len() - 1)];
    let (tx, ty) = (c.0 - a.0, c.1 - a.1);
    let tl = (tx * tx + ty * ty).sqrt().max(1e-3);
    (b, (tx / tl, ty / tl))
}

/// Fill remaining voids: find the emptiest pocket, route from the nearest curve toward it; repeat.
fn fill_voids(buf: &mut [u8], n: usize, detail: [u8; 3], mask: &mut [bool], p: &ShrubTileParams, curves: &mut Vec<Vec<(f32, f32)>>) {
    let parent_clear = p.thickness + 1.5;
    let lo = p.margin;
    let hi = (n as f32 - p.margin).max(lo);
    for _ in 0..p.void_max_fills {
        let dist = distance_to_ink(mask, n);

        let mut best = -1;
        let mut target = (0.0f32, 0.0f32);
        for y in 0..n {
            for x in 0..n {
                let (fx, fy) = (x as f32, y as f32);
                if fx < lo || fy < lo || fx > hi || fy > hi {
                    continue;
                }
                let d = dist[y * n + x];
                if d != i32::MAX && d > best {
                    best = d;
                    target = (fx, fy);
                }
            }
        }
        if (best as f32) < p.void_min_dist {
            break;
        }

        let Some((ci, pi)) = nearest_curve_point(curves, target) else {
            break;
        };
        let parent = curves[ci].clone();
        let (base, tang) = point_and_tangent(&parent, pi);

        let raw = (target.0 - base.0, target.1 - base.1);
        let rl = (raw.0 * raw.0 + raw.1 * raw.1).sqrt().max(1e-3);
        let dir = (raw.0 / rl, raw.1 / rl);
        let run = open_run(mask, n, base, dir, rl, p.thickness, &parent, parent_clear, p.margin);
        let aim = (run * 0.95).max(0.0);
        let dest = (
            (base.0 + dir.0 * aim).clamp(lo, hi),
            (base.1 + dir.1 * aim).clamp(lo, hi),
        );
        let cross = tang.0 * (dest.1 - base.1) - tang.1 * (dest.0 - base.0);
        let side = if cross >= 0.0 { 1.0 } else { -1.0 };

        let drawn = sweep_to(buf, n, detail, mask, &parent, p, base, tang, dest, side);
        if drawn.len() < 2 {
            break;
        }
        curves.push(drawn);
    }
}

/// Rasterize a shrub tile: `canvas*canvas*4` RGBA8, fully opaque. `fill` = ground tone;
/// `detail` = complement tone for vines. Deterministic in `seed`.
pub fn rasterize_shrub(p: &ShrubTileParams, fill: [u8; 3], detail: [u8; 3], seed: u64) -> Vec<u8> {
    let n = p.canvas;
    let mut buf = vec![0u8; n * n * 4];
    if n == 0 {
        return buf;
    }
    let mut rng = StdRng::seed_from_u64(seed);
    let mut mask = vec![false; n * n]; // detail-ink occupancy

    for px in buf.chunks_exact_mut(4) {
        let j = rng.random_range(-p.fill_jitter..=p.fill_jitter);
        px[0] = (fill[0] as i32 + j).clamp(0, 255) as u8;
        px[1] = (fill[1] as i32 + j).clamp(0, 255) as u8;
        px[2] = (fill[2] as i32 + j).clamp(0, 255) as u8;
        px[3] = 255;
    }

    let mark = blend(fill, detail, p.mark_blend);

    if p.stage >= 1 {
        let trunk = draw_trunk(&mut buf, n, mark, &mut mask, &mut rng, p);

        let mut all_curves = vec![trunk.clone()];
        let mut parents = vec![trunk];
        let recursion = p.stage.saturating_sub(1).min(p.max_recursion);
        for depth in 0..recursion {
            let mut next_parents = Vec::new();
            for parent in &parents {
                let children = branches_off(&mut buf, n, mark, &mut mask, &mut rng, p, parent, depth);
                all_curves.extend(children.iter().cloned());
                next_parents.extend(children);
            }
            parents = next_parents;
        }

        if p.stage > p.max_recursion + 1 {
            fill_voids(&mut buf, n, mark, &mut mask, p, &mut all_curves);
        }
    }

    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d2(a: [u8; 3], b: [u8; 3]) -> i32 {
        let dr = a[0] as i32 - b[0] as i32;
        let dg = a[1] as i32 - b[1] as i32;
        let db = a[2] as i32 - b[2] as i32;
        dr * dr + dg * dg + db * db
    }

    fn mark_count(buf: &[u8], fill: [u8; 3], mark: [u8; 3]) -> usize {
        buf.chunks_exact(4)
            .filter(|px| {
                let c = [px[0], px[1], px[2]];
                d2(c, mark) < d2(c, fill)
            })
            .count()
    }

    #[test]
    fn fills_the_tile_as_an_opaque_box() {
        let p = ShrubTileParams::default();
        let buf = rasterize_shrub(&p, [30, 80, 25], [90, 100, 25], 1);
        assert!(
            buf.chunks_exact(4).all(|px| px[3] == 255),
            "the shrub fills its tile — no transparent pixels (keeps the box)"
        );
    }

    #[test]
    fn grows_vines_over_a_fill_ground() {
        let p = ShrubTileParams::default();
        let fill = [30, 80, 25];
        let detail = [200, 200, 40];
        let buf = rasterize_shrub(&p, fill, detail, 7);
        let mark = blend(fill, detail, p.mark_blend);
        let total = p.canvas * p.canvas;
        let vines = mark_count(&buf, fill, mark);
        assert!(vines > 0, "the vine network is drawn");
        assert!(vines < total, "the fill ground still shows through");
    }

    #[test]
    fn stage_zero_is_bare_fill() {
        let p = ShrubTileParams { stage: 0, ..ShrubTileParams::default() };
        let (fill, detail) = ([30, 80, 25], [200, 200, 40]);
        let buf = rasterize_shrub(&p, fill, detail, 7);
        let mark = blend(fill, detail, p.mark_blend);
        assert_eq!(mark_count(&buf, fill, mark), 0, "no marks at stage 0");
    }

    #[test]
    fn branches_always_appear() {
        let one = ShrubTileParams { stage: 1, ..ShrubTileParams::default() };
        let two = ShrubTileParams { stage: 2, ..ShrubTileParams::default() };
        let (fill, detail) = ([30, 80, 25], [200, 200, 40]);
        let mark = blend(fill, detail, one.mark_blend);
        for seed in 0..40 {
            let trunk_only = mark_count(&rasterize_shrub(&one, fill, detail, seed), fill, mark);
            let with_branches = mark_count(&rasterize_shrub(&two, fill, detail, seed), fill, mark);
            assert!(with_branches > trunk_only, "seed {seed}: branches must appear");
        }
    }

    #[test]
    fn deeper_passes_add_more_recursion() {
        let (fill, detail) = ([30, 80, 25], [200, 200, 40]);
        let mark = blend(fill, detail, ShrubTileParams::default().mark_blend);
        for seed in 0..20 {
            let s2 = ShrubTileParams { stage: 2, ..ShrubTileParams::default() };
            let s3 = ShrubTileParams { stage: 3, ..ShrubTileParams::default() };
            let c2 = mark_count(&rasterize_shrub(&s2, fill, detail, seed), fill, mark);
            let c3 = mark_count(&rasterize_shrub(&s3, fill, detail, seed), fill, mark);
            assert!(c3 >= c2, "seed {seed}: a deeper pass never removes ink");
        }
    }

    #[test]
    fn void_fill_never_removes_ink() {
        let (fill, detail) = ([30, 80, 25], [200, 200, 40]);
        let mark = blend(fill, detail, ShrubTileParams::default().mark_blend);
        let s3 = ShrubTileParams { stage: 3, ..ShrubTileParams::default() };
        let s4 = ShrubTileParams { stage: 4, ..ShrubTileParams::default() };
        for seed in 0..20 {
            let c3 = mark_count(&rasterize_shrub(&s3, fill, detail, seed), fill, mark);
            let c4 = mark_count(&rasterize_shrub(&s4, fill, detail, seed), fill, mark);
            assert!(c4 >= c3, "seed {seed}: void fill never removes ink");
        }
    }

    #[test]
    fn lean_mirrors_the_trunk() {
        let right = ShrubTileParams { lean_left: false, ..ShrubTileParams::default() };
        let left = ShrubTileParams { lean_left: true, ..ShrubTileParams::default() };
        let (fill, detail) = ([30, 80, 25], [90, 100, 25]);
        assert_ne!(
            rasterize_shrub(&right, fill, detail, 3),
            rasterize_shrub(&left, fill, detail, 3),
        );
    }

    #[test]
    fn deterministic_in_seed() {
        let p = ShrubTileParams::default();
        let (fill, detail) = ([30, 80, 25], [90, 100, 25]);
        assert_eq!(
            rasterize_shrub(&p, fill, detail, 4),
            rasterize_shrub(&p, fill, detail, 4),
            "same tones and seed → identical tile"
        );
        assert_ne!(
            rasterize_shrub(&p, fill, detail, 4),
            rasterize_shrub(&p, fill, detail, 5),
            "a different seed grows a different tangle"
        );
    }

    #[test]
    fn blend_interpolates_between_tones() {
        let fill = [30, 80, 25];
        let detail = [90, 100, 25];
        assert_eq!(blend(fill, detail, 0.0), fill, "t=0 is the base tone");
        assert_eq!(blend(fill, detail, 1.0), detail, "t=1 is the complement");
        let mark = blend(fill, detail, 0.4);
        assert!(d2(mark, fill) < d2(mark, detail), "0.4 sits closer to the base tone");
    }

    #[test]
    fn degenerate_canvas_is_empty() {
        let p = ShrubTileParams { canvas: 0, ..ShrubTileParams::default() };
        assert!(rasterize_shrub(&p, [0, 0, 0], [1, 1, 1], 1).is_empty());
    }
}
