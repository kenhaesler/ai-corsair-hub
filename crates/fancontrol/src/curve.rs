use anyhow::{bail, Result};
use corsair_common::config::CurvePoint;

/// Fan curve: interpolates between user-defined (temp, duty) points.
pub struct FanCurve {
    points: Vec<CurvePoint>,
}

impl FanCurve {
    /// Create a new fan curve. Requires at least 2 points. Points are sorted by temp.
    pub fn new(mut points: Vec<CurvePoint>) -> Result<Self> {
        if points.len() < 2 {
            bail!("Fan curve requires at least 2 points, got {}", points.len());
        }
        points.sort_by(|a, b| a.temp.partial_cmp(&b.temp).unwrap());
        Ok(Self { points })
    }

    /// Evaluate the curve at a given temperature. Returns duty 0-100.
    pub fn evaluate(&self, temp: f64) -> f64 {
        // Below first point → clamp to first duty
        if temp <= self.points[0].temp {
            return self.points[0].duty;
        }

        // Above last point → clamp to last duty
        let last = &self.points[self.points.len() - 1];
        if temp >= last.temp {
            return last.duty;
        }

        // Find the two points to interpolate between
        for i in 0..self.points.len() - 1 {
            let lo = &self.points[i];
            let hi = &self.points[i + 1];
            if temp >= lo.temp && temp <= hi.temp {
                let t = (temp - lo.temp) / (hi.temp - lo.temp);
                return lo.duty + t * (hi.duty - lo.duty);
            }
        }

        // Shouldn't reach here, but return last duty as fallback
        last.duty
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn curve_points() -> Vec<CurvePoint> {
        vec![
            CurvePoint { temp: 30.0, duty: 25.0 },
            CurvePoint { temp: 50.0, duty: 40.0 },
            CurvePoint { temp: 70.0, duty: 70.0 },
            CurvePoint { temp: 85.0, duty: 100.0 },
        ]
    }

    #[test]
    fn test_below_range() {
        let curve = FanCurve::new(curve_points()).unwrap();
        assert_eq!(curve.evaluate(10.0), 25.0);
    }

    #[test]
    fn test_above_range() {
        let curve = FanCurve::new(curve_points()).unwrap();
        assert_eq!(curve.evaluate(95.0), 100.0);
    }

    #[test]
    fn test_exact_point() {
        let curve = FanCurve::new(curve_points()).unwrap();
        assert_eq!(curve.evaluate(50.0), 40.0);
    }

    #[test]
    fn test_interpolation() {
        let curve = FanCurve::new(curve_points()).unwrap();
        // Midpoint between (30, 25) and (50, 40): temp=40 → duty=32.5
        let duty = curve.evaluate(40.0);
        assert!((duty - 32.5).abs() < 0.01, "got {}", duty);
    }

    #[test]
    fn test_single_point_error() {
        let result = FanCurve::new(vec![CurvePoint { temp: 50.0, duty: 50.0 }]);
        assert!(result.is_err());
    }
}
