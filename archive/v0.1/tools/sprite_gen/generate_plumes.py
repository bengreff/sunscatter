#!/usr/bin/env python3
"""
Generate engine plume animation sprites for Sunscatter.

Chemical: 3 propellants x 4 frames = 12 PNGs (320x800, billowy layered plumes).
Interstellar: 6 plume types x 4 frames = 24 PNGs (various sizes, beam/fireball).

Each chemical plume has layered structure: hot core + mid zone + turbulent outer.
Interstellar plumes range from Orion's discrete fireballs to AM Torch's hairline beam.
Uses noise-displaced boundaries + Gaussian blur for smooth look.

Reference nozzle width: 200px. Game scales to match actual engine nozzle.
"""

from PIL import Image, ImageDraw, ImageFilter
import math
import os
import random

PAD = 6
NUM_FRAMES = 4
NOZZLE_W = 200

# Canvas: room for expansion + noise + blur margin
CANVAS_W = 320
CANVAS_H = 800

# ================================================================
# Propellant color palettes: (R, G, B, A)
# ================================================================

PALETTES = {
    "kerolox": {
        "core":  (255, 255, 210, 255),       # white-yellow
        "mid":   (255, 165,  50, 235),        # bright orange
        "outer": (205,  75,  15, 160),        # deep orange-red
    },
    "methalox": {
        "core":  (235, 228, 255, 255),        # white with subtle purple
        "mid":   (175, 145, 220, 225),        # muted purple
        "outer": (125,  90, 190, 130),        # desaturated purple
    },
    "hydrolox": {
        "core":  (245, 248, 255, 235),        # near-white
        "mid":   (190, 210, 255, 140),        # very pale blue
        "outer": (135, 170, 250,  60),        # very transparent pale blue
    },
}


# ================================================================
# Noise
# ================================================================

def _hash(ix, iy, seed):
    h = (ix * 374761393 + iy * 668265263 + seed) & 0xFFFFFFFF
    h = ((h ^ (h >> 13)) * 1274126177) & 0xFFFFFFFF
    return (h & 0xFFFF) / 65535.0 * 2.0 - 1.0


def value_noise(x, seed, scale):
    """1D value noise with smoothstep interpolation."""
    sx = x * scale
    ix = int(math.floor(sx))
    fx = sx - ix
    fx = fx * fx * (3 - 2 * fx)  # smoothstep
    v0 = _hash(ix, 0, seed)
    v1 = _hash(ix + 1, 0, seed)
    return v0 + fx * (v1 - v0)


def fbm(x, seed, octaves=3, scale=0.015, lacunarity=2.0, gain=0.5):
    """Fractal Brownian motion — sum of noise at multiple scales."""
    val = 0.0
    amp = 1.0
    s = scale
    for i in range(octaves):
        val += amp * value_noise(x, seed + i * 1337, s)
        amp *= gain
        s *= lacunarity
    return val


# ================================================================
# Plume shape
# ================================================================

def plume_taper(frac):
    """Relaxed plume envelope: slight expansion near nozzle, then gradual taper."""
    if frac < 0.08:
        # Expand slightly near nozzle exit
        return 1.0 + 0.25 * (frac / 0.08)
    elif frac < 0.25:
        # Settle back toward nozzle width
        return 1.25 - 0.2 * ((frac - 0.08) / 0.17)
    else:
        # Stay wide, taper late
        t = (frac - 0.25) / 0.75
        return 1.05 * max(0.0, 1.0 - t ** 1.8)


