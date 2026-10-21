//! 2D/3D projective incidence, Manhattan snapping, and ego-motion motors.
//!
//! The 2D side-scroller and the 3D corridor share one camera model: a
//! translational `Motor` in `G(3,0,1)`. A 2D side-scroller only uses the
//! `e01`/`e02` translators, while a 3D ego camera adds a `e12` yaw rotor
//! followed by an `e03` forward translator.

use clifford::ops::{Antiwedge, Transform, Wedge};
use clifford::specialized::projective::dim2::{
    Line as Line2, Motor as Motor2, Point as Point2,
};
use clifford::specialized::projective::dim3::{Line, Motor, Plane, Point};

/// Result of a ray/plane incidence query.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RayHit {
    /// Intersection point (already finite: `w > 0`).
    pub point: Point<f64>,
    /// Parameter along the *unit* direction: `origin + t * dir`.
    pub t: f64,
    /// Distance from the ray origin, `|t|`.
    pub distance: f64,
}

/// Exact ray/plane intersection for a unit direction.
///
/// `plane` is any finite plane blade. Returns `None` for a parallel ray or a
/// back-facing hit when `two_sided` is false.
pub fn ray_hit_plane(
    origin: &Point<f64>,
    direction: [f64; 3],
    plane: &Plane<f64>,
    two_sided: bool,
) -> Option<RayHit> {
    let n = plane.normal();
    let n = [n.x(), n.y(), n.z()];
    let denom = n[0] * direction[0] + n[1] * direction[1] + n[2] * direction[2];
    if denom.abs() < 1e-12 {
        return None;
    }
    let d = plane.signed_distance(origin);
    let t = -d / denom;
    if !two_sided && t <= 0.0 {
        return None;
    }
    let p = Point::from_cartesian(
        origin.cartesian_x() + direction[0] * t,
        origin.cartesian_y() + direction[1] * t,
        origin.cartesian_z() + direction[2] * t,
    );
    Some(RayHit {
        point: p,
        t,
        distance: t.abs(),
    })
}

/// Same incidence computed through the PGA regressive product `Line ⊻ Plane`.
///
/// Kept next to the analytic form so tests can assert they agree; the runtime
/// uses the analytic path because it also yields `t`.
///
/// Note: in `clifford 0.3` the regressive product reads a plane's `dist`
/// coefficient as `⟨n, x⟩ = dist`, while `from_normal_and_distance` and
/// `signed_distance` use `⟨n, x⟩ + dist = 0`. The meet plane is rebuilt with a
/// flipped offset to bridge the two conventions.
pub fn ray_hit_plane_pga(
    origin: &Point<f64>,
    direction: [f64; 3],
    plane: &Plane<f64>,
) -> Option<Point<f64>> {
    let unit = plane.unitized();
    let n = unit.normal();
    let meet_plane =
        Plane::from_normal_and_distance(n.x(), n.y(), n.z(), -unit.distance_from_origin());
    let ideal = Point::ideal(direction[0], direction[1], direction[2]);
    let ray: Line<f64> = origin.wedge(&ideal);
    let p = ray.antiwedge(&meet_plane);
    if p.is_finite(1e-9) {
        Some(p)
    } else {
        None
    }
}

/// A 2D oriented line in `G(2,0,1)` used as a platform boundary.
///
/// `one_way` restricts hits to motion crossing from the positive normal side
/// (`⟨v, n⟩ < 0`), which models SMB3 pastel screw-boxes: solid on top,
/// permeable from below and the sides.
#[derive(Clone, Copy, Debug)]
pub struct OrientedLine2 {
    pub line: Line2<f64>,
    pub normal: [f64; 2],
    pub one_way: bool,
}

impl OrientedLine2 {
    /// Horizontal platform top at `y`, solid from above only.
    pub fn platform_top(y: f64) -> Self {
        // Screen coordinates: +y is down. The top surface normal points up.
        let line = Line2::from_equation(0.0, -1.0, y);
        let n = line.normal();
        Self {
            line,
            normal: [n.x(), n.y()],
            one_way: true,
        }
    }

