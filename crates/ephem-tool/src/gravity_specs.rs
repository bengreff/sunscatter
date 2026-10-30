//! Which gravity field each body gets (D090): the source, the truncation
//! degree (the measured convergence of the game's lowest orbits:
//! `docs/technical/solar-system.md`, "Gravity fields") and the frame.

use crate::gravity::iau;
use sim::body::Rotation;

/// Where a model's coefficients come from.
pub enum Source {
    /// A PDS SHADR table in `data/external/gravity/` (downloaded from `url`).
    Shadr { file: &'static str, url: &'static str },
    /// An ICGEM `.gfc` file in `data/external/gravity/` (from `url`, unzipped).
    Icgem { file: &'static str, url: &'static str },
    /// Values printed in a paper: reference radius (km), GM (km³/s², `None`:
    /// the ephemeris'), `(n, m, C, S)`.
    Paper { radius_km: f64, gm_km3: Option<f64>, normalized: bool, coefficients: &'static [(usize, usize, f64, f64)] },
}

/// The model's frame against the body-fixed frame of `body.ron`'s rotation.
pub enum Frame {
    /// The same (or, for a zonal field, the same pole).
    Same { note: &'static str },
    /// A fixed rotation, rows from ours to the model's.
    Fixed { rows: [[f64; 3]; 3], note: &'static str },
    /// The model's own rotation model: the rotation between the two at the
    /// game's start (2030-01-01 TDB) is applied throughout.
    Model { rotation: Box<Rotation>, note: &'static str },
}

impl Frame {
    pub fn note(&self) -> Option<&'static str> {
        match self {
            Frame::Same { note } | Frame::Fixed { note, .. } | Frame::Model { note, .. } => {
                (!note.is_empty()).then_some(*note)
            }
        }
    }
}

pub struct Spec {
    /// Directory under `data/bodies`.
    pub body: &'static str,
    pub name: &'static str,
    pub model: &'static str,
    pub degree: usize,
    pub source: Source,
    pub frame: Frame,
    /// Keep only the zonal coefficients (`m = 0`).
    pub zonal_only: bool,
    /// Comment lines for the file: citation, what the data is, choices.
    pub notes: &'static [&'static str],
}

pub fn specs() -> Vec<Spec> {
    let mut v = planets();
    v.extend(giants());
    v.extend(crate::gravity_moons::moons());
    v
}

/// The passive (coordinate) rotation about axis `axis` (0 x, 1 y, 2 z) by
/// `arcsec`, as the lunar frame papers write `Rx`, `Ry`, `Rz`.
pub fn passive(axis: usize, arcsec: f64) -> glam::DMat3 {
    use glam::{DMat3, DVec3};
    let a = arcsec / 3600.0 * std::f64::consts::PI / 180.0;
    let (s, c) = (sim::math::sin(a), sim::math::cos(a));
    // Rows of the passive rotation; DMat3 takes columns, so transpose.
    let rows = match axis {
        0 => [[1.0, 0.0, 0.0], [0.0, c, s], [0.0, -s, c]],
        1 => [[c, 0.0, -s], [0.0, 1.0, 0.0], [s, 0.0, c]],
        _ => [[c, s, 0.0], [-s, c, 0.0], [0.0, 0.0, 1.0]],
    };
    DMat3::from_cols(DVec3::from(rows[0]), DVec3::from(rows[1]), DVec3::from(rows[2])).transpose()
}

/// The Moon's mean-Earth frame (our IAU series' approximation of it) into
/// the DE430 principal-axis frame of GRAIL's fields: the inverse of
/// `M = Rx(−0.285″) Ry(−78.580″) Rz(−67.573″) P` (Williams, Boggs &
/// Folkner 2013, JPL IOM 335-JW,DB,WF-20130722-016, eqs. 4–5; Folkner et
/// al. 2014, IPN PR 42-196, eq. 19), rows.
fn moon_pa_from_me() -> [[f64; 3]; 3] {
    let m = passive(2, 67.573) * passive(1, 78.580) * passive(0, 0.285);
    m.transpose().to_cols_array_2d()
}

fn planets() -> Vec<Spec> {
    vec![
        Spec {
            body: "earth",
            name: "Earth",
            model: "GGM05C",
            degree: 30,
            source: Source::Icgem {
                file: "GGM05C.ICGEM",
                url: "https://download.csr.utexas.edu/pub/grace/GGM05/GGM05C.ICGEM.gz",
            },
            frame: Frame::Same {
                note: "ITRF; ours is the IAU rotation (no nutation, polar motion or UT1): see solar-system.md.",
            },
            zonal_only: false,
            notes: &[
                "GGM05C: Ries et al. 2016, The Combined Global Gravity Model GGM05C, GFZ Data Services,",
                "doi:10.5880/icgem.2016.002 (CSR Report CSR-16-02, UT Austin): GRACE, GOCE and DTU13 surface",
                "gravity, complete to degree 360; C20 from satellite laser ranging. Zero-tide system: the",
                "permanent tidal deformation is in C20, as no tides are modelled.",
            ],
        },
        Spec {
            body: "moon",
            name: "The Moon",
            model: "GRGM1200A",
            degree: 100,
            source: Source::Shadr {
                file: "gggrx_1200a_sha.tab",
                url: "https://pds-geosciences.wustl.edu/grail/grail-l-lgrs-5-rdr-v1/grail_1001/shadr/gggrx_1200a_sha.tab",
            },
            frame: Frame::Fixed {
                rows: moon_pa_from_me(),
                note: "DE430 principal axes (the model's) from body.ron's mean-Earth frame: Rz(67.573\") Ry(78.580\") Rx(0.285\")",
            },
            zonal_only: false,
            notes: &[
                "GRGM1200A: Lemoine et al. 2014, GRL 41, 3382, doi:10.1002/2014GL060027 (GSFC; GRAIL primary",
                "and extended missions, degree 1200, a Kaula constraint only above 600).",
            ],
        },
        Spec {
            body: "mars",
            name: "Mars",
            model: "MRO120F",
            degree: 50,
            source: Source::Shadr {
                file: "jgmro_120f_sha.tab",
                url: "https://pds-geosciences.wustl.edu/mro/mro-m-rss-5-sdp-v1/mrors_1xxx/data/shadr/jgmro_120f_sha.tab",
            },
            frame: Frame::Same {
                note: "Konopliv's Mars frame, within ~25 m at the surface of body.ron's IAU 2015 (without its nutation and seasonal spin terms, <0.5\").",
            },
            zonal_only: false,
            notes: &[
                "MRO120F: Konopliv et al. 2020, GRL 47, e2020GL090568, doi:10.1029/2020GL090568 (JPL; MGS, Mars",
                "Odyssey and MRO tracking to 2019, Viking, Pathfinder and Opportunity landers; a Kaula",
                "constraint from degree 80). Its GM is the Mars system's (Phobos and Deimos), like the",
                "ephemeris' Mars. The seasonal J3 (3e-9) is left out.",
            ],
        },
        Spec {
            body: "venus",
            name: "Venus",
            model: "MGNP180U",
            degree: 40,
            source: Source::Shadr {
                file: "shgj180u.a01",
                url: "https://pds-geosciences.wustl.edu/mgn/mgn-v-rss-5-gravity-l2-v1/mg_5201/gravity/shgj180u.a01",
            },
            frame: Frame::Same {
                note: "the IAU Venus frame of body.ron (pole 272.76, 67.16; W 160.20 - 1.4813688 d).",
            },
            zonal_only: false,
            notes: &[
                "MGNP180U: Konopliv, Banerdt & Sjogren 1999, Icarus 139, 3, doi:10.1006/icar.1999.6086 (JPL;",
                "Magellan and Pioneer Venus Orbiter tracking, degree 180).",
            ],
        },
        Spec {
            body: "mercury",
            name: "Mercury",
            model: "HgM008",
            degree: 40,
            source: Source::Shadr {
                file: "ggmes_100v08_sha.tab",
                url: "https://pds-geosciences.wustl.edu/messenger/mess-h-rss_mla-5-sdp-v1/messrs_1001/data/shadr/ggmes_100v08_sha.tab",
            },
            frame: Frame::Model {
                rotation: Box::new(iau(281.0082, 61.4164, -0.0328, -0.0049, 329.75, 6.138505441)),
                note: "the model's (pole 281.0082, 61.4164; W 329.75 + 6.138505441 d), turned into body.ron's at 2030.",
            },
            zonal_only: false,
            notes: &[
                "HgM008: Genova et al. 2019, GRL 46, 3625, doi:10.1029/2018GL081135 (GSFC; MESSENGER tracking",
                "to the end of the mission, degree 100, a Kaula constraint 4e-5/n^2).",
            ],
        },
    ]
}

fn giants() -> Vec<Spec> {
    vec![
        Spec {
            body: "jupiter",
            name: "Jupiter",
            model: "Durante2020",
            degree: 12,
            source: Source::Shadr {
                file: "GRV_JUGR_SHA_JUP_U_DUR2020_20200203_V01.TAB",
                url:
                    "https://atmos.nmsu.edu/PDS/data/jnogrv_0002/DATA/SHADR/GRV_JUGR_SHA_JUP_U_DUR2020_20200203_V01.TAB",
            },
            frame: Frame::Same { note: "zonal: only the pole of body.ron is used." },
            zonal_only: true,
            notes: &[
                "Durante et al. 2020, GRL 47, e2019GL086572, doi:10.1029/2019GL086572 (Juno perijoves 1-17;",
                "J2 includes the permanent tide). The zonals J2-J12; the degree-2 tesserals (~1e-9, within",
                "~2 sigma of zero) are left out, which keeps the field zonal.",
            ],
        },
        Spec {
            body: "saturn",
            name: "Saturn",
            model: "Iess2019",
            degree: 10,
            source: Source::Paper {
                radius_km: 60330.0,
                gm_km3: None,
                normalized: false,
                coefficients: &[
                    (2, 0, -16290.573e-6, 0.0),
                    (4, 0, 935.314e-6, 0.0),
                    (6, 0, -86.340e-6, 0.0),
                    (8, 0, 14.624e-6, 0.0),
                    (10, 0, -4.672e-6, 0.0),
                ],
            },
            frame: Frame::Same { note: "zonal: only the pole of body.ron is used." },
            zonal_only: true,
            notes: &[
                "Iess et al. 2019, Science 364, eaat2965, doi:10.1126/science.aat2965 (Cassini Grand Finale):",
                "J2 16290.573, J4 -935.314, J6 86.340, J8 -14.624, J10 4.672 (1e-6, R 60,330 km) as quoted by",
                "Fortney et al. 2023, Saturn's Interior After the Cassini Grand Finale, arXiv:2304.09215, Table",
                "4.1. The odd zonals (~1e-7) are left out.",
            ],
        },
    ]
}
