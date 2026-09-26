//! Geometry creation utilities for rendering shapes

use super::types::Vertex;

/// Create vertices and indices for a ship triangle
/// The triangle points in the direction of `rotation` (0 = right, PI/2 = up)
pub fn create_ship_triangle(
    cx: f32,
    cy: f32,
    size: f32,
    rotation: f32,
    color: [f32; 4],
) -> (Vec<Vertex>, Vec<u32>) {
    let mut vertices = Vec::with_capacity(3);
    let mut indices = Vec::with_capacity(3);

    // Triangle pointing in direction of rotation
    // Nose (front), and two back corners
    let nose_angle = rotation;
    let back_left_angle = rotation + std::f32::consts::PI * 0.8;
    let back_right_angle = rotation - std::f32::consts::PI * 0.8;

    // Nose vertex (front)
    vertices.push(Vertex::new([
            cx + size * nose_angle.cos(),
            cy + size * nose_angle.sin(),
        ], color));

    // Back left vertex
    vertices.push(Vertex::new([
            cx + size * 0.6 * back_left_angle.cos(),
            cy + size * 0.6 * back_left_angle.sin(),
        ], color));

    // Back right vertex
    vertices.push(Vertex::new([
            cx + size * 0.6 * back_right_angle.cos(),
            cy + size * 0.6 * back_right_angle.sin(),
        ], color));

    // Single triangle
    indices.push(0);
    indices.push(1);
    indices.push(2);

    (vertices, indices)
}

/// Create vertices and indices for a filled circle
pub fn create_circle(
    cx: f32,
    cy: f32,
    radius: f32,
    segments: u32,
    color: [f32; 4],
) -> (Vec<Vertex>, Vec<u32>) {
    let mut vertices = Vec::with_capacity(segments as usize + 1);
    let mut indices = Vec::with_capacity(segments as usize * 3);

    // Center vertex
    vertices.push(Vertex::new([cx, cy], color));

    // Edge vertices
    for i in 0..segments {
        let angle = (i as f32 / segments as f32) * std::f32::consts::TAU;
        let x = cx + radius * angle.cos();
        let y = cy + radius * angle.sin();
        vertices.push(Vertex::new([x, y], color));
    }

    // Triangle fan indices
    for i in 0..segments {
        indices.push(0); // Center
        indices.push(i + 1);
        indices.push((i + 1) % segments + 1);
    }

    (vertices, indices)
}

/// Create vertices and indices for a ring (unfilled circle outline)
pub fn create_ring(
    cx: f32,
    cy: f32,
    radius: f32,
    thickness: f32,
    segments: u32,
    color: [f32; 4],
) -> (Vec<Vertex>, Vec<u32>) {
    let inner_radius = radius - thickness / 2.0;
    let outer_radius = radius + thickness / 2.0;

    let mut vertices = Vec::with_capacity((segments * 2) as usize);
    let mut indices = Vec::with_capacity((segments * 6) as usize);

    for i in 0..segments {
        let angle = (i as f32 / segments as f32) * std::f32::consts::TAU;
        let cos_a = angle.cos();
        let sin_a = angle.sin();

        // Inner vertex
        vertices.push(Vertex::new([cx + inner_radius * cos_a, cy + inner_radius * sin_a], color));
        // Outer vertex
        vertices.push(Vertex::new([cx + outer_radius * cos_a, cy + outer_radius * sin_a], color));
    }

    // Create quads between adjacent pairs
    for i in 0..segments {
        let i0 = i * 2;
        let i1 = i * 2 + 1;
        let i2 = ((i + 1) % segments) * 2;
        let i3 = ((i + 1) % segments) * 2 + 1;

        // Two triangles per quad
        indices.push(i0);
        indices.push(i1);
        indices.push(i2);

        indices.push(i2);
        indices.push(i1);
        indices.push(i3);
    }

    (vertices, indices)
}