    /// Solid oriented line with outward normal `(nx, ny)` through `point`.
    pub fn solid_through(point: [f64; 2], normal: [f64; 2]) -> Self {
        let len = (normal[0] * normal[0] + normal[1] * normal[1]).sqrt().max(1e-12);
        let (nx, ny) = (normal[0] / len, normal[1] / len);
        let c = -(nx * point[0] + ny * point[1]);
        let line = Line2::from_equation(nx, ny, c);
        Self {
            line,
            normal: [nx, ny],
            one_way: false,
        }
    }

    /// Signed side value `n·p + d`.
    pub fn side(&self, p: [f64; 2]) -> f64 {
        self.normal[0] * p[0] + self.normal[1] * p[1] + self.line.distance_from_origin()
    }
}

/// Segment/oriented-line hit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SegmentHit {
    /// Parameter in `[0, 1]` along `p0 -> p1`.
    pub t: f64,
    /// Intersection point.
    pub point: [f64; 2],
    /// Unit normal of the line at the hit.
    pub normal: [f64; 2],
}

/// Intersect a segment with an oriented 2D line.
///
/// Returns `Some` when `p0 -> p1` crosses the line within the segment. For a
/// one-way line the approach must satisfy `⟨v, n⟩ < 0` from the positive side,
/// so an actor rising through a pastel platform passes freely.
pub fn segment_hit_oriented_line(
    p0: [f64; 2],
    p1: [f64; 2],
    line: &OrientedLine2,
) -> Option<SegmentHit> {
    let f0 = line.side(p0);
    let f1 = line.side(p1);
    if f0 == f1 {
        return None;
    }
    let d = f0 - f1;
    if d.abs() < 1e-12 {
        return None;
    }
    let t = f0 / d;
    if !(0.0..=1.0).contains(&t) {
        return None;
    }
    let vel = [p1[0] - p0[0], p1[1] - p0[1]];
    let approach = vel[0] * line.normal[0] + vel[1] * line.normal[1];
    if line.one_way && !(f0 >= 0.0 && approach < 0.0) {
        return None;
    }
    let point = [p0[0] + (p1[0] - p0[0]) * t, p0[1] + (p1[1] - p0[1]) * t];
    Some(SegmentHit {
        t,
        point,
        normal: line.normal,
    })
}

/// Snap a normal to the nearest signed Manhattan axis.
///
/// Returns `None` when the vector is degenerate or more than `max_angle_deg`
/// away from every axis.
pub fn snap_manhattan_normal(n: [f64; 3], max_angle_deg: f64) -> Option<[f64; 3]> {
    let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    if len < 1e-9 {
        return None;
    }
    let mut axis = 0;
    for i in 1..3 {
        if n[i].abs() > n[axis].abs() {
            axis = i;
        }
    }
    let cos = n[axis].abs() / len;
    if cos < max_angle_deg.to_radians().cos() {
        return None;
    }
    let s = if n[axis] < 0.0 { -1.0 } else { 1.0 };
    let mut out = [0.0; 3];
    out[axis] = s;
    Some(out)
}

/// Plane through a Cartesian point with the given (not necessarily unit) normal.
///
/// `Plane::from_normal_and_distance` takes the constant term of
/// `n·x + d = 0`, so the offset is `d = -⟨n̂, p⟩`.
pub fn plane_through_point(normal: [f64; 3], point: [f64; 3]) -> Plane<f64> {
    let len = (normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2])
        .sqrt()
        .max(1e-12);
    let n = [normal[0] / len, normal[1] / len, normal[2] / len];
    let d = -(n[0] * point[0] + n[1] * point[1] + n[2] * point[2]);
    Plane::from_normal_and_distance(n[0], n[1], n[2], d)
}

/// Snap a plane's normal to a Manhattan axis, preserving its offset.
pub fn snap_manhattan_plane(plane: &Plane<f64>, max_angle_deg: f64) -> Option<Plane<f64>> {
    let unit = plane.unitized();
    let n = unit.normal();
    let snapped = snap_manhattan_normal([n.x(), n.y(), n.z()], max_angle_deg)?;
    let dist = unit.distance_from_origin();
    Some(Plane::from_normal_and_distance(
        snapped[0], snapped[1], snapped[2], dist,
    ))
}

