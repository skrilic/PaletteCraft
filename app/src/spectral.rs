//! Spectral reflectance for Kubelka-Munk mixing.
//!
//! A paint is known only by its masstone colour (sRGB), so its reflectance curve is
//! reconstructed: the smoothest curve between 0 and 1 that has exactly that colour
//! (Burns' "least hyperbolic tangent slope squared" method, solved with Newton's method).
//! Mixing then runs per wavelength band, and the mixture's curve is turned back into a
//! colour with the CIE 1931 2° observer under a daylight illuminant.
//!
//! * Bands: `BANDS` wavelengths from `LAMBDA_MIN` nm in steps of `LAMBDA_STEP` nm.
//! * Observer: the multi-lobe Gaussian fit of the CIE 1931 colour matching functions
//!   (Wyman, Sloan and Shirley 2013).
//! * Illuminant: a black body at `ILLUMINANT_K` kelvin (close to daylight D65); the
//!   weights are scaled so a perfect white reflector gives linear sRGB (1, 1, 1).

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::OnceLock;

/// Number of wavelength bands.
pub const BANDS: usize = 31;
/// First band (nm).
pub const LAMBDA_MIN: f64 = 400.0;
/// Band spacing (nm).
pub const LAMBDA_STEP: f64 = 10.0;
/// Colour temperature of the illuminant (K).
pub const ILLUMINANT_K: f64 = 6504.0;
/// Reflectance is kept inside this margin from 0 and 1 when reconstructing.
const EDGE: f64 = 1e-4;

pub type Spectrum = [f32; BANDS];
/// Rows of the matrix from reflectance per band to linear sRGB.
pub type Basis = [[f32; BANDS]; 3];

pub fn wavelength(i: usize) -> f64 {
    LAMBDA_MIN + LAMBDA_STEP * i as f64
}

fn lobe(l: f64, mu: f64, s1: f64, s2: f64) -> f64 {
    let t = (l - mu) / if l < mu { s1 } else { s2 };
    (-0.5 * t * t).exp()
}

/// CIE 1931 2° colour matching functions x̄, ȳ, z̄ at `l` nm (analytic fit).
pub fn cmf(l: f64) -> [f64; 3] {
    let x = 1.056 * lobe(l, 599.8, 37.9, 31.0) + 0.362 * lobe(l, 442.0, 16.0, 26.7) - 0.065 * lobe(l, 501.1, 20.4, 26.2);
    let y = 0.821 * lobe(l, 568.8, 46.9, 40.5) + 0.286 * lobe(l, 530.9, 16.3, 31.1);
    let z = 1.217 * lobe(l, 437.0, 11.8, 36.0) + 0.681 * lobe(l, 459.0, 26.0, 13.8);
    [x, y, z]
}

/// Black-body spectral radiance at `l` nm (arbitrary scale).
fn planck(l: f64, t: f64) -> f64 {
    let m = l * 1e-9;
    1.0 / (m.powi(5) * ((1.438_776_9e-2 / (m * t)).exp() - 1.0))
}

fn basis64() -> &'static [[f64; BANDS]; 3] {
    static B: OnceLock<[[f64; BANDS]; 3]> = OnceLock::new();
    B.get_or_init(|| {
        const M: [[f64; 3]; 3] = [
            [3.240_454_2, -1.537_138_5, -0.498_531_4],
            [-0.969_266_0, 1.876_010_8, 0.041_556_0],
            [0.055_643_4, -0.204_025_9, 1.057_225_2],
        ];
        let mut t = [[0.0; BANDS]; 3];
        for i in 0..BANDS {
            let l = wavelength(i);
            let xyz = cmf(l);
            let e = planck(l, ILLUMINANT_K);
            for c in 0..3 {
                t[c][i] = e * (M[c][0] * xyz[0] + M[c][1] * xyz[1] + M[c][2] * xyz[2]);
            }
        }
        for row in &mut t {
            let sum: f64 = row.iter().sum();
            for v in row.iter_mut() {
                *v /= sum;
            }
        }
        t
    })
}

/// Matrix from reflectance per band to linear sRGB (each row sums to 1).
pub fn basis() -> Basis {
    let b = basis64();
    let mut out = [[0.0f32; BANDS]; 3];
    for c in 0..3 {
        for i in 0..BANDS {
            out[c][i] = b[c][i] as f32;
        }
    }
    out
}

/// Linear sRGB of a reflectance curve.
#[inline]
pub fn to_linear(t: &Basis, r: &[f32]) -> [f32; 3] {
    let mut out = [0.0f32; 3];
    for c in 0..3 {
        let mut acc = 0.0;
        for i in 0..BANDS {
            acc += t[c][i] * r[i];
        }
        out[c] = acc.clamp(0.0, 1.0);
    }
    out
}

