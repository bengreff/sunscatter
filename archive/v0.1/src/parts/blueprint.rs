use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use super::FuelType;
use crate::colony::ContractPayload;

/// Shape of a user-constructed fairing shell
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FairingShape {
    /// Each entry: (half_width_grid, y_offset_grid) — in grid squares from base top center
    pub vertices: Vec<(f64, f64)>,
    pub closed: bool,  // true if terminated with a center-line point (triangle tip)
}

/// Which half of a fairing shell to render (for debris)
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum FairingHalf {
    Left,
    Right,
}

/// A unique identifier for a placed part in a blueprint
pub type PlacedPartId = u64;

/// A saveable vessel blueprint (stored in RON files)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct VesselBlueprint {
    pub name: String,
    pub parts: Vec<BlueprintPart>,
    pub root_part_index: usize,
    pub stages: Vec<Vec<usize>>,
}

impl Default for VesselBlueprint {
    fn default() -> Self {
        Self {
            name: String::new(),
            parts: Vec::new(),
            root_part_index: 0,
            stages: Vec::new(),
        }
    }
}

impl VesselBlueprint {
    pub fn new(name: String) -> Self {
        Self {
            name,
            parts: Vec::new(),
            root_part_index: 0,
            stages: Vec::new(),
        }
    }

    /// Validate the blueprint for launch readiness
    pub fn validate(&self) -> Result<(), String> {
        if self.parts.is_empty() {
            return Err("Blueprint has no parts".to_string());
        }

        if self.root_part_index >= self.parts.len() {
            return Err("Invalid root part index".to_string());
        }

        // Check that all parent indices are valid
        for (i, part) in self.parts.iter().enumerate() {
            if let Some(parent_idx) = part.parent_index {
                if parent_idx >= self.parts.len() {
                    return Err(format!("Part {} has invalid parent index", i));
                }
            }
        }

        Ok(())
    }

    /// Calculate the full vessel cost in dollars (material breakdown + loaded fuel).
    /// Mirrors `EditorState::calculate_vessel_cost()`.
    pub fn calculate_cost(&self, part_defs: &super::PartDefinitions) -> f64 {
        use crate::colony::economy::{part_dry_earth_cost, part_filled_fuel_cost};

        self.parts.iter()
            .filter_map(|part| {
                let def = part_defs.get(&part.definition_id)?;
                Some(part_dry_earth_cost(def)
                    + part_filled_fuel_cost(def, part.fuel_type, part.fill_fraction))
            })
            .sum()
    }

    /// Get the root part's definition ID
    pub fn root_part_id(&self) -> Option<&str> {
        self.parts.get(self.root_part_index).map(|p| p.definition_id.as_str())
    }
}

/// A placed part within a blueprint
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct BlueprintPart {
    pub definition_id: String,
    pub position: [f64; 2],
    pub rotation: f64,
    pub parent_index: Option<usize>,
    pub attachment_type: AttachmentType,
    pub stage: u32,
    #[serde(default)]
    pub fuel_type: FuelType,
    #[serde(default)]
    pub tank_filled: bool,
    #[serde(default)]
    pub fill_fraction: f64,
    #[serde(default)]
    pub crossfeed_enabled: bool,
    #[serde(default)]
    pub mirror_partner_index: Option<usize>,
    #[serde(default)]
    pub fairing_shape: Option<FairingShape>,
    #[serde(default = "default_true")]
    pub deployed: bool,
    #[serde(default)]
    pub cargo_resources: Vec<(String, f64)>,  // (resource_name, kg)
    #[serde(default)]
    pub cargo_buildings: Vec<String>,  // BuildingType display names
    #[serde(default)]
    pub cargo_payloads: Vec<ContractPayload>,  // Contract payloads
}

impl Default for BlueprintPart {
    fn default() -> Self {
        Self {
            definition_id: String::new(),
            position: [0.0, 0.0],
            rotation: 0.0,
            parent_index: None,
            attachment_type: AttachmentType::Root,
            stage: 0,
            fuel_type: FuelType::default(),
            tank_filled: false,
            fill_fraction: 0.0,
            crossfeed_enabled: false,
            mirror_partner_index: None,
            fairing_shape: None,
            deployed: true,
            cargo_resources: Vec::new(),
            cargo_buildings: Vec::new(),
            cargo_payloads: Vec::new(),
        }
    }
}

