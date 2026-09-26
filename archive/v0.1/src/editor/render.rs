use crate::parts::{PartDefinitions, PartDefinition, PartCategory, FairingShape, GRID_SQUARE_SIZE};
use crate::render::Vertex;
use crate::render::sprites::SpriteAtlas;
use super::EditorState;

/// Grid line color
const GRID_COLOR: [f32; 4] = [0.2, 0.2, 0.3, 0.5];
const GRID_MAJOR_COLOR: [f32; 4] = [0.3, 0.3, 0.4, 0.7];

/// Part colors - grey boxes
const PART_COLOR: [f32; 4] = [0.4, 0.4, 0.45, 1.0];

// Engine base colors (darker than regular parts)
// First stage engines are darker, upper stage are slightly lighter
const ENGINE_NOZZLE_DARK: [f32; 4] = [0.08, 0.08, 0.10, 1.0];       // Very dark for first stage
const ENGINE_NOZZLE_LIGHT: [f32; 4] = [0.15, 0.15, 0.17, 1.0];      // Lighter for upper stage
const ENGINE_CHAMBER_COLOR: [f32; 4] = [0.18, 0.18, 0.20, 1.0];     // Combustion chamber
const ENGINE_RING_DARK: [f32; 4] = [0.14, 0.14, 0.16, 1.0];         // Rings on dark nozzles
const ENGINE_RING_LIGHT: [f32; 4] = [0.22, 0.22, 0.24, 1.0];        // Rings on light nozzles
const ENGINE_TURBOPUMP_COLOR: [f32; 4] = [0.20, 0.20, 0.22, 1.0];   // Turbopump housing
const ENGINE_GAS_GEN_COLOR: [f32; 4] = [0.16, 0.16, 0.18, 1.0];     // Gas generator box

// Pod colors
const POD_COLOR: [f32; 4] = [0.15, 0.15, 0.18, 1.0];               // Dark grey pod
const POD_WINDOW_COLOR: [f32; 4] = [0.9, 0.9, 0.95, 1.0];          // White/light grey window

// Decoupler colors
const DECOUPLER_RING_COLOR: [f32; 4] = [0.4, 0.4, 0.45, 1.0];      // Matches fuel tank color

// Heat shield colors
const HEAT_SHIELD_FACE_COLOR: [f32; 4] = [0.05, 0.05, 0.05, 1.0];  // Near-black ablative face
const HEAT_SHIELD_BACK_COLOR: [f32; 4] = [0.12, 0.12, 0.12, 1.0];  // Dark backing structure

// Fairing colors
const FAIRING_BASE_COLOR: [f32; 4] = [0.4, 0.4, 0.45, 1.0];        // Matches fuel tank color
const FAIRING_SHELL_COLOR: [f32; 4] = [0.4, 0.4, 0.45, 1.0];       // Matches fuel tank color
const FAIRING_SHELL_LINE_COLOR: [f32; 4] = [0.33, 0.35, 0.40, 1.0]; // Panel seam lines

/// Rotate a slice of vertices around a center point by the given angle (radians).
fn rotate_vertices_around(vertices: &mut [Vertex], cx: f32, cy: f32, angle: f64) {
    if angle.abs() < 1e-6 {
        return;
    }
    let cos_a = angle.cos() as f32;
    let sin_a = angle.sin() as f32;
    for v in vertices.iter_mut() {
        let dx = v.position[0] - cx;
        let dy = v.position[1] - cy;
        v.position[0] = cx + dx * cos_a - dy * sin_a;
        v.position[1] = cy + dx * sin_a + dy * cos_a;
    }
}

/// Emit 6 sprite vertices (2 triangles) for a textured quad at the given position and size.
/// Uses the part's visual dimensions (width/height in meters).
fn generate_sprite_quad(
    vertices: &mut Vec<Vertex>,
    rect: &crate::render::sprites::SpriteRect,
    x: f32,
    y: f32,
    half_w: f32,
    half_h: f32,
    tint: [f32; 4],
) {
    let u0 = rect.u_min;
    let v0 = rect.v_min;
    let u1 = rect.u_max;
    let v1 = rect.v_max;

    // Two triangles: bottom-left, bottom-right, top-right / bottom-left, top-right, top-left
    // Note: sprite v0 is atlas top, v1 is atlas bottom. In game coords, +y is up, so
    // bottom of quad (y - half_h) maps to v1 (bottom of sprite) and top (y + half_h) maps to v0.
    vertices.push(Vertex::sprite([x - half_w, y - half_h], [u0, v1], tint));
    vertices.push(Vertex::sprite([x + half_w, y - half_h], [u1, v1], tint));
    vertices.push(Vertex::sprite([x + half_w, y + half_h], [u1, v0], tint));

    vertices.push(Vertex::sprite([x - half_w, y - half_h], [u0, v1], tint));
    vertices.push(Vertex::sprite([x + half_w, y + half_h], [u1, v0], tint));
    vertices.push(Vertex::sprite([x - half_w, y + half_h], [u0, v0], tint));
}

/// Render a radiator using the same stowed/deployed logic as solar panels.
/// Stowed (deploy_fraction 0): grey base rectangle (narrow mast).
/// Deploying: sprite reveals from the bottom upward, same as solar panels.
/// Fully deployed (deploy_fraction 1): full sprite visible.
fn generate_radiator_partial(
    vertices: &mut Vec<Vertex>,
    rect: &crate::render::sprites::SpriteRect,
    def: &PartDefinition,
    x: f32,
    y: f32,
    deploy_fraction: f64,
    alpha: f32,
) {
    generate_solar_panel_partial(vertices, rect, def, x, y, deploy_fraction, alpha);
}

/// Render a partially deployed solar panel.
/// When deploy_fraction < 1.0, draws a grey base rectangle at the bottom and a partial
/// sprite quad showing only the deployed portion. When deploy_fraction == 0.0, only the base.
fn generate_solar_panel_partial(
    vertices: &mut Vec<Vertex>,
    rect: &crate::render::sprites::SpriteRect,
    def: &PartDefinition,
    x: f32,
    y: f32,
    deploy_fraction: f64,
    alpha: f32,
) {
    let (sp_hw, sp_hh, sp_ox, sp_oy) = sprite_placement(def, Some(rect));
    let sp_x = x + sp_ox;
    let sp_y = y + sp_oy;
    let panel_bottom = sp_y - sp_hh;

    // Base height: 1 grid square for narrow panels, 2 for wide (>=2 grid wide)
    let base_squares: f32 = if def.grid_width >= 2.0 { 2.0 } else { 1.0 };
    let base_height: f32 = base_squares * GRID_SQUARE_SIZE as f32;

    // Grey base rectangle: when retracted (deploy_fraction == 0), draw a narrow
    // stowed mast (0.2 grid squares wide, full panel height). When deploying, draw
    // the wider base at the bottom.
    let base_color = [0.45, 0.45, 0.48, alpha];
    let (base_hw, base_bottom, base_top) = if deploy_fraction <= 0.0 {
        let stowed_height = 0.2 * GRID_SQUARE_SIZE as f32;
        (sp_hw, panel_bottom, panel_bottom + stowed_height)
    } else {
        (sp_hw, panel_bottom, panel_bottom + base_height)
    };
    vertices.push(Vertex::new([sp_x - base_hw, base_bottom], base_color));
    vertices.push(Vertex::new([sp_x + base_hw, base_bottom], base_color));
    vertices.push(Vertex::new([sp_x + base_hw, base_top], base_color));
    vertices.push(Vertex::new([sp_x - base_hw, base_bottom], base_color));
    vertices.push(Vertex::new([sp_x + base_hw, base_top], base_color));
    vertices.push(Vertex::new([sp_x - base_hw, base_top], base_color));

    // Partial sprite (if deploying)
    let f = deploy_fraction as f32;
    if f > 0.0 {
        let full_height = 2.0 * sp_hh;
        let visible_height = base_height + f * (full_height - base_height);
        let quad_top = panel_bottom + visible_height;

        // UV mapping: v_max is the bottom of the sprite in atlas, v_min is top.
        // We show from panel_bottom to quad_top, which maps to v_max down to some v.
        let v_top = rect.v_max - (visible_height / full_height) * (rect.v_max - rect.v_min);
        let v_bottom = rect.v_max;

        let tint = [1.0, 1.0, 1.0, alpha];
        vertices.push(Vertex::sprite([sp_x - sp_hw, panel_bottom], [rect.u_min, v_bottom], tint));
        vertices.push(Vertex::sprite([sp_x + sp_hw, panel_bottom], [rect.u_max, v_bottom], tint));
        vertices.push(Vertex::sprite([sp_x + sp_hw, quad_top], [rect.u_max, v_top], tint));
        vertices.push(Vertex::sprite([sp_x - sp_hw, panel_bottom], [rect.u_min, v_bottom], tint));
        vertices.push(Vertex::sprite([sp_x + sp_hw, quad_top], [rect.u_max, v_top], tint));
        vertices.push(Vertex::sprite([sp_x - sp_hw, quad_top], [rect.u_min, v_top], tint));
    }
}

/// Compute sprite quad placement: (half_w, half_h, x_offset, y_offset) for a given part.
/// Accounts for per-category alignment rules so sprites line up with procedural renderers.
/// For engines, `sprite_rect` provides image pixel dimensions to compute the correct aspect ratio,
/// decoupling the sprite quad size from `flight_hitbox_height` (which is used only for collision).
fn sprite_placement(def: &PartDefinition, sprite_rect: Option<&crate::render::sprites::SpriteRect>) -> (f32, f32, f32, f32) {
    let hitbox_half_w = (def.hitbox_width() / 2.0) as f32;
    let hitbox_half_h = (def.hitbox_height() / 2.0) as f32;
    let visual_half_w = (def.width() / 2.0) as f32;
    let visual_half_h = (def.height() / 2.0) as f32;

    // Engines: use flight hitbox width, compute height from sprite image aspect ratio
    if def.engine.is_some() {
        let sprite_half_w = (def.flight_hitbox_width_m() / 2.0) as f32;
        let sprite_half_h = match sprite_rect {
            Some(rect) if rect.pixel_width > 0 => {
                sprite_half_w * (rect.pixel_height as f32 / rect.pixel_width as f32)
            }
            _ => (def.flight_hitbox_height_m() / 2.0) as f32,
        };
        let y_offset = hitbox_half_h - sprite_half_h;
        return (sprite_half_w, sprite_half_h, 0.0, y_offset);
    }

    // Stack decouplers: hitbox width, visual height, bottom-aligned within hitbox
    if let Some(ref dec) = def.decoupler {
        if !dec.is_radial {
            let y_offset = -(hitbox_half_h - visual_half_h);
            return (hitbox_half_w, visual_half_h, 0.0, y_offset);
        }
    }

    // Heat shields: hitbox width, visual height, top-aligned within hitbox
    if def.is_heat_shield {
        let y_offset = hitbox_half_h - visual_half_h;
        return (hitbox_half_w, visual_half_h, 0.0, y_offset);
    }

    // RCS (standalone, not pods): visual dims, side-offset
    if def.rcs.is_some() && def.category != PartCategory::Pods {
        let is_mirrored = def.rcs.as_ref().map(|r| r.is_mirrored).unwrap_or(false);
        let sign: f32 = if is_mirrored { -1.0 } else { 1.0 };
        let x_offset = sign * (hitbox_half_w - visual_half_w + 0.1);
        return (visual_half_w, visual_half_h, x_offset, 0.0);
    }

    // Default (tanks, pods, radial decouplers, fairings): hitbox dims, centered
    (hitbox_half_w, hitbox_half_h, 0.0, 0.0)
}

