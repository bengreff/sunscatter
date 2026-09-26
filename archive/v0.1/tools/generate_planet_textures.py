#!/usr/bin/env python3
"""Generate 1024x1024 polar-projection textures for catalog bodies.

Reads body data from src/galaxy/catalog/catalog_data.rs and generates
procedural textures for each non-star body that doesn't already have
a hand-made texture.

Dependencies: numpy, Pillow (both standard in most Python installs).

Usage:
    python3 tools/generate_planet_textures.py                       # All bodies
    python3 tools/generate_planet_textures.py --body caldera        # Single body
    python3 tools/generate_planet_textures.py --system "Alpha Centauri"
    python3 tools/generate_planet_textures.py --type gas_giant      # By type
    python3 tools/generate_planet_textures.py --skip-existing       # Don't overwrite
    python3 tools/generate_planet_textures.py --list                # List all bodies
"""

import argparse
import hashlib
import os
import re
import sys
import time
from dataclasses import dataclass, field
from enum import Enum, auto
from pathlib import Path
from typing import Optional

import numpy as np
from PIL import Image

# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------

SIZE = 1024
HALF = SIZE // 2
OUTPUT_DIR = Path("data/textures/bodies")
CATALOG_PATH = Path("src/galaxy/catalog/catalog_data.rs")

# Bodies that already have hand-made NASA-based textures
SKIP_BODIES = {
    "sun", "mercury", "venus", "earth", "moon", "mars", "phobos", "deimos",
    "jupiter", "io", "europa", "ganymede", "callisto", "saturn", "titan",
    "rhea", "iapetus", "dione", "uranus", "neptune", "goldenvale",
}


# ---------------------------------------------------------------------------
# Body type classification
# ---------------------------------------------------------------------------

class BodyType(Enum):
    AIRLESS_ROCKY = auto()
    HOT_ROCKY = auto()
    VENUS_ANALOG = auto()
    MARS_ANALOG = auto()
    TEMPERATE_ROCKY = auto()
    OCEAN_WORLD = auto()
    LAVA_WORLD = auto()
    GAS_GIANT = auto()
    ICE_GIANT = auto()
    ICE_DWARF = auto()
    ICY_MOON = auto()
    VOLCANIC_MOON = auto()


class OceanType(Enum):
    NONE = auto()
    WATER_BLUE = auto()
    BRINE_DARK = auto()
    IRON_TINTED = auto()
    MINERAL_GREEN = auto()


class CraterDensity(Enum):
    NONE = auto()
    LIGHT = auto()
    MODERATE = auto()
    HEAVY = auto()


# ---------------------------------------------------------------------------
# Parsed body data
# ---------------------------------------------------------------------------

@dataclass
class AtmosphereData:
    pressure_atm: float
    composition: str
    scale_height_km: float


@dataclass
class CatalogBodyData:
    name: str
    system_name: str
    designation: str
    is_moon: bool
    mass_earth: float
    radius_earth: float
    temperature_k: float
    atmosphere: Optional[AtmosphereData]
    is_gas_giant: bool
    has_life: bool
    description: str
    resources: list


@dataclass
class PlanetParams:
    name: str
    body_type: BodyType
    seed: int
    # Surface colors (R, G, B) tuples
    base_color_1: tuple = (128, 128, 128)
    base_color_2: tuple = (100, 100, 100)
    base_color_3: tuple = (80, 80, 80)
    # Features
    crater_density: CraterDensity = CraterDensity.NONE
    ocean_type: OceanType = OceanType.NONE
    ocean_coverage: float = 0.0
    has_polar_ice: bool = False
    has_cloud_layer: bool = False
    cloud_coverage: float = 0.0
    has_fractures: bool = False
    band_count: int = 0
    lava_coverage: float = 0.0
    # Life-bearing
    has_life: bool = False
    life_color_1: tuple = (80, 140, 40)
    life_color_2: tuple = (60, 120, 30)
    # Temperature
    temperature_k: float = 200.0
    # Extra
    description: str = ""


# ---------------------------------------------------------------------------
# Catalog parser
# ---------------------------------------------------------------------------

def _find_balanced_brace(text, start):
    """Find position of matching closing brace, handling nested braces."""
    depth = 0
    for i in range(start, len(text)):
        if text[i] == '{':
            depth += 1
        elif text[i] == '}':
            depth -= 1
            if depth == 0:
                return i
    return -1


def parse_catalog(catalog_path: Path) -> list:
    """Parse catalog_data.rs and extract all CatalogBody entries."""
    text = catalog_path.read_text()

    bodies = []

    # Find each CatalogSystem block and extract its name + bodies
    system_starts = [m.start() for m in re.finditer(r'CatalogSystem\s*\{', text)]
    system_starts.append(len(text))  # sentinel

    for i in range(len(system_starts) - 1):
        chunk = text[system_starts[i]:system_starts[i + 1]]

        # Extract system name
        sys_match = re.search(r'name:\s*"([^"]+)"', chunk)
        current_system = sys_match.group(1) if sys_match else "Unknown"

        # Find CatalogBody blocks with balanced brace matching
        search_start = 0
        while True:
            idx = chunk.find('CatalogBody {', search_start)
            if idx == -1:
                break
            brace_start = chunk.index('{', idx)
            brace_end = _find_balanced_brace(chunk, brace_start)
            if brace_end == -1:
                break
            body_text = chunk[brace_start + 1:brace_end]
            search_start = brace_end + 1

            def extract(key, default=""):
                # Try quoted string first (handles commas in values)
                m = re.search(rf'{key}:\s*"([^"]*)"', body_text)
                if m:
                    return m.group(1)
                # Fall back to unquoted value
                m = re.search(rf'{key}:\s*([^,\n]+)', body_text)
                if m:
                    return m.group(1).strip()
                return default

            name = extract("name")
            if not name:
                continue

            # Parse atmosphere
            atmo = None
            atmo_match = re.search(
                r'Some\(CatalogAtmosphere\s*\{(.*?)\}\)',
                body_text, re.DOTALL
            )
            if atmo_match:
                atmo_text = atmo_match.group(1)
                pressure = float(re.search(r'pressure_atm:\s*([\d.]+)', atmo_text).group(1))
                comp = re.search(r'composition:\s*"([^"]*)"', atmo_text).group(1)
                sh = float(re.search(r'scale_height_km:\s*([\d.]+)', atmo_text).group(1))
                atmo = AtmosphereData(pressure, comp, sh)

            # Parse resources list
            res_match = re.search(r'resources:\s*&\[(.*?)\]', body_text, re.DOTALL)
            resources = []
            if res_match:
                resources = re.findall(r'ResourceType::(\w+)', res_match.group(1))

            bodies.append(CatalogBodyData(
                name=name,
                system_name=current_system or "Unknown",
                designation=extract("designation"),
                is_moon=extract("is_moon") == "true",
                mass_earth=float(extract("mass_earth", "0")),
                radius_earth=float(extract("radius_earth", "0")),
                temperature_k=float(extract("temperature_k", "200")),
                atmosphere=atmo,
                is_gas_giant=extract("is_gas_giant") == "true",
                has_life=extract("has_life") == "true",
                description=extract("description"),
                resources=resources,
            ))

    return bodies