fn default_true() -> bool { true }

/// How a part is attached to its parent
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AttachmentType {
    Root,           // No parent (command pod)
    Stack,          // Top-bottom stack attachment
    Radial,         // Radial mount (side attachment)
}

/// A placed part in the editor (runtime representation)
#[derive(Debug, Clone)]
pub struct PlacedPart {
    pub id: PlacedPartId,
    pub definition_id: String,
    pub position: [f64; 2],
    pub rotation: f64,
    pub parent_id: Option<PlacedPartId>,
    pub children: Vec<PlacedPartId>,
    pub attachment_type: AttachmentType,
    pub stage: u32,
    // Tank-specific state
    pub fuel_type: FuelType,  // What fuel type is loaded (Empty = no fuel)
    pub fill_fraction: f64,   // How full the tank is (0.0 = empty, 1.0 = full)
    // Decoupler-specific state
    pub crossfeed_enabled: bool, // Whether fuel can flow through this decoupler
    // Mirror symmetry link
    pub mirror_partner: Option<PlacedPartId>,
    // Fairing shell shape (if this is a fairing base)
    pub fairing_shape: Option<FairingShape>,
    // Solar panel deploy state (true = extended, false = retracted)
    pub deployed: bool,
    // Cargo container manifest
    pub cargo_resources: Vec<(String, f64)>,  // (resource_name, kg)
    pub cargo_buildings: Vec<String>,  // BuildingType display names
    pub cargo_payloads: Vec<ContractPayload>,  // Contract payloads
}

impl PlacedPart {
    pub fn new(
        id: PlacedPartId,
        definition_id: String,
        position: [f64; 2],
    ) -> Self {
        Self {
            id,
            definition_id,
            position,
            rotation: 0.0,
            parent_id: None,
            children: Vec::new(),
            attachment_type: AttachmentType::Root,
            stage: 0,
            fuel_type: FuelType::Empty,
            fill_fraction: 0.0,
            crossfeed_enabled: false,
            mirror_partner: None,
            fairing_shape: None,
            deployed: true,
            cargo_resources: Vec::new(),
            cargo_buildings: Vec::new(),
            cargo_payloads: Vec::new(),
        }
    }

    pub fn with_parent(mut self, parent_id: PlacedPartId, attachment: AttachmentType) -> Self {
        self.parent_id = Some(parent_id);
        self.attachment_type = attachment;
        self
    }
}

/// Symmetry mode for placing parts
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SymmetryMode {
    #[default]
    Off,
    Mirror,
}

impl SymmetryMode {
    pub fn count(&self) -> usize {
        match self {
            SymmetryMode::Off => 1,
            SymmetryMode::Mirror => 2,
        }
    }

    pub fn cycle_next(&self) -> SymmetryMode {
        match self {
            SymmetryMode::Off => SymmetryMode::Mirror,
            SymmetryMode::Mirror => SymmetryMode::Off,
        }
    }

    pub fn display(&self) -> &'static str {
        match self {
            SymmetryMode::Off => "Off",
            SymmetryMode::Mirror => "Mirror",
        }
    }
}

