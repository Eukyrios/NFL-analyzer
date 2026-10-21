//! Team-picker map: a simplified lower-48 outline, stadium locations, an
//! Albers equal-area projection and label decluttering. Pure math, no GUI.

/// Simplified outline of the contiguous US as (lon, lat), clockwise from the
/// Pacific Northwest. The Canadian border runs through the Great Lakes.
pub const OUTLINE: &[(f32, f32)] = &[
    (-124.7, 48.4), (-122.8, 49.0), (-95.2, 49.0), (-95.2, 49.4), (-94.8, 49.3), (-93.0, 48.6), (-90.0, 48.1),
    (-89.6, 48.0), (-84.8, 46.9), (-84.1, 46.5), (-82.4, 45.3), (-82.5, 43.0), (-83.1, 42.0), (-82.4, 41.7),
    (-81.0, 42.3), (-79.1, 43.1), (-79.0, 43.5), (-76.4, 44.2), (-75.0, 44.9), (-71.5, 45.0), (-70.8, 45.4),
    (-70.0, 46.7), (-69.2, 47.4), (-68.2, 47.3), (-67.8, 47.1), (-67.8, 45.7), (-67.0, 44.9), (-68.9, 44.2),
    (-70.2, 43.7), (-70.8, 42.9), (-70.6, 42.6), (-71.0, 42.3), (-70.0, 42.05), (-69.95, 41.7), (-70.6, 41.5),
    (-71.4, 41.4), (-72.9, 41.2), (-74.0, 40.6), (-74.1, 39.8), (-74.9, 38.95), (-75.1, 38.4), (-75.4, 37.9),
    (-76.0, 37.0), (-75.7, 36.2), (-75.5, 35.2), (-76.5, 34.7), (-77.9, 33.9), (-79.2, 33.2), (-80.4, 32.5),
    (-81.2, 31.6), (-81.4, 30.7), (-81.3, 29.9), (-80.6, 28.4), (-80.0, 26.7), (-80.1, 25.8), (-80.4, 25.2),
    (-81.1, 25.1), (-81.7, 25.9), (-82.1, 26.6), (-82.7, 27.5), (-82.8, 28.2), (-82.7, 29.0), (-83.7, 29.9),
    (-84.4, 30.0), (-85.3, 29.7), (-86.3, 30.4), (-87.5, 30.3), (-88.4, 30.4), (-89.4, 30.2), (-89.6, 29.3),
    (-90.2, 29.1), (-91.3, 29.3), (-92.3, 29.6), (-93.8, 29.7), (-94.7, 29.4), (-95.4, 28.9), (-96.6, 28.3),
    (-97.4, 27.4), (-97.2, 25.95), (-99.1, 26.4), (-99.5, 27.5), (-100.3, 28.3), (-101.0, 29.4), (-102.4, 29.8),
    (-103.1, 29.0), (-104.5, 29.6), (-105.0, 30.7), (-106.5, 31.8), (-108.2, 31.8), (-108.2, 31.3), (-111.1, 31.3),
    (-114.8, 32.5), (-114.7, 32.7), (-117.1, 32.5), (-117.3, 33.1), (-118.4, 33.8), (-118.9, 34.0), (-120.6, 34.6),
    (-120.9, 35.4), (-121.9, 36.3), (-121.9, 36.9), (-122.5, 37.5), (-123.0, 38.0), (-123.7, 38.9), (-123.8, 39.8),
    (-124.4, 40.4), (-124.1, 41.0), (-124.2, 42.0), (-124.5, 42.8), (-124.1, 43.7), (-123.9, 45.5), (-124.0, 46.3),
    (-124.1, 47.0),
];

