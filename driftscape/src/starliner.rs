//! Starliner — multicolored planets fly past a spaceship window (doc03.03.01).
//!
//! The camera sits at the origin looking down +z; the ship flies forward so
//! every object's z shrinks over time. A single divide-by-z projection turns
//! that into perspective: a planet emerges small near the vanishing point,
//! grows as it nears, and slews off the side as it passes the glass. Stars
//! ride the same projection and streak outward — the warp-flyby field. Each
//! planet is a flat-lit disc — no dark side — drawn as a bright outline ring
//! with a tight specular spot where it faces the light; when an object reaches
//! the camera it respawns far away with fresh parameters, scattered across a
//! depth-band so the field never pulses and never repeats.

use crate::canvas::Canvas;
use crate::color::{hsv_to_rgb, scale, Rgb};
use crate::scene::Scene;
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};

const PLANET_COUNT: usize = 14;
/// Field-of-view: focal length is this fraction of the canvas width, so the
/// FOV reads the same at any terminal size.
const FOV_FRAC: f32 = 0.9;
/// Nearest z before an object is considered past the camera and respawns.
const Z_NEAR: f32 = 1.0;
/// Farthest z an object spawns at — the vanishing-point distance.
const Z_FAR: f32 = 60.0;
/// World depth-band a respawn scatters across, just short of the far plane.
/// Without it every object would wink in at exactly `Z_FAR` on the same tick,
/// so the field pulses; scattering the respawn distance keeps it continuous.
const RESPAWN_JITTER: f32 = 10.0;
/// World units/sec the ship flies forward (objects' z shrinks by this).
const SHIP_SPEED: f32 = 14.0;
/// Gentle lateral world units/sec the ship drifts, so the flyby isn't symmetric.
const SHIP_DRIFT: f32 = 0.6;
/// World half-width of the planet spawn box (off-axis spread at the far plane).
const LATERAL_SPREAD: f32 = 22.0;
/// Stars spread wider than planets so the field wraps around the flight path.
const STAR_SPREAD: f32 = 40.0;
/// Real radius of a planet in world units (min..max at spawn).
const MIN_RADIUS: f32 = 0.6;
const MAX_RADIUS: f32 = 3.2;
/// Stars per pixel-cell of frame area, sets the field density.
const STAR_DENSITY: f32 = 0.012;
const STAR_COLOR: Rgb = (180, 200, 255);
const SPACE_BG: Rgb = (4, 3, 12);
/// Flat fill brightness of a planet's body — the same everywhere, so the disc
/// has no dark side, only a uniformly lit face under its outline and highlight.
const BODY_LEVEL: f32 = 0.72;
/// Where the outline ring begins, as a squared-radius fraction of the disc.
/// Pixels past this light up to `OUTLINE_GAIN`, drawing the planet's edge.
const EDGE_START: f32 = 0.82;
const OUTLINE_GAIN: f32 = 1.3;
/// The specular bright spot: `ndotl ^ SPOT_POWER` keeps it a tight highlight
/// rather than a whole lit hemisphere, lifted `SPOT_GAIN` above the body.
const SPOT_POWER: f32 = 6.0;
const SPOT_GAIN: f32 = 0.7;
/// Light direction (unit), upper-left and toward the viewer.
const LIGHT: (f32, f32, f32) = (-0.471, -0.589, 0.657);

struct Planet {
    x: f32,
    y: f32,
    /// Distance down the +z axis. Shrinks over time; respawns at Z_FAR.
    z: f32,
    /// Real radius in world units; on-screen size is `world_r / z`-scaled.
    world_r: f32,
    hue: f32,
    sat: f32,
}

struct Star {
    x: f32,
    y: f32,
    z: f32,
}

pub struct Starliner {
    cols: f32,
    ph: f32,
    /// Focal length in pixels (derived from FOV_FRAC * cols).
    focal: f32,
    planets: Vec<Planet>,
    stars: Vec<Star>,
    rng: SmallRng,
}