# ---------------------------------------------------------------------------
# Body type classification
# ---------------------------------------------------------------------------

def classify_body(body: CatalogBodyData) -> BodyType:
    """Classify a body into a visual type based on physical properties."""
    desc = body.description.lower()
    temp = body.temperature_k
    pressure = body.atmosphere.pressure_atm if body.atmosphere else 0.0

    # Gas giants first
    if body.is_gas_giant:
        if any(kw in desc for kw in ["ice giant", "neptune"]):
            return BodyType.ICE_GIANT
        if body.mass_earth < 20:
            return BodyType.ICE_GIANT  # Mini-Neptunes are ice giants
        return BodyType.GAS_GIANT

    # Lava worlds
    if temp > 900 or any(kw in desc for kw in ["lava", "magma"]):
        return BodyType.LAVA_WORLD

    # Icy moons (check before volcanic since "cryovolcanic" contains "volcanic")
    if body.is_moon and any(kw in desc for kw in ["icy", "europa", "fracture", "cryovolcanic", "subsurface ocean"]):
        return BodyType.ICY_MOON

    # Volcanic moons (Io-analog)
    if body.is_moon and any(kw in desc for kw in ["volcanic", "io-analog"]):
        return BodyType.VOLCANIC_MOON

    # Venus analogs: thick atmosphere + hot
    if pressure > 10.0 and temp > 400:
        return BodyType.VENUS_ANALOG

    # Ocean worlds
    if "ocean world" in desc or ("ocean" in desc and body.has_life):
        return BodyType.OCEAN_WORLD

    # Life-bearing temperate worlds
    if body.has_life:
        return BodyType.TEMPERATE_ROCKY

    # Temperate rocky: moderate atmosphere, habitable range
    if pressure > 0.1 and 200 < temp < 350:
        return BodyType.TEMPERATE_ROCKY

    # Mars analogs: thin atmosphere, cold
    if 0.002 < pressure < 0.5 and temp < 300:
        return BodyType.MARS_ANALOG

    # Ice dwarfs: very cold, small
    if temp < 100 and body.mass_earth < 0.2:
        return BodyType.ICE_DWARF

    # Hot rocky: no/trace atmosphere, hot
    if temp > 350:
        return BodyType.HOT_ROCKY

    # Default: airless rocky
    return BodyType.AIRLESS_ROCKY


# ---------------------------------------------------------------------------
# Parameter generation
# ---------------------------------------------------------------------------

def body_seed(name: str) -> int:
    """Deterministic seed from body name."""
    return int(hashlib.md5(name.encode()).hexdigest()[:8], 16)