/// Home stadium (2021 season): abbreviation, city, lat, lon.
pub const STADIUMS: [(&str, &str, f32, f32); 32] = [
    ("ARI", "Glendale, AZ", 33.528, -112.263),
    ("ATL", "Atlanta, GA", 33.755, -84.401),
    ("BAL", "Baltimore, MD", 39.278, -76.623),
    ("BUF", "Orchard Park, NY", 42.774, -78.787),
    ("CAR", "Charlotte, NC", 35.226, -80.853),
    ("CHI", "Chicago, IL", 41.862, -87.617),
    ("CIN", "Cincinnati, OH", 39.095, -84.516),
    ("CLE", "Cleveland, OH", 41.506, -81.700),
    ("DAL", "Arlington, TX", 32.747, -97.095),
    ("DEN", "Denver, CO", 39.744, -105.020),
    ("DET", "Detroit, MI", 42.340, -83.046),
    ("GB", "Green Bay, WI", 44.501, -88.062),
    ("HOU", "Houston, TX", 29.685, -95.411),
    ("IND", "Indianapolis, IN", 39.760, -86.164),
    ("JAX", "Jacksonville, FL", 30.324, -81.637),
    ("KC", "Kansas City, MO", 39.049, -94.484),
    ("LA", "Inglewood, CA", 33.954, -118.339),
    ("LAC", "Inglewood, CA", 33.954, -118.339),
    ("LV", "Las Vegas, NV", 36.091, -115.183),
    ("MIA", "Miami Gardens, FL", 25.958, -80.239),
    ("MIN", "Minneapolis, MN", 44.974, -93.258),
    ("NE", "Foxborough, MA", 42.091, -71.264),
    ("NO", "New Orleans, LA", 29.951, -90.081),
    ("NYG", "East Rutherford, NJ", 40.814, -74.075),
    ("NYJ", "East Rutherford, NJ", 40.814, -74.075),
    ("PHI", "Philadelphia, PA", 39.901, -75.168),
    ("PIT", "Pittsburgh, PA", 40.447, -80.016),
    ("SEA", "Seattle, WA", 47.595, -122.332),
    ("SF", "Santa Clara, CA", 37.403, -121.970),
    ("TB", "Tampa, FL", 27.976, -82.503),
    ("TEN", "Nashville, TN", 36.166, -86.771),
    ("WAS", "Landover, MD", 38.908, -76.865),
];

pub fn stadium(abbr: &str) -> Option<(&'static str, f32, f32)> {
    STADIUMS.iter().find(|s| s.0 == abbr).map(|s| (s.1, s.2, s.3))
}

/// Albers equal-area conic (standard parallels 29.5° / 45.5°, origin 23° N,
/// 96° W), the usual projection for US maps. Returns unitless (x east, y north).
pub fn albers(lon: f32, lat: f32) -> (f32, f32) {
    let (p1, p2, p0, l0) = (29.5f32.to_radians(), 45.5f32.to_radians(), 23.0f32.to_radians(), (-96.0f32).to_radians());
    let n = (p1.sin() + p2.sin()) / 2.0;
    let c = p1.cos().powi(2) + 2.0 * n * p1.sin();
    let rho = |p: f32| (c - 2.0 * n * p.sin()).max(0.0).sqrt() / n;
    let (phi, lam) = (lat.to_radians(), lon.to_radians());
    let theta = n * (lam - l0);
    let r = rho(phi);
    (r * theta.sin(), rho(p0) - r * theta.cos())
}

/// Bounding box (min x, min y, max x, max y) of the projected outline.
pub fn outline_bounds() -> (f32, f32, f32, f32) {
    OUTLINE.iter().map(|&(lon, lat)| albers(lon, lat)).fold(
        (f32::MAX, f32::MAX, f32::MIN, f32::MIN),
        |(a, b, c, d), (x, y)| (a.min(x), b.min(y), c.max(x), d.max(y)),
    )
}

/// Maps projected points into a screen rect (y down), keeping the aspect ratio.
#[derive(Debug, Clone, Copy)]
pub struct MapFit {
    pub scale: f32,
    pub ox: f32,
    pub oy: f32,
    pub min_x: f32,
    pub max_y: f32,
}

impl MapFit {
    pub fn new(left: f32, top: f32, width: f32, height: f32) -> Self {
        let (x0, y0, x1, y1) = outline_bounds();
        let scale = (width / (x1 - x0)).min(height / (y1 - y0));
        let ox = left + (width - (x1 - x0) * scale) / 2.0;
        let oy = top + (height - (y1 - y0) * scale) / 2.0;
        MapFit { scale, ox, oy, min_x: x0, max_y: y1 }
    }
    pub fn to_screen(&self, lon: f32, lat: f32) -> (f32, f32) {
        let (x, y) = albers(lon, lat);
        (self.ox + (x - self.min_x) * self.scale, self.oy + (self.max_y - y) * self.scale)
    }
}

/// Horizontal spans inside a (possibly concave) polygon at height `y`, using
/// the even-odd rule. Used to fill the outline with thin rows, since the GUI
/// painter only fills convex shapes.
pub fn scanline_spans(poly: &[(f32, f32)], y: f32) -> Vec<(f32, f32)> {
    let mut xs: Vec<f32> = Vec::new();
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
        if (a.1 <= y) != (b.1 <= y) {
            xs.push(a.0 + (y - a.1) / (b.1 - a.1) * (b.0 - a.0));
        }
    }
    xs.sort_by(f32::total_cmp);
    xs.chunks_exact(2).map(|c| (c[0], c[1])).collect()
}

