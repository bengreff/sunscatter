use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use super::buildings::BuildingType;

/// A tech tree node definition, loaded from data/tech/tree.ron.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TechNodeData {
    pub id: String,
    pub name: String,
    pub era: u32,
    pub cost: f64,
    pub prerequisites: Vec<String>,
    /// Horizontal position in tech tree graph (0 = leftmost).
    #[serde(default)]
    pub col: u32,
    /// Vertical position in tech tree graph (0 = topmost).
    #[serde(default)]
    pub row: u32,
    #[serde(default)]
    pub unlocks_parts: Vec<String>,
    #[serde(default)]
    pub unlocks_buildings: Vec<BuildingType>,
}

/// An efficiency upgrade line definition, loaded from data/tech/tree.ron.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TechLineData {
    pub id: String,
    pub name: String,
    pub base_cost: f64,
    pub prerequisite_node: String,
    /// Some lines require another line to reach a certain tier first.
    #[serde(default)]
    pub prerequisite_line_tier: Option<(String, u32)>,
    /// Recipe gates: at certain tiers, new recipes become available.
    #[serde(default)]
    pub recipe_gates: Vec<(u32, String)>,
    /// Horizontal position in tech tree graph (0 = leftmost).
    #[serde(default)]
    pub col: u32,
    /// Vertical position in tech tree graph (0 = topmost).
    #[serde(default)]
    pub row: u32,
}

/// Top-level RON file format for the tech tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TechTreeFile {
    pub nodes: Vec<TechNodeData>,
    pub efficiency_lines: Vec<TechLineData>,
    #[serde(default)]
    pub default_unlocked_parts: Vec<String>,
}

/// Runtime tech tree state. Node/line definitions are loaded from the data file;
/// unlock state is persisted in save games.
#[derive(Debug, Clone)]
pub struct TechTree {
    pub nodes: Vec<TechNodeData>,
    pub unlocked: HashSet<String>,
    pub efficiency_lines: Vec<TechLineData>,
    pub line_tiers: HashMap<String, u32>,
    pub default_unlocked_parts: Vec<String>,
}

impl Default for TechTree {
    fn default() -> Self {
        Self {
            nodes: Vec::new(),
            unlocked: HashSet::new(),
            efficiency_lines: Vec::new(),
            line_tiers: HashMap::new(),
            default_unlocked_parts: Vec::new(),
        }
    }
}