/// Translational camera motor for the 2D orthographic pipeline.
///
/// `M_ego = exp(-½ (v_x e01 + v_y e02) Δt)`.
pub fn integrate_2d_camera(vx: f64, vy: f64, dt: f64) -> Motor<f64> {
    Motor::from_translation(vx * dt, vy * dt, 0.0)
}

/// Ego camera motor for the 3D perspective pipeline.
///
/// Forward translation along the local `+x` axis first, then a `e12` yaw of the
/// whole step: `M = exp(½ θ e12) · T(d, 0, 0)`.
pub fn integrate_3d_ego(forward: f64, yaw_rate: f64, dt: f64) -> Motor<f64> {
    Motor::from_rotation_z(yaw_rate * dt) * Motor::from_translation(forward * dt, 0.0, 0.0)
}

/// A 2D camera pose with a screen-space origin.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Camera2D {
    pub x: f64,
    pub y: f64,
}

impl Camera2D {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    /// Advance by a translational camera motor.
    pub fn step(&mut self, vx: f64, vy: f64, dt: f64) {
        self.x += vx * dt;
        self.y += vy * dt;
    }

    /// Apply the equivalent motor to a world point.
    pub fn apply(&self, motor: &Motor<f64>, p: &Point<f64>) -> Point<f64> {
        motor.transform(p)
    }

    /// Screen-space position of a world point.
    pub fn to_screen(&self, world: [f64; 2]) -> [f64; 2] {
        [world[0] - self.x, world[1] - self.y]
    }

    /// World-space position of a screen point.
    pub fn to_world(&self, screen: [f64; 2]) -> [f64; 2] {
        [screen[0] + self.x, screen[1] + self.y]
    }
}

/// 2D PGA motor wrapper for tile-local platform transforms.
pub fn platform_motor_2d(dx: f64, dy: f64) -> Motor2<f64> {
    Motor2::from_translation(dx, dy)
}

/// Transform a 2D point by a 2D PGA motor.
pub fn transform_point_2d(motor: &Motor2<f64>, p: &Point2<f64>) -> Point2<f64> {
    motor.transform(p)
}

// -- Pop-up page helpers (3D projective) ------------------------------------

/// Spine crease: the y-axis through the origin.
pub fn spine_line() -> Line<f64> {
    Line::<f64>::y_axis()
}

/// Fold the right-hand page around the spine by `theta` radians.
///
/// This is `M_fold = exp(theta/2 · L_spine)` in motor form.
pub fn fold_motor(theta: f64) -> Motor<f64> {
    Motor::<f64>::from_line(&spine_line(), theta, 0.0)
}

/// Rigid pose for a flat cutout: rotate by `heading`, then translate.
///
/// The multiplication order is deliberate: `T * R` maps the local origin
/// to `(x, y, 0)` and the local +x axis to `(x + cos θ, y + sin θ, 0)`.
pub fn pose_motor(x: f64, y: f64, heading: f64) -> Motor<f64> {
    Motor::<f64>::from_translation(x, y, 0.0) * Motor::<f64>::from_rotation_z(heading)
}

/// Apply a motor to a point with the sandwich product.
pub fn transform_point(motor: &Motor<f64>, point: &Point<f64>) -> Point<f64> {
    motor.transform(point)
}