/// Generate vertices for the editor grid (as thin quads for triangle rendering)
/// Note: Vertices are output in CAMERA-RELATIVE coordinates (shader expects this)
pub fn generate_grid_vertices(
    editor: &EditorState,
    screen_width: f32,
    screen_height: f32,
) -> Vec<Vertex> {
    let mut vertices = Vec::new();

    let zoom = editor.camera_zoom;
    let cam_x = editor.camera_offset[0] as f32;
    let cam_y = editor.camera_offset[1] as f32;

    // Calculate visible area in world coordinates
    let half_width = (screen_width / 2.0) / zoom;
    let half_height = (screen_height / 2.0) / zoom;

    // World coordinate bounds
    let min_x = cam_x - half_width;
    let max_x = cam_x + half_width;
    let min_y = cam_y - half_height;
    let max_y = cam_y + half_height;

    // Grid spacing (in meters)
    let minor_spacing = 0.5;
    let major_spacing = 2.5;

    // Hide minor lines when zoomed out enough that 20+ major lines fit horizontally
    let aspect_ratio = screen_width / screen_height;
    let visible_world_width = 2.0 * aspect_ratio / zoom;
    let major_lines_visible = visible_world_width / major_spacing;
    let draw_minor = major_lines_visible < 20.0;

    // Line thickness in world units (thinner at higher zoom)
    let line_thickness = 0.005 / zoom.sqrt();

    // Calculate grid line positions
    let start_x = (min_x / minor_spacing).floor() * minor_spacing;
    let start_y = (min_y / minor_spacing).floor() * minor_spacing;

    // Helper to add a line as a quad (two triangles)
    // Input: world coordinates. Output: camera-relative coordinates.
    let mut add_line = |x1: f32, y1: f32, x2: f32, y2: f32, color: [f32; 4]| {
        // Convert to camera-relative coordinates
        let x1 = x1 - cam_x;
        let y1 = y1 - cam_y;
        let x2 = x2 - cam_x;
        let y2 = y2 - cam_y;

        // Calculate perpendicular direction for line thickness
        let dx = x2 - x1;
        let dy = y2 - y1;
        let len = (dx * dx + dy * dy).sqrt();
        if len < 0.0001 {
            return;
        }
        let nx = -dy / len * line_thickness;
        let ny = dx / len * line_thickness;

        // Four corners of the line quad
        let p1 = [x1 - nx, y1 - ny];
        let p2 = [x1 + nx, y1 + ny];
        let p3 = [x2 + nx, y2 + ny];
        let p4 = [x2 - nx, y2 - ny];

        // Triangle 1
        vertices.push(Vertex::new(p1, color));
        vertices.push(Vertex::new(p2, color));
        vertices.push(Vertex::new(p3, color));

        // Triangle 2
        vertices.push(Vertex::new(p1, color));
        vertices.push(Vertex::new(p3, color));
        vertices.push(Vertex::new(p4, color));
    };

    // Vertical grid lines
    let mut x = start_x;
    while x <= max_x {
        let is_major = (x / major_spacing).abs().fract() < 0.01 || x.abs() < 0.01;
        if is_major {
            add_line(x, min_y, x, max_y, GRID_MAJOR_COLOR);
        } else if draw_minor {
            add_line(x, min_y, x, max_y, GRID_COLOR);
        }
        x += minor_spacing;
    }

    // Horizontal grid lines
    let mut y = start_y;
    while y <= max_y {
        let is_major = (y / major_spacing).abs().fract() < 0.01 || y.abs() < 0.01;
        if is_major {
            add_line(min_x, y, max_x, y, GRID_MAJOR_COLOR);
        } else if draw_minor {
            add_line(min_x, y, max_x, y, GRID_COLOR);
        }
        y += minor_spacing;
    }

    vertices
}

/// Generate vertices for placed parts
/// Note: Vertices are output in CAMERA-RELATIVE coordinates
pub fn generate_part_vertices(
    editor: &EditorState,
    part_defs: &PartDefinitions,
    sprite_atlas: Option<&SpriteAtlas>,
) -> Vec<Vertex> {
    let mut vertices = Vec::new();

    let cam_x = editor.camera_offset[0] as f32;
    let cam_y = editor.camera_offset[1] as f32;

    for (id, part) in &editor.parts {
        let Some(def) = part_defs.get(&part.definition_id) else {
            continue;
        };

        let vert_start = vertices.len();

        let is_selected = editor.selected_placed_part == Some(*id)
            || editor.selected_placed_part.and_then(|sel| editor.parts.get(&sel)?.mirror_partner) == Some(*id);
        let is_hovered = editor.hovered_part == Some(*id)
            || editor.hovered_part.and_then(|hov| editor.parts.get(&hov)?.mirror_partner) == Some(*id);
        let is_dragging = editor.dragging_part == Some(*id)
            || editor.dragging_part.and_then(|d| editor.parts.get(&d)?.mirror_partner) == Some(*id);
        let drag_invalid = is_dragging && !editor.drag_valid;

        let half_w = (def.width() / 2.0) as f32;
        let half_h = (def.height() / 2.0) as f32;
        // Convert to camera-relative coordinates
        let x = part.position[0] as f32 - cam_x;
        let y = part.position[1] as f32 - cam_y;
        if let Some(atlas) = sprite_atlas {
            if def.fairing.is_none() {
                if let Some(rect) = atlas.parts.get(&def.id) {
                    // Solar panel / radiator partial deployment in editor
                    if (def.solar_panel.is_some() || def.radiator.is_some()) && !part.deployed {
                        generate_solar_panel_partial(&mut vertices, rect, def, x, y, 0.0, 1.0);
                    } else if def.radiator.is_some() && part.deployed {
                        generate_radiator_partial(&mut vertices, rect, def, x, y, 1.0, 1.0);
                    } else {
                        let (sp_hw, sp_hh, sp_ox, sp_oy) = sprite_placement(def, Some(rect));
                        let sp_x = x + sp_ox;
                        let sp_y = y + sp_oy;
                        generate_sprite_quad(&mut vertices, rect, sp_x, sp_y, sp_hw, sp_hh, [1.0, 1.0, 1.0, 1.0]);

                        // Overlay RCS nozzle bumps on pod sprites
                        if def.category == PartCategory::Pods && def.rcs.is_some() {
                            generate_pod_rcs_nozzles(&mut vertices, def, sp_x, sp_y, 1.0);
                        }
                    }

                    // Draw overlay for selection, hover, or invalid drag
                    if is_selected || is_hovered || drag_invalid {
                        let highlight_color = if drag_invalid {
                            [0.9, 0.2, 0.2, 0.4]
                        } else if is_selected {
                            [0.5, 0.7, 1.0, 0.3]
                        } else {
                            [0.55, 0.55, 0.6, 0.2]
                        };
                        // For engines, use flight hitbox dimensions (matches visible content)
                        let (hl_hw, hl_hh, hl_cy) = if def.engine.is_some() {
                            let fhw = (def.flight_hitbox_width_m() / 2.0) as f32;
                            let fhh = (def.flight_hitbox_height_m() / 2.0) as f32;
                            let offset = ((def.hitbox_height() - def.flight_hitbox_height_m()) / 2.0) as f32;
                            (fhw, fhh, y + offset)
                        } else {
                            (half_w, half_h, y)
                        };
                        vertices.push(Vertex::new([x - hl_hw, hl_cy - hl_hh], highlight_color));
                        vertices.push(Vertex::new([x + hl_hw, hl_cy - hl_hh], highlight_color));
                        vertices.push(Vertex::new([x + hl_hw, hl_cy + hl_hh], highlight_color));
                        vertices.push(Vertex::new([x - hl_hw, hl_cy - hl_hh], highlight_color));
                        vertices.push(Vertex::new([x + hl_hw, hl_cy + hl_hh], highlight_color));
                        vertices.push(Vertex::new([x - hl_hw, hl_cy + hl_hh], highlight_color));
                    }
                    rotate_vertices_around(&mut vertices[vert_start..], x, y, part.rotation);
                    continue;
                }
            }
        }

        // Fairing bases use procedural rendering (shell is deferred to third pass)
        if def.fairing.is_some() {
            let base_alpha = if is_hovered { 0.5 } else { 1.0 };
            generate_fairing_base_details(&mut vertices, def, x, y, base_alpha);

            if is_selected || is_hovered || drag_invalid {
                let highlight_color = if drag_invalid {
                    [0.9, 0.2, 0.2, 0.4]
                } else if is_selected {
                    [0.5, 0.7, 1.0, 0.3]
                } else {
                    [0.55, 0.55, 0.6, 0.2]
                };
                let hitbox_half_h = (def.hitbox_height() / 2.0) as f32;
                vertices.push(Vertex::new([x - half_w, y - hitbox_half_h], highlight_color));
                vertices.push(Vertex::new([x + half_w, y - hitbox_half_h], highlight_color));
                vertices.push(Vertex::new([x + half_w, y + hitbox_half_h], highlight_color));
                vertices.push(Vertex::new([x - half_w, y - hitbox_half_h], highlight_color));
                vertices.push(Vertex::new([x + half_w, y + hitbox_half_h], highlight_color));
                vertices.push(Vertex::new([x - half_w, y + hitbox_half_h], highlight_color));
            }
            rotate_vertices_around(&mut vertices[vert_start..], x, y, part.rotation);
            continue;
        }

        rotate_vertices_around(&mut vertices[vert_start..], x, y, part.rotation);
    }

    // Second pass: draw adapter trapezoids for decouplers
    for (_, part) in &editor.parts {
        let Some(def) = part_defs.get(&part.definition_id) else {
            continue;
        };
        if def.decoupler.is_none() {
            continue;
        }
        let draw_x = part.position[0] as f32 - cam_x;
        let draw_y = part.position[1] as f32 - cam_y;
        let world_x = part.position[0] as f32;
        let world_y = part.position[1] as f32;
        generate_decoupler_adapter(&mut vertices, def, draw_x, draw_y, world_x, world_y, &editor.parts, part_defs, 1.0);
    }

    // Third pass: draw fairing shells on top of all parts (z-ordering)
    for (id, part) in &editor.parts {
        let Some(def) = part_defs.get(&part.definition_id) else {
            continue;
        };
        if def.fairing.is_none() {
            continue;
        }
        if let Some(ref shape) = part.fairing_shape {
            let x = part.position[0] as f32 - cam_x;
            let y = part.position[1] as f32 - cam_y;
            let is_hovered = editor.hovered_part == Some(*id)
                || editor.hovered_part.and_then(|hov| editor.parts.get(&hov)?.mirror_partner) == Some(*id);
            let shell_alpha = if is_hovered { 0.5 } else { 1.0 };
            generate_fairing_shell_vertices(&mut vertices, shape, x, y, def, shell_alpha, None);
        }
    }

    // Fourth pass: draw fairing build preview (in-progress shell)
    if let Some(ref build) = editor.fairing_build_mode {
        if let Some(def) = part_defs.get(
            &editor.parts.get(&build.part_id).map(|p| p.definition_id.clone()).unwrap_or_default()
        ) {
            let base_x = build.base_center_x as f32 - cam_x;
            let base_top_y = build.base_top_y as f32 - cam_y;
            generate_fairing_build_preview(&mut vertices, build, def, base_x, base_top_y);
        }
    }

    vertices
}

/// Generate ghost vertices for a single ghost at the given world position
fn generate_single_ghost_vertices(
    vertices: &mut Vec<Vertex>,
    def: &PartDefinition,
    position: [f64; 2],
    ghost_valid: bool,
    rotation: f64,
    editor: &EditorState,
    _part_defs: &PartDefinitions,
    sprite_atlas: Option<&SpriteAtlas>,
) {
    let vert_start = vertices.len();
    let ghost_alpha = 0.5;

    let half_w = (def.width() / 2.0) as f32;
    let cam_x = editor.camera_offset[0] as f32;
    let cam_y = editor.camera_offset[1] as f32;
    let x = position[0] as f32 - cam_x;
    let y = position[1] as f32 - cam_y;

    // Try sprite-based ghost rendering (skip fairings — they have shell geometry)
    if let Some(atlas) = sprite_atlas {
        if def.fairing.is_none() {
            if let Some(rect) = atlas.parts.get(&def.id) {
                let tint = if ghost_valid {
                    [0.3, 0.9, 0.3, 0.5]
                } else {
                    [0.9, 0.3, 0.3, 0.5]
                };
                let (sp_hw, sp_hh, sp_ox, sp_oy) = sprite_placement(def, Some(rect));
                let sp_x = x + sp_ox;
                let sp_y = y + sp_oy;
                generate_sprite_quad(vertices, rect, sp_x, sp_y, sp_hw, sp_hh, tint);

                // Overlay RCS nozzle bumps on pod sprites
                if def.category == PartCategory::Pods && def.rcs.is_some() {
                    generate_pod_rcs_nozzles(vertices, def, sp_x, sp_y, ghost_alpha);
                }
                rotate_vertices_around(&mut vertices[vert_start..], x, y, rotation);
                return;
            }
        }
    }

    // Fairing bases use procedural rendering (shell geometry handled separately)
    if def.fairing.is_some() {
        generate_fairing_base_details(vertices, def, x, y, ghost_alpha);

        let overlay_color = if ghost_valid {
            [0.3, 0.9, 0.3, 0.25]
        } else {
            [0.9, 0.3, 0.3, 0.25]
        };
        let hitbox_half_h = (def.hitbox_height() / 2.0) as f32;
        vertices.push(Vertex::new([x - half_w, y - hitbox_half_h], overlay_color));
        vertices.push(Vertex::new([x + half_w, y - hitbox_half_h], overlay_color));
        vertices.push(Vertex::new([x + half_w, y + hitbox_half_h], overlay_color));
        vertices.push(Vertex::new([x - half_w, y - hitbox_half_h], overlay_color));
        vertices.push(Vertex::new([x + half_w, y + hitbox_half_h], overlay_color));
        vertices.push(Vertex::new([x - half_w, y + hitbox_half_h], overlay_color));
        rotate_vertices_around(&mut vertices[vert_start..], x, y, rotation);
    }
}

