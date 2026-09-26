mod common;

use sunscatter_app::render::{create_ship_triangle, create_circle, create_ring};

#[test]
fn ship_triangle_3_vertices() {
    let (vertices, indices) = create_ship_triangle(0.0, 0.0, 1.0, 0.0, [1.0, 1.0, 1.0, 1.0]);
    assert_eq!(
        vertices.len(),
        3,
        "Ship triangle should have exactly 3 vertices, got {}",
        vertices.len()
    );
    assert_eq!(
        indices.len(),
        3,
        "Ship triangle should have exactly 3 indices, got {}",
        indices.len()
    );
}

#[test]
fn ship_triangle_indices_valid() {
    let (vertices, indices) = create_ship_triangle(0.0, 0.0, 1.0, 0.0, [1.0, 1.0, 1.0, 1.0]);
    let max_index = vertices.len() as u32;
    for (i, &idx) in indices.iter().enumerate() {
        assert!(
            idx < max_index,
            "Ship triangle index [{}] = {} is out of bounds (vertex count = {})",
            i,
            idx,
            vertices.len()
        );
    }
}

#[test]
fn circle_vertex_count() {
    for segments in [4u32, 16, 32] {
        let (vertices, indices) = create_circle(0.0, 0.0, 1.0, segments, [1.0; 4]);
        let expected_vertices = segments as usize + 1;
        let expected_indices = segments as usize * 3;
        assert_eq!(
            vertices.len(),
            expected_vertices,
            "Circle with {} segments should have {} vertices (center + edge), got {}",
            segments,
            expected_vertices,
            vertices.len()
        );
        assert_eq!(
            indices.len(),
            expected_indices,
            "Circle with {} segments should have {} indices (3 per triangle fan), got {}",
            segments,
            expected_indices,
            indices.len()
        );
    }
}

#[test]
fn circle_indices_valid() {
    let segments = 32u32;
    let (vertices, indices) = create_circle(0.0, 0.0, 1.0, segments, [1.0; 4]);
    let max_index = vertices.len() as u32;
    for (i, &idx) in indices.iter().enumerate() {
        assert!(
            idx < max_index,
            "Circle index [{}] = {} is out of bounds (vertex count = {}, segments = {})",
            i,
            idx,
            vertices.len(),
            segments
        );
    }
}

#[test]
fn circle_center_position() {
    let (vertices, _indices) = create_circle(5.0, 7.0, 1.0, 8, [1.0; 4]);
    assert_eq!(
        vertices[0].position,
        [5.0, 7.0],
        "First vertex (center) should be at the specified center position (5.0, 7.0), got {:?}",
        vertices[0].position
    );
}

#[test]
fn ring_vertex_count() {
    for segments in [4u32, 16, 32] {
        let (vertices, indices) = create_ring(0.0, 0.0, 1.0, 0.1, segments, [1.0; 4]);
        let expected_vertices = segments as usize * 2;
        let expected_indices = segments as usize * 6;
        assert_eq!(
            vertices.len(),
            expected_vertices,
            "Ring with {} segments should have {} vertices (inner + outer per segment), got {}",
            segments,
            expected_vertices,
            vertices.len()
        );
        assert_eq!(
            indices.len(),
            expected_indices,
            "Ring with {} segments should have {} indices (6 per quad), got {}",
            segments,
            expected_indices,
            indices.len()
        );
    }
}

#[test]
fn ring_indices_valid() {
    let segments = 32u32;
    let (vertices, indices) = create_ring(0.0, 0.0, 1.0, 0.1, segments, [1.0; 4]);
    let max_index = vertices.len() as u32;
    for (i, &idx) in indices.iter().enumerate() {
        assert!(
            idx < max_index,
            "Ring index [{}] = {} is out of bounds (vertex count = {}, segments = {})",
            i,
            idx,
            vertices.len(),
            segments
        );
    }
}