/// Smoothest reflectance curve with linear sRGB `rgb` (each channel strictly inside 0..1),
/// or `None` when Newton's method does not converge.
fn lhtss(rgb: [f64; 3]) -> Option<[f64; BANDS]> {
    const N: usize = BANDS;
    const NN: usize = BANDS + 3;
    let t = basis64();
    let mut z = [0.0f64; N];
    let mut lam = [0.0f64; 3];
    for _ in 0..60 {
        let mut rho = [0.0; N];
        let mut d1 = [0.0; N];
        let mut d2 = [0.0; N];
        let mut tl = [0.0; N];
        for i in 0..N {
            let th = z[i].tanh();
            let sech2 = 1.0 - th * th;
            rho[i] = 0.5 * (th + 1.0);
            d1[i] = 0.5 * sech2;
            d2[i] = -sech2 * th;
            tl[i] = t[0][i] * lam[0] + t[1][i] * lam[1] + t[2][i] * lam[2];
        }
        // F: gradient of the slope sum plus the constraint term, then the colour error.
        let mut f = [0.0; NN];
        for i in 0..N {
            let left = if i > 0 { z[i] - z[i - 1] } else { 0.0 };
            let right = if i + 1 < N { z[i + 1] - z[i] } else { 0.0 };
            f[i] = 2.0 * (left - right) + d1[i] * tl[i];
        }
        for c in 0..3 {
            f[N + c] = (0..N).map(|i| t[c][i] * rho[i]).sum::<f64>() - rgb[c];
        }
        let colour_err = (f[N].abs()).max(f[N + 1].abs()).max(f[N + 2].abs());
        let norm: f64 = f.iter().map(|v| v * v).sum::<f64>().sqrt();
        if norm < 1e-10 || (colour_err < 1e-7 && norm < 1e-6) {
            return Some(rho);
        }
        // Jacobian.
        let mut j = [[0.0f64; NN]; NN];
        for i in 0..N {
            let ends = i == 0 || i + 1 == N;
            j[i][i] = if ends { 2.0 } else { 4.0 } + d2[i] * tl[i];
            if i > 0 {
                j[i][i - 1] = -2.0;
            }
            if i + 1 < N {
                j[i][i + 1] = -2.0;
            }
            for c in 0..3 {
                j[i][N + c] = d1[i] * t[c][i];
                j[N + c][i] = t[c][i] * d1[i];
            }
        }
        let mut rhs = [0.0; NN];
        for k in 0..NN {
            rhs[k] = -f[k];
        }
        let delta = solve(&mut j, &mut rhs)?;
        for i in 0..N {
            z[i] += delta[i];
        }
        for c in 0..3 {
            lam[c] += delta[N + c];
        }
        if !z.iter().chain(lam.iter()).all(|v| v.is_finite()) {
            return None;
        }
    }
    None
}

/// Gaussian elimination with partial pivoting.
fn solve<const N: usize>(a: &mut [[f64; N]; N], b: &mut [f64; N]) -> Option<[f64; N]> {
    for col in 0..N {
        let piv = (col..N).max_by(|&x, &y| a[x][col].abs().total_cmp(&a[y][col].abs()))?;
        if a[piv][col].abs() < 1e-14 {
            return None;
        }
        a.swap(col, piv);
        b.swap(col, piv);
        for row in col + 1..N {
            let f = a[row][col] / a[col][col];
            if f != 0.0 {
                for k in col..N {
                    a[row][k] -= f * a[col][k];
                }
                b[row] -= f * b[col];
            }
        }
    }
    let mut x = [0.0; N];
    for row in (0..N).rev() {
        let mut s = b[row];
        for k in row + 1..N {
            s -= a[row][k] * x[k];
        }
        x[row] = s / a[row][row];
    }
    Some(x)
}

thread_local! {
    static CACHE: RefCell<HashMap<[u32; 3], Spectrum>> = RefCell::new(HashMap::new());
}

/// Reflectance curve of a colour given in linear sRGB. Colours that no curve between 0 and
/// 1 can reach (none of the sRGB cube in practice) are moved towards their grey until one does.
pub fn reflectance(lin: [f32; 3]) -> Spectrum {
    let key = lin.map(f32::to_bits);
    if let Some(s) = CACHE.with(|c| c.borrow().get(&key).copied()) {
        return s;
    }
    let target = lin.map(|v| (v as f64).clamp(EDGE, 1.0 - EDGE));
    let grey = 0.212_672_9 * target[0] + 0.715_152_2 * target[1] + 0.072_175 * target[2];
    let mut out = [grey as f32; BANDS];
    for f in [1.0, 0.97, 0.9, 0.8, 0.6, 0.4, 0.2] {
        let t = target.map(|v| grey + f * (v - grey));
        if let Some(r) = lhtss(t) {
            for i in 0..BANDS {
                out[i] = r[i] as f32;
            }
            break;
        }
    }
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        if c.len() > 4096 {
            c.clear();
        }
        c.insert(key, out);
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::colour::{delta_e, lin_to_lab, srgb8_to_lin};

    #[test]
    fn white_reflector_is_white() {
        let t = basis();
        let w = to_linear(&t, &[1.0; BANDS]);
        for c in w {
            assert!((c - 1.0).abs() < 1e-4, "{w:?}");
        }
    }

    #[test]
    fn curves_reproduce_their_colour_and_stay_in_range() {
        let t = basis();
        let mut worst = 0.0f32;
        for r in (0..=255).step_by(15) {
            for g in (0..=255).step_by(15) {
                for b in (0..=255).step_by(15) {
                    let lin = srgb8_to_lin([r as u8, g as u8, b as u8]);
                    let s = reflectance(lin);
                    assert!(s.iter().all(|v| (0.0..=1.0).contains(v)), "{r} {g} {b}: {s:?}");
                    let back = to_linear(&t, &s);
                    worst = worst.max(delta_e(lin_to_lab(back), lin_to_lab(lin)));
                }
            }
        }
        assert!(worst < 1.0, "worst round trip dE {worst}");
    }

    #[test]
    fn library_colours_round_trip_exactly() {
        let t = basis();
        for p in crate::palette::LIBRARY {
            let lin = srgb8_to_lin(p.color);
            let back = to_linear(&t, &reflectance(lin));
            let de = delta_e(lin_to_lab(back), lin_to_lab(lin));
            assert!(de < 0.2, "{} dE {de}", p.name);
        }
    }

    #[test]
    fn grey_is_flat() {
        let s = reflectance([0.2, 0.2, 0.2]);
        for v in s {
            assert!((v - 0.2).abs() < 0.01, "{s:?}");
        }
    }
}