/// Generate vertices for the ghost preview (primary + mirror if applicable)
/// Note: Vertices are output in CAMERA-RELATIVE coordinates
pub fn generate_ghost_vertices(
    editor: &EditorState,
    part_defs: &PartDefinitions,
    sprite_atlas: Option<&SpriteAtlas>,
) -> Vec<Vertex> {
    let mut vertices = Vec::new();

    let Some(ref def_id) = editor.selected_part_def else {
        return vertices;
    };

    let Some(position) = editor.ghost_position else {
        return vertices;
    };

    let Some(def) = part_defs.get(def_id) else {
        return vertices;
    };

    // Render primary ghost
    generate_single_ghost_vertices(&mut vertices, def, position, editor.ghost_valid, editor.ghost_rotation, editor, part_defs, sprite_atlas);

    // Render mirror ghost if applicable, using mirror def if available
    if let Some(mirror_pos) = editor.mirror_ghost_position {
        let mirror_def = editor.mirror_ghost_def_id.as_ref()
            .and_then(|mid| part_defs.get(mid))
            .unwrap_or(def);
        generate_single_ghost_vertices(&mut vertices, mirror_def, mirror_pos, editor.ghost_valid, -editor.ghost_rotation, editor, part_defs, sprite_atlas);
    }

    vertices
}

/// Generate vertices for engine details (nozzle, pipes, rings, turbopump)
/// This draws the detailed engine visuals - replaces the base part shape entirely
pub fn generate_engine_details(
    vertices: &mut Vec<Vertex>,
    def: &PartDefinition,
    x: f32,
    y: f32,
    alpha: f32,
) {
    let Some(ref engine) = def.engine else { return };

    let half_w = (def.width() / 2.0) as f32;
    let half_h = (def.height() / 2.0) as f32;
    let half_top_w = (def.top_width() / 2.0) as f32;

    // Determine engine characteristics for visual style
    // First stage = good sea level performance (ASL thrust > 70% of vacuum)
    let is_first_stage = engine.thrust_asl > engine.thrust_vac * 0.7;
    let has_gimbal = engine.gimbal_range > 2.0;

    // Apply alpha to colors
    let apply_alpha = |c: [f32; 4]| -> [f32; 4] {
        [c[0], c[1], c[2], c[3] * alpha]
    };

    // Choose colors based on engine type
    let nozzle_color = if is_first_stage {
        apply_alpha(ENGINE_NOZZLE_DARK)
    } else {
        apply_alpha(ENGINE_NOZZLE_LIGHT)
    };
    let ring_color = if is_first_stage {
        apply_alpha(ENGINE_RING_DARK)
    } else {
        apply_alpha(ENGINE_RING_LIGHT)
    };
    let chamber_color = apply_alpha(ENGINE_CHAMBER_COLOR);
    let turbopump_color = apply_alpha(ENGINE_TURBOPUMP_COLOR);
    let gas_gen_color = apply_alpha(ENGINE_GAS_GEN_COLOR);

    // Draw main nozzle bell (trapezoid)
    vertices.push(Vertex::new([x - half_w, y - half_h], nozzle_color));
    vertices.push(Vertex::new([x + half_w, y - half_h], nozzle_color));
    vertices.push(Vertex::new([x + half_top_w, y + half_h], nozzle_color));
    vertices.push(Vertex::new([x - half_w, y - half_h], nozzle_color));
    vertices.push(Vertex::new([x + half_top_w, y + half_h], nozzle_color));
    vertices.push(Vertex::new([x - half_top_w, y + half_h], nozzle_color));

    // Engine-specific details based on ID
    generate_engine_specific_details(
        vertices,
        def,
        x, y,
        half_w, half_h, half_top_w,
        is_first_stage, has_gimbal,
        nozzle_color, chamber_color, ring_color, turbopump_color, gas_gen_color, alpha,
    );
}

/// Draw engine-specific details based on the engine ID
fn generate_engine_specific_details(
    vertices: &mut Vec<Vertex>,
    def: &PartDefinition,
    x: f32, y: f32,
    half_w: f32, half_h: f32, half_top_w: f32,
    is_first_stage: bool, has_gimbal: bool,
    _nozzle_color: [f32; 4],
    chamber_color: [f32; 4],
    ring_color: [f32; 4],
    turbopump_color: [f32; 4],
    gas_gen_color: [f32; 4],
    _alpha: f32,
) {
    match def.id.as_str() {
        // TINY ENGINES
        "engine_hummingbird" => {
            // Small methalox probe engine - compact, few rings
            draw_combustion_chamber(vertices, x, y + half_h * 0.7, half_top_w * 0.7, half_h * 0.3, chamber_color);
            draw_nozzle_rings(vertices, x, y, half_w, half_top_w, half_h, 4, ring_color);
        }
        "engine_gecko" => {
            // Kerolox cluster engine - compact with rings
            draw_combustion_chamber(vertices, x, y + half_h * 0.65, half_top_w * 0.8, half_h * 0.35, chamber_color);
            draw_nozzle_rings(vertices, x, y, half_w, half_top_w, half_h, 5, ring_color);
        }
        "engine_firefly" => {
            // Vacuum hydrolox - smooth bell, minimal rings
            draw_combustion_chamber(vertices, x, y + half_h * 0.75, half_top_w * 0.6, half_h * 0.25, chamber_color);
            draw_nozzle_rings(vertices, x, y, half_w, half_top_w, half_h, 2, ring_color);
        }
        // SMALL ENGINES
        "engine_wolf" => {
            // Merlin-style reusable kerolox - many cooling rings
            draw_combustion_chamber(vertices, x, y + half_h * 0.7, half_top_w * 0.9, half_h * 0.3, chamber_color);
            draw_nozzle_rings(vertices, x, y, half_w, half_top_w, half_h, 8, ring_color);
            draw_turbopump_box(vertices, x, y + half_h * 0.95, half_top_w * 0.6, half_h * 0.08, turbopump_color);
        }
        "engine_falcon" => {
            // Raptor-style methalox - compact, ringed
            draw_combustion_chamber(vertices, x, y + half_h * 0.65, half_top_w * 0.85, half_h * 0.35, chamber_color);
            draw_nozzle_rings(vertices, x, y, half_w, half_top_w, half_h, 6, ring_color);
            draw_gimbal_actuators(vertices, x, y + half_h * 0.2, half_top_w * 0.7, half_h * 0.1, ring_color);
        }
        "engine_wren" => {
            // Compact hydrolox upper stage - small chamber, 2 rings
            draw_combustion_chamber(vertices, x, y + half_h * 0.8, half_top_w * 0.6, half_h * 0.2, chamber_color);
            draw_nozzle_rings(vertices, x, y, half_w, half_top_w, half_h, 2, ring_color);
        }
        "engine_owl" => {
            // Vacuum hydrolox - large bell, few rings
            draw_combustion_chamber(vertices, x, y + half_h * 0.8, half_top_w * 0.7, half_h * 0.2, chamber_color);
            draw_nozzle_rings(vertices, x, y, half_w, half_top_w, half_h, 3, ring_color);
        }
        "engine_viper" => {
            // High thrust fixed nozzle - dense rings
            draw_combustion_chamber(vertices, x, y + half_h * 0.7, half_top_w * 0.95, half_h * 0.3, chamber_color);
            draw_nozzle_rings(vertices, x, y, half_w, half_top_w, half_h, 10, ring_color);
            draw_turbopump_box(vertices, x, y + half_h * 0.95, half_top_w * 0.7, half_h * 0.08, turbopump_color);
        }

        // MEDIUM ENGINES
        "engine_bear" => {
            // Methalox workhorse - balanced design
            draw_combustion_chamber(vertices, x, y + half_h * 0.75, half_top_w * 0.85, half_h * 0.25, chamber_color);
            draw_nozzle_rings(vertices, x, y, half_w, half_top_w, half_h, 7, ring_color);
            draw_gimbal_actuators(vertices, x, y + half_h * 0.25, half_top_w * 0.6, half_h * 0.08, ring_color);
            draw_turbopump_box(vertices, x, y + half_h * 0.95, half_top_w * 0.5, half_h * 0.06, turbopump_color);
        }
        "engine_eagle" => {
            // RS-25 heritage - many rings, complex
            draw_combustion_chamber(vertices, x, y + half_h * 0.8, half_top_w * 0.65, half_h * 0.2, chamber_color);
            draw_nozzle_rings(vertices, x, y, half_w, half_top_w, half_h, 12, ring_color);
            draw_gimbal_actuators(vertices, x, y + half_h * 0.3, half_top_w * 0.5, half_h * 0.1, ring_color);
            draw_turbopump_box(vertices, x, y + half_h * 0.95, half_top_w * 0.4, half_h * 0.06, turbopump_color);
        }
        "engine_panther" => {
            // RD-180 twin chamber style - gas generator on side
            draw_combustion_chamber(vertices, x, y + half_h * 0.7, half_top_w * 0.9, half_h * 0.3, chamber_color);
            draw_nozzle_rings(vertices, x, y, half_w, half_top_w, half_h, 6, ring_color);
            draw_gas_generator(vertices, x + half_top_w * 0.9, y + half_h * 0.5, half_top_w * 0.25, half_h * 0.3, gas_gen_color);
        }
        "engine_crane" => {
            // Landing engine - big gimbal
            draw_combustion_chamber(vertices, x, y + half_h * 0.65, half_top_w * 0.8, half_h * 0.35, chamber_color);
            draw_nozzle_rings(vertices, x, y, half_w, half_top_w, half_h, 5, ring_color);
            draw_gimbal_actuators(vertices, x, y + half_h * 0.2, half_top_w * 0.95, half_h * 0.12, ring_color);
        }

        // LARGE ENGINES
        "engine_mammoth" => {
            // F-1 heritage - massive, many rings, gas generator
            draw_combustion_chamber(vertices, x, y + half_h * 0.8, half_top_w * 0.85, half_h * 0.2, chamber_color);
            draw_nozzle_rings(vertices, x, y, half_w, half_top_w, half_h, 14, ring_color);
            draw_turbopump_box(vertices, x, y + half_h * 0.95, half_top_w * 0.6, half_h * 0.06, turbopump_color);
            draw_gas_generator(vertices, x + half_top_w * 0.85, y + half_h * 0.6, half_top_w * 0.3, half_h * 0.25, gas_gen_color);
        }
        "engine_whale" => {
            // RS-68 class hydrolox - large bell, moderate rings
            draw_combustion_chamber(vertices, x, y + half_h * 0.8, half_top_w * 0.7, half_h * 0.2, chamber_color);
            draw_nozzle_rings(vertices, x, y, half_w, half_top_w, half_h, 8, ring_color);
            draw_turbopump_box(vertices, x, y + half_h * 0.95, half_top_w * 0.5, half_h * 0.06, turbopump_color);
        }
        "engine_bison" => {
            // Raptor vacuum style - ringed, gimbaled
            draw_combustion_chamber(vertices, x, y + half_h * 0.75, half_top_w * 0.9, half_h * 0.25, chamber_color);
            draw_nozzle_rings(vertices, x, y, half_w, half_top_w, half_h, 8, ring_color);
            draw_gimbal_actuators(vertices, x, y + half_h * 0.3, half_top_w * 0.85, half_h * 0.1, ring_color);
            draw_turbopump_box(vertices, x, y + half_h * 0.95, half_top_w * 0.65, half_h * 0.06, turbopump_color);
        }
        "engine_titan" => {
            // Large vacuum kerolox - huge bell, many rings
            draw_combustion_chamber(vertices, x, y + half_h * 0.85, half_top_w * 0.6, half_h * 0.15, chamber_color);
            draw_nozzle_rings(vertices, x, y, half_w, half_top_w, half_h, 10, ring_color);
            draw_turbopump_box(vertices, x, y + half_h * 0.95, half_top_w * 0.4, half_h * 0.05, turbopump_color);
        }

        // Fallback for unknown engines
        _ => {
            draw_combustion_chamber(vertices, x, y + half_h * 0.75, half_top_w * 0.8, half_h * 0.25, chamber_color);
            let ring_count = if is_first_stage { 8 } else { 4 };
            draw_nozzle_rings(vertices, x, y, half_w, half_top_w, half_h, ring_count, ring_color);
            if has_gimbal {
                draw_gimbal_actuators(vertices, x, y + half_h * 0.3, half_top_w * 0.6, half_h * 0.1, ring_color);
            }
        }
    }
}