impl TechTree {
    /// Load tech tree definitions from a RON file.
    pub fn load(path: &str) -> Result<Self, String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read tech tree file {}: {}", path, e))?;
        Self::load_str(path, &content)
    }

    /// Parse a tech tree RON document already in memory.
    pub fn load_str(source: &str, content: &str) -> Result<Self, String> {
        let file: TechTreeFile = ron::from_str(content)
            .map_err(|e| format!("Failed to parse tech tree {}: {}", source, e))?;
        let mut unlocked = HashSet::new();
        unlocked.insert("basic_rocketry".to_string());
        Ok(Self {
            nodes: file.nodes,
            unlocked,
            efficiency_lines: file.efficiency_lines,
            line_tiers: HashMap::new(),
            default_unlocked_parts: file.default_unlocked_parts,
        })
    }

    /// Load the canonical tech tree. On wasm this uses an embedded RON; on
    /// desktop it reads `data/tech/tree.ron` for fast iteration.
    pub fn load_default() -> Result<Self, String> {
        #[cfg(target_arch = "wasm32")]
        {
            const TREE_RON: &str = include_str!("../../data/tech/tree.ron");
            Self::load_str("embedded:tech/tree.ron", TREE_RON)
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            Self::load("data/tech/tree.ron")
        }
    }

    pub fn is_unlocked(&self, node_id: &str) -> bool {
        self.unlocked.contains(node_id)
    }

    /// Check if a node can be unlocked (all prerequisites met).
    pub fn can_unlock(&self, node_id: &str) -> bool {
        let node = match self.nodes.iter().find(|n| n.id == node_id) {
            Some(n) => n,
            None => return false,
        };
        if self.unlocked.contains(node_id) {
            return false;
        }
        node.prerequisites.iter().all(|pre| self.unlocked.contains(pre))
    }

    /// Unlock a tech node. Returns false if already unlocked or prerequisites not met.
    pub fn unlock(&mut self, node_id: &str) -> bool {
        if !self.can_unlock(node_id) {
            return false;
        }
        self.unlocked.insert(node_id.to_string());
        true
    }

    /// Check if a part is available (either default unlocked or from an unlocked node).
    pub fn is_part_available(&self, part_id: &str) -> bool {
        if self.default_unlocked_parts.iter().any(|p| p == part_id) {
            return true;
        }
        self.nodes.iter().any(|node| {
            self.unlocked.contains(&node.id)
                && node.unlocks_parts.iter().any(|p| p == part_id)
        })
    }

    /// Check if a building type is available (unlocked by some researched node).
    pub fn is_building_available(&self, building: BuildingType) -> bool {
        self.nodes.iter().any(|node| {
            self.unlocked.contains(&node.id)
                && node.unlocks_buildings.contains(&building)
        })
    }

    /// Check if a factory recipe is available based on efficiency line tier gates.
    pub fn is_recipe_available(&self, recipe_id: &str) -> bool {
        for line in &self.efficiency_lines {
            for &(tier, ref gated_recipe) in &line.recipe_gates {
                if gated_recipe == recipe_id {
                    let current_tier = self.line_tier(&line.id);
                    return current_tier >= tier;
                }
            }
        }
        // Recipes without gates are always available (once the base tech is unlocked)
        true
    }

    /// Get the current tier of an efficiency line.
    pub fn line_tier(&self, line_id: &str) -> u32 {
        self.line_tiers.get(line_id).copied().unwrap_or(0)
    }

    /// Cost for the next tier of an efficiency line: base_cost * tier^1.7
    pub fn tier_cost(&self, line_id: &str) -> Option<f64> {
        let line = self.efficiency_lines.iter().find(|l| l.id == line_id)?;
        let current = self.line_tier(line_id);
        if current >= 15 {
            return None; // Max tier
        }
        let next = current + 1;
        Some(line.base_cost * (next as f64).powf(1.7))
    }

    /// Upgrade an efficiency line by one tier. Returns false if at max or prerequisites not met.
    pub fn upgrade_line(&mut self, line_id: &str) -> bool {
        let line = match self.efficiency_lines.iter().find(|l| l.id == line_id) {
            Some(l) => l.clone(),
            None => return false,
        };
        let current = self.line_tier(line_id);
        if current >= 15 {
            return false;
        }
        // Check prerequisite node is unlocked
        if !self.unlocked.contains(&line.prerequisite_node) {
            return false;
        }
        // Check prerequisite line tier (if any)
        if let Some((ref pre_line, pre_tier)) = line.prerequisite_line_tier {
            if self.line_tier(pre_line) < pre_tier {
                return false;
            }
        }
        self.line_tiers.insert(line_id.to_string(), current + 1);
        true
    }

    /// Tier multiplier: 1.11^tier
    pub fn tier_multiplier(tier: u32) -> f64 {
        1.11_f64.powi(tier as i32)
    }

    /// Apply unlock state from a save game.
    pub fn apply_save_state(&mut self, unlocked: HashSet<String>, line_tiers: HashMap<String, u32>) {
        self.unlocked = unlocked;
        self.line_tiers = line_tiers;
        self.grant_patch_unlocks();
    }

    /// Auto-unlock tech nodes that were ADDED after this save was created and
    /// whose prerequisites were already unlocked. Prevents an old save from
    /// stranding the player without newly-added parts. Only listed node IDs
    /// get this treatment, not every prereq-satisfied node — players may
    /// have intentionally skipped some pre-existing nodes.
    fn grant_patch_unlocks(&mut self) {
        // Add a node ID here when introducing a new tech node whose parts
        // should be available to players whose existing saves already meet
        // its prerequisites.
        const PATCH_GRANTS: &[&str] = &["bulk_am_storage"];
        for &grant in PATCH_GRANTS {
            if self.unlocked.contains(grant) {
                continue;
            }
            let Some(node) = self.nodes.iter().find(|n| n.id == grant) else {
                continue;
            };
            if node.prerequisites.iter().all(|p| self.unlocked.contains(p)) {
                self.unlocked.insert(grant.to_string());
            }
        }
    }
}

/// Tracks one-time discovery milestones for science.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DiscoveryTracker {
    pub first_suborbital: bool,
    pub first_orbit: bool,
    pub geostationary: bool,
    /// Body indices where the player has achieved orbit.
    #[serde(default)]
    pub body_orbited: HashSet<usize>,
    /// Body indices where the player has landed.
    #[serde(default)]
    pub body_landed: HashSet<usize>,
    /// Crewed milestones (for government contract payouts).
    #[serde(default)]
    pub first_crewed_orbit: bool,
    #[serde(default)]
    pub first_crewed_lunar: bool,
    #[serde(default)]
    pub first_crewed_mars: bool,
}

/// Science state: tracks available science points and cumulative sources.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ScienceState {
    /// Spendable science points.
    pub available: f64,
    /// Cumulative science earned from one-time discoveries.
    pub cumulative_discovery: f64,
    /// Cumulative science earned from R&D spending.
    pub cumulative_rd: f64,
    /// Cumulative science earned from lab buildings.
    pub cumulative_lab: f64,
    /// Milestone and per-body discovery tracking.
    pub discoveries: DiscoveryTracker,
}

impl Default for ScienceState {
    fn default() -> Self {
        Self {
            available: 0.0,
            cumulative_discovery: 0.0,
            cumulative_rd: 0.0,
            cumulative_lab: 0.0,
            discoveries: DiscoveryTracker::default(),
        }
    }
}
