#!/usr/bin/env python3
"""Prints Earth's atmosphere table for data/bodies/earth/body.ron.

U.S. Standard Atmosphere, 1976 (NOAA-S/T 76-1562, NASA-TM-X-74335):
- 0-86 km (geometric): computed from the standard's defining equations
  (geopotential layers with their lapse rates, hydrostatic pressure, the
  molecular-weight ratio M/M0 of Table 8 between 80 and 86 km);
- 86-1000 km: the standard's tabulated values (Table I, geometric altitude:
  kinetic temperature, pressure, density) at the altitudes listed in UPPER,
  and between them, where log-linear interpolation of the table would be
  off by more than ~0.5 %, the standard's temperature profile (its
  equations for 86-1000 km) with pressure and density from Braeunig's
  polynomial fits of Table I (http://www.braeunig.us/space/atmmodel.htm;
  within 0.1 % of the table at every tabulated point, as run through the
  `pyatmos` package's `coesa76`). The mean molar mass is M = rho*R*T/p.
Rows: (altitude m, temperature K, pressure Pa, density kg/m^3, molar mass kg/mol).
"""
import math

R_STAR = 8314.32  # J/(kmol K), the standard's gas constant
G0 = 9.80665
M0 = 28.9644
R0 = 6356766.0  # m, for geopotential altitude
LAYERS = [  # (base geopotential km', lapse K/km')
    (0.0, -6.5), (11.0, 0.0), (20.0, 1.0), (32.0, 2.8), (47.0, 0.0), (51.0, -2.8), (71.0, -2.0), (84.8520, None)]
# Table 8: M/M0 from 80 to 86 km geometric, every 0.5 km.
MRATIO = [1.0, 0.999996, 0.999989, 0.999971, 0.999941, 0.999909, 0.999870,
          0.999829, 0.999786, 0.999741, 0.999694, 0.999641, 0.999579]
UPPER = [  # Table I: (km, T K, p Pa, rho kg/m^3)
    (86, 186.87, 3.7338e-1, 6.958e-6), (90, 186.87, 1.8359e-1, 3.416e-6), (95, 188.42, 7.5966e-2, 1.393e-6),
    (100, 195.08, 3.2011e-2, 5.604e-7), (105, 208.84, 1.4481e-2, 2.325e-7), (110, 240.00, 7.1042e-3, 9.708e-8),
    (115, 300.00, 4.0096e-3, 4.289e-8), (120, 360.00, 2.5382e-3, 2.222e-8), (130, 469.27, 1.2505e-3, 8.152e-9),
    (140, 559.63, 7.2028e-4, 3.831e-9), (150, 634.39, 4.5422e-4, 2.076e-9), (160, 696.29, 3.0395e-4, 1.233e-9),
    (170, 747.57, 2.1210e-4, 7.815e-10), (180, 790.07, 1.5271e-4, 5.194e-10), (190, 825.31, 1.1266e-4, 3.581e-10),
    (200, 854.56, 8.4736e-5, 2.541e-10), (250, 941.33, 2.4767e-5, 6.073e-11), (300, 976.01, 8.7704e-6, 1.916e-11),
    (350, 990.06, 3.4498e-6, 7.014e-12), (400, 995.83, 1.4518e-6, 2.803e-12), (450, 998.22, 6.4468e-7, 1.184e-12),
    (500, 999.24, 3.0236e-7, 5.215e-13), (600, 999.85, 8.2130e-8, 1.137e-13), (700, 999.97, 3.1908e-8, 3.070e-14),
    (800, 999.99, 1.7036e-8, 1.136e-14), (900, 1000.0, 1.0873e-8, 5.759e-15), (1000, 1000.0, 7.5138e-9, 3.561e-15)]


def lower(z):
    """US76 at geometric altitude z (m) <= 86 km: (T, p, rho, M kg/mol)."""
    h = R0 * z / (R0 + z) / 1000.0
    tb, pb = 288.15, 101325.0
    for (hb, lb), (hn, _) in zip(LAYERS, LAYERS[1:]):
        top = min(h, hn)
        dh = top - hb
        if lb == 0.0:
            p = pb * math.exp(-G0 * M0 * dh * 1000.0 / (R_STAR * tb))
        else:
            p = pb * (tb / (tb + lb * dh)) ** (G0 * M0 / (R_STAR * lb / 1000.0))
        tm = tb + lb * dh
        if h <= hn:
            break
        tb, pb = tm, p
    ratio = 1.0
    if z > 80e3:
        x = (z - 80e3) / 500.0
        i = min(int(x), 11)
        ratio = MRATIO[i] + (MRATIO[i + 1] - MRATIO[i]) * (x - i)
    rho = p * M0 / (R_STAR * tm)
    return tm * ratio, p, rho, M0 * ratio / 1000.0


def rows():
    # Every 1 km, plus the geometric altitudes of the layer bases.
    zs = set(range(0, 86001, 1000))
    for hb, _ in LAYERS[1:-1]:
        zs.add(round(R0 * hb * 1000.0 / (R0 - hb * 1000.0), 1))
    out = []
    for z in sorted(zs):
        if z >= 86000:
            continue
        out.append((float(z),) + lower(z))
    upper = {km: (t, p, rho) for km, t, p, rho in UPPER}
    for km in FILL:
        upper.setdefault(km, fitted(km))
    for km in sorted(upper):
        t, p, rho = upper[km]
        out.append((km * 1000.0, t, p, rho, rho * R_STAR * t / p / 1000.0))
    return out


# Intermediate altitudes (km) above 86 km, from the fits.
FILL = [92.5, 97.5, 102.5] + list(range(106, 125)) + [122.5, 125, 127.5, 132.5, 135, 137.5, 142.5, 145, 147.5, 155, 165, 175, 185, 195, 210, 220, 230, 240,
        260, 270, 280, 290, 320, 340, 375, 425, 475, 525, 550, 575, 625, 650, 675, 725, 750, 775, 825, 850, 875,
        950]


def fitted(km):
    """Braeunig's fits of Table I (via pyatmos.coesa76): (T, p, rho), rounded like the table."""
    import warnings
    warnings.filterwarnings("ignore")
    from pyatmos import coesa76
    a = coesa76([km])
    sig = lambda x: float(f"{x:.4e}")
    return round(float(a.T[0]), 2), sig(a.P[0]), sig(a.rho[0])


if __name__ == "__main__":
    for z, t, p, rho, m in rows():
        print(f"                ({z:.1f}, {t:.3f}, {p:.5e}, {rho:.5e}, {m:.7f}),")