// Helper functions for drawing engine details

/// Draw a rectangular turbopump housing at the top of the engine
fn draw_turbopump_box(
    vertices: &mut Vec<Vertex>,
    x: f32, y: f32,
    half_w: f32, half_h: f32,
    color: [f32; 4],
) {
    // Rectangle centered at (x, y)
    vertices.push(Vertex::new([x - half_w, y - half_h], color));
    vertices.push(Vertex::new([x + half_w, y - half_h], color));
    vertices.push(Vertex::new([x + half_w, y + half_h], color));
    vertices.push(Vertex::new([x - half_w, y - half_h], color));
    vertices.push(Vertex::new([x + half_w, y + half_h], color));
    vertices.push(Vertex::new([x - half_w, y + half_h], color));
}

/// Draw cooling rings/tubes wrapping all around the nozzle (horizontal bands)
fn draw_nozzle_rings(
    vertices: &mut Vec<Vertex>,
    x: f32, y: f32,
    half_w: f32, half_top_w: f32, half_h: f32,
    count: u32,
    color: [f32; 4],
) {
    let ring_thickness = half_h * 0.04;

    for i in 0..count {
        // Position along nozzle (from bottom toward top, but not all the way)
        let t = (i as f32 + 1.0) / (count as f32 + 1.5) * 0.7;
        let y_pos = y - half_h + half_h * 2.0 * t;

        // Interpolate width at this height (nozzle tapers from bottom to top)
        let width_at_y = half_w + (half_top_w - half_w) * t;

        // Draw ring as horizontal band across the entire nozzle width
        vertices.push(Vertex::new([x - width_at_y, y_pos - ring_thickness], color));
        vertices.push(Vertex::new([x + width_at_y, y_pos - ring_thickness], color));
        vertices.push(Vertex::new([x + width_at_y, y_pos + ring_thickness], color));
        vertices.push(Vertex::new([x - width_at_y, y_pos - ring_thickness], color));
        vertices.push(Vertex::new([x + width_at_y, y_pos + ring_thickness], color));
        vertices.push(Vertex::new([x - width_at_y, y_pos + ring_thickness], color));
    }
}

/// Draw gimbal actuator brackets
fn draw_gimbal_actuators(
    vertices: &mut Vec<Vertex>,
    x: f32, y: f32,
    half_w: f32, half_h: f32,
    color: [f32; 4],
) {
    // Left actuator (angled bracket)
    let left_x = x - half_w;
    vertices.push(Vertex::new([left_x, y - half_h], color));
    vertices.push(Vertex::new([left_x - half_w * 0.3, y], color));
    vertices.push(Vertex::new([left_x, y + half_h], color));

    // Right actuator
    let right_x = x + half_w;
    vertices.push(Vertex::new([right_x, y - half_h], color));
    vertices.push(Vertex::new([right_x + half_w * 0.3, y], color));
    vertices.push(Vertex::new([right_x, y + half_h], color));
}

/// Draw the combustion chamber at the top of the engine (rectangular box)
fn draw_combustion_chamber(
    vertices: &mut Vec<Vertex>,
    x: f32, y: f32,
    half_w: f32, half_h: f32,
    color: [f32; 4],
) {
    // Rectangle centered at (x, y)
    vertices.push(Vertex::new([x - half_w, y - half_h], color));
    vertices.push(Vertex::new([x + half_w, y - half_h], color));
    vertices.push(Vertex::new([x + half_w, y + half_h], color));
    vertices.push(Vertex::new([x - half_w, y - half_h], color));
    vertices.push(Vertex::new([x + half_w, y + half_h], color));
    vertices.push(Vertex::new([x - half_w, y + half_h], color));
}

/// Draw a gas generator box on the side of the engine (like F-1 or RD-180)
fn draw_gas_generator(
    vertices: &mut Vec<Vertex>,
    x: f32, y: f32,
    half_w: f32, half_h: f32,
    color: [f32; 4],
) {
    // Rectangle centered at (x, y)
    vertices.push(Vertex::new([x - half_w, y - half_h], color));
    vertices.push(Vertex::new([x + half_w, y - half_h], color));
    vertices.push(Vertex::new([x + half_w, y + half_h], color));
    vertices.push(Vertex::new([x - half_w, y - half_h], color));
    vertices.push(Vertex::new([x + half_w, y + half_h], color));
    vertices.push(Vertex::new([x - half_w, y + half_h], color));
}

/// Generate pod details (dark grey trapezoid with white circle window)
pub fn generate_pod_details(
    vertices: &mut Vec<Vertex>,
    def: &PartDefinition,
    x: f32,
    y: f32,
    alpha: f32,
) {
    let half_w = (def.width() / 2.0) as f32;
    let half_h = (def.height() / 2.0) as f32;
    let half_top_w = (def.top_width() / 2.0) as f32;

    // Apply alpha to colors
    let pod_color = [POD_COLOR[0], POD_COLOR[1], POD_COLOR[2], POD_COLOR[3] * alpha];
    let window_color = [POD_WINDOW_COLOR[0], POD_WINDOW_COLOR[1], POD_WINDOW_COLOR[2], POD_WINDOW_COLOR[3] * alpha];

    // Draw main pod body (trapezoid)
    vertices.push(Vertex::new([x - half_w, y - half_h], pod_color));
    vertices.push(Vertex::new([x + half_w, y - half_h], pod_color));
    vertices.push(Vertex::new([x + half_top_w, y + half_h], pod_color));
    vertices.push(Vertex::new([x - half_w, y - half_h], pod_color));
    vertices.push(Vertex::new([x + half_top_w, y + half_h], pod_color));
    vertices.push(Vertex::new([x - half_top_w, y + half_h], pod_color));

    // Draw white circular window in the center
    // Window size proportional to pod size
    let window_radius = half_w.min(half_h) * 0.25;
    let window_y = y + half_h * 0.2;  // Slightly above center
    draw_circle(vertices, x, window_y, window_radius, window_color);

    // Draw small triangular RCS nozzles if pod has built-in RCS
    if def.rcs.is_some() {
        let nozzle_color = [RCS_NOZZLE_COLOR[0], RCS_NOZZLE_COLOR[1], RCS_NOZZLE_COLOR[2], RCS_NOZZLE_COLOR[3] * alpha];
        // Nozzle position: ~80% up the pod height, at left and right edges
        let nozzle_y = y - half_h + half_h * 2.0 * 0.8;
        // Interpolate pod width at nozzle height (trapezoid shape)
        let t = 0.8; // fraction from bottom to top
        let edge_x = half_w + t * (half_top_w - half_w); // half-width at nozzle height
        let nozzle_hw: f32 = 0.04; // half-width of nozzle base
        let nozzle_len: f32 = 0.08; // length of nozzle tip from base

        // Right nozzle: triangle pointing right, base on pod edge
        let r_base_x = x + edge_x;
        vertices.push(Vertex::new([r_base_x, nozzle_y - nozzle_hw], nozzle_color));
        vertices.push(Vertex::new([r_base_x, nozzle_y + nozzle_hw], nozzle_color));
        vertices.push(Vertex::new([r_base_x + nozzle_len, nozzle_y], nozzle_color));

        // Left nozzle: triangle pointing left, base on pod edge
        let l_base_x = x - edge_x;
        vertices.push(Vertex::new([l_base_x, nozzle_y - nozzle_hw], nozzle_color));
        vertices.push(Vertex::new([l_base_x, nozzle_y + nozzle_hw], nozzle_color));
        vertices.push(Vertex::new([l_base_x - nozzle_len, nozzle_y], nozzle_color));
    }
}

/// Generate just the RCS nozzle bumps for a pod (extracted from generate_pod_details).
/// Used to overlay nozzle bumps on top of pod sprites.
pub fn generate_pod_rcs_nozzles(
    vertices: &mut Vec<Vertex>,
    def: &PartDefinition,
    x: f32,
    y: f32,
    alpha: f32,
) {
    if def.rcs.is_none() { return; }

    let half_w = (def.width() / 2.0) as f32;
    let half_h = (def.height() / 2.0) as f32;
    let half_top_w = (def.top_width() / 2.0) as f32;

    let nozzle_color = [RCS_NOZZLE_COLOR[0], RCS_NOZZLE_COLOR[1], RCS_NOZZLE_COLOR[2], RCS_NOZZLE_COLOR[3] * alpha];
    let nozzle_y = y - half_h + half_h * 2.0 * 0.8;
    let t = 0.8;
    let edge_x = half_w + t * (half_top_w - half_w);
    let nozzle_hw: f32 = 0.04;
    let nozzle_len: f32 = 0.08;

    // Right nozzle
    let r_base_x = x + edge_x;
    vertices.push(Vertex::new([r_base_x, nozzle_y - nozzle_hw], nozzle_color));
    vertices.push(Vertex::new([r_base_x, nozzle_y + nozzle_hw], nozzle_color));
    vertices.push(Vertex::new([r_base_x + nozzle_len, nozzle_y], nozzle_color));

    // Left nozzle
    let l_base_x = x - edge_x;
    vertices.push(Vertex::new([l_base_x, nozzle_y - nozzle_hw], nozzle_color));
    vertices.push(Vertex::new([l_base_x, nozzle_y + nozzle_hw], nozzle_color));
    vertices.push(Vertex::new([l_base_x - nozzle_len, nozzle_y], nozzle_color));
}

/// Generate decoupler ring details (dark horizontal band on bottom half of hitbox)
/// For radial decouplers, renders as a simple dark rectangle instead.
pub fn generate_decoupler_details(
    vertices: &mut Vec<Vertex>,
    def: &PartDefinition,
    x: f32,
    y: f32,
    alpha: f32,
) {
    // Radial decoupler: simple dark rectangle filling the part bounds
    if def.decoupler.as_ref().map(|d| d.is_radial).unwrap_or(false) {
        let color = [0.1, 0.1, 0.1, alpha];
        let half_w = (def.width() / 2.0) as f32;
        let half_h = (def.height() / 2.0) as f32;
        vertices.push(Vertex::new([x - half_w, y - half_h], color));
        vertices.push(Vertex::new([x + half_w, y - half_h], color));
        vertices.push(Vertex::new([x + half_w, y + half_h], color));
        vertices.push(Vertex::new([x - half_w, y - half_h], color));
        vertices.push(Vertex::new([x + half_w, y + half_h], color));
        vertices.push(Vertex::new([x - half_w, y + half_h], color));
        return;
    }

    let half_w = (def.width() / 2.0) as f32;
    let hitbox_half_h = (def.hitbox_height() / 2.0) as f32;
    let visual_h = (def.height()) as f32;

    let ring_color = [DECOUPLER_RING_COLOR[0], DECOUPLER_RING_COLOR[1], DECOUPLER_RING_COLOR[2], DECOUPLER_RING_COLOR[3] * alpha];

    // Ring is drawn on the bottom half of the hitbox
    let ring_bottom = y - hitbox_half_h;
    let ring_top = ring_bottom + visual_h;

    // Two triangles for the ring rectangle
    vertices.push(Vertex::new([x - half_w, ring_bottom], ring_color));
    vertices.push(Vertex::new([x + half_w, ring_bottom], ring_color));
    vertices.push(Vertex::new([x + half_w, ring_top], ring_color));

    vertices.push(Vertex::new([x - half_w, ring_bottom], ring_color));
    vertices.push(Vertex::new([x + half_w, ring_top], ring_color));
    vertices.push(Vertex::new([x - half_w, ring_top], ring_color));
}

