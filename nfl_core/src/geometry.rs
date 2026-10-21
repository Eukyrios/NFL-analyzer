//! Field geometry and animation helpers. Field coordinates are yards on the
//! normalized field (offense moves left → right): x ∈ [0, 120] along the
//! length, y ∈ [0, 53.3] across, y pointing "up" (toward the far sideline).

use crate::blob::{EntityPos, Frame};
use crate::style::Rgb;

pub const LEN: f32 = 120.0;
pub const WID: f32 = 53.3;
pub const FPS: f32 = 10.0;
/// Hash mark y positions (yards) and the sideline ticks 1 ft inside the field.
pub const HASHES: [f32; 4] = [0.33, 23.58, 29.75, WID - 0.33];

/// Flips a play that ran right → left: x' = 120 − x, y' = 53.3 − y, θ' = θ + 180.
pub fn normalize(p: EntityPos) -> EntityPos {
    let flip = |a: Option<f32>| a.map(|a| (a + 180.0).rem_euclid(360.0));
    EntityPos { x: LEN - p.x, y: WID - p.y, s: p.s, o: flip(p.o), dir: flip(p.dir) }
}

/// Direction for an angle measured clockwise from +y: (sin θ, cos θ).
pub fn angle_vec(deg: f32) -> (f32, f32) {
    let r = deg.to_radians();
    (r.sin(), r.cos())
}

/// Positions at fractional frame `t` between `a` and `b`. Angles and speed come
/// from the nearer frame (as in the web viewer); entities missing from `b` hold
/// their position from `a`.
pub fn lerp_frame(a: &Frame, b: &Frame, t: f32) -> Frame {
    let t = t.clamp(0.0, 1.0);
    a.iter()
        .enumerate()
        .map(|(i, pa)| {
            let pa = (*pa)?;
            let pb = b.get(i).copied().flatten().unwrap_or(pa);
            let near = if t < 0.5 { pa } else { pb };
            Some(EntityPos { x: pa.x + (pb.x - pa.x) * t, y: pa.y + (pb.y - pa.y) * t, ..near })
        })
        .collect()
}

/// Positions at fractional frame index `f` (clamped to the play).
pub fn frame_at(frames: &[Frame], f: f32) -> Frame {
    if frames.is_empty() {
        return Vec::new();
    }
    let n = frames.len() - 1;
    let f = f.clamp(0.0, n as f32);
    let i = (f.floor() as usize).min(n);
    let j = (i + 1).min(n);
    lerp_frame(&frames[i], &frames[j], f - i as f32)
}

/// A screen rectangle the whole 120 × 53.3 field is fitted into. In portrait
/// the field is rotated 90°: the offense drives bottom → top.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Viewport {
    pub left: f32,
    pub top: f32,
    pub width: f32,
    pub height: f32,
    pub portrait: bool,
}

impl Viewport {
    /// Largest field-shaped rectangle centered in the given area.
    pub fn fit(left: f32, top: f32, width: f32, height: f32, portrait: bool) -> Self {
        let aspect = if portrait { WID / LEN } else { LEN / WID };
        let (w, h) = if width / height > aspect { (height * aspect, height) } else { (width, width / aspect) };
        Viewport { left: left + (width - w) / 2.0, top: top + (height - h) / 2.0, width: w, height: h, portrait }
    }

    /// Pixels per yard.
    pub fn scale(&self) -> f32 {
        if self.portrait { self.width / WID } else { self.width / LEN }
    }

    pub fn to_screen(&self, x: f32, y: f32) -> (f32, f32) {
        if self.portrait {
            (self.left + y / WID * self.width, self.top + (1.0 - x / LEN) * self.height)
        } else {
            (self.left + x / LEN * self.width, self.top + (1.0 - y / WID) * self.height)
        }
    }

    pub fn to_field(&self, sx: f32, sy: f32) -> (f32, f32) {
        let u = (sx - self.left) / self.width;
        let v = (sy - self.top) / self.height;
        if self.portrait { ((1.0 - v) * LEN, u * WID) } else { (u * LEN, (1.0 - v) * WID) }
    }

    /// A field direction (from [`angle_vec`]) as a screen direction (y down).
    pub fn dir_to_screen(&self, (dx, dy): (f32, f32)) -> (f32, f32) {
        if self.portrait { (dy, -dx) } else { (dx, -dy) }
    }
}