def generate_boundary(plume_h, nozzle_hw, width_fn, seed, noise_amp, noise_scale):
    """Generate left and right half-width arrays with noise displacement.

    width_fn: callable(frac) -> multiplier relative to base envelope.
    Returns: (left_hw[], right_hw[]) arrays of length plume_h.
    """
    left = []
    right = []
    for row in range(plume_h):
        frac = row / max(1, plume_h - 1)
        base_hw = plume_taper(frac) * nozzle_hw * width_fn(frac)

        # Noise envelope: smooth at top, increasingly turbulent toward bottom
        noise_env = frac ** 0.6

        # Large-scale billowy noise (different seeds for left/right -> asymmetry)
        nl = fbm(row, seed, octaves=3, scale=noise_scale) * noise_amp * noise_env
        nr = fbm(row, seed + 7777, octaves=3, scale=noise_scale) * noise_amp * noise_env

        # Smaller-scale texture on top
        nl += fbm(row, seed + 500, octaves=2, scale=noise_scale * 3) * noise_amp * 0.25 * noise_env
        nr += fbm(row, seed + 8277, octaves=2, scale=noise_scale * 3) * noise_amp * 0.25 * noise_env

        left.append(max(0.0, base_hw + nl))
        right.append(max(0.0, base_hw + nr))

    return left, right


# ================================================================
# Drawing
# ================================================================

def draw_layer(cw, ch, left_hw, right_hw, color_rgba, cx, y0, plume_h, tip_power=1.3):
    """Draw a single plume layer as row-by-row horizontal lines."""
    img = Image.new("RGBA", (cw, ch), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)

    base_alpha = color_rgba[3]
    rgb = color_rgba[:3]

    for row in range(plume_h):
        y = y0 + row
        frac = row / max(1, plume_h - 1)

        lw = left_hw[row]
        rw = right_hw[row]
        if lw < 0.5 and rw < 0.5:
            continue

        # Alpha fades toward tip
        tip_fade = max(0.0, 1.0 - frac ** tip_power)
        a = int(base_alpha * tip_fade)
        if a < 2:
            continue

        x1 = max(0, int(cx - lw))
        x2 = min(cw - 1, int(cx + rw))
        if x1 >= x2:
            continue

        d.line([(x1, y), (x2, y)], fill=(*rgb, a), width=1)

    return img


def generate_plume_frame(propellant, frame_idx):
    """Generate a single plume animation frame."""
    palette = PALETTES[propellant]
    rng = random.Random(propellant + str(frame_idx))
    seed = rng.randint(0, 1000000)

    cw, ch = CANVAS_W, CANVAS_H
    plume_h = ch - PAD * 2
    nozzle_hw = NOZZLE_W / 2.0
    cx = cw / 2.0
    y0 = PAD

    # Layer width functions (fraction of base envelope at each height)
    # Core is wider near nozzle, narrows further down
    def core_width(frac):
        return 0.35 - 0.15 * frac

    def mid_width(frac):
        return 0.58 - 0.10 * frac

    def outer_width(frac):
        return 1.0

    # Noise amplitudes and scale
    outer_amp = nozzle_hw * 0.30
    mid_amp = nozzle_hw * 0.18
    core_amp = nozzle_hw * 0.10
    noise_scale = 0.013

    # Generate boundaries for each layer
    outer_l, outer_r = generate_boundary(
        plume_h, nozzle_hw, outer_width, seed, outer_amp, noise_scale)
    mid_l, mid_r = generate_boundary(
        plume_h, nozzle_hw, mid_width, seed + 1000, mid_amp, noise_scale * 1.3)
    core_l, core_r = generate_boundary(
        plume_h, nozzle_hw, core_width, seed + 2000, core_amp, noise_scale * 1.8)

    # Clamp to ensure proper nesting: core <= mid <= outer
    for row in range(plume_h):
        mid_l[row] = min(mid_l[row], outer_l[row])
        mid_r[row] = min(mid_r[row], outer_r[row])
        core_l[row] = min(core_l[row], mid_l[row])
        core_r[row] = min(core_r[row], mid_r[row])

    # Draw each layer on its own image
    outer_img = draw_layer(cw, ch, outer_l, outer_r, palette["outer"],
                           cx, y0, plume_h, tip_power=1.2)
    mid_img = draw_layer(cw, ch, mid_l, mid_r, palette["mid"],
                         cx, y0, plume_h, tip_power=1.3)
    core_img = draw_layer(cw, ch, core_l, core_r, palette["core"],
                          cx, y0, plume_h, tip_power=1.5)

    # Composite layers: outer (bottom) -> mid -> core (top)
    result = Image.new("RGBA", (cw, ch), (0, 0, 0, 0))
    result = Image.alpha_composite(result, outer_img)
    result = Image.alpha_composite(result, mid_img)
    result = Image.alpha_composite(result, core_img)

    # Gaussian blur for smooth, billowy look
    result = result.filter(ImageFilter.GaussianBlur(radius=5))

    return result