/// Perspective projection: world `(x, y, z)` to pixel coordinates about the
/// canvas center `(cx, cy)`. The single `1/z` is the whole illusion — an
/// off-axis point's `x/z` grows as z shrinks, so it slides outward on approach.
fn project(x: f32, y: f32, z: f32, focal: f32, cx: f32, cy: f32) -> (f32, f32) {
    (cx + focal * x / z, cy + focal * y / z)
}

/// On-screen radius of a sphere of real radius `world_r` at distance `z`.
/// Strictly larger as z shrinks, so planets grow as they approach.
fn screen_radius(world_r: f32, z: f32, focal: f32) -> f32 {
    focal * world_r / z
}

/// A gentle surface-brightness multiplier in `0.85..=1.0`, hashed from the
/// quantized surface normal so the texture sticks to the sphere as it turns and
/// nears — a funky stipple that mottles the body without darkening it. A classic
/// sin-fract hash, quantized coarse so it reads as blotches, not noise.
fn surface_stipple(nx: f32, ny: f32, nz: f32) -> f32 {
    let q = |v: f32| (v * 7.0).floor();
    let h = q(nx) * 73.0 + q(ny) * 179.0 + q(nz) * 283.0;
    let s = (h.sin() * 43758.547).fract().abs();
    0.85 + 0.15 * s
}

/// Brightness of a planet pixel from its squared radius `d2` (0 center, 1 rim)
/// and `ndotl` (how squarely it faces the light). The body is a flat `BODY_LEVEL`
/// with no dark side; the rim lights up as an outline and the light-facing pole
/// keeps a tight specular spot. The result never dips below `BODY_LEVEL`.
fn planet_shade(d2: f32, ndotl: f32) -> f32 {
    let edge = ((d2 - EDGE_START) / (1.0 - EDGE_START)).clamp(0.0, 1.0);
    let outline = edge * edge * OUTLINE_GAIN;
    let spot = BODY_LEVEL + ndotl.powf(SPOT_POWER) * SPOT_GAIN;
    outline.max(spot)
}

impl Starliner {
    pub fn new(cols: usize, rows: usize, seed: u64) -> Self {
        let mut rng = SmallRng::seed_from_u64(seed);
        let cols_f = cols as f32;
        let ph = (rows * 2) as f32;
        let focal = FOV_FRAC * cols_f;

        // Planets spawn staggered in z so some are always near and some far.
        let planets = (0..PLANET_COUNT)
            .map(|_| Planet {
                x: rng.random_range(-LATERAL_SPREAD..LATERAL_SPREAD),
                y: rng.random_range(-LATERAL_SPREAD..LATERAL_SPREAD),
                z: rng.random_range(Z_NEAR..Z_FAR),
                world_r: rng.random_range(MIN_RADIUS..MAX_RADIUS),
                hue: rng.random_range(0.0..360.0),
                sat: rng.random_range(0.55..0.95),
            })
            .collect();

        let star_count = ((cols_f * ph) * STAR_DENSITY) as usize;
        let stars = (0..star_count)
            .map(|_| Star {
                x: rng.random_range(-STAR_SPREAD..STAR_SPREAD),
                y: rng.random_range(-STAR_SPREAD..STAR_SPREAD),
                z: rng.random_range(Z_NEAR..Z_FAR),
            })
            .collect();

        Self { cols: cols_f, ph, focal, planets, stars, rng }
    }

    /// Re-roll a planet at the far plane once it has passed the camera.
    fn respawn_planet(&mut self, i: usize) {
        let p = &mut self.planets[i];
        p.x = self.rng.random_range(-LATERAL_SPREAD..LATERAL_SPREAD);
        p.y = self.rng.random_range(-LATERAL_SPREAD..LATERAL_SPREAD);
        p.z = Z_FAR - self.rng.random_range(0.0..RESPAWN_JITTER);
        p.world_r = self.rng.random_range(MIN_RADIUS..MAX_RADIUS);
        p.hue = self.rng.random_range(0.0..360.0);
        p.sat = self.rng.random_range(0.55..0.95);
    }