def make_params(body: CatalogBodyData, btype: BodyType) -> PlanetParams:
    """Generate rendering parameters for a body."""
    seed = body_seed(body.name)
    rng = np.random.RandomState(seed)
    desc = body.description.lower()

    p = PlanetParams(
        name=body.name,
        body_type=btype,
        seed=seed,
        temperature_k=body.temperature_k,
        description=body.description,
    )

    if btype == BodyType.AIRLESS_ROCKY:
        # Gray/tan moon-like surface
        gray = rng.randint(110, 160)
        p.base_color_1 = (gray, gray, gray - 5)
        p.base_color_2 = (gray - 20, gray - 18, gray - 25)
        p.base_color_3 = (gray + 15, gray + 12, gray + 5)
        p.crater_density = CraterDensity.HEAVY

    elif btype == BodyType.HOT_ROCKY:
        # Dark brownish
        base = rng.randint(80, 120)
        p.base_color_1 = (base + 15, base, base - 10)
        p.base_color_2 = (base - 10, base - 15, base - 20)
        p.base_color_3 = (base + 5, base - 5, base - 15)
        p.crater_density = CraterDensity.MODERATE
        if "irradiated" in desc:
            p.base_color_1 = (base + 20, base + 5, base - 15)

    elif btype == BodyType.VENUS_ANALOG:
        p.base_color_1 = (210, 185, 120)
        p.base_color_2 = (195, 170, 105)
        p.base_color_3 = (225, 200, 140)
        p.has_cloud_layer = True
        p.cloud_coverage = 1.0

    elif btype == BodyType.MARS_ANALOG:
        # Rust-red/orange
        p.base_color_1 = (175, 110, 65)
        p.base_color_2 = (155, 90, 50)
        p.base_color_3 = (190, 130, 80)
        p.crater_density = CraterDensity.MODERATE
        p.has_polar_ice = body.temperature_k < 250
        # Vary tint per body
        shift = rng.randint(-15, 15)
        p.base_color_1 = (175 + shift, 110 + shift // 2, 65)
        p.base_color_2 = (155 + shift, 90 + shift // 2, 50)

    elif btype == BodyType.TEMPERATE_ROCKY:
        if body.has_life:
            if "iron-oxidizing" in desc or "ferr" in body.name.lower():
                # Ferrus: rust-red biofilms
                p.base_color_1 = (160, 120, 80)
                p.base_color_2 = (140, 100, 65)
                p.base_color_3 = (175, 140, 95)
                p.has_life = True
                p.life_color_1 = (145, 75, 45)
                p.life_color_2 = (125, 60, 35)
            elif "sulfur-weaver" in desc or "aurelian" in body.name.lower():
                # Aurelian: amber sulfur forests
                p.base_color_1 = (155, 135, 75)
                p.base_color_2 = (140, 120, 60)
                p.base_color_3 = (170, 150, 90)
                p.has_life = True
                p.life_color_1 = (185, 155, 45)
                p.life_color_2 = (165, 135, 35)
            elif "electrophilic" in desc or "pelagius" in body.name.lower():
                # Pelagius: copper-green ocean life
                p.base_color_1 = (130, 140, 110)
                p.base_color_2 = (115, 125, 95)
                p.base_color_3 = (145, 155, 120)
                p.has_life = True
                p.life_color_1 = (55, 145, 120)
                p.life_color_2 = (40, 125, 100)
                p.ocean_type = OceanType.MINERAL_GREEN
                p.ocean_coverage = 0.7
            else:
                p.base_color_1 = (145, 135, 90)
                p.base_color_2 = (130, 120, 75)
                p.base_color_3 = (160, 150, 100)
                p.has_life = True
                p.life_color_1 = (80, 140, 50)
                p.life_color_2 = (60, 120, 35)
        else:
            # Non-life temperate: brownish-tan with some variety
            base_r = 145 + rng.randint(-20, 20)
            base_g = 130 + rng.randint(-20, 20)
            base_b = 100 + rng.randint(-15, 15)
            p.base_color_1 = (base_r, base_g, base_b)
            p.base_color_2 = (base_r - 15, base_g - 15, base_b - 10)
            p.base_color_3 = (base_r + 10, base_g + 10, base_b + 5)

        # All temperate rocky can have oceans
        if p.ocean_type == OceanType.NONE:
            if "Water" in body.resources or "ocean" in desc or "sea" in desc or "lake" in desc:
                if "brine" in desc:
                    p.ocean_type = OceanType.BRINE_DARK
                elif "mineral" in desc:
                    p.ocean_type = OceanType.MINERAL_GREEN
                else:
                    p.ocean_type = OceanType.WATER_BLUE
                p.ocean_coverage = 0.3 + rng.random() * 0.3

        p.has_polar_ice = body.temperature_k < 290
        p.has_cloud_layer = True
        p.cloud_coverage = 0.15 + rng.random() * 0.2

    elif btype == BodyType.OCEAN_WORLD:
        p.base_color_1 = (130, 135, 100)
        p.base_color_2 = (115, 120, 85)
        p.base_color_3 = (145, 150, 110)
        if "electrophilic" in desc:
            p.ocean_type = OceanType.MINERAL_GREEN
        else:
            p.ocean_type = OceanType.WATER_BLUE
        p.ocean_coverage = 0.85
        p.has_polar_ice = body.temperature_k < 280
        p.has_cloud_layer = True
        p.cloud_coverage = 0.25

    elif btype == BodyType.LAVA_WORLD:
        p.base_color_1 = (55, 45, 40)
        p.base_color_2 = (40, 32, 28)
        p.base_color_3 = (70, 55, 48)
        p.lava_coverage = 0.25 + rng.random() * 0.15
        if body.temperature_k > 1200:
            p.lava_coverage += 0.1

    elif btype == BodyType.GAS_GIANT:
        temp = body.temperature_k
        mass = body.mass_earth
        if temp > 400:
            # Hot Jupiter: muted orange/brown bands
            p.base_color_1 = (180, 145, 100)
            p.base_color_2 = (155, 120, 85)
            p.base_color_3 = (200, 165, 120)
        elif mass > 500:
            # Super-Jupiter: deep ochre/brown
            p.base_color_1 = (175, 155, 120)
            p.base_color_2 = (150, 130, 95)
            p.base_color_3 = (195, 170, 135)
        else:
            # Standard gas giant: Jupiter-like tans/oranges
            p.base_color_1 = (190, 170, 140)
            p.base_color_2 = (165, 142, 110)
            p.base_color_3 = (210, 188, 158)
        p.band_count = 8 + rng.randint(0, 8)
        # Vary colors per body
        shift = rng.randint(-15, 15, 3)
        p.base_color_1 = tuple(np.clip(np.array(p.base_color_1) + shift, 30, 240))
        p.base_color_2 = tuple(np.clip(np.array(p.base_color_2) + shift, 30, 240))

    elif btype == BodyType.ICE_GIANT:
        # Blue-teal base
        p.base_color_1 = (100, 145, 175)
        p.base_color_2 = (80, 125, 155)
        p.base_color_3 = (120, 165, 190)
        p.band_count = 4 + rng.randint(0, 4)
        shift = rng.randint(-10, 10, 3)
        p.base_color_1 = tuple(np.clip(np.array(p.base_color_1) + shift, 30, 240))

    elif btype == BodyType.ICE_DWARF:
        # Pale gray-white
        gray = rng.randint(175, 210)
        p.base_color_1 = (gray, gray + 2, gray + 5)
        p.base_color_2 = (gray - 15, gray - 12, gray - 8)
        p.base_color_3 = (gray + 8, gray + 10, gray + 12)
        p.crater_density = CraterDensity.HEAVY
        if "methane" in desc:
            p.base_color_1 = (gray - 10, gray - 5, gray + 5)

    elif btype == BodyType.ICY_MOON:
        # White/light gray
        p.base_color_1 = (200, 205, 210)
        p.base_color_2 = (180, 185, 192)
        p.base_color_3 = (215, 218, 222)
        p.has_fractures = True
        p.crater_density = CraterDensity.LIGHT
        if "cryovolcanic" in desc:
            p.base_color_1 = (195, 200, 212)

    elif btype == BodyType.VOLCANIC_MOON:
        # Io-like: yellow/orange/sulfur
        p.base_color_1 = (210, 190, 80)
        p.base_color_2 = (185, 140, 50)
        p.base_color_3 = (230, 210, 100)

    return p


# ---------------------------------------------------------------------------
# Vectorized 3D value noise (pure NumPy, no np.vectorize)
# ---------------------------------------------------------------------------

def _hash_array(x, y, z, seed=0):
    """Vectorized deterministic hash for 3D integer coordinate arrays.
    Returns float array in [0, 1]."""
    # Work in uint32 to match scalar version behavior
    x = x.astype(np.int64)
    y = y.astype(np.int64)
    z = z.astype(np.int64)
    M1 = np.int64(374761393)
    M2 = np.int64(668265263)
    M3 = np.int64(1274126177)
    MASK = np.int64(0xFFFFFFFF)
    h = np.int64(seed)
    h = (h * M1 + x * M2) & MASK
    h = (h * M1 + y * M2) & MASK
    h = (h * M1 + z * M2) & MASK
    h = ((h ^ (h >> 13)) * M3) & MASK
    h = h ^ (h >> 16)
    return (h & MASK).astype(np.float64) / 4294967295.0


def _smoothstep(t):
    return t * t * (3.0 - 2.0 * t)


def value_noise_3d(x, y, z, seed=0):
    """Single-octave 3D value noise, fully vectorized."""
    xi = np.floor(x).astype(np.int64)
    yi = np.floor(y).astype(np.int64)
    zi = np.floor(z).astype(np.int64)
    xf = _smoothstep(x - xi)
    yf = _smoothstep(y - yi)
    zf = _smoothstep(z - zi)

    c000 = _hash_array(xi, yi, zi, seed)
    c100 = _hash_array(xi + 1, yi, zi, seed)
    c010 = _hash_array(xi, yi + 1, zi, seed)
    c110 = _hash_array(xi + 1, yi + 1, zi, seed)
    c001 = _hash_array(xi, yi, zi + 1, seed)
    c101 = _hash_array(xi + 1, yi, zi + 1, seed)
    c011 = _hash_array(xi, yi + 1, zi + 1, seed)
    c111 = _hash_array(xi + 1, yi + 1, zi + 1, seed)

    x0 = c000 * (1.0 - xf) + c100 * xf
    x1 = c010 * (1.0 - xf) + c110 * xf
    x2 = c001 * (1.0 - xf) + c101 * xf
    x3 = c011 * (1.0 - xf) + c111 * xf
    y0 = x0 * (1.0 - yf) + x1 * yf
    y1 = x2 * (1.0 - yf) + x3 * yf
    return y0 * (1.0 - zf) + y1 * zf


def fbm_3d(x, y, z, octaves=6, lacunarity=2.0, gain=0.5, seed=0):
    """Fractal Brownian Motion — layered value noise."""
    result = np.zeros_like(x, dtype=np.float64)
    amplitude = 1.0
    frequency = 1.0
    total_amp = 0.0
    for i in range(octaves):
        result += amplitude * value_noise_3d(
            x * frequency, y * frequency, z * frequency, seed=seed + i * 1337
        )
        total_amp += amplitude
        amplitude *= gain
        frequency *= lacunarity
    return result / total_amp


def ridged_noise(x, y, z, octaves=5, lacunarity=2.0, gain=0.5, seed=0):
    """Ridged multi-fractal noise — creates sharp ridges/cracks."""
    result = np.zeros_like(x, dtype=np.float64)
    amplitude = 1.0
    frequency = 1.0
    total_amp = 0.0
    for i in range(octaves):
        n = value_noise_3d(x * frequency, y * frequency, z * frequency, seed=seed + i * 1337)
        n = 1.0 - np.abs(2.0 * n - 1.0)  # Creates sharp ridges
        result += amplitude * n
        total_amp += amplitude
        amplitude *= gain
        frequency *= lacunarity
    return result / total_amp


# ---------------------------------------------------------------------------
# Shared coordinate grids (computed once, reused for all bodies)
# ---------------------------------------------------------------------------

def make_grids():
    """Build polar projection coordinate grids."""
    ys, xs = np.mgrid[0:SIZE, 0:SIZE]
    cx = (xs - HALF + 0.5) / HALF
    cy = (ys - HALF + 0.5) / HALF
    dist = np.sqrt(cx**2 + cy**2)
    in_disc = dist <= 1.0

    # Polar projection -> spherical coords (northern hemisphere only)
    lat = np.where(in_disc, (1.0 - dist) * np.pi / 2.0, 0.0)
    lon = np.arctan2(cy, cx)

    # Spherical -> 3D Cartesian
    cos_lat = np.cos(lat)
    sx = cos_lat * np.cos(lon)
    sy = cos_lat * np.sin(lon)
    sz = np.sin(lat)

    return cx, cy, dist, in_disc, lat, lon, sx, sy, sz


# ---------------------------------------------------------------------------
# Crater overlay
# ---------------------------------------------------------------------------

def generate_craters(sx, sy, sz, in_disc, density, seed):
    """Generate crater-like features using absolute-value noise trick."""
    if density == CraterDensity.NONE:
        return np.zeros((SIZE, SIZE), dtype=np.float64)

    octaves = {CraterDensity.LIGHT: 3, CraterDensity.MODERATE: 4, CraterDensity.HEAVY: 5}[density]
    strength = {CraterDensity.LIGHT: 0.06, CraterDensity.MODERATE: 0.10, CraterDensity.HEAVY: 0.15}[density]

    # Large craters
    n1 = value_noise_3d(sx * 6, sy * 6, sz * 6, seed=seed + 5000)
    craters = np.abs(n1 - 0.5) * 2.0  # Creates circular rim-like features
    craters = 1.0 - craters  # Invert so rims are bright, centers dark

    # Medium craters
    n2 = value_noise_3d(sx * 14, sy * 14, sz * 14, seed=seed + 5100)
    craters += (np.abs(n2 - 0.5) * 2.0) * 0.4
    craters /= 1.4

    # Small craters for heavy density
    if density == CraterDensity.HEAVY:
        n3 = value_noise_3d(sx * 30, sy * 30, sz * 30, seed=seed + 5200)
        craters = craters * 0.7 + (1.0 - np.abs(n3 - 0.5) * 2.0) * 0.3

    # Normalize and scale to darkening/brightening
    craters = (craters - 0.5) * strength
    return np.where(in_disc, craters, 0.0)


# ---------------------------------------------------------------------------
# Generators per body type
# ---------------------------------------------------------------------------

def generate_airless_rocky(p, grids):
    """Gray/tan surface with craters."""
    _, _, dist, in_disc, lat, lon, sx, sy, sz = grids

    terrain = fbm_3d(sx * 3, sy * 3, sz * 3, octaves=5, seed=p.seed)
    detail = fbm_3d(sx * 10, sy * 10, sz * 10, octaves=3, seed=p.seed + 100)

    c1 = np.array(p.base_color_1, dtype=float)
    c2 = np.array(p.base_color_2, dtype=float)
    c3 = np.array(p.base_color_3, dtype=float)

    # Blend between colors based on terrain
    t = np.clip(terrain * 2 - 0.3, 0, 1)
    color = np.zeros((SIZE, SIZE, 3), dtype=float)
    for c in range(3):
        low = c2[c] * (1 - t) + c1[c] * t
        color[:, :, c] = low * (1 - np.clip(t * 1.5 - 0.5, 0, 1)) + c3[c] * np.clip(t * 1.5 - 0.5, 0, 1)

    # Add detail variation
    detail_shift = (detail - 0.5) * 18
    color[:, :, 0] += detail_shift
    color[:, :, 1] += detail_shift * 0.9
    color[:, :, 2] += detail_shift * 0.85

    # Craters
    craters = generate_craters(sx, sy, sz, in_disc, p.crater_density, p.seed)
    for c in range(3):
        color[:, :, c] += craters * 180

    return finalize(color, in_disc)


def generate_hot_rocky(p, grids):
    """Dark brownish surface, optionally with subtle glow."""
    _, _, dist, in_disc, lat, lon, sx, sy, sz = grids

    terrain = fbm_3d(sx * 3.5, sy * 3.5, sz * 3.5, octaves=5, seed=p.seed)
    detail = fbm_3d(sx * 12, sy * 12, sz * 12, octaves=3, seed=p.seed + 100)

    c1 = np.array(p.base_color_1, dtype=float)
    c2 = np.array(p.base_color_2, dtype=float)

    t = terrain
    color = np.zeros((SIZE, SIZE, 3), dtype=float)
    for c in range(3):
        color[:, :, c] = c2[c] * (1 - t) + c1[c] * t

    detail_shift = (detail - 0.5) * 14
    color[:, :, 0] += detail_shift
    color[:, :, 1] += detail_shift * 0.8
    color[:, :, 2] += detail_shift * 0.6

    # Craters
    craters = generate_craters(sx, sy, sz, in_disc, p.crater_density, p.seed)
    for c in range(3):
        color[:, :, c] += craters * 140

    # Subtle orange glow in cracks for very hot bodies
    if p.temperature_k > 700:
        cracks = ridged_noise(sx * 4, sy * 4, sz * 4, octaves=3, seed=p.seed + 300)
        glow_strength = np.clip((cracks - 0.6) * 3, 0, 1) * min((p.temperature_k - 700) / 500, 1.0)
        color[:, :, 0] += glow_strength * 80
        color[:, :, 1] += glow_strength * 30
        color[:, :, 2] += glow_strength * 5

    return finalize(color, in_disc)


def generate_venus_analog(p, grids):
    """Smooth golden cloud deck, no surface visible."""
    _, _, dist, in_disc, lat, lon, sx, sy, sz = grids

    # Smooth low-frequency cloud swirls
    cloud1 = fbm_3d(sx * 1.5, sy * 1.5, sz * 1.5, octaves=4, seed=p.seed, lacunarity=1.8)
    cloud2 = fbm_3d(sx * 2.5, sy * 2.5, sz * 2.5, octaves=3, seed=p.seed + 200, lacunarity=1.7)

    c1 = np.array(p.base_color_1, dtype=float)
    c2 = np.array(p.base_color_2, dtype=float)
    c3 = np.array(p.base_color_3, dtype=float)

    t = cloud1
    color = np.zeros((SIZE, SIZE, 3), dtype=float)
    for c in range(3):
        color[:, :, c] = c2[c] * (1 - t) + c1[c] * t

    # Add secondary cloud layer
    shift = (cloud2 - 0.5) * 0.5
    for c in range(3):
        color[:, :, c] += shift * (c3[c] - c2[c])

    # Subtle latitude banding (Venus has faint bands)
    band = np.sin(lat * 6) * 0.03
    color[:, :, 0] += band * 40
    color[:, :, 1] += band * 30
    color[:, :, 2] += band * 15

    return finalize(color, in_disc)


def generate_mars_analog(p, grids):
    """Rust-red/orange terrain with craters and optional polar ice."""
    _, _, dist, in_disc, lat, lon, sx, sy, sz = grids

    terrain = fbm_3d(sx * 3, sy * 3, sz * 3, octaves=6, seed=p.seed)
    detail = fbm_3d(sx * 10, sy * 10, sz * 10, octaves=4, seed=p.seed + 100)

    c1 = np.array(p.base_color_1, dtype=float)
    c2 = np.array(p.base_color_2, dtype=float)
    c3 = np.array(p.base_color_3, dtype=float)

    # Height-based coloring
    t_low = np.clip(terrain * 2, 0, 1)
    t_high = np.clip(terrain * 2 - 0.6, 0, 1)
    color = np.zeros((SIZE, SIZE, 3), dtype=float)
    for c in range(3):
        base = c2[c] * (1 - t_low) + c1[c] * t_low
        color[:, :, c] = base * (1 - t_high) + c3[c] * t_high

    # Detail
    detail_shift = (detail - 0.5) * 16
    color[:, :, 0] += detail_shift * 1.0
    color[:, :, 1] += detail_shift * 0.6
    color[:, :, 2] += detail_shift * 0.3

    # Craters
    craters = generate_craters(sx, sy, sz, in_disc, p.crater_density, p.seed)
    for c in range(3):
        color[:, :, c] += craters * 160

    # Polar ice
    if p.has_polar_ice:
        ice_color = np.array([225, 230, 240], dtype=float)
        ice_noise = fbm_3d(sx * 8, sy * 8, sz * 8, octaves=3, seed=p.seed + 400)
        ice_t = np.clip((lat - 1.25) / 0.3, 0, 1)
        ice_t = np.clip(ice_t + (ice_noise - 0.5) * 0.3, 0, 1)
        ice_t *= (lat > 1.15).astype(float)
        for c in range(3):
            color[:, :, c] = color[:, :, c] * (1 - ice_t) + ice_color[c] * ice_t

    return finalize(color, in_disc)


def generate_temperate_rocky(p, grids):
    """Continent/ocean world with optional life and clouds."""
    _, _, dist, in_disc, lat, lon, sx, sy, sz = grids

    # Terrain for continent shapes
    terrain = fbm_3d(sx * 2.5, sy * 2.5, sz * 2.5, octaves=7, seed=p.seed)
    terrain += 0.3 * fbm_3d(sx * 5, sy * 5, sz * 5, octaves=5, seed=p.seed + 50)
    terrain /= 1.3

    detail = fbm_3d(sx * 12, sy * 12, sz * 12, octaves=4, seed=p.seed + 100)
    fine = fbm_3d(sx * 25, sy * 25, sz * 25, octaves=3, seed=p.seed + 150)

    c1 = np.array(p.base_color_1, dtype=float)
    c2 = np.array(p.base_color_2, dtype=float)
    c3 = np.array(p.base_color_3, dtype=float)

    # Ocean setup
    if p.ocean_type != OceanType.NONE and p.ocean_coverage > 0:
        sea_level = np.percentile(terrain[in_disc], p.ocean_coverage * 100)
    else:
        sea_level = terrain.min() - 1  # No ocean

    is_land = terrain > sea_level
    land_height = np.clip(
        np.where(is_land, (terrain - sea_level) / max(terrain[in_disc].max() - sea_level, 0.01), 0),
        0, 1
    )
    ocean_depth = np.clip(
        np.where(~is_land, (sea_level - terrain) / max(sea_level - terrain[in_disc].min(), 0.01), 0),
        0, 1
    )

    # Land coloring
    t1 = np.clip(land_height * 3, 0, 1)
    t2 = np.clip((land_height - 0.33) * 3, 0, 1)
    t3 = np.clip((land_height - 0.7) * 4, 0, 1)

    land_color = np.zeros((SIZE, SIZE, 3), dtype=float)
    for c in range(3):
        v = c2[c] * (1 - t1) + c1[c] * t1
        v = v * (1 - t2) + c3[c] * t2
        # High mountains: rocky gray
        rock = np.array([155, 142, 125])[c]
        land_color[:, :, c] = v * (1 - t3) + rock * t3

    # Life overlay on land
    if p.has_life:
        lc1 = np.array(p.life_color_1, dtype=float)
        lc2 = np.array(p.life_color_2, dtype=float)
        biome_noise = fbm_3d(sx * 1.8, sy * 1.8, sz * 1.8, octaves=3, seed=p.seed + 500)
        # Life grows in low-to-mid elevations
        life_mask = np.clip(1.0 - land_height * 2, 0, 1) * is_land.astype(float)
        life_t = np.clip((biome_noise - 0.3) / 0.4, 0, 1)
        for c in range(3):
            life_col = lc2[c] * (1 - life_t) + lc1[c] * life_t
            land_color[:, :, c] = land_color[:, :, c] * (1 - life_mask * 0.6) + life_col * (life_mask * 0.6)

    # Detail variation on land
    detail_shift = (detail - 0.5) * 18
    land_color[:, :, 0] += detail_shift * is_land
    land_color[:, :, 1] += detail_shift * 0.7 * is_land
    land_color[:, :, 2] += detail_shift * 0.3 * is_land

    fine_shift = (fine - 0.5) * 10
    land_color[:, :, 0] += fine_shift * is_land
    land_color[:, :, 1] += fine_shift * 0.6 * is_land

    # Ocean coloring
    ocean_colors = {
        OceanType.WATER_BLUE: ((60, 115, 160), (18, 45, 82)),
        OceanType.BRINE_DARK: ((50, 70, 85), (20, 30, 40)),
        OceanType.IRON_TINTED: ((80, 90, 100), (35, 40, 50)),
        OceanType.MINERAL_GREEN: ((50, 120, 110), (20, 55, 50)),
    }
    shallow, deep = ocean_colors.get(p.ocean_type, ((60, 115, 160), (18, 45, 82)))
    shallow = np.array(shallow, dtype=float)
    deep = np.array(deep, dtype=float)

    ocean_color = np.zeros((SIZE, SIZE, 3), dtype=float)
    for c in range(3):
        ocean_color[:, :, c] = shallow[c] * (1 - ocean_depth) + deep[c] * ocean_depth

    # Ocean detail
    ocean_detail = (detail - 0.5) * 8
    ocean_color[:, :, 1] += ocean_detail * 0.5 * (~is_land)
    ocean_color[:, :, 2] += ocean_detail * (~is_land)

    # Coastal blending
    coast_blend = np.clip((terrain - sea_level) / 0.02, 0, 1)
    color = np.zeros((SIZE, SIZE, 3), dtype=float)
    for c in range(3):
        color[:, :, c] = ocean_color[:, :, c] * (1 - coast_blend) + land_color[:, :, c] * coast_blend

    # Polar ice
    if p.has_polar_ice:
        ice_color = np.array([230, 235, 245], dtype=float)
        ice_noise = fbm_3d(sx * 8, sy * 8, sz * 8, octaves=3, seed=p.seed + 600)
        ice_t = np.clip((lat - 1.30) / 0.2, 0, 1)
        ice_t = np.clip(ice_t + (ice_noise - 0.5) * 0.25, 0, 1)
        ice_t *= (lat > 1.20).astype(float)
        for c in range(3):
            color[:, :, c] = color[:, :, c] * (1 - ice_t) + ice_color[c] * ice_t

    # Clouds
    if p.has_cloud_layer and p.cloud_coverage > 0:
        cloud = fbm_3d(sx * 4, sy * 4, sz * 4, octaves=4, seed=p.seed + 700)
        cloud_t = np.clip((cloud - (1.0 - p.cloud_coverage)) / 0.25, 0, 1) * 0.5
        cloud_color = np.array([240, 242, 248], dtype=float)
        for c in range(3):
            color[:, :, c] = color[:, :, c] * (1 - cloud_t) + cloud_color[c] * cloud_t

    return finalize(color, in_disc)


def generate_ocean_world(p, grids):
    """Global ocean with scattered volcanic islands."""
    # Same as temperate rocky but with very high ocean coverage
    return generate_temperate_rocky(p, grids)


def generate_lava_world(p, grids):
    """Dark crust with bright orange lava veins."""
    _, _, dist, in_disc, lat, lon, sx, sy, sz = grids

    # Crust plates
    terrain = fbm_3d(sx * 3, sy * 3, sz * 3, octaves=5, seed=p.seed)

    # Lava veins using ridged noise (sharp linear features)
    veins = ridged_noise(sx * 3.5, sy * 3.5, sz * 3.5, octaves=4, seed=p.seed + 300)
    veins2 = ridged_noise(sx * 7, sy * 7, sz * 7, octaves=3, seed=p.seed + 400)

    c1 = np.array(p.base_color_1, dtype=float)
    c2 = np.array(p.base_color_2, dtype=float)

    # Dark crust base
    t = terrain
    color = np.zeros((SIZE, SIZE, 3), dtype=float)
    for c in range(3):
        color[:, :, c] = c2[c] * (1 - t) + c1[c] * t

    detail = fbm_3d(sx * 15, sy * 15, sz * 15, octaves=3, seed=p.seed + 100)
    color[:, :, 0] += (detail - 0.5) * 10
    color[:, :, 1] += (detail - 0.5) * 8
    color[:, :, 2] += (detail - 0.5) * 6

    # Lava glow in thin veins — narrow bright lines on dark crust
    lava_mask = np.clip((veins - 0.75) * 8, 0, 1) * p.lava_coverage * 1.8
    lava_mask += np.clip((veins2 - 0.78) * 7, 0, 1) * p.lava_coverage * 1.0
    lava_mask = np.clip(lava_mask, 0, 1)

    # Lava color gradient: bright orange-yellow core -> dark red edge
    lava_r = np.clip(lava_mask * 1.5, 0, 1) * 255
    lava_g = np.clip(lava_mask * 1.5 - 0.3, 0, 1) * 160
    lava_b = np.clip(lava_mask * 2.0 - 1.2, 0, 1) * 30

    color[:, :, 0] = color[:, :, 0] * (1 - lava_mask) + lava_r
    color[:, :, 1] = color[:, :, 1] * (1 - lava_mask) + lava_g
    color[:, :, 2] = color[:, :, 2] * (1 - lava_mask) + lava_b

    return finalize(color, in_disc)


def generate_gas_giant(p, grids):
    """Banded atmosphere with noise-perturbed boundaries."""
    _, _, dist, in_disc, lat, lon, sx, sy, sz = grids

    c1 = np.array(p.base_color_1, dtype=float)
    c2 = np.array(p.base_color_2, dtype=float)
    c3 = np.array(p.base_color_3, dtype=float)

    # Gas giant: concentric bands ARE correct for polar projection (this is
    # how Jupiter looks from above the pole). But we add heavy noise-based
    # distortion so band edges are ragged, and large-scale color variation
    # along bands so each band segment looks different.

    # Strong warp of latitude using 3D noise (varies with longitude naturally)
    warp = fbm_3d(sx * 4, sy * 4, sz * 4, octaves=5, seed=p.seed + 100)
    warp2 = fbm_3d(sx * 8, sy * 8, sz * 8, octaves=3, seed=p.seed + 120)
    effective_lat = lat + (warp - 0.5) * 0.45 + (warp2 - 0.5) * 0.15

    # Create bands
    band_freq = p.band_count * 2.0 / np.pi
    band_val = np.sin(effective_lat * band_freq * np.pi)

    # Band color mixing
    color = np.zeros((SIZE, SIZE, 3), dtype=float)
    band_t = (band_val + 1) / 2
    for c in range(3):
        color[:, :, c] = c2[c] * (1 - band_t) + c1[c] * band_t

    # Bright zones
    bright_zones = np.clip(band_val * 2 - 0.5, 0, 1)
    for c in range(3):
        color[:, :, c] = color[:, :, c] * (1 - bright_zones * 0.4) + c3[c] * (bright_zones * 0.4)

    # Large-scale along-band color variation (breaks uniform appearance)
    along = fbm_3d(sx * 3, sy * 3, sz * 1.5, octaves=4, seed=p.seed + 200)
    along_shift = (along - 0.5) * 35
    color[:, :, 0] += along_shift * 0.9
    color[:, :, 1] += along_shift * 0.7
    color[:, :, 2] += along_shift * 0.35

    # Medium eddies (isotropic, creates swirls)
    eddy = fbm_3d(sx * 10, sy * 10, sz * 10, octaves=3, seed=p.seed + 250)
    eddy_shift = (eddy - 0.5) * 16
    color[:, :, 0] += eddy_shift * 0.6
    color[:, :, 1] += eddy_shift * 0.5
    color[:, :, 2] += eddy_shift * 0.25

    # Fine detail
    fine = fbm_3d(sx * 18, sy * 18, sz * 18, octaves=3, seed=p.seed + 300)
    fine_shift = (fine - 0.5) * 12
    color[:, :, 0] += fine_shift
    color[:, :, 1] += fine_shift * 0.7
    color[:, :, 2] += fine_shift * 0.4

    # Optional storm spot (25% chance based on seed)
    rng = np.random.RandomState(p.seed + 999)
    if rng.random() < 0.25:
        # Place storm at random location within disc
        storm_lat = rng.uniform(0.3, 1.0)
        storm_lon = rng.uniform(-np.pi, np.pi)
        storm_x = np.cos(storm_lat) * np.cos(storm_lon)
        storm_y = np.cos(storm_lat) * np.sin(storm_lon)
        storm_z = np.sin(storm_lat)
        storm_dist = np.sqrt((sx - storm_x)**2 + (sy - storm_y)**2 + (sz - storm_z)**2)
        storm_r = 0.12 + rng.random() * 0.08
        storm_mask = np.clip(1.0 - storm_dist / storm_r, 0, 1) ** 2
        # Storm is slightly darker/redder
        color[:, :, 0] += storm_mask * 25
        color[:, :, 1] -= storm_mask * 10
        color[:, :, 2] -= storm_mask * 15

    return finalize(color, in_disc)


def generate_ice_giant(p, grids):
    """Blue-teal with subtle bands."""
    _, _, dist, in_disc, lat, lon, sx, sy, sz = grids

    c1 = np.array(p.base_color_1, dtype=float)
    c2 = np.array(p.base_color_2, dtype=float)
    c3 = np.array(p.base_color_3, dtype=float)

    # Same warped-latitude approach as gas giant, subtler
    warp = fbm_3d(sx * 3, sy * 3, sz * 3, octaves=4, seed=p.seed + 100)
    warp2 = fbm_3d(sx * 7, sy * 7, sz * 7, octaves=3, seed=p.seed + 120)
    effective_lat = lat + (warp - 0.5) * 0.35 + (warp2 - 0.5) * 0.10

    band_val = np.sin(effective_lat * p.band_count * 2.0)

    color = np.zeros((SIZE, SIZE, 3), dtype=float)
    band_t = (band_val + 1) / 2
    for c in range(3):
        color[:, :, c] = c2[c] * (1 - band_t) + c1[c] * band_t

    # Lighter zones
    bright = np.clip(band_val, 0, 1)
    for c in range(3):
        color[:, :, c] = color[:, :, c] * (1 - bright * 0.2) + c3[c] * (bright * 0.2)

    # Along-band color variation
    along = fbm_3d(sx * 3, sy * 3, sz * 1.5, octaves=4, seed=p.seed + 200)
    turb_shift = (along - 0.5) * 22
    color[:, :, 0] += turb_shift * 0.5
    color[:, :, 1] += turb_shift * 0.7
    color[:, :, 2] += turb_shift * 0.9

    # Fine haze
    fine = fbm_3d(sx * 14, sy * 14, sz * 14, octaves=3, seed=p.seed + 300)
    color[:, :, 0] += (fine - 0.5) * 8
    color[:, :, 1] += (fine - 0.5) * 10
    color[:, :, 2] += (fine - 0.5) * 12

    return finalize(color, in_disc)


def generate_ice_dwarf(p, grids):
    """Pale gray-white with heavy craters."""
    # Very similar to airless rocky but with ice-dwarf palette
    _, _, dist, in_disc, lat, lon, sx, sy, sz = grids

    terrain = fbm_3d(sx * 3, sy * 3, sz * 3, octaves=5, seed=p.seed)
    detail = fbm_3d(sx * 10, sy * 10, sz * 10, octaves=3, seed=p.seed + 100)

    c1 = np.array(p.base_color_1, dtype=float)
    c2 = np.array(p.base_color_2, dtype=float)
    c3 = np.array(p.base_color_3, dtype=float)

    t = terrain
    color = np.zeros((SIZE, SIZE, 3), dtype=float)
    for c in range(3):
        color[:, :, c] = c2[c] * (1 - t) + c1[c] * t

    detail_shift = (detail - 0.5) * 12
    color[:, :, 0] += detail_shift
    color[:, :, 1] += detail_shift
    color[:, :, 2] += detail_shift * 1.1

    # Heavy craters
    craters = generate_craters(sx, sy, sz, in_disc, p.crater_density, p.seed)
    for c in range(3):
        color[:, :, c] += craters * 150

    return finalize(color, in_disc)


def generate_icy_moon(p, grids):
    """White/gray surface with fracture lines."""
    _, _, dist, in_disc, lat, lon, sx, sy, sz = grids

    terrain = fbm_3d(sx * 3, sy * 3, sz * 3, octaves=4, seed=p.seed)

    c1 = np.array(p.base_color_1, dtype=float)
    c2 = np.array(p.base_color_2, dtype=float)

    t = terrain
    color = np.zeros((SIZE, SIZE, 3), dtype=float)
    for c in range(3):
        color[:, :, c] = c2[c] * (1 - t) + c1[c] * t

    detail = fbm_3d(sx * 12, sy * 12, sz * 12, octaves=3, seed=p.seed + 100)
    color[:, :, 0] += (detail - 0.5) * 10
    color[:, :, 1] += (detail - 0.5) * 10
    color[:, :, 2] += (detail - 0.5) * 12

    # Fracture lines using ridged noise
    if p.has_fractures:
        frac1 = ridged_noise(sx * 4, sy * 4, sz * 4, octaves=4, seed=p.seed + 500)
        frac2 = ridged_noise(sx * 8, sy * 8, sz * 8, octaves=3, seed=p.seed + 600)
        # Fractures are darker blue-ish lines
        frac_mask = np.clip((frac1 - 0.7) * 5, 0, 1) * 0.35
        frac_mask += np.clip((frac2 - 0.72) * 5, 0, 1) * 0.2
        frac_mask = np.clip(frac_mask, 0, 1)
        # Darker brown-tan fracture color
        frac_color = np.array([140, 120, 100], dtype=float)
        for c in range(3):
            color[:, :, c] = color[:, :, c] * (1 - frac_mask) + frac_color[c] * frac_mask

    # Light craters
    craters = generate_craters(sx, sy, sz, in_disc, p.crater_density, p.seed)
    for c in range(3):
        color[:, :, c] += craters * 100

    return finalize(color, in_disc)


def generate_volcanic_moon(p, grids):
    """Io-like yellow/orange/sulfur surface."""
    _, _, dist, in_disc, lat, lon, sx, sy, sz = grids

    terrain = fbm_3d(sx * 3, sy * 3, sz * 3, octaves=5, seed=p.seed)
    detail = fbm_3d(sx * 8, sy * 8, sz * 8, octaves=4, seed=p.seed + 100)

    c1 = np.array(p.base_color_1, dtype=float)  # Yellow
    c2 = np.array(p.base_color_2, dtype=float)  # Orange
    c3 = np.array(p.base_color_3, dtype=float)  # Pale yellow

    t = terrain
    color = np.zeros((SIZE, SIZE, 3), dtype=float)
    for c in range(3):
        color[:, :, c] = c2[c] * (1 - t) + c1[c] * t

    # Sulfur deposit patches (green-tinted)
    sulfur_noise = fbm_3d(sx * 5, sy * 5, sz * 5, octaves=3, seed=p.seed + 300)
    sulfur_mask = np.clip((sulfur_noise - 0.6) * 4, 0, 1)
    sulfur_color = np.array([180, 180, 50], dtype=float)
    for c in range(3):
        color[:, :, c] = color[:, :, c] * (1 - sulfur_mask * 0.4) + sulfur_color[c] * (sulfur_mask * 0.4)

    # Dark volcanic vents / calderas
    vent_noise = value_noise_3d(sx * 6, sy * 6, sz * 6, seed=p.seed + 400)
    vent_mask = np.clip((vent_noise - 0.82) * 8, 0, 1)
    vent_color = np.array([40, 30, 25], dtype=float)
    for c in range(3):
        color[:, :, c] = color[:, :, c] * (1 - vent_mask) + vent_color[c] * vent_mask

    # Red/orange lava flows near vents
    flow_noise = ridged_noise(sx * 5, sy * 5, sz * 5, octaves=3, seed=p.seed + 500)
    flow_mask = vent_mask * np.clip((flow_noise - 0.5) * 3, 0, 1) * 0.5
    color[:, :, 0] += flow_mask * 200
    color[:, :, 1] += flow_mask * 80

    # Detail
    detail_shift = (detail - 0.5) * 15
    color[:, :, 0] += detail_shift
    color[:, :, 1] += detail_shift * 0.8
    color[:, :, 2] += detail_shift * 0.3

    return finalize(color, in_disc)


# ---------------------------------------------------------------------------
# Common finalization
# ---------------------------------------------------------------------------

def finalize(color, in_disc):
    """Clip colors, apply disc mask, return RGBA array."""
    color = np.clip(color, 0, 255)
    img = np.zeros((SIZE, SIZE, 4), dtype=np.uint8)
    img[:, :, 0] = color[:, :, 0].astype(np.uint8)
    img[:, :, 1] = color[:, :, 1].astype(np.uint8)
    img[:, :, 2] = color[:, :, 2].astype(np.uint8)
    img[:, :, 3] = np.where(in_disc, 255, 0).astype(np.uint8)
    return img


# ---------------------------------------------------------------------------
# Dispatch table
# ---------------------------------------------------------------------------

GENERATORS = {
    BodyType.AIRLESS_ROCKY: generate_airless_rocky,
    BodyType.HOT_ROCKY: generate_hot_rocky,
    BodyType.VENUS_ANALOG: generate_venus_analog,
    BodyType.MARS_ANALOG: generate_mars_analog,
    BodyType.TEMPERATE_ROCKY: generate_temperate_rocky,
    BodyType.OCEAN_WORLD: generate_ocean_world,
    BodyType.LAVA_WORLD: generate_lava_world,
    BodyType.GAS_GIANT: generate_gas_giant,
    BodyType.ICE_GIANT: generate_ice_giant,
    BodyType.ICE_DWARF: generate_ice_dwarf,
    BodyType.ICY_MOON: generate_icy_moon,
    BodyType.VOLCANIC_MOON: generate_volcanic_moon,
}


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def generate_body(body: CatalogBodyData, grids, output_dir: Path, skip_existing: bool) -> float:
    """Generate texture for a single body. Returns time taken."""
    filename = body.name.lower().replace(" ", "_") + ".png"
    out_path = output_dir / filename

    if skip_existing and out_path.exists():
        return 0.0

    t0 = time.time()
    btype = classify_body(body)
    params = make_params(body, btype)
    generator = GENERATORS[btype]
    img_array = generator(params, grids)

    result = Image.fromarray(img_array, 'RGBA')
    result.save(out_path)
    elapsed = time.time() - t0
    return elapsed


def main():
    parser = argparse.ArgumentParser(description="Generate planet textures for catalog bodies")
    parser.add_argument("--body", type=str, help="Generate single body by name")
    parser.add_argument("--system", type=str, help="Generate bodies for a specific system")
    parser.add_argument("--type", type=str, help="Generate bodies of specific type (e.g. gas_giant)")
    parser.add_argument("--skip-existing", action="store_true", help="Don't overwrite existing textures")
    parser.add_argument("--list", action="store_true", help="List all bodies with their types")
    args = parser.parse_args()

    # Parse catalog
    if not CATALOG_PATH.exists():
        print(f"Error: catalog not found at {CATALOG_PATH}", file=sys.stderr)
        print("Run from project root directory.", file=sys.stderr)
        sys.exit(1)

    bodies = parse_catalog(CATALOG_PATH)
    print(f"Parsed {len(bodies)} bodies from catalog")

    # Filter out bodies that have hand-made textures
    bodies = [b for b in bodies if b.name.lower() not in SKIP_BODIES]

    # Handle duplicate names (keep first occurrence)
    seen_names = set()
    unique_bodies = []
    for b in bodies:
        lower = b.name.lower().replace(" ", "_")
        if lower in seen_names:
            print(f"  Warning: duplicate name '{b.name}' in {b.system_name}, skipping")
            continue
        seen_names.add(lower)
        unique_bodies.append(b)
    bodies = unique_bodies

    # Classify all bodies
    classified = [(b, classify_body(b)) for b in bodies]

    # --list mode
    if args.list:
        type_counts = {}
        for body, btype in classified:
            type_counts[btype.name] = type_counts.get(btype.name, 0) + 1
            moon_tag = " (moon)" if body.is_moon else ""
            life_tag = " [LIFE]" if body.has_life else ""
            print(f"  {body.name:25s} {btype.name:20s} {body.temperature_k:6.0f}K  {body.system_name}{moon_tag}{life_tag}")
        print(f"\nType distribution:")
        for t, count in sorted(type_counts.items()):
            print(f"  {t:20s}: {count}")
        print(f"Total: {len(classified)}")
        return

    # Apply filters
    if args.body:
        target = args.body.lower()
        classified = [(b, t) for b, t in classified if b.name.lower() == target]
        if not classified:
            print(f"Error: body '{args.body}' not found in catalog", file=sys.stderr)
            sys.exit(1)

    if args.system:
        target = args.system.lower()
        classified = [(b, t) for b, t in classified if b.system_name.lower() == target]
        if not classified:
            print(f"Error: no bodies found for system '{args.system}'", file=sys.stderr)
            sys.exit(1)

    if args.type:
        target = args.type.upper()
        classified = [(b, t) for b, t in classified if t.name == target]
        if not classified:
            print(f"Error: no bodies of type '{args.type}'", file=sys.stderr)
            sys.exit(1)

    print(f"Generating {len(classified)} textures...")
    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)

    # Build coordinate grids once
    print("Building coordinate grids...")
    grids = make_grids()

    total_time = 0.0
    for i, (body, btype) in enumerate(classified):
        t = generate_body(body, grids, OUTPUT_DIR, args.skip_existing)
        total_time += t
        if t > 0:
            remaining = len(classified) - i - 1
            eta = (total_time / (i + 1)) * remaining
            eta_str = f"ETA: {eta / 60:.1f}m" if eta > 60 else f"ETA: {eta:.0f}s"
            print(f"  [{i + 1}/{len(classified)}] {body.name:25s} {t:.1f}s  ({btype.name}, {eta_str})")
        else:
            print(f"  [{i + 1}/{len(classified)}] {body.name:25s} skipped (exists)")

    print(f"\nDone! Generated {len(classified)} textures in {total_time:.1f}s")


if __name__ == "__main__":
    main()