/// Generate heat shield details (black ablative face with convex dome, dark backing band)
/// Drawn on the upper half of the hitbox
pub fn generate_heat_shield_details(
    vertices: &mut Vec<Vertex>,
    def: &PartDefinition,
    x: f32,
    y: f32,
    alpha: f32,
) {
    let half_w = (def.width() / 2.0) as f32;
    let hitbox_half_h = (def.hitbox_height() / 2.0) as f32;
    let visual_h = def.height() as f32;

    let face_color = [HEAT_SHIELD_FACE_COLOR[0], HEAT_SHIELD_FACE_COLOR[1], HEAT_SHIELD_FACE_COLOR[2], HEAT_SHIELD_FACE_COLOR[3] * alpha];
    let back_color = [HEAT_SHIELD_BACK_COLOR[0], HEAT_SHIELD_BACK_COLOR[1], HEAT_SHIELD_BACK_COLOR[2], HEAT_SHIELD_BACK_COLOR[3] * alpha];

    // Heat shield is drawn on the upper half of the hitbox
    let shield_top = y + hitbox_half_h;
    let shield_bottom = shield_top - visual_h;

    // Backing structure (top 40%) — flat rectangle
    let back_bottom = shield_top - visual_h * 0.4;
    vertices.push(Vertex::new([x - half_w, back_bottom], back_color));
    vertices.push(Vertex::new([x + half_w, back_bottom], back_color));
    vertices.push(Vertex::new([x + half_w, shield_top], back_color));
    vertices.push(Vertex::new([x - half_w, back_bottom], back_color));
    vertices.push(Vertex::new([x + half_w, shield_top], back_color));
    vertices.push(Vertex::new([x - half_w, shield_top], back_color));

    // Ablative face (bottom 60%) — convex dome with curved bottom edge
    let face_top = back_bottom;
    let face_flat_bottom = shield_bottom;
    let sag = visual_h * 0.3; // How far the dome bulges downward
    let segments = 8;

    for i in 0..segments {
        // Theta spans from -PI/2 to PI/2 across the width
        let theta0 = -std::f32::consts::FRAC_PI_2
            + std::f32::consts::PI * (i as f32) / (segments as f32);
        let theta1 = -std::f32::consts::FRAC_PI_2
            + std::f32::consts::PI * ((i + 1) as f32) / (segments as f32);

        // X positions along the width
        let x0 = x + half_w * theta0.sin();
        let x1 = x + half_w * theta1.sin();

        // Bottom edge Y dips by sag * cos(theta) — deepest at center
        let y0_bot = face_flat_bottom - sag * theta0.cos();
        let y1_bot = face_flat_bottom - sag * theta1.cos();

        // Two triangles: top-left to bottom edge segment
        // Triangle 1: top-left, top-right, bottom-right
        vertices.push(Vertex::new([x0, face_top], face_color));
        vertices.push(Vertex::new([x1, face_top], face_color));
        vertices.push(Vertex::new([x1, y1_bot], face_color));

        // Triangle 2: top-left, bottom-right, bottom-left
        vertices.push(Vertex::new([x0, face_top], face_color));
        vertices.push(Vertex::new([x1, y1_bot], face_color));
        vertices.push(Vertex::new([x0, y0_bot], face_color));
    }
}

// RCS colors
const RCS_BODY_COLOR: [f32; 4] = [0.20, 0.20, 0.22, 1.0];     // Dark grey body
const RCS_NOZZLE_COLOR: [f32; 4] = [0.12, 0.12, 0.14, 1.0];   // Darker nozzle tips

/// Generate RCS thruster details — thin side-mount with 3 directional nozzles.
/// Right-mount (default): body on right side of hitbox, nozzles point left/up/down.
/// Left-mount (is_mirrored): body on left side of hitbox, nozzles point right/up/down.
pub fn generate_rcs_details(
    vertices: &mut Vec<Vertex>,
    def: &PartDefinition,
    x: f32,
    y: f32,
    alpha: f32,
) {
    let is_mirrored = def.rcs.as_ref().map(|r| r.is_mirrored).unwrap_or(false);
    // sign: 1.0 = right-mount (sprite on right side, nozzles face left)
    //       -1.0 = left-mount  (sprite on left side, nozzles face right)
    let sign: f32 = if is_mirrored { -1.0 } else { 1.0 };

    let half_w = (def.width() / 2.0) as f32;   // visual half-width (0.125m for 0.5 grid)
    let half_h = (def.height() / 2.0) as f32;  // visual half-height (0.25m for 1.0 grid)
    let hitbox_half_w = (def.hitbox_width() / 2.0) as f32;  // hitbox half-width (0.25m for 1 grid)

    // Offset visual center to the side of the hitbox
    let cx = x + sign * (hitbox_half_w - half_w);

    let body_color = [RCS_BODY_COLOR[0], RCS_BODY_COLOR[1], RCS_BODY_COLOR[2], RCS_BODY_COLOR[3] * alpha];
    let nozzle_color = [RCS_NOZZLE_COLOR[0], RCS_NOZZLE_COLOR[1], RCS_NOZZLE_COLOR[2], RCS_NOZZLE_COLOR[3] * alpha];

    // Body: rectangle covering 80% of visual extents
    let body_hw = half_w * 0.8;
    let body_hh = half_h * 0.8;
    vertices.push(Vertex::new([cx - body_hw, y - body_hh], body_color));
    vertices.push(Vertex::new([cx + body_hw, y - body_hh], body_color));
    vertices.push(Vertex::new([cx + body_hw, y + body_hh], body_color));
    vertices.push(Vertex::new([cx - body_hw, y - body_hh], body_color));
    vertices.push(Vertex::new([cx + body_hw, y + body_hh], body_color));
    vertices.push(Vertex::new([cx - body_hw, y + body_hh], body_color));

    // Nozzle dimensions
    let nozzle_len = half_h * 0.3;
    let nozzle_hw = half_w * 0.35;  // half-width of nozzle base

    // Lateral nozzle: points away from vessel (left for right-mount, right for left-mount)
    let lateral_base_x = cx - sign * body_hw;
    vertices.push(Vertex::new([lateral_base_x, y - nozzle_hw], nozzle_color));
    vertices.push(Vertex::new([lateral_base_x, y + nozzle_hw], nozzle_color));
    vertices.push(Vertex::new([lateral_base_x - sign * nozzle_len, y], nozzle_color));

    // Top nozzle
    vertices.push(Vertex::new([cx - nozzle_hw, y + body_hh], nozzle_color));
    vertices.push(Vertex::new([cx + nozzle_hw, y + body_hh], nozzle_color));
    vertices.push(Vertex::new([cx, y + body_hh + nozzle_len], nozzle_color));

    // Bottom nozzle
    vertices.push(Vertex::new([cx - nozzle_hw, y - body_hh], nozzle_color));
    vertices.push(Vertex::new([cx + nozzle_hw, y - body_hh], nozzle_color));
    vertices.push(Vertex::new([cx, y - body_hh - nozzle_len], nozzle_color));
}

/// Generate white RCS plume vertices for active nozzles.
/// Called at origin (0,0) in part-local space, like engine plumes.
pub fn generate_rcs_plume_vertices(
    vertices: &mut Vec<Vertex>,
    def: &PartDefinition,
    x: f32,
    y: f32,
    nozzle_state: &crate::render::RcsNozzleState,
) {
    let is_mirrored = def.rcs.as_ref().map(|r| r.is_mirrored).unwrap_or(false);
    let sign: f32 = if is_mirrored { -1.0 } else { 1.0 };

    let half_w = (def.width() / 2.0) as f32;
    let half_h = (def.height() / 2.0) as f32;
    let hitbox_half_w = (def.hitbox_width() / 2.0) as f32;
    let cx = x + sign * (hitbox_half_w - half_w);

    let body_hw = half_w * 0.8;
    let body_hh = half_h * 0.8;
    let nozzle_len = half_h * 0.3;
    let nozzle_hw = half_w * 0.35;

    // Plume dimensions: extends outward from nozzle tip
    let plume_len = nozzle_len * 1.5;
    let plume_hw = nozzle_hw * 0.6;

    let plume_color: [f32; 4] = [0.95, 0.95, 1.0, 0.85];

    // Lateral plume
    if nozzle_state.lateral {
        let tip_x = cx - sign * (body_hw + nozzle_len);
        let end_x = tip_x - sign * plume_len;
        // Rectangle as two triangles
        vertices.push(Vertex::new([tip_x, y - plume_hw], plume_color));
        vertices.push(Vertex::new([tip_x, y + plume_hw], plume_color));
        vertices.push(Vertex::new([end_x, y + plume_hw], plume_color));
        vertices.push(Vertex::new([tip_x, y - plume_hw], plume_color));
        vertices.push(Vertex::new([end_x, y + plume_hw], plume_color));
        vertices.push(Vertex::new([end_x, y - plume_hw], plume_color));
    }

    // Top plume
    if nozzle_state.up {
        let tip_y = y + body_hh + nozzle_len;
        let end_y = tip_y + plume_len;
        vertices.push(Vertex::new([cx - plume_hw, tip_y], plume_color));
        vertices.push(Vertex::new([cx + plume_hw, tip_y], plume_color));
        vertices.push(Vertex::new([cx + plume_hw, end_y], plume_color));
        vertices.push(Vertex::new([cx - plume_hw, tip_y], plume_color));
        vertices.push(Vertex::new([cx + plume_hw, end_y], plume_color));
        vertices.push(Vertex::new([cx - plume_hw, end_y], plume_color));
    }

    // Bottom plume
    if nozzle_state.down {
        let tip_y = y - body_hh - nozzle_len;
        let end_y = tip_y - plume_len;
        vertices.push(Vertex::new([cx - plume_hw, tip_y], plume_color));
        vertices.push(Vertex::new([cx + plume_hw, tip_y], plume_color));
        vertices.push(Vertex::new([cx + plume_hw, end_y], plume_color));
        vertices.push(Vertex::new([cx - plume_hw, tip_y], plume_color));
        vertices.push(Vertex::new([cx + plume_hw, end_y], plume_color));
        vertices.push(Vertex::new([cx - plume_hw, end_y], plume_color));
    }
}

/// Generate RCS plume vertices for pods with built-in bilateral RCS nozzles.
/// Pods have nozzles on both left and right sides near the top.
/// `lateral` = left nozzle fires (exhaust left), `lateral_mirrored` = right nozzle fires (exhaust right).
pub fn generate_pod_rcs_plume_vertices(
    vertices: &mut Vec<Vertex>,
    def: &PartDefinition,
    x: f32,
    y: f32,
    nozzle_state: &crate::render::RcsNozzleState,
) {
    let half_w = (def.width() / 2.0) as f32;
    let half_h = (def.height() / 2.0) as f32;
    let half_top_w = (def.top_width() / 2.0) as f32;

    // Nozzle position matches generate_pod_details: 80% up the pod
    let nozzle_y = y - half_h + half_h * 2.0 * 0.8;
    let t = 0.8;
    let edge_x = half_w + t * (half_top_w - half_w);
    let nozzle_len: f32 = 0.08; // matches nozzle triangle length

    let plume_len: f32 = 0.12;
    let plume_hw: f32 = 0.03;
    let plume_color: [f32; 4] = [0.95, 0.95, 1.0, 0.85];

    // Left nozzle plume (exhaust goes left, from triangle tip)
    if nozzle_state.lateral {
        let tip_x = x - edge_x - nozzle_len;
        let end_x = tip_x - plume_len;
        vertices.push(Vertex::new([tip_x, nozzle_y - plume_hw], plume_color));
        vertices.push(Vertex::new([tip_x, nozzle_y + plume_hw], plume_color));
        vertices.push(Vertex::new([end_x, nozzle_y + plume_hw], plume_color));
        vertices.push(Vertex::new([tip_x, nozzle_y - plume_hw], plume_color));
        vertices.push(Vertex::new([end_x, nozzle_y + plume_hw], plume_color));
        vertices.push(Vertex::new([end_x, nozzle_y - plume_hw], plume_color));
    }

    // Right nozzle plume (exhaust goes right, from triangle tip)
    if nozzle_state.lateral_mirrored {
        let tip_x = x + edge_x + nozzle_len;
        let end_x = tip_x + plume_len;
        vertices.push(Vertex::new([tip_x, nozzle_y - plume_hw], plume_color));
        vertices.push(Vertex::new([tip_x, nozzle_y + plume_hw], plume_color));
        vertices.push(Vertex::new([end_x, nozzle_y + plume_hw], plume_color));
        vertices.push(Vertex::new([tip_x, nozzle_y - plume_hw], plume_color));
        vertices.push(Vertex::new([end_x, nozzle_y + plume_hw], plume_color));
        vertices.push(Vertex::new([end_x, nozzle_y - plume_hw], plume_color));
    }
}

