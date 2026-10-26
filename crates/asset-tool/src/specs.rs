//! Each body's map sources and procedural parameters ([`crate::planets`]).
//! Sources, licences and the statistics behind the procedural values are
//! cited in `docs/features/solar-visuals.md` (LANDS section); in short:
//! USGS Astrogeology global DEMs and mosaics (public domain, NASA missions),
//! crater densities as cumulative `N(>1 km)` per km² for a `D^−2` law
//! (near empirical saturation, ~0.1, on the old icy satellites: Melosh 1989
//! ch. 10; Zahnle et al. 2003; Kirchoff & Schenk 2010), transition diameters
//! and relaxed depths from Schenk (2002, Nature 417, 419) and Schenk et al.
//! (2018, Enceladus and the Icy Moons of Saturn), triaxial radii from the
//! IAU (pck00011) or the body's `body.ron` source.

use crate::planets::{ColorSource, Colors, HeightSource, Heights, Spec};
use crate::relief::{Craters, Landform, Relief};

const fn mosaic(file: &'static str, bands: [usize; 3]) -> Colors {
    Colors::Mosaic(ColorSource { url: file, file, bands, zero_is_no_data: true })
}

const GREY: [usize; 3] = [0, 0, 0];
const RGB: [usize; 3] = [0, 1, 2];

/// Heavily cratered icy surface near empirical saturation.
const fn icy(n1: f64, transition_km: f64, max_depth_m: f64, fresh: f64) -> Craters {
    Craters { n1, slope: 2.0, d_max_km: 400.0, transition_km, max_depth_m, fresh }
}

const fn relief(
    axes: Option<(f64, f64, f64)>,
    craters: Option<Craters>,
    gazetteer: Option<&'static str>,
    relief_m: f64,
    seed: u64,
) -> Heights {
    Heights::Procedural(Relief {
        axes,
        craters,
        gazetteer,
        relief_m,
        relief_wavelength_m: 400_000.0,
        seed,
        landforms: &[],
    })
}

/// Io's named landforms (Gazetteer positions and diameters; heights are
/// typical values): mountains 6 km on average and up to 17.5 km high with
/// flat-topped, block-like forms (Schenk et al. 2001, JGR 106, 33201),
/// mensae and plana 1-3 km, paterae ~1 km deep, flat-floored (Radebaugh et
/// al. 2001, JGR 106, 33005).
const IO_LANDFORMS: &[Landform] = &[
    Landform { kind: "Mons", height_per_diameter: 0.04, min_m: 2000.0, max_m: 17_500.0, flat: 0.3 },
    Landform { kind: "Mensa", height_per_diameter: 0.01, min_m: 1000.0, max_m: 3000.0, flat: 0.6 },
    Landform { kind: "Planum", height_per_diameter: 0.005, min_m: 500.0, max_m: 2000.0, flat: 0.6 },
    Landform { kind: "Patera", height_per_diameter: -0.02, min_m: 500.0, max_m: 2000.0, flat: 0.7 },
];

const fn spec(body: &'static str, height: Heights, color: Colors, widths: (usize, usize, usize)) -> Spec {
    Spec { body, height, color, height_scale: 1.0, height_w: widths.0, color_w: widths.1, zones_w: widths.2 }
}