    fn respawn_star(&mut self, i: usize) {
        let s = &mut self.stars[i];
        s.x = self.rng.random_range(-STAR_SPREAD..STAR_SPREAD);
        s.y = self.rng.random_range(-STAR_SPREAD..STAR_SPREAD);
        s.z = Z_FAR - self.rng.random_range(0.0..RESPAWN_JITTER);
    }

    fn draw_planet(&self, canvas: &mut Canvas, p: &Planet) {
        let (cx, cy) = project(p.x, p.y, p.z, self.focal, self.cols / 2.0, self.ph / 2.0);
        let r = screen_radius(p.world_r, p.z, self.focal);
        if r < 0.5 {
            // Sub-pixel: a far planet reads as a single dim point.
            canvas.put(cx as i32, cy as i32, scale(hsv_to_rgb(p.hue, p.sat, 1.0), 0.5));
            return;
        }
        let base = hsv_to_rgb(p.hue, p.sat, 1.0);
        // Nearness brightens the planet as it approaches (clamped ~1/z exposure).
        let exposure = (0.55 + Z_NEAR / p.z).min(1.3);
        let (x0, x1) = ((cx - r).floor() as i32, (cx + r).ceil() as i32);
        let (y0, y1) = ((cy - r).floor() as i32, (cy + r).ceil() as i32);
        for py in y0..=y1 {
            for px in x0..=x1 {
                let dx = (px as f32 + 0.5 - cx) / r;
                let dy = (py as f32 + 0.5 - cy) / r;
                let d2 = dx * dx + dy * dy;
                if d2 > 1.0 {
                    continue;
                }
                // Surface normal of the sphere at this pixel.
                let z = (1.0 - d2).sqrt();
                let ndotl = (dx * LIGHT.0 + dy * LIGHT.1 + z * LIGHT.2).max(0.0);
                let intensity = planet_shade(d2, ndotl) * exposure * surface_stipple(dx, dy, z);
                canvas.put(px, py, scale(base, intensity));
            }
        }
    }
}

impl Scene for Starliner {
    fn step(&mut self, dt: f32) {
        for i in 0..self.planets.len() {
            let p = &mut self.planets[i];
            p.z -= SHIP_SPEED * dt;
            p.x -= SHIP_DRIFT * dt;
            if p.z < Z_NEAR {
                self.respawn_planet(i);
            }
        }
        for i in 0..self.stars.len() {
            let s = &mut self.stars[i];
            s.z -= SHIP_SPEED * dt;
            s.x -= SHIP_DRIFT * dt;
            if s.z < Z_NEAR {
                self.respawn_star(i);
            }
        }
    }

    fn draw(&self, canvas: &mut Canvas) {
        canvas.clear(SPACE_BG);
        let (cx, cy) = (self.cols / 2.0, self.ph / 2.0);

        // Star streaks: a segment from where the star was last frame to now, so
        // the streak lengthens as the star nears — the classic warp field.
        for s in &self.stars {
            let (x1, y1) = project(s.x, s.y, s.z, self.focal, cx, cy);
            // Previous position is one ship-step farther away.
            let prev_z = s.z + SHIP_SPEED * (1.0 / 30.0);
            let (x0, y0) = project(s.x + SHIP_DRIFT * (1.0 / 30.0), s.y, prev_z, self.focal, cx, cy);
            // Nearer stars shine brighter (clamped ~1/z).
            let bright = (0.35 + Z_NEAR / s.z).min(1.0);
            canvas.line(x0 as i32, y0 as i32, x1 as i32, y1 as i32, scale(STAR_COLOR, bright));
        }

        // Painter's order: far planets first (large z) so near ones overwrite.
        let mut order: Vec<&Planet> = self.planets.iter().collect();
        order.sort_by(|a, b| b.z.partial_cmp(&a.z).unwrap());
        for p in order {
            self.draw_planet(canvas, p);
        }
    }