/// Generate adapter trapezoid connecting the closest aligned fuel tank above to a decoupler ring.
/// Draws from two of the tank's bottom vertices to two of the decoupler's top vertices.
/// draw_x/draw_y are camera-relative coords for rendering; world_x/world_y are world coords for adjacency checks.
fn generate_decoupler_adapter(
    vertices: &mut Vec<Vertex>,
    decoupler_def: &PartDefinition,
    draw_x: f32,
    draw_y: f32,
    world_x: f32,
    world_y: f32,
    parts: &std::collections::HashMap<crate::parts::PlacedPartId, crate::parts::PlacedPart>,
    part_defs: &PartDefinitions,
    alpha: f32,
) {
    // No adapter for radial decouplers
    if decoupler_def.decoupler.as_ref().map(|d| d.is_radial).unwrap_or(false) {
        return;
    }
    let decoupler_hitbox_half_h = (decoupler_def.hitbox_height() / 2.0) as f32;
    let decoupler_visual_h = decoupler_def.height() as f32;
    let decoupler_half_w = (decoupler_def.width() / 2.0) as f32;
    let decoupler_ring_top_world = world_y - decoupler_hitbox_half_h + decoupler_visual_h;

    let tolerance = 0.01;

    // Find the closest aligned fuel tank above the decoupler ring
    let mut best_dist = f32::MAX;
    let mut best_tank_bottom_world: f32 = 0.0;
    let mut best_tank_half_w: f32 = 0.0;

    for (_, other_part) in parts {
        let Some(other_def) = part_defs.get(&other_part.definition_id) else {
            continue;
        };

        if other_def.tank.is_none() && other_def.pod.is_none() {
            continue;
        }

        let other_x = other_part.position[0] as f32;
        let other_y = other_part.position[1] as f32;

        // Must be aligned (same center x)
        if (other_x - world_x).abs() > tolerance {
            continue;
        }

        // Tank must be above the decoupler ring top
        let tank_bottom = other_y - (other_def.hitbox_height() / 2.0) as f32;
        if tank_bottom < decoupler_ring_top_world - tolerance {
            continue;
        }

        // Pick closest
        let dist = tank_bottom - decoupler_ring_top_world;
        if dist < best_dist {
            best_dist = dist;
            best_tank_bottom_world = tank_bottom;
            best_tank_half_w = (other_def.width() / 2.0) as f32;
        }
    }

    if best_dist == f32::MAX {
        return;
    }

    // Convert tank bottom from world to camera-relative
    let cam_offset_y = world_y - draw_y;
    let adapter_bottom_draw = draw_y - decoupler_hitbox_half_h + decoupler_visual_h;
    let adapter_top_draw = best_tank_bottom_world - cam_offset_y;

    let adapter_color = [PART_COLOR[0], PART_COLOR[1], PART_COLOR[2], PART_COLOR[3] * alpha];

    // Bottom edge = decoupler ring top (decoupler width), top edge = tank bottom (tank width)
    vertices.push(Vertex::new([draw_x - decoupler_half_w, adapter_bottom_draw], adapter_color));
    vertices.push(Vertex::new([draw_x + decoupler_half_w, adapter_bottom_draw], adapter_color));
    vertices.push(Vertex::new([draw_x + best_tank_half_w, adapter_top_draw], adapter_color));

    vertices.push(Vertex::new([draw_x - decoupler_half_w, adapter_bottom_draw], adapter_color));
    vertices.push(Vertex::new([draw_x + best_tank_half_w, adapter_top_draw], adapter_color));
    vertices.push(Vertex::new([draw_x - best_tank_half_w, adapter_top_draw], adapter_color));

    // Draw detail lines on the adapter surface.
    // Model as a frustum viewed from the side: lines are evenly spaced in angle,
    // so near edges they follow the trapezoid slope and near center they're vertical.
    // 3 lines per grid square of the decoupler's width.
    let num_lines = (decoupler_def.grid_width * 3.0).round() as u32;
    if num_lines == 0 {
        return;
    }
    let line_color = [0.18, 0.18, 0.20, alpha];
    let line_half_thickness = 0.008_f32;

    for i in 0..num_lines {
        // Evenly space in angle across the visible half-cylinder (-π/2 to π/2)
        let theta = -std::f32::consts::FRAC_PI_2
            + std::f32::consts::PI * (i as f32 + 1.0) / (num_lines as f32 + 1.0);
        let s = theta.sin();

        // Bottom x (at decoupler ring top) and top x (at tank bottom)
        let bot_x = draw_x + decoupler_half_w * s;
        let top_x = draw_x + best_tank_half_w * s;

        // Direction vector of the line
        let dx = top_x - bot_x;
        let dy = adapter_top_draw - adapter_bottom_draw;
        let len = (dx * dx + dy * dy).sqrt();
        if len < 0.0001 {
            continue;
        }
        // Perpendicular for thickness
        let nx = -dy / len * line_half_thickness;
        let ny = dx / len * line_half_thickness;

        // Quad for the line
        let bl = [bot_x - nx, adapter_bottom_draw - ny];
        let br = [bot_x + nx, adapter_bottom_draw + ny];
        let tr = [top_x + nx, adapter_top_draw + ny];
        let tl = [top_x - nx, adapter_top_draw - ny];

        vertices.push(Vertex::new(bl, line_color));
        vertices.push(Vertex::new(br, line_color));
        vertices.push(Vertex::new(tr, line_color));

        vertices.push(Vertex::new(bl, line_color));
        vertices.push(Vertex::new(tr, line_color));
        vertices.push(Vertex::new(tl, line_color));
    }
}

/// Generate adapter trapezoid for flight rendering.
/// Works with ShipPartRenderData (local coordinates relative to vessel COM).
pub fn generate_flight_decoupler_adapter(
    vertices: &mut Vec<Vertex>,
    decoupler_def: &PartDefinition,
    dec_x: f32,
    dec_y: f32,
    parts: &[crate::render::ShipPartRenderData],
    part_defs: &PartDefinitions,
    alpha: f32,
) {
    // No adapter for radial decouplers
    if decoupler_def.decoupler.as_ref().map(|d| d.is_radial).unwrap_or(false) {
        return;
    }
    let decoupler_hitbox_half_h = (decoupler_def.hitbox_height() / 2.0) as f32;
    let decoupler_visual_h = decoupler_def.height() as f32;
    let decoupler_half_w = (decoupler_def.width() / 2.0) as f32;
    let decoupler_ring_top = dec_y - decoupler_hitbox_half_h + decoupler_visual_h;

    let tolerance = 0.01;

    // Find the closest aligned fuel tank above the decoupler ring
    let mut best_dist = f32::MAX;
    let mut best_tank_bottom: f32 = 0.0;
    let mut best_tank_half_w: f32 = 0.0;

    for other in parts {
        let Some(other_def) = part_defs.get(&other.definition_id) else {
            continue;
        };

        if other_def.tank.is_none() && other_def.pod.is_none() {
            continue;
        }

        let other_x = other.local_x as f32;
        let other_y = other.local_y as f32;

        // Must be aligned (same center x)
        if (other_x - dec_x).abs() > tolerance {
            continue;
        }

        // Tank must be above the decoupler ring top
        let tank_bottom = other_y - (other_def.hitbox_height() / 2.0) as f32;
        if tank_bottom < decoupler_ring_top - tolerance {
            continue;
        }

        let dist = tank_bottom - decoupler_ring_top;
        if dist < best_dist {
            best_dist = dist;
            best_tank_bottom = tank_bottom;
            best_tank_half_w = (other_def.width() / 2.0) as f32;
        }
    }

    if best_dist == f32::MAX {
        return;
    }

    let adapter_bottom = decoupler_ring_top;
    let adapter_top = best_tank_bottom;

    let adapter_color = [PART_COLOR[0], PART_COLOR[1], PART_COLOR[2], PART_COLOR[3] * alpha];

    // Trapezoid: bottom = decoupler width, top = tank width
    vertices.push(Vertex::new([dec_x - decoupler_half_w, adapter_bottom], adapter_color));
    vertices.push(Vertex::new([dec_x + decoupler_half_w, adapter_bottom], adapter_color));
    vertices.push(Vertex::new([dec_x + best_tank_half_w, adapter_top], adapter_color));

    vertices.push(Vertex::new([dec_x - decoupler_half_w, adapter_bottom], adapter_color));
    vertices.push(Vertex::new([dec_x + best_tank_half_w, adapter_top], adapter_color));
    vertices.push(Vertex::new([dec_x - best_tank_half_w, adapter_top], adapter_color));

    // Detail lines (same frustum projection as editor)
    let num_lines = (decoupler_def.grid_width * 3.0).round() as u32;
    if num_lines == 0 {
        return;
    }
    let line_color = [0.18, 0.18, 0.20, alpha];
    let line_half_thickness = 0.008_f32;

    for i in 0..num_lines {
        let theta = -std::f32::consts::FRAC_PI_2
            + std::f32::consts::PI * (i as f32 + 1.0) / (num_lines as f32 + 1.0);
        let s = theta.sin();

        let bot_x = dec_x + decoupler_half_w * s;
        let top_x = dec_x + best_tank_half_w * s;

        let dx = top_x - bot_x;
        let dy = adapter_top - adapter_bottom;
        let len = (dx * dx + dy * dy).sqrt();
        if len < 0.0001 {
            continue;
        }
        let nx = -dy / len * line_half_thickness;
        let ny = dx / len * line_half_thickness;

        let bl = [bot_x - nx, adapter_bottom - ny];
        let br = [bot_x + nx, adapter_bottom + ny];
        let tr = [top_x + nx, adapter_top + ny];
        let tl = [top_x - nx, adapter_top - ny];

        vertices.push(Vertex::new(bl, line_color));
        vertices.push(Vertex::new(br, line_color));
        vertices.push(Vertex::new(tr, line_color));

        vertices.push(Vertex::new(bl, line_color));
        vertices.push(Vertex::new(tr, line_color));
        vertices.push(Vertex::new(tl, line_color));
    }
}

/// Draw a filled circle
pub fn draw_circle(
    vertices: &mut Vec<Vertex>,
    x: f32, y: f32,
    radius: f32,
    color: [f32; 4],
) {
    let segments = 12;
    for i in 0..segments {
        let a1 = (i as f32 / segments as f32) * std::f32::consts::TAU;
        let a2 = ((i + 1) as f32 / segments as f32) * std::f32::consts::TAU;
        vertices.push(Vertex::new([x, y], color));
        vertices.push(Vertex::new([x + a1.cos() * radius, y + a1.sin() * radius], color));
        vertices.push(Vertex::new([x + a2.cos() * radius, y + a2.sin() * radius], color));
    }
}

/// Generate exhaust plume vertices for a firing engine.
/// Uses sprite plume animation if available, otherwise falls back to procedural triangles.
pub fn generate_engine_plume_vertices(
    vertices: &mut Vec<Vertex>,
    def: &PartDefinition,
    x: f32,
    y: f32,
    throttle: f32,
    sprite_atlas: Option<&SpriteAtlas>,
    plume_elapsed_secs: f64,
) {
    if throttle <= 0.0 {
        return;
    }

    // Nozzle position = bottom of flight hitbox (top-aligned within editor hitbox)
    let editor_half_h = (def.hitbox_height() / 2.0) as f32;
    let flight_h = def.flight_hitbox_height_m() as f32;
    let nozzle_y = y + editor_half_h - flight_h;
    let nozzle_width = def.flight_hitbox_width_m() as f32;
    let half_nozzle = nozzle_width / 2.0;

    // Try sprite plume: engine-specific plume first, then propellant-based fallback
    if let Some(atlas) = sprite_atlas {
        if let Some(ref engine) = def.engine {
            // Try engine-specific plume (e.g., "engine_orion_pulse" → "orion_pulse")
            let engine_plume = def.id.strip_prefix("engine_").and_then(|name| {
                // Some engines share plumes: daedalus_s1/s2 → daedalus, zpinch_probe/advanced → zpinch
                let plume_name = if name.starts_with("daedalus_") { "daedalus" }
                    else if name.starts_with("zpinch_") { "zpinch" }
                    else { name };
                atlas.plumes.get(plume_name)
            });
            let propellant_name = match engine.propellant {
                crate::parts::Propellant::Kerolox => "kerolox",
                crate::parts::Propellant::Methalox => "methalox",
                crate::parts::Propellant::Hydrolox => "hydrolox",
                crate::parts::Propellant::Hydrogen => "hydrolox",
                crate::parts::Propellant::Xenon => "xenon",
                crate::parts::Propellant::FusionFuel => "daedalus",
                crate::parts::Propellant::Antimatter => "amcat",
                crate::parts::Propellant::NuclearPulse => "orion_pulse",
            };
            if let Some(anim) = engine_plume.or_else(|| atlas.plumes.get(propellant_name)) {
                let frame_idx = (plume_elapsed_secs * 10.0) as usize % 4;
                let rect = &anim.frames[frame_idx];

                let brightness = 0.5 + 0.5 * throttle;
                let tint = [brightness, brightness, brightness, 1.0];

                if let Some(ref offsets) = engine.nozzle_offsets {
                    let n = offsets.len() as f32;
                    let per_half_w = half_nozzle / n * 1.2;
                    let per_height = nozzle_width / n * 5.0 * throttle;
                    for &off in offsets {
                        let px = x + off as f32 * GRID_SQUARE_SIZE as f32;
                        let py = nozzle_y - per_height / 2.0;
                        generate_sprite_quad(vertices, rect, px, py, per_half_w, per_height / 2.0, tint);
                    }
                } else {
                    let plume_half_w = half_nozzle * 1.2;
                    let plume_height = nozzle_width * 5.0 * throttle;
                    let plume_center_y = nozzle_y - plume_height / 2.0;
                    generate_sprite_quad(vertices, rect, x, plume_center_y, plume_half_w, plume_height / 2.0, tint);
                }
                return;
            }
        }
    }

    // Procedural fallback
    let offsets: &[f64] = if let Some(ref engine) = def.engine {
        engine.nozzle_offsets.as_deref().unwrap_or(&[0.0])
    } else {
        &[0.0]
    };
    let n = offsets.len() as f32;
    let plume_length = nozzle_width / n * 4.0 * throttle;
    let plume_half_w = half_nozzle / n * 1.2;

    let red = [1.0, 0.2, 0.0, 0.9];
    let yellow = [1.0, 0.9, 0.1, 1.0];
    let inner_half_w = plume_half_w * 0.6;
    let inner_length = plume_length * 0.4;

    for &off in offsets {
        let px = x + off as f32 * GRID_SQUARE_SIZE as f32;
        vertices.push(Vertex::new([px - plume_half_w, nozzle_y], red));
        vertices.push(Vertex::new([px + plume_half_w, nozzle_y], red));
        vertices.push(Vertex::new([px, nozzle_y - plume_length], red));

        vertices.push(Vertex::new([px - inner_half_w, nozzle_y], yellow));
        vertices.push(Vertex::new([px + inner_half_w, nozzle_y], yellow));
        vertices.push(Vertex::new([px, nozzle_y - inner_length], yellow));
    }
}