/// The nflId of the closest player (ball excluded: `None` id) within `radius`
/// of `p`. Coordinates may be screen or field units as long as they agree.
pub fn hit_test(ents: &[(Option<i64>, f32, f32)], p: (f32, f32), radius: f32) -> Option<i64> {
    ents.iter()
        .filter_map(|&(id, x, y)| Some((id?, (x - p.0).hypot(y - p.1))))
        .filter(|&(_, d)| d <= radius)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(id, _)| id)
}

/// First-down line x for a normalized play, clamped to the goal line (x = 110).
/// The flag is true when the line to gain is the goal line ("& Goal").
pub fn first_down_x(los: f32, yards_to_go: i64) -> (f32, bool) {
    let x = los + yards_to_go as f32;
    if x >= 110.0 { (110.0, true) } else { (x, false) }
}

/// Line of scrimmage on the normalized field. `absoluteYardlineNumber` is on
/// the raw tracking axis, so plays that ran left are mirrored.
pub fn los_x(absolute_yardline: f32, play_left: bool) -> f32 {
    if play_left { LEN - absolute_yardline } else { absolute_yardline }
}

/// Line of scrimmage on the normalized field from the yard line: on the
/// offense's own side it's `10 + n`, otherwise `110 − n`. This needs no play
/// direction, unlike `absoluteYardlineNumber`.
pub fn los_from_yardline(offense: &str, side: Option<&str>, number: i64) -> f32 {
    if number == 50 || side == Some(offense) { 10.0 + number as f32 } else { 110.0 - number as f32 }
}

/// Playback clock for a play: fractional frame index, 10 fps × speed.
#[derive(Debug, Clone, PartialEq)]
pub struct Playback {
    pub frame: f32,
    pub playing: bool,
    pub speed: f32,
    pub frames: usize,
}

impl Playback {
    pub const SPEEDS: [f32; 3] = [0.5, 1.0, 2.0];

    pub fn new(frames: usize) -> Self {
        Playback { frame: 0.0, playing: false, speed: 1.0, frames }
    }
    fn last(&self) -> f32 {
        self.frames.saturating_sub(1) as f32
    }
    pub fn at_end(&self) -> bool {
        self.frame >= self.last()
    }
    /// Advances by `dt` seconds; stops at the last frame.
    pub fn tick(&mut self, dt: f32) {
        if !self.playing {
            return;
        }
        self.frame = (self.frame + dt.max(0.0) * FPS * self.speed).min(self.last());
        if self.at_end() {
            self.playing = false;
        }
    }
    /// Toggles play/pause; playing from the end restarts.
    pub fn toggle(&mut self) {
        if !self.playing && self.at_end() {
            self.frame = 0.0;
        }
        self.playing = !self.playing && self.frames > 1;
    }
    pub fn scrub(&mut self, frame: f32) {
        self.frame = frame.clamp(0.0, self.last());
    }
    pub fn step(&mut self, by: i32) {
        self.playing = false;
        self.scrub(self.frame.round() + by as f32);
    }
    /// Seconds relative to the snap (0-based snap frame).
    pub fn from_snap(&self, snap: usize) -> f32 {
        (self.frame - snap as f32) / FPS
    }
}

/// WCAG relative luminance (same formula as the web viewer).
pub fn luminance((r, g, b): Rgb) -> f32 {
    let ch = |v: u8| {
        let v = v as f32 / 255.0;
        if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * ch(r) + 0.7152 * ch(g) + 0.0722 * ch(b)
}

/// Colors with luminance above 0.35 get dark text (`#111111`).
pub fn is_light(c: Rgb) -> bool {
    luminance(c) > 0.35
}

#[derive(Debug, Clone, PartialEq)]
pub struct Marker {
    /// 0-based frame index.
    pub frame: usize,
    pub label: &'static str,
}

/// Short timeline label for a stored event, or `None` for events that aren't shown.
pub fn event_label(event: &str) -> Option<&'static str> {
    Some(match event {
        "line_set" => "Set",
        "man_in_motion" => "Motion",
        "ball_snap" => "Snap",
        "play_action" => "PA",
        "pass_forward" => "Throw",
        "qb_sack" | "qb_strip_sack" => "Sack",
        "run" => "Run",
        "pass_arrived" => "Arrived",
        "pass_outcome_caught" => "Caught",
        "pass_outcome_incomplete" => "Inc",
        "autoevent_passinterrupted" => "Interrupted",
        "fumble" => "Fumble",
        _ => return None,
    })
}

