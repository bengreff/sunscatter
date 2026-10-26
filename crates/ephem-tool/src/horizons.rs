//! JPL Horizons state vectors (https://ssd-api.jpl.nasa.gov/doc/horizons.html),
//! fetched with `curl` into `data/external/horizons/` (gitignored) and read
//! from there: ICRF axes, km and km/s, geometric states, TDB.

use glam::DVec3;
use std::path::{Path, PathBuf};

/// Where downloaded vectors are cached.
pub const CACHE_DIR: &str = "data/external/horizons";

/// Julian date (TDB) of the epoch 2030-01-01T00:00:00 TDB.
pub const EPOCH_JD: f64 = 2_462_502.5;

/// Years after the epoch of the reference states (golden tests).
pub const REFERENCE_YEARS: [f64; 6] = [0.0, 1.0, 5.0, 10.0, 25.0, 50.0];

/// One request: `command`'s states relative to `center` at Julian dates `jds`.
#[derive(Clone, Debug, PartialEq)]
pub struct Query {
    pub command: String,
    pub center: String,
    pub jds: Vec<f64>,
}

impl Query {
    /// The reference dates from the epoch (Julian years of 365.25 days).
    pub fn at_reference_dates(command: &str, center: &str) -> Self {
        let jds = REFERENCE_YEARS.iter().map(|y| EPOCH_JD + y * 365.25).collect();
        Query { command: command.into(), center: center.into(), jds }
    }

    pub fn at(command: &str, center: &str, jd: f64) -> Self {
        Query { command: command.into(), center: center.into(), jds: vec![jd] }
    }

    /// Cache file of this request (the command, centre and dates in the name).
    pub fn file(&self) -> PathBuf {
        let clean = |s: &str| s.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect::<String>();
        let first = self.jds.first().copied().unwrap_or(0.0);
        let name = format!("{}@{}-{}x{}.txt", clean(&self.command), clean(&self.center), first, self.jds.len());
        Path::new(CACHE_DIR).join(name)
    }

    fn url(&self) -> String {
        let enc = |s: &str| s.replace(';', "%3B").replace(' ', "%20");
        let tlist: Vec<String> = self.jds.iter().map(|jd| format!("{jd:.6}")).collect();
        format!(
            "https://ssd.jpl.nasa.gov/api/horizons.api?format=text&COMMAND='{}'&OBJ_DATA='YES'&MAKE_EPHEM='YES'\
             &EPHEM_TYPE='VECTORS'&CENTER='{}'&REF_PLANE='FRAME'&REF_SYSTEM='ICRF'&OUT_UNITS='KM-S'&VEC_TABLE='2'\
             &CSV_FORMAT='YES'&TLIST='{}'&TIME_TYPE='TDB'&VEC_LABELS='NO'",
            enc(&self.command),
            enc(&self.center),
            enc(&tlist.join(" "))
        )
    }

    /// Downloads the vectors unless they are cached.
    pub fn fetch(&self) {
        let file = self.file();
        if file.exists() {
            return;
        }
        std::fs::create_dir_all(CACHE_DIR).expect("create the Horizons cache");
        let out =
            std::process::Command::new("curl").args(["-s", "-f", "-m", "120", &self.url()]).output().expect("run curl");
        assert!(out.status.success(), "curl failed for {} @ {}", self.command, self.center);
        let text = String::from_utf8(out.stdout).expect("utf-8 reply");
        assert!(text.contains("$$SOE"), "no vectors for {} @ {}:\n{text}", self.command, self.center);
        std::fs::write(&file, text).expect("write the Horizons cache");
        println!("fetched {}", file.display());
    }

    /// The cached states (m, m/s), one per date, in order.
    pub fn states(&self) -> Vec<(DVec3, DVec3)> {
        let file = self.file();
        let text = std::fs::read_to_string(&file)
            .unwrap_or_else(|e| panic!("{}: {e} (run `ephem-tool fetch` first)", file.display()));
        let states = parse(&text);
        assert_eq!(states.len(), self.jds.len(), "{}: expected {} states", file.display(), self.jds.len());
        states
    }
}

/// The state rows between `$$SOE` and `$$EOE` (CSV: JD, date, X, Y, Z, VX, VY, VZ).
pub fn parse(text: &str) -> Vec<(DVec3, DVec3)> {
    text.lines()
        .skip_while(|l| !l.starts_with("$$SOE"))
        .skip(1)
        .take_while(|l| !l.starts_with("$$EOE"))
        .map(|line| {
            let f: Vec<f64> = line.split(',').skip(2).take(6).map(|x| x.trim().parse().expect("a number")).collect();
            (DVec3::new(f[0], f[1], f[2]) * 1e3, DVec3::new(f[3], f[4], f[5]) * 1e3)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_parse_to_si_units() {
        let text = "header\n$$SOE\n2462502.500000000, A.D. 2030-Jan-01 00:00:00.0000, -9.036422519486943E+03, \
                    -3.794726188193626E+05, -1.808662125507735E+05,  1.738168275657445E+01, -4.934579414403819E-01, \
                     5.720299031391152E-02,\n$$EOE\nfooter";
        let s = parse(text);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].0.x, -9.036422519486943e6);
        assert_eq!(s[0].1.y, -4.934579414403819e2);
        let q = Query::at_reference_dates("1;", "500@0");
        assert_eq!(q.jds.len(), REFERENCE_YEARS.len());
        assert!(q.url().contains("COMMAND='1%3B'"));
        assert!(q.file().to_str().unwrap().ends_with("1_@500_0-2462502.5x6.txt"));
    }
}