/// Generate vertices for a single part at the given (x, y) center position.
/// Dispatches to the correct shape/category renderer.
/// Used by both the editor and flight rendering.
pub fn generate_part_shape_vertices(
    vertices: &mut Vec<Vertex>,
    def: &PartDefinition,
    x: f32,
    y: f32,
    alpha: f32,
    sprite_atlas: Option<&SpriteAtlas>,
    deploy_fraction: Option<f64>,
) {
    // Try sprite-based rendering (skip fairings — they have shell geometry)
    if let Some(atlas) = sprite_atlas {
        if def.fairing.is_none() {
            if let Some(rect) = atlas.parts.get(&def.id) {
                // Solar panel partial deployment
                if let Some(frac) = deploy_fraction {
                    if def.solar_panel.is_some() && frac < 1.0 {
                        generate_solar_panel_partial(vertices, rect, def, x, y, frac, alpha);
                        return;
                    }
                }

                // Radiator deployment: sprite stretches horizontally
                if def.radiator.is_some() {
                    let frac = deploy_fraction.unwrap_or(0.0);
                    generate_radiator_partial(vertices, rect, def, x, y, frac, alpha);
                    return;
                }

                let (sp_hw, sp_hh, sp_ox, sp_oy) = sprite_placement(def, Some(rect));
                let sp_x = x + sp_ox;
                let sp_y = y + sp_oy;
                generate_sprite_quad(vertices, rect, sp_x, sp_y, sp_hw, sp_hh, [1.0, 1.0, 1.0, alpha]);

                // Overlay RCS nozzle bumps on pod sprites
                if def.category == PartCategory::Pods && def.rcs.is_some() {
                    generate_pod_rcs_nozzles(vertices, def, sp_x, sp_y, alpha);
                }
                return;
            }
        }
    }

    // Fairing bases use procedural rendering (shell geometry handled separately)
    if def.fairing.is_some() {
        generate_fairing_base_details(vertices, def, x, y, alpha);
    }
}

/// Convert screen coordinates to world coordinates in the editor
pub fn screen_to_world(
    screen_x: f32,
    screen_y: f32,
    screen_width: f32,
    screen_height: f32,
    editor: &EditorState,
) -> [f64; 2] {
    let zoom = editor.camera_zoom;
    let offset = editor.camera_offset;
    let aspect_ratio = screen_width / screen_height;

    // Convert from screen space (0,0 at top-left) to NDC (-1 to 1)
    let ndc_x = (screen_x / screen_width) * 2.0 - 1.0;
    let ndc_y = 1.0 - (screen_y / screen_height) * 2.0; // Flip Y

    // Invert the shader transform: shader does (world * zoom) / aspect for x
    // So world_x = ndc_x * aspect / zoom
    let world_x = (ndc_x * aspect_ratio / zoom) as f64 + offset[0];
    let world_y = (ndc_y / zoom) as f64 + offset[1];

    [world_x, world_y]
}

/// Convert world coordinates to screen coordinates in the editor
pub fn world_to_screen(
    world_x: f64,
    world_y: f64,
    screen_width: f32,
    screen_height: f32,
    editor: &EditorState,
) -> [f32; 2] {
    let zoom = editor.camera_zoom;
    let offset = editor.camera_offset;

    let rel_x = world_x - offset[0];
    let rel_y = world_y - offset[1];

    let screen_x = (rel_x * zoom as f64) as f32 + screen_width / 2.0;
    let screen_y = screen_height / 2.0 - (rel_y * zoom as f64) as f32; // Flip Y

    [screen_x, screen_y]
}

/// Generate fairing base disc details (similar to decoupler ring but lighter)
pub fn generate_fairing_base_details(
    vertices: &mut Vec<Vertex>,
    def: &PartDefinition,
    x: f32,
    y: f32,
    alpha: f32,
) {
    let half_w = (def.width() / 2.0) as f32;
    let hitbox_half_h = (def.hitbox_height() / 2.0) as f32;

    let base_color = [FAIRING_BASE_COLOR[0], FAIRING_BASE_COLOR[1], FAIRING_BASE_COLOR[2], FAIRING_BASE_COLOR[3] * alpha];

    // Base disc fills the full hitbox (1 tile tall)
    let disc_bottom = y - hitbox_half_h;
    let disc_top = y + hitbox_half_h;

    vertices.push(Vertex::new([x - half_w, disc_bottom], base_color));
    vertices.push(Vertex::new([x + half_w, disc_bottom], base_color));
    vertices.push(Vertex::new([x + half_w, disc_top], base_color));

    vertices.push(Vertex::new([x - half_w, disc_bottom], base_color));
    vertices.push(Vertex::new([x + half_w, disc_top], base_color));
    vertices.push(Vertex::new([x - half_w, disc_top], base_color));
}

/// Generate fairing shell vertices from a completed FairingShape.
/// `x, y` is the part center. Shell starts from the base top edge.
/// `fairing_half`: None = both halves, Some(Left) = left only, Some(Right) = right only.
pub fn generate_fairing_shell_vertices(
    vertices: &mut Vec<Vertex>,
    shape: &FairingShape,
    x: f32,
    y: f32,
    def: &PartDefinition,
    alpha: f32,
    fairing_half: Option<crate::parts::FairingHalf>,
) {
    if shape.vertices.is_empty() {
        return;
    }

    let hitbox_half_h = (def.hitbox_height() / 2.0) as f32;
    let base_top = y + hitbox_half_h;  // Top edge of the fairing base hitbox
    let gs = GRID_SQUARE_SIZE as f32;

    let shell_color = [FAIRING_SHELL_COLOR[0], FAIRING_SHELL_COLOR[1], FAIRING_SHELL_COLOR[2], FAIRING_SHELL_COLOR[3] * alpha];
    let line_color = [FAIRING_SHELL_LINE_COLOR[0], FAIRING_SHELL_LINE_COLOR[1], FAIRING_SHELL_LINE_COLOR[2], FAIRING_SHELL_LINE_COLOR[3] * alpha];

    use crate::parts::FairingHalf;
    let draw_left = fairing_half != Some(FairingHalf::Right);
    let draw_right = fairing_half != Some(FairingHalf::Left);

    // Starting point: top corners of the base disc
    let base_half_w = (def.width() / 2.0) as f32;
    let mut prev_half_w = base_half_w;
    let mut prev_y = base_top;

    for &(hw_grid, y_off_grid) in &shape.vertices {
        let hw = hw_grid as f32 * gs;
        let seg_y = base_top + y_off_grid as f32 * gs;

        if draw_left {
            // Left trapezoid half
            vertices.push(Vertex::new([x - prev_half_w, prev_y], shell_color));
            vertices.push(Vertex::new([x, prev_y], shell_color));
            vertices.push(Vertex::new([x, seg_y], shell_color));

            vertices.push(Vertex::new([x - prev_half_w, prev_y], shell_color));
            vertices.push(Vertex::new([x, seg_y], shell_color));
            vertices.push(Vertex::new([x - hw, seg_y], shell_color));
        }

        if draw_right {
            // Right trapezoid half
            vertices.push(Vertex::new([x, prev_y], shell_color));
            vertices.push(Vertex::new([x + prev_half_w, prev_y], shell_color));
            vertices.push(Vertex::new([x + hw, seg_y], shell_color));

            vertices.push(Vertex::new([x, prev_y], shell_color));
            vertices.push(Vertex::new([x + hw, seg_y], shell_color));
            vertices.push(Vertex::new([x, seg_y], shell_color));
        }

        // Horizontal seam line at this vertex
        if hw > 0.001 {
            let line_half_t = 0.008_f32;
            let seam_left = if draw_left { x - hw } else { x };
            let seam_right = if draw_right { x + hw } else { x };
            vertices.push(Vertex::new([seam_left, seg_y - line_half_t], line_color));
            vertices.push(Vertex::new([seam_right, seg_y - line_half_t], line_color));
            vertices.push(Vertex::new([seam_right, seg_y + line_half_t], line_color));
            vertices.push(Vertex::new([seam_left, seg_y - line_half_t], line_color));
            vertices.push(Vertex::new([seam_right, seg_y + line_half_t], line_color));
            vertices.push(Vertex::new([seam_left, seg_y + line_half_t], line_color));
        }

        prev_half_w = hw;
        prev_y = seg_y;
    }

    // Vertical seam line down the center of the shell
    if let Some(last) = shape.vertices.last() {
        let shell_top_y = base_top + last.1 as f32 * gs;
        let line_half_t = 0.008_f32;
        vertices.push(Vertex::new([x - line_half_t, base_top], line_color));
        vertices.push(Vertex::new([x + line_half_t, base_top], line_color));
        vertices.push(Vertex::new([x + line_half_t, shell_top_y], line_color));
        vertices.push(Vertex::new([x - line_half_t, base_top], line_color));
        vertices.push(Vertex::new([x + line_half_t, shell_top_y], line_color));
        vertices.push(Vertex::new([x - line_half_t, shell_top_y], line_color));
    }
}

/// Generate the in-progress fairing build preview (ghost segments + cursor guide)
fn generate_fairing_build_preview(
    vertices: &mut Vec<Vertex>,
    build: &super::FairingBuildState,
    def: &PartDefinition,
    base_x: f32,     // camera-relative x
    base_top_y: f32,  // camera-relative base top y
) {
    let gs = GRID_SQUARE_SIZE as f32;
    let shell_color = [FAIRING_SHELL_COLOR[0], FAIRING_SHELL_COLOR[1], FAIRING_SHELL_COLOR[2], 0.7];

    // Draw completed segments
    let base_half_w = (def.width() / 2.0) as f32;
    let mut prev_half_w = base_half_w;
    let mut prev_y = base_top_y;

    for &(hw_grid, y_off_grid) in &build.vertices {
        let hw = hw_grid as f32 * gs;
        let seg_y = base_top_y + y_off_grid as f32 * gs;

        // Left side
        vertices.push(Vertex::new([base_x - prev_half_w, prev_y], shell_color));
        vertices.push(Vertex::new([base_x, prev_y], shell_color));
        vertices.push(Vertex::new([base_x, seg_y], shell_color));
        vertices.push(Vertex::new([base_x - prev_half_w, prev_y], shell_color));
        vertices.push(Vertex::new([base_x, seg_y], shell_color));
        vertices.push(Vertex::new([base_x - hw, seg_y], shell_color));

        // Right side
        vertices.push(Vertex::new([base_x, prev_y], shell_color));
        vertices.push(Vertex::new([base_x + prev_half_w, prev_y], shell_color));
        vertices.push(Vertex::new([base_x + hw, seg_y], shell_color));
        vertices.push(Vertex::new([base_x, prev_y], shell_color));
        vertices.push(Vertex::new([base_x + hw, seg_y], shell_color));
        vertices.push(Vertex::new([base_x, seg_y], shell_color));

        prev_half_w = hw;
        prev_y = seg_y;
    }

    // Draw ghost segment from last vertex to cursor
    if let Some([gx, gy]) = build.ghost_point {
        let ghost_hw = (gx - build.base_center_x).abs() as f32;
        let ghost_y = gy as f32 - (build.base_top_y as f32 - base_top_y); // Convert to camera-relative

        let ghost_color = if build.ghost_valid {
            [0.3, 0.9, 0.3, 0.3]
        } else {
            [0.9, 0.3, 0.3, 0.3]
        };

        // Left side ghost
        vertices.push(Vertex::new([base_x - prev_half_w, prev_y], ghost_color));
        vertices.push(Vertex::new([base_x, prev_y], ghost_color));
        vertices.push(Vertex::new([base_x, ghost_y], ghost_color));
        vertices.push(Vertex::new([base_x - prev_half_w, prev_y], ghost_color));
        vertices.push(Vertex::new([base_x, ghost_y], ghost_color));
        vertices.push(Vertex::new([base_x - ghost_hw, ghost_y], ghost_color));

        // Right side ghost
        vertices.push(Vertex::new([base_x, prev_y], ghost_color));
        vertices.push(Vertex::new([base_x + prev_half_w, prev_y], ghost_color));
        vertices.push(Vertex::new([base_x + ghost_hw, ghost_y], ghost_color));
        vertices.push(Vertex::new([base_x, prev_y], ghost_color));
        vertices.push(Vertex::new([base_x + ghost_hw, ghost_y], ghost_color));
        vertices.push(Vertex::new([base_x, ghost_y], ghost_color));

        // Ghost point marker (small diamond)
        if build.ghost_valid {
            let marker_size = 0.03_f32;
            let marker_color = [0.3, 0.9, 0.3, 0.8];
            // Right side marker
            let mx = base_x + ghost_hw;
            vertices.push(Vertex::new([mx, ghost_y - marker_size], marker_color));
            vertices.push(Vertex::new([mx + marker_size, ghost_y], marker_color));
            vertices.push(Vertex::new([mx, ghost_y + marker_size], marker_color));
            vertices.push(Vertex::new([mx, ghost_y - marker_size], marker_color));
            vertices.push(Vertex::new([mx, ghost_y + marker_size], marker_color));
            vertices.push(Vertex::new([mx - marker_size, ghost_y], marker_color));
            // Left side marker (mirrored)
            let mx = base_x - ghost_hw;
            vertices.push(Vertex::new([mx, ghost_y - marker_size], marker_color));
            vertices.push(Vertex::new([mx + marker_size, ghost_y], marker_color));
            vertices.push(Vertex::new([mx, ghost_y + marker_size], marker_color));
            vertices.push(Vertex::new([mx, ghost_y - marker_size], marker_color));
            vertices.push(Vertex::new([mx, ghost_y + marker_size], marker_color));
            vertices.push(Vertex::new([mx - marker_size, ghost_y], marker_color));
        }
    }
}