# ================================================================
# Interstellar plume shapes
# ================================================================

def beam_taper(frac, fade_power=1.5):
    """Beam envelope: constant width, fading at tip. No billowy expansion."""
    tip_start = 0.85
    if frac < tip_start:
        return 1.0
    t = (frac - tip_start) / (1.0 - tip_start)
    return max(0.0, 1.0 - t ** fade_power)


def generate_beam_boundary(plume_h, beam_hw, seed, noise_amp, noise_scale):
    """Generate beam boundaries with very low noise (magnetically confined)."""
    left = []
    right = []
    for row in range(plume_h):
        frac = row / max(1, plume_h - 1)
        base_hw = beam_taper(frac) * beam_hw

        # Minimal noise — smooth magnetic confinement
        noise_env = 0.3 + 0.7 * frac  # slight increase toward tip
        nl = fbm(row, seed, octaves=2, scale=noise_scale) * noise_amp * noise_env
        nr = fbm(row, seed + 7777, octaves=2, scale=noise_scale) * noise_amp * noise_env

        left.append(max(0.0, base_hw + nl))
        right.append(max(0.0, base_hw + nr))

    return left, right


# ================================================================
# Beam plume configs
# ================================================================

DAEDALUS_CONFIG = {
    "canvas_w": 320, "canvas_h": 2000,
    "beam_hw": 20,        # narrow focused beam
    "glow_hw": 55,        # soft outer glow
    "core_color":  (230, 240, 255, 255),   # white-blue
    "mid_color":   (180, 200, 255, 160),   # pale blue
    "outer_color": (150, 180, 255,  70),   # faint blue-white
    "noise_amp": 5.0,
    "ignition_radius": 35,   # bright ICF pellet detonation point
    "blur_radius": 6,
}

ZPINCH_CONFIG = {
    "canvas_w": 320, "canvas_h": 1200,
    "beam_hw": 22,
    "glow_hw": 50,
    "core_color":  (220, 235, 255, 255),   # electric blue-white
    "mid_color":   (170, 195, 255, 150),   # pale blue
    "outer_color": (140, 175, 255,  60),   # faint blue
    "noise_amp": 4.0,
    "ignition_radius": 0,
    "blur_radius": 5,
}

AMCAT_CONFIG = {
    "canvas_w": 320, "canvas_h": 2400,
    "beam_hw": 18,
    "glow_hw": 50,
    "core_color":  (240, 225, 255, 255),   # white-purple
    "mid_color":   (190, 160, 240, 155),   # purple-blue
    "outer_color": (155, 130, 220,  65),   # faint purple
    "noise_amp": 4.5,
    "ignition_radius": 30,   # visible antimatter catalyzed reaction
    "blur_radius": 6,
}

AM_TORCH_CONFIG = {
    "canvas_w": 200, "canvas_h": 4000,
    "beam_hw": 6,         # razor-thin — hairline beam
    "glow_hw": 16,        # barely-there glow
    "core_color":  (255, 255, 255, 255),   # brilliant white
    "mid_color":   (140, 150, 220,  80),   # blue-purple edge
    "outer_color": ( 40,  50, 140,  35),   # dark blue fringe
    "noise_amp": 1.0,     # near-zero noise for hard edges
    "ignition_radius": 0,
    "blur_radius": 2,     # minimal blur — crisp beam
}

GAMMA_CONFIG = {
    "canvas_w": 400, "canvas_h": 5000,
    # Inner beam — same razor-thin style as AM Torch
    "beam_hw": 6,
    "glow_hw": 16,
    "core_color":  (220, 255, 230, 255),   # pale green-white
    "mid_color":   (200, 240, 255,  80),   # bluish-white
    "outer_color": (190, 225, 250,  25),   # barely visible
    "noise_amp": 1.0,
    "ignition_radius": 0,
    "blur_radius": 2,
    # Faint wider beam surrounding the core (full length)
    "wide_beam_hw": 45,       # wider beam half-width
    "wide_beam_color": (190, 210, 245, 40),  # faint translucent
}


