//! `ephem-tool gravity`: the bodies' gravity fields (D090) from the published
//! models into `data/bodies/<body>/gravity.ron` (read by `sim::gravity::file`).
//!
//! Raw downloads live in `data/external/gravity/` (gitignored; each spec
//! names its URL). The files written are truncated to the degree the game's
//! orbits need ([`crate::gravity_specs`]: the measured convergence) and
//! carry the source, the model's reference radius and GM, and the rotation
//! from the body-fixed frame of the body's rotation model (`body.ron`) into
//! the model's frame where the two differ.

use crate::gravity_specs::{specs, Frame, Source, Spec};
use sim::body::Rotation;
use sim::gravity::file;
use std::fmt::Write as _;
use std::path::Path;

/// The coefficients and constants read from a source.
pub struct Model {
    /// Reference radius (m).
    pub radius: f64,
    /// GM (m³/s²), if the model states one.
    pub gm: Option<f64>,
    pub normalized: bool,
    /// (n, m, C, S) up to the source's degree.
    pub coefficients: Vec<(usize, usize, f64, f64)>,
}

/// Runs the tool: every spec (or only `only`), up to `degree` if given
/// (for convergence studies), into `out/<body>/gravity.ron`.
pub fn run(external: &Path, out: &Path, only: Option<&str>, degree: Option<usize>) {
    for spec in specs().iter().filter(|s| only.is_none_or(|o| o == s.body)) {
        let model = read(external, &spec.source);
        let degree = degree.unwrap_or(spec.degree);
        let text = render(spec, &model, degree, &frame_matrix(spec, &out.join(spec.body)));
        // The file must read back as the field it describes.
        let field = file::parse(&text).unwrap_or_else(|e| panic!("{}: {e}", spec.body));
        let path = out.join(spec.body).join(file::GRAVITY_FILE);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, &text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        println!(
            "{:10} {:12} degree {:3} ({} bytes) -> {}",
            spec.body,
            spec.model,
            field.degree(),
            text.len(),
            path.display()
        );
    }
}

/// Reads a model from its source.
pub fn read(external: &Path, source: &Source) -> Model {
    match source {
        Source::Shadr { file, .. } => shadr(&read_text(&external.join(file))),
        Source::Icgem { file, .. } => icgem(&read_text(&external.join(file))),
        Source::Paper { radius_km, gm_km3, normalized, coefficients } => Model {
            radius: radius_km * 1e3,
            gm: gm_km3.map(|g| g * 1e9),
            normalized: *normalized,
            coefficients: coefficients.to_vec(),
        },
    }
}

fn read_text(path: &Path) -> String {
    let bytes = std::fs::read(path)
        .unwrap_or_else(|e| panic!("{}: {e} (download it first: see the spec's URL)", path.display()));
    String::from_utf8_lossy(&bytes).into_owned()
}

/// A PDS SHADR table: a header line (reference radius km, GM km³/s², its
/// uncertainty, degree, order, normalization 1/0, reference longitude and
/// latitude), then `n, m, C, S, σC, σS` rows. A PDS3 label attached before
/// it (Magellan's) is skipped: the header is the first line of eight numbers.
pub fn shadr(text: &str) -> Model {
    // A field's last word: Magellan's header follows an SFDU marker on its line.
    let numbers = |line: &str| -> Option<Vec<f64>> {
        let fields: Vec<&str> = line.split(',').filter_map(|f| f.split_whitespace().last()).collect();
        fields.iter().map(|f| f.parse::<f64>().ok()).collect()
    };
    let mut lines = text.lines();
    let header = lines.by_ref().find_map(|l| numbers(l).filter(|v| v.len() == 8)).expect("a SHADR header");
    let mut coefficients = Vec::new();
    for line in lines {
        let Some(v) = numbers(line) else { continue };
        if v.len() >= 4 {
            coefficients.push((v[0] as usize, v[1] as usize, v[2], v[3]));
        }
    }
    Model { radius: header[0] * 1e3, gm: Some(header[1] * 1e9), normalized: header[5] == 1.0, coefficients }
}

/// An ICGEM `.gfc` file (SI units, `D` exponents allowed).
pub fn icgem(text: &str) -> Model {
    let value = |key: &str| -> Option<String> {
        text.lines().find_map(|l| l.strip_prefix(key).map(|v| v.trim().replace(['D', 'd'], "E")))
    };
    let radius: f64 = value("radius").expect("radius").parse().unwrap();
    let gm: f64 = value("earth_gravity_constant").expect("GM").parse().unwrap();
    let normalized = value("norm").is_none_or(|n| n.to_lowercase().starts_with("fully"));
    let mut coefficients = Vec::new();
    for line in text.lines().skip_while(|l| !l.starts_with("end_of_head")).filter(|l| l.starts_with("gfc ")) {
        let f: Vec<String> = line.split_whitespace().map(|s| s.replace(['D', 'd'], "E")).collect();
        let (n, m) = (f[1].parse().unwrap(), f[2].parse().unwrap());
        coefficients.push((n, m, f[3].parse().unwrap(), f[4].parse().unwrap()));
    }
    Model { radius, gm: Some(gm), normalized, coefficients }
}