/// Pushes overlapping markers apart until centers are at least `min_dist`
/// apart (deterministic). Identical points are split by index.
pub fn spread(points: &[(f32, f32)], min_dist: f32, iterations: usize) -> Vec<(f32, f32)> {
    let mut p = points.to_vec();
    for _ in 0..iterations {
        let mut moved = false;
        for i in 0..p.len() {
            for j in i + 1..p.len() {
                let (dx, dy) = (p[j].0 - p[i].0, p[j].1 - p[i].1);
                let d = dx.hypot(dy);
                if d >= min_dist {
                    continue;
                }
                moved = true;
                let (ux, uy) = if d < 1e-3 {
                    let a = (i * 7 + j * 13) as f32;
                    (a.cos(), a.sin())
                } else {
                    (dx / d, dy / d)
                };
                let push = (min_dist - d) / 2.0 + 0.01;
                p[i].0 -= ux * push;
                p[i].1 -= uy * push;
                p[j].0 += ux * push;
                p[j].1 += uy * push;
            }
        }
        if !moved {
            break;
        }
    }
    p
}

/// Two-letter state of a team's home stadium, e.g. `"MD"` for BAL.
pub fn state_of(abbr: &str) -> Option<&'static str> {
    stadium(abbr).and_then(|(city, _, _)| city.rsplit(", ").next())
}

/// Full name of a state that has an NFL stadium.
pub fn state_name(code: &str) -> &'static str {
    match code {
        "AZ" => "Arizona", "CA" => "California", "CO" => "Colorado", "FL" => "Florida", "GA" => "Georgia",
        "IL" => "Illinois", "IN" => "Indiana", "LA" => "Louisiana", "MA" => "Massachusetts", "MD" => "Maryland",
        "MI" => "Michigan", "MN" => "Minnesota", "MO" => "Missouri", "NC" => "North Carolina", "NJ" => "New Jersey",
        "NV" => "Nevada", "NY" => "New York", "OH" => "Ohio", "PA" => "Pennsylvania", "TN" => "Tennessee",
        "TX" => "Texas", "WA" => "Washington", "WI" => "Wisconsin",
        _ => "Unknown state",
    }
}

/// Teams whose home stadium is in `state`, in table order.
pub fn teams_in_state(state: &str) -> Vec<&'static str> {
    STADIUMS.iter().filter(|t| state_of(t.0) == Some(state)).map(|t| t.0).collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn states_group_teams() {
        assert_eq!(state_of("BAL"), Some("MD"));
        assert_eq!(teams_in_state("CA"), vec!["LA", "LAC", "SF"]);
        assert_eq!(teams_in_state("NJ"), vec!["NYG", "NYJ"]);
        assert_eq!(teams_in_state("FL"), vec!["JAX", "MIA", "TB"]);
        for t in STADIUMS {
            assert_ne!(state_name(state_of(t.0).unwrap()), "Unknown state", "{}", t.0);
        }
    }

    use super::*;
    use proptest::prelude::*;

    #[test]
    fn stadiums_cover_teams() {
        for t in crate::teams::TEAMS {
            assert!(stadium(t.abbr).is_some(), "{}", t.abbr);
        }
    }

    #[test]
    fn projection_orientation() {
        let sea = albers(-122.3, 47.6);
        let mia = albers(-80.2, 25.9);
        assert!(sea.0 < mia.0, "west is left");
        assert!(sea.1 > mia.1, "north is up");
        let fit = MapFit::new(0.0, 0.0, 1000.0, 600.0);
        for &(lon, lat) in OUTLINE {
            let (x, y) = fit.to_screen(lon, lat);
            assert!((-0.5..=1000.5).contains(&x) && (-0.5..=600.5).contains(&y));
        }
        for (_, _, lat, lon) in STADIUMS {
            let (x, y) = fit.to_screen(lon, lat);
            assert!((0.0..=1000.0).contains(&x) && (0.0..=600.0).contains(&y));
        }
    }

    #[test]
    fn concave_spans() {
        // A "U" shape: two spans across the arms, one across the base.
        let u = [(0.0, 0.0), (3.0, 0.0), (3.0, 3.0), (2.0, 3.0), (2.0, 1.0), (1.0, 1.0), (1.0, 3.0), (0.0, 3.0)];
        assert_eq!(scanline_spans(&u, 2.0), vec![(0.0, 1.0), (2.0, 3.0)]);
        assert_eq!(scanline_spans(&u, 0.5), vec![(0.0, 3.0)]);
        assert!(scanline_spans(&u, 5.0).is_empty());
    }

    proptest! {
        // Feature: native-play-viewer, Property 8: Decluttered markers don't overlap
        #[test]
        fn spread_separates(pts in prop::collection::vec((0.0f32..100.0, 0.0f32..100.0), 2..12)) {
            let out = spread(&pts, 10.0, 500);
            for i in 0..out.len() {
                for j in i + 1..out.len() {
                    let d = (out[i].0 - out[j].0).hypot(out[i].1 - out[j].1);
                    prop_assert!(d >= 9.9, "{} {} {}", i, j, d);
                }
            }
        }
    }
}
