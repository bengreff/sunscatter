//! The moons' and Ceres' gravity fields (D090): what spacecraft measured,
//! degree 2 and up where the solution gives it (unnormalized values are
//! as printed, `C_n0 = −J_n`).

use crate::gravity::iau;
use crate::gravity_specs::{Frame, Source, Spec};

const SYNCHRONOUS: &str = "body.ron's IAU frame (synchronous: x near the mean sub-planet point, as the solution's).";

pub fn moons() -> Vec<Spec> {
    let mut v = galilean();
    v.extend(saturnian());
    v.push(Spec {
        body: "ceres",
        name: "Ceres",
        model: "CERES18D",
        degree: 18,
        source: Source::Shadr {
            file: "JGDWN_CER18D_SHA.TAB",
            url: "https://sbnarchive.psi.edu/pds3/dawn/grav/DWNCGRS_2_v4/DATA/SHADR/JGDWN_CER18D_SHA.TAB",
        },
        frame: Frame::Model {
            rotation: Box::new(iau(291.42763, 66.76033, 0.0, 0.0, 170.309, 952.1532635)),
            note: "the model's (pole 291.42763, 66.76033; W 170.309 + 952.1532635 d), turned into body.ron's at 2030.",
        },
        zonal_only: false,
        notes: &[
            "CERES18D: Konopliv (JPL), Dawn gravity, PDS SBN DWNCGRS_2 v4 (2024); the field of Konopliv et al.",
            "2018, Icarus 299, 411, doi:10.1016/j.icarus.2017.08.005, to degree 18 (Dawn tracking 2015-2016).",
        ],
    });
    v
}

fn galilean() -> Vec<Spec> {
    vec![
        Spec {
            body: "io",
            name: "Io",
            model: "Park2025",
            degree: 2,
            source: Source::Paper {
                radius_km: 1829.4,
                gm_km3: Some(5959.8948),
                normalized: false,
                coefficients: &[(2, 0, -1834.6e-6, 0.0), (2, 1, 0.0, -4.0e-6), (2, 2, 549.6e-6, 0.5e-6)],
            },
            frame: Frame::Same { note: SYNCHRONOUS },
            zonal_only: false,
            notes: &[
                "Park et al. 2025, Nature 638, 69, doi:10.1038/s41586-024-08442-5, Extended Data Table 1 (Juno",
                "and Galileo; no hydrostatic constraint; J2 and C22 include the permanent tide).",
            ],
        },
        Spec {
            body: "europa",
            name: "Europa",
            model: "GomezCasajus2021",
            degree: 2,
            source: Source::Paper {
                radius_km: 1565.0,
                gm_km3: None,
                normalized: false,
                coefficients: &[(2, 0, -461.39e-6, 0.0), (2, 2, 138.42e-6, 0.0)],
            },
            frame: Frame::Same { note: SYNCHRONOUS },
            zonal_only: false,
            notes: &[
                "Gomez Casajus et al. 2021, Icarus 358, 114187, doi:10.1016/j.icarus.2020.114187, Table 2,",
                "SOL-B (Galileo flybys reanalysed; J2/C22 = 10/3 imposed, as Anderson et al. 1998 did).",
            ],
        },
        Spec {
            body: "ganymede",
            name: "Ganymede",
            model: "GomezCasajus2022",
            degree: 5,
            source: Source::Paper {
                radius_km: 2631.2,
                gm_km3: None,
                normalized: true,
                coefficients: &[
                    (2, 0, -5.948166672982249e-05, 0.0),
                    (2, 1, -3.376656360991323e-06, -6.198605016943528e-06),
                    (2, 2, 6.128512804347833e-05, 1.719778241510138e-06),
                    (3, 0, 5.516019563935706e-08, 0.0),
                    (3, 1, -4.969821745652446e-07, -3.388234871569233e-06),
                    (3, 2, -2.471188520778999e-06, -2.631456832761436e-06),
                    (3, 3, 1.861910193237492e-06, 6.252561114408979e-06),
                    (4, 0, -9.040863626893271e-07, 0.0),
                    (4, 1, -1.97971372279629e-07, -8.428043988647186e-07),
                    (4, 2, -6.637786140450445e-07, -1.324113927222187e-06),
                    (4, 3, -2.334828330766576e-06, 4.558173462251096e-07),
                    (4, 4, 1.118029420047734e-06, 7.808563741354416e-07),
                    (5, 0, -4.378039311621959e-07, 0.0),
                    (5, 1, 3.416987791621206e-07, 4.322974722792315e-09),
                    (5, 2, -2.217219743375731e-07, -3.777319461161418e-07),
                    (5, 3, -1.765682886830437e-06, 5.742255184900602e-08),
                    (5, 4, -1.264081055558542e-07, 1.763636474322547e-06),
                    (5, 5, 1.253876417049197e-06, 3.604329783440386e-07),
                ],
            },
            frame: Frame::Same { note: SYNCHRONOUS },
            zonal_only: false,
            notes: &[
                "Gomez Casajus et al. 2022, GRL 49, e2022GL099475, doi:10.1029/2022GL099475 (Juno's 2021",
                "flyby and Galileo; degree 5, no hydrostatic constraint): the data set doi:10.5281/zenodo.6949645,",
                "SOI_gravity_field.txt (fully normalized, R 2631.2 km).",
            ],
        },
        Spec {
            body: "callisto",
            name: "Callisto",
            model: "Anderson2001",
            degree: 2,
            source: Source::Paper {
                radius_km: 2410.3,
                gm_km3: Some(7179.292),
                normalized: false,
                coefficients: &[(2, 0, -32.7e-6, 0.0), (2, 2, 10.2e-6, 0.0)],
            },
            frame: Frame::Same { note: SYNCHRONOUS },
            zonal_only: false,
            notes: &[
                "Anderson et al. 2001, Icarus 153, 157, doi:10.1006/icar.2001.6664 (Galileo flybys; J2/C22 = 10/3",
                "imposed), as tabulated by Schubert et al. 2004 (Jupiter, CUP, ch. 13, Table 13.1).",
            ],
        },
    ]
}