/// Timeline markers from `(frameId, event)` pairs (frameId is 1-based).
pub fn phase_markers(events: &[(i64, String)]) -> Vec<Marker> {
    let mut m: Vec<Marker> = events
        .iter()
        .filter_map(|(f, e)| Some(Marker { frame: (*f - 1).max(0) as usize, label: event_label(e)? }))
        .collect();
    m.sort_by_key(|m| m.frame);
    m
}

/// Dropback span (0-based frames): snap → throw or sack, if both are known.
pub fn dropback_span(snap: i64, throw: Option<i64>, sack: Option<i64>) -> Option<(usize, usize)> {
    let end = throw.or(sack)?;
    (end >= snap).then(|| ((snap - 1).max(0) as usize, (end - 1).max(0) as usize))
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn p(x: f32, y: f32) -> Option<EntityPos> {
        Some(EntityPos { x, y, s: 1.0, o: Some(10.0), dir: Some(20.0) })
    }

    #[test]
    fn angles() {
        let (x, y) = angle_vec(0.0);
        assert!(x.abs() < 1e-6 && (y - 1.0).abs() < 1e-6);
        let (x, y) = angle_vec(90.0);
        assert!((x - 1.0).abs() < 1e-6 && y.abs() < 1e-6);
    }

    #[test]
    fn landscape_and_portrait_corners() {
        let v = Viewport { left: 10.0, top: 20.0, width: 240.0, height: 106.6, portrait: false };
        assert_eq!(v.to_screen(0.0, 0.0), (10.0, 126.6));
        assert_eq!(v.to_screen(LEN, WID), (250.0, 20.0));
        let v = Viewport { left: 0.0, top: 0.0, width: 53.3, height: 120.0, portrait: true };
        assert_eq!(v.to_screen(0.0, 0.0), (0.0, 120.0));
        assert_eq!(v.to_screen(LEN, WID), (53.3, 0.0));
    }

    #[test]
    fn midpoint() {
        let m = lerp_frame(&vec![p(0.0, 0.0)], &vec![p(10.0, 4.0)], 0.5);
        let m = m[0].unwrap();
        assert!((m.x - 5.0).abs() < 1e-6 && (m.y - 2.0).abs() < 1e-6);
    }

    #[test]
    fn markers_and_span() {
        let ev = vec![(40, "pass_forward".to_string()), (11, "ball_snap".to_string()), (3, "None".to_string())];
        let m = phase_markers(&ev);
        assert_eq!(m, vec![Marker { frame: 10, label: "Snap" }, Marker { frame: 39, label: "Throw" }]);
        assert_eq!(dropback_span(11, Some(40), None), Some((10, 39)));
        assert_eq!(dropback_span(11, None, None), None);
    }

    #[test]
    fn goal_clamp() {
        assert_eq!(first_down_x(30.0, 10), (40.0, false));
        assert_eq!(first_down_x(105.0, 8), (110.0, true));
        assert_eq!(los_x(43.0, true), 77.0);
    }

    #[test]
    fn los_from_side() {
        assert_eq!(los_from_yardline("TB", Some("TB"), 33), 43.0);
        assert_eq!(los_from_yardline("TB", Some("DAL"), 40), 70.0);
        assert_eq!(los_from_yardline("TB", None, 50), 60.0);
    }

    #[test]
    fn playback() {
        let mut p = Playback::new(43);
        p.tick(1.0);
        assert_eq!(p.frame, 0.0, "paused doesn't move");
        p.toggle();
        p.tick(0.5);
        assert!((p.frame - 5.0).abs() < 1e-4);
        p.speed = 2.0;
        p.tick(0.5);
        assert!((p.frame - 15.0).abs() < 1e-4);
        p.speed = 0.5;
        p.tick(1.0);
        assert!((p.frame - 20.0).abs() < 1e-4);
        p.tick(100.0);
        assert_eq!(p.frame, 42.0);
        assert!(!p.playing && p.at_end());
        p.toggle();
        assert!(p.playing && p.frame == 0.0, "play from the end restarts");
        p.scrub(99.0);
        assert_eq!(p.frame, 42.0);
        p.step(-1);
        assert_eq!(p.frame, 41.0);
        assert!(!p.playing);
        assert!((p.from_snap(11) - 3.0).abs() < 1e-4);
    }

    #[test]
    fn light_teams_get_dark_text() {
        use crate::teams::primary;
        assert!(is_light(primary("NO")));
        assert!(is_light(primary("PIT")));
        assert!(!is_light(primary("BAL")));
        assert!(!is_light(primary("LV")));
    }

    #[test]
    fn hit_test_skips_ball_and_far() {
        let e = [(Some(1), 0.0, 0.0), (Some(2), 3.0, 0.0), (None, 1.0, 0.0)];
        assert_eq!(hit_test(&e, (1.2, 0.0), 2.0), Some(1));
        assert_eq!(hit_test(&e, (10.0, 0.0), 2.0), None);
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(200))]

        // Feature: native-play-viewer, Property 2: Normalization is an involution
        #[test]
        fn normalize_involution(x in 0.0f32..120.0, y in 0.0f32..53.3, o in prop::option::of(0.0f32..360.0)) {
            let a = EntityPos { x, y, s: 1.0, o, dir: o };
            let n = normalize(a);
            prop_assert!((0.0..=LEN).contains(&n.x) && (0.0..=WID + 1e-4).contains(&n.y));
            let b = normalize(n);
            prop_assert!((a.x - b.x).abs() < 1e-3 && (a.y - b.y).abs() < 1e-3);
            if let (Some(p), Some(q)) = (a.o, b.o) {
                let d = (p - q).rem_euclid(360.0);
                prop_assert!(d.min(360.0 - d) < 1e-2);
            }
        }

        // Feature: native-play-viewer, Property 3: Angle to unit vector
        #[test]
        fn angle_unit(t in -720.0f32..720.0) {
            let (x, y) = angle_vec(t);
            prop_assert!(((x * x + y * y) - 1.0).abs() < 1e-4);
            let (x2, y2) = angle_vec(t + 360.0);
            prop_assert!((x - x2).abs() < 1e-3 && (y - y2).abs() < 1e-3);
        }

        // Feature: native-play-viewer, Property 5: Interpolation bounds
        #[test]
        fn interpolation_bounds(ax in 0.0f32..120.0, ay in 0.0f32..53.3, bx in 0.0f32..120.0, by in 0.0f32..53.3, t in 0.0f32..=1.0) {
            let (a, b) = (vec![p(ax, ay)], vec![p(bx, by)]);
            let m = lerp_frame(&a, &b, t)[0].unwrap();
            prop_assert!(m.x >= ax.min(bx) - 1e-4 && m.x <= ax.max(bx) + 1e-4);
            prop_assert!(m.y >= ay.min(by) - 1e-4 && m.y <= ay.max(by) + 1e-4);
            prop_assert_eq!(lerp_frame(&a, &b, 0.0), a.clone());
            let end = lerp_frame(&a, &b, 1.0)[0].unwrap();
            prop_assert!((end.x - bx).abs() < 1e-4 && (end.y - by).abs() < 1e-4);
        }

        // Feature: native-play-viewer, Property 6: Hit-test returns nearest within radius
        #[test]
        fn hit_nearest(pts in prop::collection::vec((prop::option::of(1i64..1000), 0.0f32..100.0, 0.0f32..100.0), 0..23),
                       px in 0.0f32..100.0, py in 0.0f32..100.0, r in 0.5f32..20.0) {
            let best = pts.iter().filter_map(|&(id, x, y)| Some((id?, (x - px).hypot(y - py))))
                .filter(|&(_, d)| d <= r).map(|(_, d)| d).fold(f32::INFINITY, f32::min);
            match hit_test(&pts, (px, py), r) {
                None => prop_assert!(best.is_infinite()),
                Some(id) => {
                    let d = pts.iter().filter(|e| e.0 == Some(id)).map(|e| (e.1 - px).hypot(e.2 - py)).fold(f32::INFINITY, f32::min);
                    prop_assert!((d - best).abs() < 1e-4);
                }
            }
        }

        // Feature: native-play-viewer, Property 7: Field mapping corners
        #[test]
        fn mapping_inverts(w in 100.0f32..3000.0, h in 100.0f32..3000.0, portrait: bool, x in 0.0f32..120.0, y in 0.0f32..53.3) {
            let v = Viewport::fit(5.0, 7.0, w, h, portrait);
            let (sx, sy) = v.to_screen(x, y);
            prop_assert!(sx >= v.left - 1e-2 && sx <= v.left + v.width + 1e-2);
            prop_assert!(sy >= v.top - 1e-2 && sy <= v.top + v.height + 1e-2);
            let (fx, fy) = v.to_field(sx, sy);
            prop_assert!((fx - x).abs() < 1e-2 && (fy - y).abs() < 1e-2);
        }
    }
}