pub const SPECS: &[Spec] = &[
    spec(
        "mars",
        // MOLA MEGDR radius at 32 ppd (PDS Geosciences, 1.85 km): radius minus 3,396 km.
        Heights::Dem(HeightSource {
            url:
                "https://pds-geosciences.wustl.edu/mgs/urn-nasa-pds-mgs_mola_topography_derived/meg032/megr90n000fb.img",
            file: "megr90n000fb.img",
            scale: 1.0,
            offset: 0.0,
            valid: (-30000.0, 30000.0),
            sphere_m: 3_396_000.0,
            raw: Some((11520, 5760, 0.0)),
        }),
        // Viking colour mosaic (USGS, 925 m).
        mosaic("Mars_Viking_ClrMosaic_global_925m.tif", RGB),
        // 3.7 km per height sample (Earth's map is 4.9 km).
        (5760, 4096, 4096),
    ),
    spec(
        "mercury",
        // MESSENGER MDIS stereo DEM v2 (USGS, 665 m): half-metres above a 2,439.4 km sphere.
        Heights::Dem(HeightSource {
            url: "Mercury_Messenger_USGS_DEM_Global_665m_v2.tif",
            file: "Mercury_Messenger_USGS_DEM_Global_665m_v2.tif",
            scale: 0.5,
            offset: 0.0,
            valid: (-32000.0, 32000.0),
            sphere_m: 2_439_400.0,
            raw: None,
        }),
        // MESSENGER MDIS 3-colour basemap (1000, 750, 430 nm as R, G, B; enhanced).
        mosaic("Mercury_MESSENGER_MDIS_Basemap_MD3Color_Mosaic_Global_665m.tif", RGB),
        (4096, 4096, 4096),
    ),
    spec(
        "ceres",
        // Dawn FC HAMO DTM (DLR, 60 ppd, ~137 m): metres above a 470 km sphere.
        Heights::Dem(HeightSource {
            url: "Ceres_Dawn_FC_HAMO_DTM_DLR_Global_60ppd_Oct2016.tif",
            file: "Ceres_Dawn_FC_HAMO_DTM_DLR_Global_60ppd_Oct2016.tif",
            scale: 1.0,
            offset: 0.0,
            valid: (-30000.0, 30000.0),
            sphere_m: 470_000.0,
            raw: None,
        }),
        // Dawn FC clear-filter mosaic (DLR, 20 ppd, 400 m).
        mosaic("Ceres_Dawn_FC_DLR_global_20ppd_Oct2015.tif", GREY),
        (2048, 2048, 2048),
    ),
    spec(
        "pluto",
        // New Horizons LORRI/MVIC stereo DEM (USGS/LPI, 300 m): metres above a 1,188.3 km sphere;
        // the far side's no-data areas are filled smoothly.
        Heights::Dem(HeightSource {
            url: "Pluto_NewHorizons_Global_DEM_300m_Jul2017_16bit.tif",
            file: "Pluto_NewHorizons_Global_DEM_300m_Jul2017_16bit.tif",
            scale: 1.0,
            offset: 0.0,
            valid: (-20000.0, 20000.0),
            sphere_m: 1_188_300.0,
            raw: None,
        }),
        mosaic("Pluto_NewHorizons_Global_Mosaic_300m_Jul2017_8bit.tif", GREY),
        (4096, 2048, 2048),
    ),
    spec(
        "charon",
        Heights::Dem(HeightSource {
            url: "Charon_NewHorizons_Global_DEM_300m_Jul2017_16bit.tif",
            file: "Charon_NewHorizons_Global_DEM_300m_Jul2017_16bit.tif",
            scale: 1.0,
            offset: 0.0,
            valid: (-20000.0, 20000.0),
            sphere_m: 606_000.0,
            raw: None,
        }),
        mosaic("Charon_NewHorizons_Global_Mosaic_300m_Jul2017_8bit.tif", GREY),
        (2048, 2048, 2048),
    ),
    spec(
        "enceladus",
        // Cassini ISS photogrammetric shape model (Bland et al. 2020, 2 ppd):
        // radius minus 251.5 km.
        Heights::Dem(HeightSource {
            url: "Enceladus/enceladus_cassini_iss_shapemodel_bland_2019/enceladus_2019pm_radius.tif",
            file: "enceladus_2019pm_radius.tif",
            scale: 1.0,
            offset: 0.0,
            valid: (-20000.0, 20000.0),
            sphere_m: 251_500.0,
            raw: None,
        }),
        mosaic("Enceladus_Cassini_mosaic_global_110m.tif", GREY),
        (720, 2048, 2048),
    ),
    // --- Mosaics with procedural relief (no global DEM exists). ---
    spec(
        "mimas",
        // Herschel (139 km, ~10 km deep with a central peak) is in the Gazetteer.
        relief(
            Some((207_800.0, 196_700.0, 190_600.0)),
            Some(icy(0.15, 25.0, 10000.0, 0.02)),
            Some("MIMAS"),
            300.0,
            0x31a,
        ),
        // Cassini ISS basemap (DLR, Roatsch et al., 2017), a member of the USGS zip.
        mosaic("Mimas/Cassini_DLR_Mimas.zip#Cassini_DLR/MI_170630_DLR_basemap_degrees.tif", GREY),
        (1024, 2048, 1024),
    ),
    spec(
        "ganymede",
        relief(None, Some(icy(0.03, 3.0, 2000.0, 0.05)), Some("GANYMEDE"), 300.0, 0x6a),
        mosaic("Ganymede_Voyager_GalileoSSI_Global_ClrMosaic_1435m.tif", RGB),
        (2048, 2048, 2048),
    ),
    spec(
        "callisto",
        relief(None, Some(icy(0.1, 3.0, 2500.0, 0.04)), Some("CALLISTO"), 300.0, 0xca),
        mosaic("Callisto_Voyager_GalileoSSI_global_mosaic_1km.tif", GREY),
        (2048, 2048, 2048),
    ),
    spec(
        "io",
        // No impact craters survive Io's resurfacing: its named mountains,
        // mensae, plana and paterae, and regional relief.
        Heights::Procedural(Relief {
            axes: Some((1_829_400.0, 1_819_400.0, 1_815_700.0)),
            craters: None,
            gazetteer: Some("IO"),
            relief_m: 500.0,
            relief_wavelength_m: 400_000.0,
            seed: 0x10,
            landforms: IO_LANDFORMS,
        }),
        mosaic("Io_GalileoSSI-Voyager_Global_Mosaic_ClrMerge_1km.tif", RGB),
        (1024, 2048, 2048),
    ),
    spec(
        "europa",
        relief(
            Some((1_562_600.0, 1_560_300.0, 1_559_500.0)),
            Some(Craters {
                n1: 2.0e-4,
                slope: 2.0,
                d_max_km: 50.0,
                transition_km: 5.0,
                max_depth_m: 800.0,
                fresh: 0.3,
            }),
            Some("EUROPA"),
            150.0,
            0xe0,
        ),
        mosaic("Europa_Voyager_GalileoSSI_global_mosaic_500m.tif", GREY),
        (1024, 2048, 2048),
    ),
    spec(
        "dione",
        relief(
            Some((563_400.0, 561_300.0, 559_600.0)),
            Some(icy(0.08, 15.0, 6000.0, 0.03)),
            Some("DIONE"),
            300.0,
            0xd1,
        ),
        mosaic("Dione_Cassini_Voyager_mosaic_global_154m.tif", GREY),
        (1024, 2048, 2048),
    ),
    spec(
        "rhea",
        relief(Some((765_000.0, 763_100.0, 762_400.0)), Some(icy(0.1, 20.0, 8000.0, 0.03)), Some("RHEA"), 400.0, 0x2e),
        mosaic("Rhea_Cassini_Voyager_mosaic_global_417m.tif", GREY),
        (1024, 2048, 2048),
    ),
    spec(
        "tethys",
        relief(
            Some((538_400.0, 528_300.0, 526_300.0)),
            Some(icy(0.1, 15.0, 5000.0, 0.03)),
            Some("TETHYS"),
            300.0,
            0x7e,
        ),
        mosaic("Tethys_Cassini_mosaic_global_293m.tif", GREY),
        (1024, 2048, 2048),
    ),
    spec(
        "iapetus",
        relief(None, Some(icy(0.1, 15.0, 10000.0, 0.02)), Some("IAPETUS"), 500.0, 0x1a),
        mosaic("Iapetus_Cassini_Voyager_mosaic_global_783m.tif", GREY),
        (1024, 2048, 2048),
    ),
    spec(
        "triton",
        relief(
            None,
            Some(Craters {
                n1: 2.7e-4,
                slope: 2.0,
                d_max_km: 40.0,
                transition_km: 10.0,
                max_depth_m: 1000.0,
                fresh: 0.3,
            }),
            Some("TRITON"),
            300.0,
            0x7a,
        ),
        mosaic("Triton_Voyager2_ClrMosaic_GlobalFill_600m.tif", RGB),
        (1024, 2048, 2048),
    ),
    // --- Procedural relief and colour: no global maps (Voyager saw only the
    // Uranian moons' southern hemispheres; the TNOs are unresolved). The
    // colour is the body's measured colour normalised to a mid-grey map
    // (only the zones' sourced albedos are drawn); fresh craters are lighter.
    Spec {
        height_scale: 2.0,
        ..spec(
            "hyperion",
            // Saturated with deep, bowl-shaped craters (Thomas et al. 2007, Nature 448, 50).
            relief(
                Some((180_100.0, 133_000.0, 102_700.0)),
                Some(icy(0.3, 100.0, 10000.0, 0.0)),
                Some("HYPERION"),
                500.0,
                0x47,
            ),
            Colors::Procedural([188, 184, 179]),
            (1024, 1024, 1024),
        )
    },
    spec(
        "phoebe",
        relief(
            Some((109_400.0, 108_500.0, 101_800.0)),
            Some(icy(0.2, 100.0, 10000.0, 0.03)),
            Some("PHOEBE"),
            500.0,
            0x9e,
        ),
        Colors::Procedural([185, 186, 188]),
        (1024, 1024, 1024),
    ),
    spec(
        "miranda",
        relief(
            Some((240_400.0, 234_200.0, 232_900.0)),
            Some(icy(0.03, 20.0, 4000.0, 0.03)),
            Some("MIRANDA"),
            1500.0,
            0x31,
        ),
        Colors::Procedural([188, 187, 186]),
        (1024, 1024, 1024),
    ),
    spec(
        "ariel",
        relief(
            Some((581_100.0, 577_900.0, 577_700.0)),
            Some(icy(0.01, 15.0, 4000.0, 0.05)),
            Some("ARIEL"),
            800.0,
            0x32,
        ),
        Colors::Procedural([188, 187, 187]),
        (1024, 1024, 1024),
    ),
    spec(
        "umbriel",
        relief(None, Some(icy(0.1, 15.0, 5000.0, 0.01)), Some("UMBRIEL"), 300.0, 0x33),
        Colors::Procedural([188, 187, 185]),
        (1024, 1024, 1024),
    ),
    spec(
        "titania",
        relief(None, Some(icy(0.03, 15.0, 5000.0, 0.03)), Some("TITANIA"), 800.0, 0x34),
        Colors::Procedural([188, 186, 182]),
        (1024, 1024, 1024),
    ),
    spec(
        "oberon",
        relief(None, Some(icy(0.1, 15.0, 6000.0, 0.03)), Some("OBERON"), 500.0, 0x35),
        Colors::Procedural([188, 185, 181]),
        (1024, 1024, 1024),
    ),
    spec(
        "proteus",
        relief(
            Some((218_000.0, 208_000.0, 201_000.0)),
            Some(icy(0.2, 100.0, 10000.0, 0.02)),
            Some("PROTEUS"),
            500.0,
            0x81,
        ),
        Colors::Procedural([188, 187, 187]),
        (1024, 1024, 1024),
    ),
    spec(
        "nereid",
        relief(None, Some(icy(0.1, 100.0, 8000.0, 0.02)), None, 300.0, 0x82),
        Colors::Procedural([188, 187, 187]),
        (512, 512, 512),
    ),
    // Trans-Neptunian objects: unresolved, so moderate cratering (Charon's
    // plains-like density) and 0.5 km of regional relief are assumptions.
    spec(
        "eris",
        relief(None, Some(icy(0.003, 15.0, 4000.0, 0.0)), None, 500.0, 0x136199),
        Colors::Procedural([188, 180, 171]),
        (1024, 1024, 1024),
    ),
    Spec {
        height_scale: 4.0,
        ..spec(
            "haumea",
            relief(
                Some((1_050_000.0, 840_000.0, 537_000.0)),
                Some(icy(0.01, 15.0, 4000.0, 0.03)),
                None,
                500.0,
                0x136108,
            ),
            Colors::Procedural([185, 187, 188]),
            (1024, 1024, 1024),
        )
    },
    spec(
        "makemake",
        relief(None, Some(icy(0.003, 15.0, 4000.0, 0.0)), None, 500.0, 0x136472),
        Colors::Procedural([188, 177, 164]),
        (1024, 1024, 1024),
    ),
    spec(
        "gonggong",
        relief(None, Some(icy(0.01, 15.0, 4000.0, 0.03)), None, 500.0, 0x225088),
        Colors::Procedural([188, 152, 112]),
        (1024, 1024, 1024),
    ),
    Spec {
        height_scale: 2.0,
        ..spec(
            "quaoar",
            relief(Some((620_700.0, 546_500.0, 511_500.0)), Some(icy(0.01, 15.0, 4000.0, 0.03)), None, 500.0, 0x50000),
            Colors::Procedural([188, 167, 148]),
            (1024, 1024, 1024),
        )
    },
    spec(
        "sedna",
        relief(None, Some(icy(0.01, 15.0, 4000.0, 0.03)), None, 500.0, 0x90377),
        Colors::Procedural([188, 157, 123]),
        (1024, 1024, 1024),
    ),
    spec(
        "orcus",
        relief(None, Some(icy(0.01, 15.0, 4000.0, 0.03)), None, 500.0, 0x90482),
        Colors::Procedural([188, 186, 184]),
        (1024, 1024, 1024),
    ),
    spec(
        "dysnomia",
        relief(None, Some(icy(0.03, 30.0, 5000.0, 0.03)), None, 400.0, 0xd5),
        Colors::Procedural([188, 187, 187]),
        (512, 512, 512),
    ),
    spec(
        "hiiaka",
        relief(None, Some(icy(0.03, 60.0, 5000.0, 0.0)), None, 400.0, 0x41),
        Colors::Procedural([188, 187, 187]),
        (512, 512, 512),
    ),
    spec(
        "vanth",
        relief(None, Some(icy(0.03, 40.0, 5000.0, 0.03)), None, 400.0, 0x7a7),
        Colors::Procedural([188, 187, 187]),
        (512, 512, 512),
    ),
    // Under thick air (agent SKY, docs/features/solar-visuals.md#venus-and-titan-surfaces).
    spec(
        "venus",
        // Magellan altimetry and stereo, global topography v2 (USGS, 4641 m): metres above a 6,051 km sphere.
        Heights::Dem(HeightSource {
            url: "Venus_Magellan_Topography_Global_4641m_v02.tif",
            file: "Venus_Magellan_Topography_Global_4641m_v02.tif",
            scale: 1.0,
            offset: 0.0,
            valid: (-30000.0, 30000.0),
            sphere_m: 6_051_000.0,
            raw: None,
        }),
        // Magellan SAR, C3-MDIR global mosaic (USGS, 2025 m): radar backscatter, which only
        // classifies (rough tesserae and flows bright, smooth plains dark); the drawn colour is the zones'.
        mosaic("Venus_Magellan_C3-MDIR_Global_Mosaic_2025m.tif", GREY),
        // 9.3 km per height sample.
        (4096, 2048, 2048),
    ),
    spec(
        "titan",
        // No public-domain global DEM (Corlies et al. 2017's map is a journal supplement): relief from
        // its statistics; Gazetteer craters, shallow (Neish et al. 2013, Icarus 223, 82), few random ones.
        relief(None, Some(Craters { fresh: 0.0, ..icy(1.0e-4, 3.0, 800.0, 0.0) }), Some("TITAN"), 300.0, 0x717a),
        // Cassini ISS 938 nm global mosaic (USGS, 4 km): the surface through the haze's window.
        mosaic("Titan_ISS_P19658_Mosaic_Global_4km.tif", GREY),
        (2048, 2048, 2048),
    ),
];