def generate_beam_plume_frame(config, frame_idx):
    """Generate a magnetically-confined beam plume frame."""
    rng = random.Random(f"beam_{config['canvas_h']}_{frame_idx}")
    seed = rng.randint(0, 1000000)

    cw = config["canvas_w"]
    ch = config["canvas_h"]
    plume_h = ch - PAD * 2
    cx = cw / 2.0
    y0 = PAD

    beam_hw = config["beam_hw"]
    glow_hw = config["glow_hw"]
    noise_amp = config["noise_amp"]
    noise_scale = 0.008  # low frequency for smooth beams

    # Generate beam boundaries for each layer
    core_l, core_r = generate_beam_boundary(
        plume_h, beam_hw, seed, noise_amp * 0.3, noise_scale)
    mid_l, mid_r = generate_beam_boundary(
        plume_h, beam_hw * 1.8, seed + 1000, noise_amp * 0.5, noise_scale)
    outer_l, outer_r = generate_beam_boundary(
        plume_h, glow_hw, seed + 2000, noise_amp, noise_scale)

    # Clamp nesting
    for row in range(plume_h):
        mid_l[row] = min(mid_l[row], outer_l[row])
        mid_r[row] = min(mid_r[row], outer_r[row])
        core_l[row] = min(core_l[row], mid_l[row])
        core_r[row] = min(core_r[row], mid_r[row])

    # Draw layers
    outer_img = draw_layer(cw, ch, outer_l, outer_r, config["outer_color"],
                           cx, y0, plume_h, tip_power=1.2)
    mid_img = draw_layer(cw, ch, mid_l, mid_r, config["mid_color"],
                         cx, y0, plume_h, tip_power=1.4)
    core_img = draw_layer(cw, ch, core_l, core_r, config["core_color"],
                          cx, y0, plume_h, tip_power=1.6)

    # Composite
    result = Image.new("RGBA", (cw, ch), (0, 0, 0, 0))

    # Faint wider beam for gamma conversion — full-length surrounding beam
    if "wide_beam_hw" in config:
        wb_hw = config["wide_beam_hw"]
        wb_color = config["wide_beam_color"]
        wb_l, wb_r = generate_beam_boundary(
            plume_h, wb_hw, seed + 5000, noise_amp * 0.6, noise_scale)
        wide_img = draw_layer(cw, ch, wb_l, wb_r, wb_color,
                              cx, y0, plume_h, tip_power=1.3)
        result = Image.alpha_composite(result, wide_img)

    result = Image.alpha_composite(result, outer_img)
    result = Image.alpha_composite(result, mid_img)
    result = Image.alpha_composite(result, core_img)

    # Ignition point — bright circle at top center (ICF detonation)
    ign_r = config.get("ignition_radius", 0)
    if ign_r > 0:
        ign_img = Image.new("RGBA", (cw, ch), (0, 0, 0, 0))
        ign_draw = ImageDraw.Draw(ign_img)
        ign_cx = int(cx)
        ign_cy = y0 + 2
        # Draw concentric circles for radial gradient
        for r in range(ign_r, 0, -1):
            t = r / ign_r
            a = int(255 * (1.0 - t * t))
            c = config["core_color"]
            ign_draw.ellipse(
                [ign_cx - r, ign_cy - r, ign_cx + r, ign_cy + r],
                fill=(c[0], c[1], c[2], a))
        result = Image.alpha_composite(result, ign_img)

    result = result.filter(ImageFilter.GaussianBlur(
        radius=config.get("blur_radius", 5)))

    return result