/// The flat page plane at `z = 0`.
pub fn page_plane() -> Plane<f64> {
    Plane::<f64>::xy()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f64, b: f64, tol: f64) {
        assert!((a - b).abs() < tol, "{a} vs {b}");
    }

    #[test]
    fn ray_hits_plane_exactly() {
        let plane = plane_through_point([0.0, 0.0, 1.0], [0.0, 0.0, 5.0]);
        let origin = Point::from_cartesian(1.0, 2.0, 0.0);
        let hit = ray_hit_plane(&origin, [0.0, 0.0, 1.0], &plane, true).unwrap();
        approx(hit.t, 5.0, 1e-12);
        approx(hit.point.cartesian_z(), 5.0, 1e-12);
        approx(plane.signed_distance(&origin), -5.0, 1e-12);
        approx(plane.signed_distance(&hit.point), 0.0, 1e-12);
        let pga = ray_hit_plane_pga(&origin, [0.0, 0.0, 1.0], &plane).unwrap();
        approx(pga.cartesian_z(), 5.0, 1e-9);
        approx(pga.cartesian_x(), 1.0, 1e-9);
        approx(pga.cartesian_y(), 2.0, 1e-9);
    }

    #[test]
    fn ray_misses_parallel_plane() {
        let plane = plane_through_point([0.0, 0.0, 1.0], [0.0, 0.0, 5.0]);
        let origin = Point::from_cartesian(0.0, 0.0, 0.0);
        assert!(ray_hit_plane(&origin, [1.0, 0.0, 0.0], &plane, true).is_none());
        // back-facing rejected when one-sided
        assert!(ray_hit_plane(&origin, [0.0, 0.0, -1.0], &plane, false).is_none());
        // accepted when two-sided
        assert!(ray_hit_plane(&origin, [0.0, 0.0, -1.0], &plane, true).is_some());
    }

    #[test]
    fn one_way_platform_blocks_falls_and_passes_rises() {
        let platform = OrientedLine2::platform_top(100.0);
        // Falling through from above: hit.
        let hit = segment_hit_oriented_line([50.0, 96.0], [50.0, 104.0], &platform);
        assert!(hit.is_some());
        // Rising from below: pass.
        assert!(segment_hit_oriented_line([50.0, 104.0], [50.0, 96.0], &platform).is_none());
        // Horizontal motion on the surface: no crossing.
        assert!(segment_hit_oriented_line([50.0, 100.0], [60.0, 100.0], &platform).is_none());
    }

    #[test]
    fn solid_line_blocks_both_directions() {
        let wall = OrientedLine2::solid_through([100.0, 50.0], [1.0, 0.0]);
        assert!(segment_hit_oriented_line([96.0, 50.0], [104.0, 50.0], &wall).is_some());
        assert!(segment_hit_oriented_line([104.0, 50.0], [96.0, 50.0], &wall).is_some());
        assert!(segment_hit_oriented_line([96.0, 40.0], [104.0, 40.0], &wall).is_some());
        assert!(segment_hit_oriented_line([96.0, 60.0], [104.0, 60.0], &wall).is_some());
    }

    #[test]
    fn manhattan_snap() {
        let n = snap_manhattan_normal([0.1, 0.08, 0.99], 20.0).unwrap();
        assert_eq!(n, [0.0, 0.0, 1.0]);
        let n = snap_manhattan_normal([-0.2, -0.9, 0.1], 20.0).unwrap();
        assert_eq!(n, [0.0, -1.0, 0.0]);
        assert!(snap_manhattan_normal([1.0, 1.0, 1.0], 20.0).is_none());
        let plane = Plane::from_normal_and_distance(0.05, 0.0, 0.998, 12.0);
        let offset = plane.unitized().distance_from_origin();
        let snapped = snap_manhattan_plane(&plane, 20.0).unwrap();
        approx(snapped.normal().x(), 0.0, 1e-12);
        approx(snapped.normal().z(), 1.0, 1e-12);
        approx(snapped.unitized().distance_from_origin(), offset, 1e-9);
    }

    #[test]
    fn camera_motors_translate_and_rotate() {
        let m2 = integrate_2d_camera(10.0, -4.0, 0.5);
        let p = transform_point(&m2, &Point::from_cartesian(1.0, 2.0, 0.0));
        approx(p.cartesian_x(), 6.0, 1e-12);
        approx(p.cartesian_y(), 0.0, 1e-12);

        // 90° yaw then forward: heading +y.
        let m3 = integrate_3d_ego(2.0, std::f64::consts::FRAC_PI_2, 1.0);
        let p = transform_point(&m3, &Point::from_cartesian(0.0, 0.0, 1.0));
        approx(p.cartesian_x(), 0.0, 1e-12);
        approx(p.cartesian_y(), 2.0, 1e-12);

        let mut cam = Camera2D::new(0.0, 0.0);
        cam.step(4.0, 0.0, 0.25);
        approx(cam.x, 1.0, 1e-12);
        let s = cam.to_screen([101.0, 50.0]);
        approx(s[0], 100.0, 1e-12);
        let w = cam.to_world([100.0, 50.0]);
        approx(w[0], 101.0, 1e-12);
    }
}