/// The rotation (rows) from the body-fixed frame of the body's rotation
/// model to the model's frame, if they differ.
fn frame_matrix(spec: &Spec, body_dir: &Path) -> Option<[[f64; 3]; 3]> {
    let ours = || sim::body::load_body(body_dir).unwrap_or_else(|e| panic!("{e}")).physical.rotation;
    match &spec.frame {
        Frame::Same { .. } => None,
        Frame::Fixed { rows, .. } => Some(*rows),
        Frame::Model { rotation, .. } => {
            // At the game's start: ours → inertial → the model's.
            let t = sim::sol::sol_epoch();
            let m = rotation.matrix(t).transpose() * ours().matrix(t);
            Some(m.transpose().to_cols_array_2d())
        }
    }
}

/// The file's text: comments with the source and the choices, then the
/// field to `degree` (coefficients that are zero are left out).
pub fn render(spec: &Spec, model: &Model, degree: usize, frame: &Option<[[f64; 3]; 3]>) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "// {}'s gravity field beyond its point mass (D090), read by sim::gravity::file.", spec.name);
    let _ = writeln!(s, "// Written by `cargo run -p ephem-tool --release -- gravity`; do not edit by hand.");
    for line in spec.notes {
        let _ = writeln!(s, "// {line}");
    }
    if let Source::Shadr { url, .. } | Source::Icgem { url, .. } = &spec.source {
        let _ = writeln!(s, "// Source: {url}");
    }
    let available = model.coefficients.iter().map(|c| c.0).max().unwrap_or(0);
    let _ = writeln!(s, "// Degree {degree} of the source's {available}.");
    if let Some(note) = spec.frame.note() {
        let _ = writeln!(s, "// Frame: {note}");
    }
    let _ = writeln!(s, "(");
    let _ = writeln!(s, "    model: {:?},", spec.model);
    let _ = writeln!(s, "    radius: {:?},", model.radius);
    match model.gm {
        Some(gm) => _ = writeln!(s, "    gm: Some({gm:?}),"),
        None => _ = writeln!(s, "    gm: None,"),
    }
    let _ = writeln!(s, "    normalized: {},", model.normalized);
    if let Some(rows) = frame {
        let row = |r: &[f64; 3]| format!("({:?}, {:?}, {:?})", r[0], r[1], r[2]);
        let _ = writeln!(s, "    frame: Some(({}, {}, {})),", row(&rows[0]), row(&rows[1]), row(&rows[2]));
    }
    let _ = writeln!(s, "    coefficients: [");
    for &(n, m, c, sn) in &model.coefficients {
        let kept = n >= 2 && n <= degree && (c != 0.0 || sn != 0.0) && (m == 0 || !spec.zonal_only);
        if kept {
            let _ = writeln!(s, "        ({n}, {m}, {c:e}, {sn:e}),");
        }
    }
    let _ = writeln!(s, "    ],");
    let _ = writeln!(s, ")");
    s
}

/// An IAU-style rotation from degrees and days (as `body.ron` writes them).
pub fn iau(ra: f64, dec: f64, ra_rate_cy: f64, dec_rate_cy: f64, w0: f64, w_rate_day: f64) -> Rotation {
    let deg = std::f64::consts::PI / 180.0;
    let century = 36_525.0 * 86_400.0;
    Rotation {
        ra: ra * deg,
        dec: dec * deg,
        ra_rate: ra_rate_cy * deg / century,
        dec_rate: dec_rate_cy * deg / century,
        w0: w0 * deg,
        w_rate: w_rate_day * deg / 86_400.0,
        librations: Rotation::NO_LIBRATIONS,
        n_librations: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_both_formats() {
        let pds = "CCSD3ZF\nEND\nSD$$MARKER##mark##NJPL3IF0003300000001  .6051000000000000E+04,  .3248585920790000E+06,  .6376000000000000E-02,  180,  180,    1,  .0E+00,  .00000\n    2,    0, -.1971E-05,  .0000000000000000E+00, 1e-9, 0\n    2,    2,  .3E-06, -.1E-06, 1e-9, 1e-9\n";
        let m = shadr(pds);
        assert_eq!((m.radius, m.gm, m.normalized), (6.051e6, Some(3.24858592079e14), true));
        assert_eq!(m.coefficients, vec![(2, 0, -1.971e-6, 0.0), (2, 2, 3e-7, -1e-7)]);
        let gfc = "modelname x\nearth_gravity_constant 3.986004415E+14\nradius 6.378136300E+06\nnorm fully_normalized\nend_of_head ===\ngfc    2    0 -4.841694573200D-04  0.0D+00  1.1D-10  0.0D+00\n";
        let m = icgem(gfc);
        assert_eq!((m.radius, m.gm), (6_378_136.3, Some(3.986004415e14)));
        assert_eq!(m.coefficients, vec![(2, 0, -4.8416945732e-4, 0.0)]);
    }
}