fn saturnian() -> Vec<Spec> {
    vec![
        Spec {
            body: "titan",
            name: "Titan",
            model: "Durante2019",
            degree: 3,
            source: Source::Paper {
                radius_km: 2575.0,
                gm_km3: Some(8978.1383),
                normalized: false,
                coefficients: &[
                    (2, 0, -33.089e-6, 0.0),
                    (2, 1, 0.513e-6, 0.612e-6),
                    (2, 2, 10.385e-6, -0.064e-6),
                    (3, 0, 0.179e-6, 0.0),
                    (3, 1, 1.481e-6, 0.811e-6),
                    (3, 2, 0.183e-6, -0.027e-6),
                    (3, 3, -0.222e-6, -0.226e-6),
                ],
            },
            frame: Frame::Same { note: SYNCHRONOUS },
            zonal_only: false,
            notes: &[
                "Durante et al. 2019, Icarus 326, 123, doi:10.1016/j.icarus.2019.03.003, Table 2 (Cassini, all",
                "flybys; degree 5, no hydrostatic constraint): degrees 2 and 3; degrees 4 and 5 are mostly",
                "within their uncertainties and left out.",
            ],
        },
        Spec {
            body: "enceladus",
            name: "Enceladus",
            model: "Iess2014",
            degree: 3,
            source: Source::Paper {
                radius_km: 254.2,
                gm_km3: Some(7.210443),
                normalized: false,
                coefficients: &[
                    (2, 0, -5435.2e-6, 0.0),
                    (2, 1, 9.2e-6, 39.8e-6),
                    (2, 2, 1549.8e-6, 22.6e-6),
                    (3, 0, 115.3e-6, 0.0),
                ],
            },
            frame: Frame::Same { note: SYNCHRONOUS },
            zonal_only: false,
            notes: &[
                "Iess et al. 2014, Science 344, 78, doi:10.1126/science.1250551, Table S3, SOL1 (Cassini flybys",
                "E9, E12, E19); R 254.2 km as Park et al. 2024 and McKinnon 2015 state it.",
            ],
        },
    ]
}
