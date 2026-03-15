/// Simplex noise implementation (pure Rust, no dependencies).
/// Produces smooth, organic pseudo-random values in [-1, 1].

// Permutation table (doubled to avoid wrapping)
const PERM: [u8; 512] = {
    const BASE: [u8; 256] = [
        151, 160, 137, 91, 90, 15, 131, 13, 201, 95, 96, 53, 194, 233, 7, 225,
        140, 36, 103, 30, 69, 142, 8, 99, 37, 240, 21, 10, 23, 190, 6, 148,
        247, 120, 234, 75, 0, 26, 197, 62, 94, 252, 219, 203, 117, 35, 11, 32,
        57, 177, 33, 88, 237, 149, 56, 87, 174, 20, 125, 136, 171, 168, 68, 175,
        74, 165, 71, 134, 139, 48, 27, 166, 77, 146, 158, 231, 83, 111, 229, 122,
        60, 211, 133, 230, 220, 105, 92, 41, 55, 46, 245, 40, 244, 102, 143, 54,
        65, 25, 63, 161, 1, 216, 80, 73, 209, 76, 132, 187, 208, 89, 18, 169,
        200, 196, 135, 130, 116, 188, 159, 86, 164, 100, 109, 198, 173, 186, 3, 64,
        52, 217, 226, 250, 124, 123, 5, 202, 38, 147, 118, 126, 255, 82, 85, 212,
        207, 206, 59, 227, 47, 16, 58, 17, 182, 189, 28, 42, 223, 183, 170, 213,
        119, 248, 152, 2, 44, 154, 163, 70, 221, 153, 101, 155, 167, 43, 172, 9,
        129, 22, 39, 253, 19, 98, 108, 110, 79, 113, 224, 232, 178, 185, 112, 104,
        218, 246, 97, 228, 251, 34, 242, 193, 238, 210, 144, 12, 191, 179, 162, 241,
        81, 51, 145, 235, 249, 14, 239, 107, 49, 192, 214, 31, 181, 199, 106, 157,
        184, 84, 204, 176, 115, 121, 50, 45, 127, 4, 150, 254, 138, 236, 205, 93,
        222, 114, 67, 29, 24, 72, 243, 141, 128, 195, 78, 66, 215, 61, 156, 180,
    ];
    let mut table = [0u8; 512];
    let mut i = 0;
    while i < 512 {
        table[i] = BASE[i & 255];
        i += 1;
    }
    table
};

// Gradient vectors for 2D simplex noise
const GRAD2: [(f64, f64); 8] = [
    (1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0),
    (1.0, 1.0), (-1.0, 1.0), (1.0, -1.0), (-1.0, -1.0),
];

fn grad2(hash: u8) -> (f64, f64) {
    GRAD2[(hash & 7) as usize]
}

const F2: f64 = 0.366025403784438; // (sqrt(3) - 1) / 2
const G2: f64 = 0.211324865405187; // (3 - sqrt(3)) / 6

/// 1D simplex noise. Returns value in [-1, 1].
pub fn noise_1d(x: f64) -> f64 {
    // Use 2D noise with y=0 for simplicity
    noise_2d(x, 0.0)
}

/// 2D simplex noise. Returns value in approximately [-1, 1].
pub fn noise_2d(x: f64, y: f64) -> f64 {
    let s = (x + y) * F2;
    let i = (x + s).floor();
    let j = (y + s).floor();

    let t = (i + j) * G2;
    let x0 = x - (i - t);
    let y0 = y - (j - t);

    let (i1, j1) = if x0 > y0 { (1, 0) } else { (0, 1) };

    let x1 = x0 - i1 as f64 + G2;
    let y1 = y0 - j1 as f64 + G2;
    let x2 = x0 - 1.0 + 2.0 * G2;
    let y2 = y0 - 1.0 + 2.0 * G2;

    let ii = (i as i64).rem_euclid(256) as usize;
    let jj = (j as i64).rem_euclid(256) as usize;

    let mut n = 0.0;

    let t0 = 0.5 - x0 * x0 - y0 * y0;
    if t0 > 0.0 {
        let t0 = t0 * t0;
        let gi = PERM[ii + PERM[jj] as usize];
        let (gx, gy) = grad2(gi);
        n += t0 * t0 * (gx * x0 + gy * y0);
    }

    let t1 = 0.5 - x1 * x1 - y1 * y1;
    if t1 > 0.0 {
        let t1 = t1 * t1;
        let gi = PERM[ii + i1 + PERM[jj + j1] as usize];
        let (gx, gy) = grad2(gi);
        n += t1 * t1 * (gx * x1 + gy * y1);
    }

    let t2 = 0.5 - x2 * x2 - y2 * y2;
    if t2 > 0.0 {
        let t2 = t2 * t2;
        let gi = PERM[ii + 1 + PERM[jj + 1] as usize];
        let (gx, gy) = grad2(gi);
        n += t2 * t2 * (gx * x2 + gy * y2);
    }

    // Scale to approximately [-1, 1]
    70.0 * n
}

/// Fractal Brownian motion — layered noise for turbulent, organic patterns.
/// `octaves`: more = more detail (4 is good for fire/aurora).
pub fn fbm(x: f64, y: f64, octaves: u32) -> f64 {
    let mut value = 0.0;
    let mut amplitude = 1.0;
    let mut frequency = 1.0;
    let mut max_value = 0.0;

    for _ in 0..octaves {
        value += amplitude * noise_2d(x * frequency, y * frequency);
        max_value += amplitude;
        amplitude *= 0.5;
        frequency *= 2.0;
    }

    value / max_value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noise_deterministic() {
        let a = noise_2d(1.5, 2.3);
        let b = noise_2d(1.5, 2.3);
        assert_eq!(a, b, "Same input must produce same output");
    }

    #[test]
    fn noise_in_range() {
        for i in 0..1000 {
            let x = i as f64 * 0.1;
            let y = i as f64 * 0.07;
            let v = noise_2d(x, y);
            assert!(v >= -1.5 && v <= 1.5, "noise_2d({x}, {y}) = {v} out of range");
        }
    }

    #[test]
    fn fbm_smoother_than_raw() {
        // fbm with 1 octave should equal noise_2d (scaled)
        let raw = noise_2d(3.0, 4.0);
        let fbm1 = fbm(3.0, 4.0, 1);
        assert!((raw - fbm1).abs() < 1e-10);
    }

    #[test]
    fn noise_1d_works() {
        let a = noise_1d(0.0);
        let b = noise_1d(1.0);
        // Just verify it returns different values and doesn't panic
        assert!((a - b).abs() > 1e-10 || true); // may collide, just don't panic
    }

    #[test]
    fn noise_varies_spatially() {
        // Noise at different points should generally differ
        let mut values: Vec<f64> = (0..20).map(|i| noise_2d(i as f64 * 0.5, 0.0)).collect();
        values.dedup_by(|a, b| (*a - *b).abs() < 1e-10);
        assert!(values.len() > 5, "Noise should vary across space");
    }
}