    fn name(&self) -> &'static str {
        "starliner"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn off_axis_point_slews_outward_as_it_nears() {
        // Same off-axis world point projects farther from center at small z.
        let (cx, cy) = (40.0, 24.0);
        let (far_x, _) = project(10.0, 0.0, 50.0, 72.0, cx, cy);
        let (near_x, _) = project(10.0, 0.0, 5.0, 72.0, cx, cy);
        assert!(
            (near_x - cx).abs() > (far_x - cx).abs(),
            "an off-axis object must slide outward as z shrinks"
        );
    }

    #[test]
    fn surface_stipple_stays_in_band() {
        // The mottle only ever dims the surface a little — never to black, never
        // brighter than lit — so the planet stays solid and the texture is subtle.
        for &(x, y, z) in &[(0.0, 0.0, 1.0), (0.5, -0.3, 0.81), (-0.9, 0.1, 0.42)] {
            let s = surface_stipple(x, y, z);
            assert!((0.85..=1.0).contains(&s), "stipple {s} out of band");
        }
    }

    #[test]
    fn planet_shade_has_no_dark_side() {
        // Across the whole disc and every lighting angle, nothing falls below the
        // flat body level — the ball has no shadowed hemisphere.
        for i in 0..=10 {
            let d2 = i as f32 / 10.0;
            for j in 0..=10 {
                let ndotl = j as f32 / 10.0;
                assert!(planet_shade(d2, ndotl) >= BODY_LEVEL);
            }
        }
    }

    #[test]
    fn planet_shade_lights_the_rim_as_an_outline() {
        // The very edge of the disc is brighter than its unlit interior.
        assert!(planet_shade(0.99, 0.0) > planet_shade(0.2, 0.0));
    }

    #[test]
    fn planet_shade_keeps_a_bright_spot() {
        // Facing the light squarely lifts a pixel above the surrounding body.
        assert!(planet_shade(0.1, 1.0) > planet_shade(0.1, 0.2));
    }

    #[test]
    fn screen_radius_grows_on_approach() {
        // Strictly larger as z decreases — planets grow as they come closer.
        assert!(screen_radius(2.0, 5.0, 72.0) > screen_radius(2.0, 50.0, 72.0));
    }

    #[test]
    fn planet_past_camera_respawns_far() {
        let mut s = Starliner::new(80, 24, 7);
        // Force one planet just in front of the camera, then step past Z_NEAR.
        s.planets[0].z = Z_NEAR + 0.01;
        s.step(0.05);
        assert!(
            s.planets[0].z >= Z_FAR - RESPAWN_JITTER,
            "a planet past the camera must respawn near the far plane, not linger in front"
        );
    }

    #[test]
    fn objects_stay_ahead_of_the_camera() {
        // Many steps: nothing should ever sit behind/at the camera (z < Z_NEAR).
        let mut s = Starliner::new(40, 12, 3);
        for _ in 0..2000 {
            s.step(0.05);
        }
        assert!(s.planets.iter().all(|p| p.z >= Z_NEAR));
        assert!(s.stars.iter().all(|st| st.z >= Z_NEAR));
    }

    #[test]
    fn draw_paints_planets_over_background() {
        let mut s = Starliner::new(60, 20, 11);
        // Pin a big near planet dead-center so at least one pixel is non-bg.
        s.planets[0].x = 0.0;
        s.planets[0].y = 0.0;
        s.planets[0].z = 3.0;
        s.planets[0].world_r = MAX_RADIUS;
        let mut c = Canvas::new(60, 20);
        s.draw(&mut c);
        let mut out = String::new();
        c.render(&mut out);
        // The frame contains color escapes beyond the bare background fill.
        let bg = format!("\x1b[38;2;{};{};{}m", SPACE_BG.0, SPACE_BG.1, SPACE_BG.2);
        let non_bg = out.matches("\x1b[38;2;").count() - out.matches(&bg).count();
        assert!(non_bg > 0, "expected lit planet pixels distinct from the background");
    }
}