def generate_orion_plume_frame(frame_idx):
    """Generate an Orion pulse drive frame — single fireball expanding over 4 frames.

    Frame 0: small bright detonation. Frame 3: fully expanded, fading.
    Each 4-frame cycle = one bomb going off.
    """
    cw, ch = 600, 600

    result = Image.new("RGBA", (cw, ch), (0, 0, 0, 0))
    fb_x = cw // 2
    fb_y = 160  # fixed position below nozzle

    # Expansion: frame 0 = tight & bright, frame 3 = large & fading
    expand_t = frame_idx / 3.0  # 0.0 -> 1.0
    min_radius = 40
    max_radius_full = 180
    max_radius = int(min_radius + (max_radius_full - min_radius) * expand_t)

    # Opacity fades as fireball expands (cooling)
    opacity_scale = 1.0 - 0.4 * expand_t

    fb_draw = ImageDraw.Draw(result)

    for r in range(max_radius, 0, -1):
        t = r / max_radius  # 1.0 at edge, 0.0 at center

        if t < 0.25:
            # White-hot core
            red, green, blue = 255, 255, 240
            alpha = int(255 * (1.0 - t * 2))
        elif t < 0.55:
            # Orange transition
            blend = (t - 0.25) / 0.30
            red = 255
            green = int(240 - 130 * blend)
            blue = int(240 - 200 * blend)
            alpha = int(220 - 60 * blend)
        else:
            # Fading red-brown edge
            blend = (t - 0.55) / 0.45
            red = int(255 - 80 * blend)
            green = int(110 - 70 * blend)
            blue = int(40 - 25 * blend)
            alpha = int(160 * (1.0 - blend ** 1.5))

        alpha = max(0, min(255, int(alpha * opacity_scale)))
        fb_draw.ellipse(
            [fb_x - r, fb_y - r, fb_x + r, fb_y + r],
            fill=(red, green, blue, alpha))

    # Heavy blur for that explosive, hot-gas look
    result = result.filter(ImageFilter.GaussianBlur(radius=8))

    return result


# ================================================================
# Interstellar plume registry
# ================================================================

INTERSTELLAR_PLUMES = {
    "orion_pulse":        generate_orion_plume_frame,
    "daedalus":           lambda f: generate_beam_plume_frame(DAEDALUS_CONFIG, f),
    "zpinch":             lambda f: generate_beam_plume_frame(ZPINCH_CONFIG, f),
    "amcat":              lambda f: generate_beam_plume_frame(AMCAT_CONFIG, f),
    "am_torch":           lambda f: generate_beam_plume_frame(AM_TORCH_CONFIG, f),
    "gamma_conversion":   lambda f: generate_beam_plume_frame(GAMMA_CONFIG, f),
}


# ================================================================
# Main
# ================================================================

PROPELLANTS = ["kerolox", "methalox", "hydrolox"]

if __name__ == "__main__":
    script_dir = os.path.dirname(os.path.abspath(__file__))
    project_root = os.path.abspath(os.path.join(script_dir, "..", ".."))
    output_dir = os.path.join(project_root, "data", "sprites", "plumes")
    os.makedirs(output_dir, exist_ok=True)

    # Clean up old ASL/vacuum files
    for f in os.listdir(output_dir):
        if f.endswith(".png") and ("_asl_" in f or "_vacuum_" in f):
            os.remove(os.path.join(output_dir, f))
            print(f"  Removed old: {f}")

    count = 0
    for propellant in PROPELLANTS:
        for frame in range(NUM_FRAMES):
            img = generate_plume_frame(propellant, frame)
            name = f"{propellant}_frame{frame}"
            out = os.path.join(output_dir, f"{name}.png")
            img.save(out)
            count += 1
            print(f"  {name:30s}  {img.size[0]:4d}x{img.size[1]:4d}")

    # Interstellar engine plumes
    for plume_name, gen_fn in INTERSTELLAR_PLUMES.items():
        for frame in range(NUM_FRAMES):
            img = gen_fn(frame)
            name = f"{plume_name}_frame{frame}"
            out = os.path.join(output_dir, f"{name}.png")
            img.save(out)
            count += 1
            print(f"  {name:30s}  {img.size[0]:4d}x{img.size[1]:4d}")

    print(f"\nGenerated {count} plume sprites in {output_dir}")