/// Generate fairing shell vertices for flight rendering
/// Similar to generate_flight_decoupler_adapter but for fairing shells
/// `fairing_half`: None = both halves, Some(Left) = left only, Some(Right) = right only.
pub fn generate_flight_fairing_shell(
    vertices: &mut Vec<Vertex>,
    shape: &FairingShape,
    part_x: f32,
    part_y: f32,
    hitbox_half_h: f32,
    base_half_w: f32,
    alpha: f32,
    fairing_half: Option<crate::parts::FairingHalf>,
) {
    if shape.vertices.is_empty() {
        return;
    }

    let base_top = part_y + hitbox_half_h;
    let gs = GRID_SQUARE_SIZE as f32;

    let shell_color = [FAIRING_SHELL_COLOR[0], FAIRING_SHELL_COLOR[1], FAIRING_SHELL_COLOR[2], FAIRING_SHELL_COLOR[3] * alpha];
    let line_color = [FAIRING_SHELL_LINE_COLOR[0], FAIRING_SHELL_LINE_COLOR[1], FAIRING_SHELL_LINE_COLOR[2], FAIRING_SHELL_LINE_COLOR[3] * alpha];

    use crate::parts::FairingHalf;
    let draw_left = fairing_half != Some(FairingHalf::Right);
    let draw_right = fairing_half != Some(FairingHalf::Left);

    let mut prev_half_w = base_half_w;
    let mut prev_y = base_top;

    for &(hw_grid, y_off_grid) in &shape.vertices {
        let hw = hw_grid as f32 * gs;
        let seg_y = base_top + y_off_grid as f32 * gs;

        if draw_left {
            // Left trapezoid half
            vertices.push(Vertex::new([part_x - prev_half_w, prev_y], shell_color));
            vertices.push(Vertex::new([part_x, prev_y], shell_color));
            vertices.push(Vertex::new([part_x, seg_y], shell_color));
            vertices.push(Vertex::new([part_x - prev_half_w, prev_y], shell_color));
            vertices.push(Vertex::new([part_x, seg_y], shell_color));
            vertices.push(Vertex::new([part_x - hw, seg_y], shell_color));
        }

        if draw_right {
            // Right trapezoid half
            vertices.push(Vertex::new([part_x, prev_y], shell_color));
            vertices.push(Vertex::new([part_x + prev_half_w, prev_y], shell_color));
            vertices.push(Vertex::new([part_x + hw, seg_y], shell_color));
            vertices.push(Vertex::new([part_x, prev_y], shell_color));
            vertices.push(Vertex::new([part_x + hw, seg_y], shell_color));
            vertices.push(Vertex::new([part_x, seg_y], shell_color));
        }

        // Horizontal seam
        if hw > 0.001 {
            let lt = 0.008_f32;
            let seam_left = if draw_left { part_x - hw } else { part_x };
            let seam_right = if draw_right { part_x + hw } else { part_x };
            vertices.push(Vertex::new([seam_left, seg_y - lt], line_color));
            vertices.push(Vertex::new([seam_right, seg_y - lt], line_color));
            vertices.push(Vertex::new([seam_right, seg_y + lt], line_color));
            vertices.push(Vertex::new([seam_left, seg_y - lt], line_color));
            vertices.push(Vertex::new([seam_right, seg_y + lt], line_color));
            vertices.push(Vertex::new([seam_left, seg_y + lt], line_color));
        }

        prev_half_w = hw;
        prev_y = seg_y;
    }

    // Vertical center seam
    if let Some(last) = shape.vertices.last() {
        let shell_top_y = base_top + last.1 as f32 * gs;
        let lt = 0.008_f32;
        vertices.push(Vertex::new([part_x - lt, base_top], line_color));
        vertices.push(Vertex::new([part_x + lt, base_top], line_color));
        vertices.push(Vertex::new([part_x + lt, shell_top_y], line_color));
        vertices.push(Vertex::new([part_x - lt, base_top], line_color));
        vertices.push(Vertex::new([part_x + lt, shell_top_y], line_color));
        vertices.push(Vertex::new([part_x - lt, shell_top_y], line_color));
    }
}

/// Find which part (if any) is at the given screen position
/// Uses hitbox dimensions for click detection
pub fn part_at_screen_pos(
    screen_x: f32,
    screen_y: f32,
    screen_width: f32,
    screen_height: f32,
    editor: &EditorState,
    part_defs: &PartDefinitions,
) -> Option<crate::parts::PlacedPartId> {
    let [world_x, world_y] = screen_to_world(screen_x, screen_y, screen_width, screen_height, editor);

    // First pass: check fairing shells (they render on top, so should be hit-tested first)
    for (id, part) in &editor.parts {
        let Some(def) = part_defs.get(&part.definition_id) else { continue };
        if def.fairing.is_none() { continue; }
        let Some(ref shape) = part.fairing_shape else { continue; };
        if shape.vertices.is_empty() { continue; }

        let gs = GRID_SQUARE_SIZE;
        let base_top_y = part.position[1] + def.hitbox_height() / 2.0;
        let base_half_w = def.width() / 2.0;
        let cx = part.position[0];

        // Check if point is within the shell envelope
        let Some(last_vert) = shape.vertices.last() else { continue; };
        let tip_y = base_top_y + last_vert.1 * gs;
        if world_y >= base_top_y && world_y <= tip_y {
            // Interpolate half-width at this y
            let mut prev_hw = base_half_w;
            let mut prev_y = base_top_y;
            for &(hw_grid, y_off_grid) in &shape.vertices {
                let seg_hw = hw_grid * gs;
                let seg_y = base_top_y + y_off_grid * gs;
                if world_y <= seg_y + 0.001 {
                    let span = seg_y - prev_y;
                    let t = if span < 0.001 { 1.0 } else { ((world_y - prev_y) / span).clamp(0.0, 1.0) };
                    let hw_at_y = prev_hw + t * (seg_hw - prev_hw);
                    if (world_x - cx).abs() <= hw_at_y + 0.001 {
                        return Some(*id);
                    }
                    break;
                }
                prev_hw = seg_hw;
                prev_y = seg_y;
            }
        }
    }

    // Second pass: check hitbox rectangles
    for (id, part) in &editor.parts {
        let Some(def) = part_defs.get(&part.definition_id) else {
            continue;
        };

        // Use rotated hitbox dimensions for click detection
        let half_w = def.rotated_hitbox_width(part.rotation) / 2.0;
        let half_h = def.rotated_hitbox_height(part.rotation) / 2.0;

        if world_x >= part.position[0] - half_w
            && world_x <= part.position[0] + half_w
            && world_y >= part.position[1] - half_h
            && world_y <= part.position[1] + half_h
        {
            return Some(*id);
        }
    }

    None
}

/// Generate vertices for a deployed parachute canopy in METER space.
///
/// All output coordinates are in meters relative to the anchor point (0,0).
/// The caller is responsible for scaling and transforming to screen coordinates.
///
/// `retro_x, retro_y`: unit vector pointing in the retrograde direction (canopy opens toward this).
/// `deployed_width_m`: full canopy diameter in meters.
/// `deploy_fraction`: 0.0-1.0 animation progress.
pub fn generate_parachute_canopy_vertices(
    vertices: &mut Vec<Vertex>,
    retro_x: f32,
    retro_y: f32,
    deployed_width_m: f64,
    deploy_fraction: f64,
    visual_width_scale: f32,
) {
    if deploy_fraction < 1e-6 { return; }

    let frac = deploy_fraction as f32;
    let radius = (deployed_width_m * 0.5 * deploy_fraction) as f32 * visual_width_scale;
    let cable_length = (deployed_width_m * 2.0 * deploy_fraction) as f32 * visual_width_scale;

    // Anchor is at origin (0,0). Canopy center is at anchor + retrograde * cable_length.
    let cx = retro_x * cable_length;
    let cy = retro_y * cable_length;

    // Perpendicular axis (rotate retrograde 90 degrees CCW)
    let perp_x = -retro_y;
    let perp_y = retro_x;

    // Draw semicircle canopy as alternating white/orange wedges
    let white: [f32; 4] = [0.90, 0.91, 0.92, 0.85 * frac];
    let orange: [f32; 4] = [0.82, 0.47, 0.16, 0.85 * frac];
    let num_wedges = 12;

    for i in 0..num_wedges {
        let color = if i % 2 == 0 { white } else { orange };
        let a0 = (i as f32 / num_wedges as f32) * std::f32::consts::PI;
        let a1 = ((i + 1) as f32 / num_wedges as f32) * std::f32::consts::PI;

        let dir0_x = -perp_x * a0.cos() + retro_x * a0.sin();
        let dir0_y = -perp_y * a0.cos() + retro_y * a0.sin();
        let dir1_x = -perp_x * a1.cos() + retro_x * a1.sin();
        let dir1_y = -perp_y * a1.cos() + retro_y * a1.sin();

        vertices.push(Vertex::new([cx, cy], color));
        vertices.push(Vertex::new([cx + dir0_x * radius, cy + dir0_y * radius], color));
        vertices.push(Vertex::new([cx + dir1_x * radius, cy + dir1_y * radius], color));
    }

    // Draw cables as quad strips from flat edge to anchor (origin)
    let cable_color: [f32; 4] = [0.9, 0.91, 0.93, 0.9 * frac];
    // Cable width: 0.1% of canopy radius, min 0.015m
    let cable_half_w = (radius * 0.001).max(0.015);
    let num_cables = 5;
    for i in 0..num_cables {
        let frac_pos = i as f32 / (num_cables - 1) as f32;
        let edge_x = cx + perp_x * (frac_pos * 2.0 - 1.0) * radius;
        let edge_y = cy + perp_y * (frac_pos * 2.0 - 1.0) * radius;

        // Direction from edge to anchor (origin)
        let dx = -edge_x;
        let dy = -edge_y;
        let len = (dx * dx + dy * dy).sqrt();
        if len < 0.01 { continue; }
        let nx = -dy / len * cable_half_w;
        let ny = dx / len * cable_half_w;

        // Quad as two triangles
        vertices.push(Vertex::new([edge_x - nx, edge_y - ny], cable_color));
        vertices.push(Vertex::new([edge_x + nx, edge_y + ny], cable_color));
        vertices.push(Vertex::new([nx, ny], cable_color));

        vertices.push(Vertex::new([edge_x - nx, edge_y - ny], cable_color));
        vertices.push(Vertex::new([nx, ny], cable_color));
        vertices.push(Vertex::new([-nx, -ny], cable_color));
    }
}