/// Convert placed parts to a saveable blueprint
pub fn parts_to_blueprint(
    parts: &HashMap<PlacedPartId, PlacedPart>,
    root_id: PlacedPartId,
    name: String,
    stages: &[Vec<PlacedPartId>],
) -> VesselBlueprint {
    // Build index mapping from PlacedPartId to blueprint index.
    // Sort by PlacedPartId first so blueprint output is deterministic
    // (HashMap iteration order is arbitrary and differs per run).
    let mut sorted_parts: Vec<(&PlacedPartId, &PlacedPart)> = parts.iter().collect();
    sorted_parts.sort_by_key(|(id, _)| **id);

    let mut id_to_index: HashMap<PlacedPartId, usize> = HashMap::new();
    let mut blueprint_parts: Vec<BlueprintPart> = Vec::new();
    let mut root_index = 0;

    // First pass: assign indices
    for (idx, (id, _)) in sorted_parts.iter().enumerate() {
        id_to_index.insert(**id, idx);
        if **id == root_id {
            root_index = idx;
        }
    }

    // Second pass: convert parts
    for (_id, part) in sorted_parts.iter() {
        let parent_index = part.parent_id.and_then(|pid| id_to_index.get(&pid).copied());

        let mirror_partner_index = part.mirror_partner.and_then(|pid| id_to_index.get(&pid).copied());

        blueprint_parts.push(BlueprintPart {
            definition_id: part.definition_id.clone(),
            position: part.position,
            rotation: part.rotation,
            parent_index,
            attachment_type: part.attachment_type,
            stage: part.stage,
            fuel_type: part.fuel_type,
            tank_filled: part.fill_fraction > 0.0,
            fill_fraction: part.fill_fraction,
            crossfeed_enabled: part.crossfeed_enabled,
            mirror_partner_index,
            fairing_shape: part.fairing_shape.clone(),
            deployed: part.deployed,
            cargo_resources: part.cargo_resources.clone(),
            cargo_buildings: part.cargo_buildings.clone(),
            cargo_payloads: part.cargo_payloads.clone(),
        });
    }

    // Convert stages
    let blueprint_stages: Vec<Vec<usize>> = stages
        .iter()
        .map(|stage| {
            stage
                .iter()
                .filter_map(|id| id_to_index.get(id).copied())
                .collect()
        })
        .collect();

    VesselBlueprint {
        name,
        parts: blueprint_parts,
        root_part_index: root_index,
        stages: blueprint_stages,
    }
}

/// Convert a blueprint back to placed parts for editing
pub fn blueprint_to_parts(
    blueprint: &VesselBlueprint,
) -> (HashMap<PlacedPartId, PlacedPart>, PlacedPartId, Vec<Vec<PlacedPartId>>) {
    let mut parts: HashMap<PlacedPartId, PlacedPart> = HashMap::new();
    let mut index_to_id: HashMap<usize, PlacedPartId> = HashMap::new();

    // First pass: create parts with IDs
    for (idx, bp_part) in blueprint.parts.iter().enumerate() {
        let id = idx as PlacedPartId + 1; // Start IDs from 1
        index_to_id.insert(idx, id);

        let mut part = PlacedPart::new(id, bp_part.definition_id.clone(), bp_part.position);
        part.rotation = bp_part.rotation;
        part.attachment_type = bp_part.attachment_type;
        part.stage = bp_part.stage;
        part.fuel_type = bp_part.fuel_type;
        part.fill_fraction = if bp_part.fill_fraction > 0.0 {
            bp_part.fill_fraction
        } else if bp_part.tank_filled {
            1.0 // old blueprints: filled = 100%
        } else {
            0.0
        };
        part.crossfeed_enabled = bp_part.crossfeed_enabled;
        part.fairing_shape = bp_part.fairing_shape.clone();
        part.deployed = bp_part.deployed;
        part.cargo_resources = bp_part.cargo_resources.clone();
        part.cargo_buildings = bp_part.cargo_buildings.clone();
        part.cargo_payloads = bp_part.cargo_payloads.clone();
        parts.insert(id, part);
    }

    // Second pass: link parents, children, and mirror partners
    for (idx, bp_part) in blueprint.parts.iter().enumerate() {
        let id = index_to_id[&idx];

        if let Some(parent_idx) = bp_part.parent_index {
            let parent_id = index_to_id[&parent_idx];

            if let Some(part) = parts.get_mut(&id) {
                part.parent_id = Some(parent_id);
            }
            if let Some(parent) = parts.get_mut(&parent_id) {
                parent.children.push(id);
            }
        }

        if let Some(mirror_idx) = bp_part.mirror_partner_index {
            if let Some(&mirror_id) = index_to_id.get(&mirror_idx) {
                if let Some(part) = parts.get_mut(&id) {
                    part.mirror_partner = Some(mirror_id);
                }
            }
        }
    }

    let root_id = index_to_id[&blueprint.root_part_index];

    // Convert stages
    let stages: Vec<Vec<PlacedPartId>> = blueprint
        .stages
        .iter()
        .map(|stage| stage.iter().filter_map(|idx| index_to_id.get(idx).copied()).collect())
        .collect();

    (parts, root_id, stages)
}
