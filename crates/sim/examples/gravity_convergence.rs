//! How many degrees of a body's gravity field an orbit needs (D090): the
//! same orbit integrated about the body alone (its point mass and field, its
//! rotation model) with the field truncated at each degree, against the
//! highest; and the cost of an evaluation at each degree.
//!
//! ```text
//! cargo run -p sim --release --example gravity_convergence -- DIR BODY ALT_KM INC_DEG DAYS N1 N2 ...
//! ```
//! `DIR/BODY/gravity.ron` (e.g. written by `ephem-tool gravity --degree 400
//! --out DIR`) and `data/bodies/BODY/body.ron` for the rotation. Prints, per
//! degree, the largest position difference from the highest degree over the
//! arc (sampled every 60 s) and at the end, and the periapsis altitude.
//! `GRAVITY_COAST=1` integrates the truncated fields at the coasts'
//! tolerance (`vessel::coast_tolerance`) against the tight reference.
use glam::DVec3;
use sim::integrate::{hermite5, Dop853, StepSample, StepState, Tolerance};
use sim::kepler::Elements;
use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 6 {
        eprintln!("usage: gravity_convergence DIR BODY ALT_KM INC_DEG DAYS N1 N2 ...");
        std::process::exit(2);
    }
    let text = std::fs::read_to_string(Path::new(&args[0]).join(&args[1]).join("gravity.ron")).unwrap();
    let field = sim::gravity::file::parse(&text).unwrap();
    let body = sim::body::load_body(&sim::body::default_bodies_dir().join(&args[1])).unwrap().physical;
    let (alt, inc, days): (f64, f64, f64) =
        (args[2].parse().unwrap(), args[3].parse().unwrap(), args[4].parse().unwrap());
    let degrees: Vec<usize> = args[5..].iter().map(|a| a.parse().unwrap()).collect();
    let gm = field.gm.expect("a model GM");
    let t0 = sim::sol::sol_epoch();
    let a = body.radius_eq + alt * 1e3;
    let el = Elements { a, e: 0.0, i: inc * sim::math::PI / 180.0, raan: 0.4, argp: 0.0, mean_anomaly: 0.0 };
    // Start in the body's equatorial frame, turned to inertial.
    let (r0, v0) = el.to_state(gm);
    let m = body.rotation.matrix(t0);
    let to_inertial = |x: DVec3| {
        // equatorial (x toward the node of the equator, z the pole) → inertial
        let w = body.rotation.angle(t0);
        let (s, c) = (sim::math::sin(w), sim::math::cos(w));
        let fixed = DVec3::new(x.x * c + x.y * s, -x.x * s + x.y * c, x.z);
        m * fixed
    };
    let (r0, v0) = (to_inertial(r0), to_inertial(v0));
    let span = days * 86_400.0;
    let tight = Tolerance { abs_r: 1e-6, abs_v: 1e-9, rel: 1e-13, h_min: 1e-6, h_max: 600.0 };
    // GRAVITY_COAST=1: the truncated runs at the coasts' tolerance (the
    // reference stays tight): the error a coast actually makes.
    let coast = std::env::var("GRAVITY_COAST").is_ok();
    let run = |n: usize, tol: Tolerance| {
        let f = |t: f64, r: DVec3, _v: DVec3| {
            let d2 = r.length_squared();
            -r * (gm / (d2 * d2.sqrt())) + field.accel(gm, r, &body.rotation, t0.add_seconds(t), n, false).0
        };
        let integ = Dop853::new(tol);
        let mut st: StepState = integ.start(&f, 0.0, r0, v0, 10.0);
        let mut samples = vec![StepSample { t: 0.0, r: r0, v: v0, a: st.a }];
        let mut lowest = f64::INFINITY;
        // Free steps, sampled every 60 s by the quintic Hermite dense output
        // between them (as a coast segment is).
        while st.t < span {
            let s = integ.step(&f, &mut st, span).expect("finite");
            lowest = lowest.min(s.r.length() - body.radius_eq);
            samples.push(s);
        }
        let steps = samples.len() - 1;
        let mut out = Vec::new();
        let mut k = 0;
        let mut j = 0;
        while (k as f64) * 60.0 <= span {
            let t = k as f64 * 60.0;
            while j + 2 < samples.len() && samples[j + 1].t < t {
                j += 1;
            }
            out.push(hermite5(&samples[j], &samples[j + 1], t).0);
            k += 1;
        }
        (out, lowest, steps)
    };
    let top = *degrees.iter().max().unwrap();
    let (reference, low_ref, _) = run(top, tight);
    // GRAVITY_HCAP=c: steps of at most c times the time to fly a half
    // wavelength of the highest degree (π r / (n v)).
    let cap: Option<f64> = std::env::var("GRAVITY_HCAP").ok().map(|c| c.parse().unwrap());
    let tol_for = |n: usize| {
        let mut tol = if coast { sim::vessel::coast_tolerance() } else { tight };
        if let Some(c) = cap {
            tol.h_max = c * sim::math::PI * a / (n.max(2) as f64 * (gm / a).sqrt());
        }
        tol
    };
    println!(
        "{} {} km circular, i {inc} deg, {days} days; reference degree {top} (lowest {:.3} km)",
        args[1],
        alt,
        low_ref / 1e3
    );
    for &n in degrees.iter().filter(|&&n| n != top || coast) {
        let (path, lowest, steps) = run(n, tol_for(n));
        let worst = path.iter().zip(&reference).map(|(a, b)| (*a - *b).length()).fold(0.0, f64::max);
        let end = (*path.last().unwrap() - *reference.last().unwrap()).length();
        println!("degree {n:4}: worst {worst:12.3} m, end {end:12.3} m, lowest {:.3} km, {steps} steps", lowest / 1e3);
    }
    // Cost of one evaluation at each degree (near the orbit).
    for &n in &degrees {
        let reps = 20_000 / (1 + n * n / 400);
        let clock = std::time::Instant::now();
        let mut acc = DVec3::ZERO;
        for i in 0..reps {
            let r = r0 * (1.0 + 1e-9 * i as f64);
            acc += field.accel(gm, r, &body.rotation, t0.add_seconds(i as f64), n, false).0;
        }
        let us = clock.elapsed().as_secs_f64() / reps as f64 * 1e6;
        println!("degree {n:4}: {us:8.2} us per evaluation ({:e})", acc.x);
    }
}
