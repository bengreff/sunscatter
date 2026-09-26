use std::collections::{BTreeMap, HashMap};

use crate::colony::{BuildingType, Colony, ColonyManager, FactoryRecipe, FleetManager, ResourceType, StoredShipId, TechTree};
use super::types::TradeAction;

// ============================================================
// Format helpers
// ============================================================

/// Format mass in kg: use "kg" for < 1000, "t" for >= 1000 kg.
fn format_colony_mass(kg: f64) -> String {
    if kg >= 1000.0 {
        format!("{:.2} t", kg / 1000.0)
    } else {
        format!("{:.1} kg", kg)
    }
}

/// Format energy in joules with SI prefixes.
fn format_energy_j(j: f64) -> String {
    if j.abs() >= 1e15 {
        format!("{:.1} PJ", j / 1e15)
    } else if j.abs() >= 1e12 {
        format!("{:.1} TJ", j / 1e12)
    } else if j.abs() >= 1e9 {
        format!("{:.1} GJ", j / 1e9)
    } else {
        format!("{:.1} MJ", j / 1e6)
    }
}

/// Format power in kW with SI prefixes.
pub(super) fn format_power_kw(kw: f64) -> String {
    if kw.abs() >= 1_000_000_000.0 {
        format!("{:.1} TW", kw / 1_000_000_000.0)
    } else if kw.abs() >= 1_000_000.0 {
        format!("{:.1} GW", kw / 1_000_000.0)
    } else if kw.abs() >= 1_000.0 {
        format!("{:.1} MW", kw / 1_000.0)
    } else {
        format!("{:.1} kW", kw)
    }
}

/// Format power in watts with SI prefixes.
pub(super) fn format_power_w(w: f64) -> String {
    if w.abs() >= 1e15 {
        format!("{:.2} PW", w / 1e15)
    } else if w.abs() >= 1e12 {
        format!("{:.1} TW", w / 1e12)
    } else if w.abs() >= 1e9 {
        format!("{:.1} GW", w / 1e9)
    } else if w.abs() >= 1e6 {
        format!("{:.1} MW", w / 1e6)
    } else {
        format!("{:.0} W", w)
    }
}

/// Format a production/consumption rate in kg/day with sign prefix.
fn format_rate(kg_per_day: f64) -> String {
    let abs = kg_per_day.abs();
    let sign = if kg_per_day >= 0.0 { "+" } else { "\u{2212}" };
    if abs >= 1000.0 {
        format!("{}{:.1} t/day", sign, abs / 1000.0)
    } else if abs >= 0.1 {
        format!("{}{:.1} kg/day", sign, abs)
    } else if abs > 0.001 {
        format!("{}{:.2} kg/day", sign, abs)
    } else {
        String::new()
    }
}

// ============================================================
// Color constants
// ============================================================

pub(super) const COLOR_GREEN: egui::Color32 = egui::Color32::from_rgb(100, 255, 100);
const COLOR_RED: egui::Color32 = egui::Color32::from_rgb(255, 100, 100);
pub(super) const COLOR_YELLOW: egui::Color32 = egui::Color32::from_rgb(220, 200, 80);
pub(super) const COLOR_ORANGE: egui::Color32 = egui::Color32::from_rgb(255, 150, 100);
const COLOR_DEG_YELLOW: egui::Color32 = egui::Color32::from_rgb(255, 180, 60);
pub(super) const COLOR_GRAY: egui::Color32 = egui::Color32::from_rgb(160, 160, 160);
pub(super) const CARD_BG: egui::Color32 = egui::Color32::from_rgba_premultiplied(30, 35, 50, 220);

// ============================================================
// UI helpers
// ============================================================

pub(super) fn card_frame() -> egui::Frame {
    egui::Frame::none()
        .fill(CARD_BG)
        .inner_margin(egui::Margin::same(12.0))
        .rounding(egui::Rounding::same(6.0))
        .outer_margin(egui::Margin::symmetric(0.0, 4.0))
}

pub(super) fn section_heading(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .size(14.0)
            .strong()
            .color(egui::Color32::WHITE),
    );
    ui.add_space(4.0);
}


// ============================================================
// Constants
// ============================================================

/// All building types available for construction via the UI.
const BUILDABLE_BUILDINGS: &[BuildingType] = &[
    BuildingType::Habitat,
    BuildingType::BasicGreenhouse,
    BuildingType::AdvancedGreenhouse,
    BuildingType::SmallSolarFarm,
    BuildingType::MediumSolarFarm,
    BuildingType::LargeSolarFarm,
    BuildingType::FissionReactor,
    BuildingType::Mine,
    BuildingType::Factory,
    BuildingType::LightConstructionRobot,
    BuildingType::ConstructionRobot,
    BuildingType::ScienceLab,
    BuildingType::Stockpile,
    BuildingType::FoodStorage,
    BuildingType::Hangar,
    BuildingType::MassDriverMk1,
    BuildingType::MassDriverMk2,
    BuildingType::MassDriverMk3,
    BuildingType::MassDriverMk4,
    BuildingType::ReceiverArray,
];

/// All factory recipes for the assignment UI.
const ALL_RECIPES: &[FactoryRecipe] = &[
    FactoryRecipe::MetalSmelting,
    FactoryRecipe::AlloyForging,
    FactoryRecipe::ElectronicsManufacturing,
    FactoryRecipe::SuperconductorFabrication,
    FactoryRecipe::PrecisionInstrumentsManufacturing,
    FactoryRecipe::Electrolysis,
    FactoryRecipe::DeuteriumExtraction,
    FactoryRecipe::SabatierReaction,
    FactoryRecipe::MethanePurification,
    FactoryRecipe::KeroseneRefining,
    FactoryRecipe::UraniumEnrichment,
    FactoryRecipe::TritiumBreeding,
    FactoryRecipe::NpuAssembly,
    FactoryRecipe::RegolithHe3Extraction,
    FactoryRecipe::GasGiantHe3Separation,
    FactoryRecipe::MirrorSegmentAssembly,
    FactoryRecipe::CollectorStationAssembly,
];

// ============================================================
// Actions
// ============================================================

/// Actions returned by the colony screen.
#[derive(Debug, Clone, PartialEq)]
pub enum ColonyScreenAction {
    None,
    QueueBuilding(usize, BuildingType, u32),
    AddMineAssignment(usize, ResourceType, u32),
    RemoveMineAssignment(usize, ResourceType, u32),
    AddCollectorAssignment(usize, ResourceType, u32),
    RemoveCollectorAssignment(usize, ResourceType, u32),
    AddFactoryAssignment(usize, FactoryRecipe, u32),
    RemoveFactoryAssignment(usize, FactoryRecipe, u32),
    ReturnToFlight,
    GoToTrackingStation,
    GoToMainMenu,
    GoToColonyOverview,
    ChangeWarp(usize),
    SwitchColony(usize),
    DebugAddResource(usize, ResourceType, f64),
    DebugAddBuilding(usize, BuildingType),
    DebugAddCrew(usize, u32),
    ScrapShip(usize, StoredShipId),
    Trade(TradeAction),
    SetStorageAllocation { body_index: usize, resource: ResourceType, percent: f64 },
    UnpinStorageAllocation { body_index: usize, resource: ResourceType },
}

// ============================================================
// Pre-computed resource rates
// ============================================================

struct ResourceRates {
    production: HashMap<ResourceType, f64>,
    consumption: HashMap<ResourceType, f64>,
}

fn compute_resource_rates(colony: &Colony, hab_mult: f64, body_radius_m: f64, tech_tree: &TechTree) -> ResourceRates {
    let mut production: HashMap<ResourceType, f64> = HashMap::new();
    let mut consumption: HashMap<ResourceType, f64> = HashMap::new();

    let mining_mult = TechTree::tier_multiplier(tech_tree.line_tier("mining"));
    let atmo_mult = TechTree::tier_multiplier(tech_tree.line_tier("atmospheric_science"));
    let agri_mult = TechTree::tier_multiplier(tech_tree.line_tier("agriculture"));
    let life_support_mult = TechTree::tier_multiplier(tech_tree.line_tier("life_support"));

    for b in &colony.buildings {
        if !b.operational {
            continue;
        }

        // Mine production
        if b.building_type == BuildingType::Mine {
            if let Some(res) = b.assigned_resource {
                let rate = 2000.0 * (1.0 - b.degradation) * colony.other_power_fraction * mining_mult;
                *production.entry(res).or_insert(0.0) += rate;
            }
        }

        // Atmospheric Collector production
        if b.building_type == BuildingType::AtmosphericCollector {
            if let Some(res) = b.assigned_resource {
                let rate = 10_000.0 * (1.0 - b.degradation) * colony.other_power_fraction * atmo_mult;
                *production.entry(res).or_insert(0.0) += rate;
            }
        }

        // Factory production/consumption
        if b.building_type == BuildingType::Factory {
            if let Some(recipe) = b.assigned_recipe {
                let batches_per_day = 24.0 / recipe.batch_time_hours();
                let factory_mult = TechTree::tier_multiplier(tech_tree.line_tier(recipe.efficiency_line_id()));
                let factor =
                    batches_per_day * (1.0 - b.degradation) * colony.other_power_fraction * factory_mult;
                for &(res, amt) in &recipe.outputs() {
                    *production.entry(res).or_insert(0.0) += amt * factor;
                }
                for &(res, amt) in &recipe.inputs() {
                    *consumption.entry(res).or_insert(0.0) += amt * factor;
                }
            }
        }

        // Greenhouse food production
        match b.building_type {
            BuildingType::BasicGreenhouse => {
                let max_water = 2_000.0;
                let rate = 0.5
                    * (b.water_fill / max_water).min(1.0)
                    * (1.0 - b.degradation)
                    * colony.other_power_fraction
                    * agri_mult;
                *production.entry(ResourceType::Food).or_insert(0.0) += rate;
            }
            BuildingType::AdvancedGreenhouse => {
                let max_water = 5_000.0;
                let rate = 2.5
                    * (b.water_fill / max_water).min(1.0)
                    * (1.0 - b.degradation)
                    * colony.other_power_fraction
                    * agri_mult;
                *production.entry(ResourceType::Food).or_insert(0.0) += rate;
            }
            _ => {}
        }
    }

    // Maintenance consumption (matches process_maintenance in simulation.rs)
    for b in &colony.buildings {
        let costs = b.building_type.maintenance_cost_per_30d();
        let hab = if b.building_type.affected_by_habitability() {
            hab_mult / life_support_mult
        } else {
            1.0
        };
        let mult = hab * b.building_type.size_multiplier(body_radius_m);
        for &(res, amt) in &costs {
            *consumption.entry(res).or_insert(0.0) += amt * mult / 30.0;
        }
    }

    // Reactor fuel consumption
    for b in &colony.buildings {
        if !b.operational {
            continue;
        }
        match b.building_type {
            BuildingType::FissionReactor => {
                *consumption
                    .entry(ResourceType::EnrichedUranium)
                    .or_insert(0.0) += 0.5;
            }
            BuildingType::FusionReactor => {
                *consumption.entry(ResourceType::Helium3).or_insert(0.0) += 3.0;
                *consumption.entry(ResourceType::Deuterium).or_insert(0.0) += 2.0;
            }
            _ => {}
        }
    }

    // Food consumption by crew
    if colony.crew > 0 {
        *consumption.entry(ResourceType::Food).or_insert(0.0) += 0.5 * colony.crew as f64;
    }

    ResourceRates {
        production,
        consumption,
    }
}

// ============================================================
// Section renderers
// ============================================================

fn render_overview_card(ui: &mut egui::Ui, colony: &Colony, body_name: &str, tech_tree: &TechTree) {
    card_frame().show(ui, |ui| {
        section_heading(ui, &colony.name);
        let location = if colony.is_orbital_station {
            format!("Orbital station, {}", body_name)
        } else {
            format!("Surface, {}", body_name)
        };
        ui.label(egui::RichText::new(location).size(12.0).color(COLOR_GRAY));

        ui.add_space(4.0);
        ui.separator();
        ui.add_space(4.0);

        egui::Grid::new("overview_grid")
            .num_columns(2)
            .spacing([40.0, 4.0])
            .show(ui, |ui| {
                // Crew
                let crew_cap = colony.crew_capacity();
                ui.label(egui::RichText::new("Crew").size(12.0));
                if crew_cap > 0 {
                    ui.label(
                        egui::RichText::new(format!("{} / {}", colony.crew, crew_cap)).size(12.0),
                    );
                } else {
                    ui.label(egui::RichText::new(format!("{}", colony.crew)).size(12.0));
                }
                ui.end_row();

                // Robots (crew needed)
                let robot_count: u32 = colony
                    .buildings
                    .iter()
                    .filter(|b| {
                        b.operational
                            && matches!(
                                b.building_type,
                                crate::colony::BuildingType::LightConstructionRobot
                                    | crate::colony::BuildingType::ConstructionRobot
                            )
                    })
                    .count() as u32;
                if robot_count > 0 {
                    ui.label(egui::RichText::new("Robots").size(12.0));
                    ui.label(
                        egui::RichText::new(format!("{} ({} crew)", robot_count, robot_count))
                            .size(12.0),
                    );
                    ui.end_row();
                }

                // Food
                let food_days = colony.food_days_remaining();
                let food_cap = colony.food_capacity();
                let food_value = if food_days.is_infinite() {
                    if food_cap > 0.0 {
                        format!(
                            "{} / {} (no crew)",
                            format_colony_mass(colony.food_stored),
                            format_colony_mass(food_cap)
                        )
                    } else {
                        format!("{} (no crew)", format_colony_mass(colony.food_stored))
                    }
                } else if food_cap > 0.0 {
                    format!(
                        "{} / {} ({:.1} days)",
                        format_colony_mass(colony.food_stored),
                        format_colony_mass(food_cap),
                        food_days
                    )
                } else {
                    format!(
                        "{} ({:.1} days)",
                        format_colony_mass(colony.food_stored),
                        food_days
                    )
                };
                let food_color = if food_days < 10.0 && !food_days.is_infinite() {
                    COLOR_RED
                } else if food_days < 30.0 && !food_days.is_infinite() {
                    COLOR_YELLOW
                } else {
                    egui::Color32::WHITE
                };
                ui.label(egui::RichText::new("Food").size(12.0));
                ui.label(egui::RichText::new(food_value).size(12.0).color(food_color));
                ui.end_row();

                // Storage
                let sail_tier = tech_tree.line_tier("sail_technology");
                let storage_used = colony.resources.total_storage_mass(sail_tier);
                let storage_cap = colony.storage_capacity();
                ui.label(egui::RichText::new("Storage").size(12.0));
                ui.label(
                    egui::RichText::new(format!(
                        "{} / {}",
                        format_colony_mass(storage_used),
                        format_colony_mass(storage_cap)
                    ))
                    .size(12.0),
                );
                ui.end_row();

                // Power
                let power_net = colony.power_generated - colony.power_consumed;
                let power_color = if power_net < 0.0 {
                    COLOR_RED
                } else {
                    COLOR_GREEN
                };
                ui.label(egui::RichText::new("Power").size(12.0));
                let mut power_text = format!(
                    "{}{}",
                    if power_net >= 0.0 { "+" } else { "" },
                    format_power_kw(power_net),
                );
                if colony.habitat_power_fraction < 1.0 && colony.crew > 0 {
                    power_text.push_str("  (CREW AT RISK)");
                }
                ui.label(egui::RichText::new(power_text).size(12.0).color(power_color));
                ui.end_row();
            });

        // Crisis alert
        if let Some(crisis_crew) = colony.crew_at_crisis_start {
            ui.add_space(4.0);
            ui.separator();
            ui.add_space(4.0);
            let deaths_per_day = crisis_crew as f64 * 0.005;
            let reason = match (
                colony.food_stored <= 0.0,
                colony.habitat_power_fraction < 1.0,
            ) {
                (true, true) => "food + power shortage",
                (true, false) => "food shortage",
                (false, true) => "power shortage",
                (false, false) => "crisis",
            };
            ui.label(
                egui::RichText::new(format!(
                    "CRISIS: ~{:.1} crew/day ({})",
                    deaths_per_day, reason
                ))
                .size(12.0)
                .strong()
                .color(COLOR_RED),
            );
        }
    });
}

fn render_power_card(ui: &mut egui::Ui, colony: &Colony, solar_power_factor: f64) {
    card_frame().show(ui, |ui| {
        section_heading(ui, "Power");

        // Net power summary (most important)
        let power_surplus = colony.power_generated - colony.power_consumed;
        let net_color = if power_surplus < 0.0 {
            COLOR_RED
        } else {
            COLOR_GREEN
        };
        ui.label(
            egui::RichText::new(format!(
                "Net: {}{}",
                if power_surplus >= 0.0 { "+" } else { "" },
                format_power_kw(power_surplus),
            ))
            .size(12.0)
            .strong()
            .color(net_color),
        );

        // Allocation warnings inline
        ui.horizontal(|ui| {
            if colony.habitat_power_fraction < 1.0 {
                ui.label(
                    egui::RichText::new(format!(
                        "Habitat: {:.0}%",
                        colony.habitat_power_fraction * 100.0
                    ))
                    .size(12.0)
                    .color(COLOR_RED),
                );
                ui.label(
                    egui::RichText::new("\u{2014} CREW AT RISK")
                        .size(12.0)
                        .strong()
                        .color(COLOR_RED),
                );
            } else if colony.crew > 0 {
                ui.label(
                    egui::RichText::new(format!(
                        "Habitat: {:.0}%",
                        colony.habitat_power_fraction * 100.0
                    ))
                    .size(12.0),
                );
            }

            if colony.other_power_fraction < 1.0 {
                if colony.crew > 0 {
                    ui.label(egui::RichText::new("  |  ").size(12.0).color(COLOR_GRAY));
                }
                let color = if colony.other_power_fraction < 0.5 {
                    COLOR_RED
                } else {
                    COLOR_YELLOW
                };
                ui.label(
                    egui::RichText::new(format!(
                        "Buildings: {:.0}%",
                        colony.other_power_fraction * 100.0
                    ))
                    .size(12.0)
                    .color(color),
                );
            }
        });

        ui.add_space(4.0);
        ui.separator();
        ui.add_space(4.0);

        // Combined production + demand grid
        // Power production by building type
        let mut prod_map: BTreeMap<&str, (u32, f64)> = BTreeMap::new();
        let mut receiver_count = 0u32;
        for b in &colony.buildings {
            if !b.operational {
                continue;
            }
            // Skip receivers — their power comes from colony.receiver_power_kw
            if b.building_type == BuildingType::ReceiverArray {
                receiver_count += 1;
                continue;
            }
            let output = b.building_type.power_output_kw();
            if output <= 0.0 {
                continue;
            }
            let is_solar = matches!(
                b.building_type,
                BuildingType::SmallSolarFarm
                    | BuildingType::MediumSolarFarm
                    | BuildingType::LargeSolarFarm
            );
            let actual = if is_solar {
                output * solar_power_factor * (1.0 - b.degradation)
            } else {
                output * (1.0 - b.degradation)
            };
            let entry = prod_map
                .entry(b.building_type.display_name())
                .or_insert((0, 0.0));
            entry.0 += 1;
            entry.1 += actual;
        }
        // Add receiver power from simulation (accounts for laser availability + degradation)
        if receiver_count > 0 {
            prod_map.insert(
                BuildingType::ReceiverArray.display_name(),
                (receiver_count, colony.receiver_power_kw),
            );
        }

        // Power demand by building type + factory recipe
        let mut demand_entries: Vec<(String, u32, f64)> = Vec::new();
        let mut demand_map: BTreeMap<String, usize> = BTreeMap::new();

        for b in &colony.buildings {
            if !b.operational {
                continue;
            }
            let draw = b.building_type.power_draw_kw();
            if draw > 0.0 {
                let key = b.building_type.display_name().to_string();
                if let Some(&idx) = demand_map.get(&key) {
                    demand_entries[idx].1 += 1;
                    demand_entries[idx].2 += draw;
                } else {
                    let idx = demand_entries.len();
                    demand_map.insert(key.clone(), idx);
                    demand_entries.push((key, 1, draw));
                }
            }
            if b.building_type == BuildingType::Factory {
                if let Some(recipe) = b.assigned_recipe {
                    let key = format!("Factory \u{2014} {}", recipe.display_name());
                    if let Some(&idx) = demand_map.get(&key) {
                        demand_entries[idx].1 += 1;
                        demand_entries[idx].2 += recipe.power_draw_kw();
                    } else {
                        let idx = demand_entries.len();
                        demand_map.insert(key.clone(), idx);
                        demand_entries.push((key, 1, recipe.power_draw_kw()));
                    }
                }
            }
        }

        let has_entries = !prod_map.is_empty() || !demand_entries.is_empty();
        if has_entries {
            egui::Grid::new("power_grid")
                .striped(true)
                .num_columns(3)
                .min_col_width(100.0)
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("Source").size(11.0).strong());
                    ui.label(egui::RichText::new("Count").size(11.0).strong());
                    ui.label(egui::RichText::new("Output").size(11.0).strong());
                    ui.end_row();

                    let receiver_name = BuildingType::ReceiverArray.display_name();
                    for (name, (count, total_kw)) in &prod_map {
                        ui.label(egui::RichText::new(*name).size(11.0));
                        ui.label(egui::RichText::new(format!("{}x", count)).size(11.0));
                        let output_resp = ui.label(
                            egui::RichText::new(format!("+{}", format_power_kw(*total_kw)))
                                .size(11.0)
                                .color(COLOR_GREEN),
                        );
                        // Receiver saturation tooltip
                        if *name == receiver_name && colony.receiver_laser_power_kw > 0.0 {
                            let laser_kw = colony.receiver_laser_power_kw;
                            let receiver_cap_kw: f64 = colony.buildings.iter()
                                .filter(|b| b.operational && b.building_type == BuildingType::ReceiverArray)
                                .map(|b| (1.0 - b.degradation) * crate::colony::dyson_swarm::MAX_RECEIVER_INPUT_W / 1000.0
                                    * crate::colony::dyson_swarm::RECEIVER_EFFICIENCY)
                                .sum();
                            let tooltip = if laser_kw < receiver_cap_kw {
                                format!(
                                    "Laser-limited: {} available, {} receiver capacity",
                                    format_power_kw(laser_kw * crate::colony::dyson_swarm::RECEIVER_EFFICIENCY),
                                    format_power_kw(receiver_cap_kw),
                                )
                            } else {
                                format!(
                                    "Receiver-limited: {} capacity / {} laser",
                                    format_power_kw(receiver_cap_kw),
                                    format_power_kw(laser_kw * crate::colony::dyson_swarm::RECEIVER_EFFICIENCY),
                                )
                            };
                            output_resp.on_hover_text(tooltip);
                        }
                        ui.end_row();
                    }

                    if !prod_map.is_empty() && !demand_entries.is_empty() {
                        ui.separator();
                        ui.separator();
                        ui.separator();
                        ui.end_row();
                    }

                    for (name, count, total_kw) in &demand_entries {
                        ui.label(egui::RichText::new(name).size(11.0));
                        ui.label(egui::RichText::new(format!("{}x", count)).size(11.0));
                        ui.label(
                            egui::RichText::new(format!(
                                "\u{2212}{}",
                                format_power_kw(*total_kw)
                            ))
                            .size(11.0)
                            .color(COLOR_ORANGE),
                        );
                        ui.end_row();
                    }
                });
        } else {
            ui.label(egui::RichText::new("No power infrastructure.").size(11.0).color(COLOR_GRAY));
        }
    });
}

fn render_buildings_card(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    colony: &Colony,
    body_index: usize,
    body_mineable: &[Vec<ResourceType>],
    body_atmospheric: &[Vec<ResourceType>],
    action: &mut ColonyScreenAction,
    tech_tree: &crate::colony::TechTree,
) {
    card_frame().show(ui, |ui| {
        section_heading(ui, "Buildings");

        // Batch size selector
        let batch_id = egui::Id::new("colony_batch_size");
        let mut batch_size: u32 = ctx.data_mut(|d| *d.get_temp_mut_or(batch_id, 1u32));
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Assign:").size(11.0));
            for &(label, val) in
                &[("1", 1u32), ("10", 10), ("100", 100), ("1000", 1000), ("All", 0)]
            {
                if ui
                    .selectable_label(batch_size == val, egui::RichText::new(label).size(11.0))
                    .clicked()
                {
                    batch_size = val;
                }
            }
        });
        ctx.data_mut(|d| d.insert_temp(batch_id, batch_size));

        if colony.buildings.is_empty() {
            ui.label(
                egui::RichText::new("No buildings.")
                    .size(12.0)
                    .color(COLOR_GRAY),
            );
            return;
        }

        ui.add_space(4.0);

        // Building counts
        let mut counts: BTreeMap<&str, (BuildingType, u32)> = BTreeMap::new();
        for b in &colony.buildings {
            let entry = counts
                .entry(b.building_type.display_name())
                .or_insert((b.building_type, 0));
            entry.1 += 1;
        }

        egui::Grid::new("building_counts_grid")
            .num_columns(2)
            .spacing([40.0, 2.0])
            .show(ui, |ui| {
                for (_name, (bt, count)) in &counts {
                    if *bt == BuildingType::Mine
                        || *bt == BuildingType::Factory
                        || *bt == BuildingType::AtmosphericCollector
                    {
                        continue;
                    }
                    ui.label(egui::RichText::new(bt.display_name()).size(11.0));
                    ui.label(egui::RichText::new(format!("{}x", count)).size(11.0));
                    ui.end_row();
                }
            });

        // Mines sub-section
        let mine_count = colony
            .buildings
            .iter()
            .filter(|b| b.building_type == BuildingType::Mine)
            .count();

        if mine_count > 0 {
            ui.add_space(4.0);
            ui.separator();
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(format!("Mines ({}x)", mine_count))
                    .size(13.0)
                    .strong()
                    .color(egui::Color32::WHITE),
            );
            ui.add_space(2.0);

            let mineable = body_mineable
                .get(body_index)
                .cloned()
                .unwrap_or_default();

            let mining_mult = TechTree::tier_multiplier(tech_tree.line_tier("mining"));
            let mut assigned_counts: HashMap<ResourceType, u32> = HashMap::new();
            let mut mine_rates: HashMap<ResourceType, f64> = HashMap::new();
            let mut unassigned = 0u32;
            for b in &colony.buildings {
                if b.building_type != BuildingType::Mine {
                    continue;
                }
                match b.assigned_resource {
                    Some(res) => {
                        *assigned_counts.entry(res).or_insert(0) += 1;
                        if b.operational {
                            *mine_rates.entry(res).or_insert(0.0) +=
                                2000.0 * (1.0 - b.degradation) * colony.other_power_fraction * mining_mult;
                        }
                    }
                    None => unassigned += 1,
                }
            }

            egui::Grid::new("mine_assign_grid")
                .min_col_width(0.0)
                .spacing(egui::vec2(4.0, 2.0))
                .show(ui, |ui| {
                    for &res in &mineable {
                        let count = assigned_counts.get(&res).copied().unwrap_or(0);
                        if count == 0 && unassigned == 0 {
                            continue;
                        }
                        ui.add_space(16.0);
                        let label = if count > 0 {
                            let rate = mine_rates.get(&res).copied().unwrap_or(0.0);
                            format!("{}: {} ({}/d)", res.display_name(), count, format_colony_mass(rate))
                        } else {
                            format!("{}: {}", res.display_name(), count)
                        };
                        ui.add_sized(
                            [220.0, ui.spacing().interact_size.y],
                            egui::Label::new(
                                egui::RichText::new(label).size(11.0),
                            ),
                        );
                        if count > 0 {
                            if ui.small_button("\u{2212}").clicked() {
                                let n = if batch_size == 0 {
                                    count
                                } else {
                                    batch_size.min(count)
                                };
                                *action =
                                    ColonyScreenAction::RemoveMineAssignment(body_index, res, n);
                            }
                        } else {
                            ui.label("");
                        }
                        if unassigned > 0 {
                            let resp = ui.small_button("+");
                            if resp.clicked() {
                                let n = if batch_size == 0 {
                                    unassigned
                                } else {
                                    batch_size.min(unassigned)
                                };
                                *action =
                                    ColonyScreenAction::AddMineAssignment(body_index, res, n);
                            }
                            resp.on_hover_text("Produces 2,000 kg/day");
                        }
                        ui.end_row();
                    }
                });

            // Non-mineable resources with assignments (edge case)
            for (&res, &count) in &assigned_counts {
                if !mineable.contains(&res) && count > 0 {
                    ui.horizontal(|ui| {
                        ui.add_space(16.0);
                        ui.label(
                            egui::RichText::new(format!("{}: {}", res.display_name(), count))
                                .size(11.0),
                        );
                        if ui.small_button("\u{2212}").clicked() {
                            let n = if batch_size == 0 {
                                count
                            } else {
                                batch_size.min(count)
                            };
                            *action =
                                ColonyScreenAction::RemoveMineAssignment(body_index, res, n);
                        }
                    });
                }
            }

            if unassigned > 0 {
                ui.horizontal(|ui| {
                    ui.add_space(16.0);
                    ui.label(
                        egui::RichText::new(format!("Unassigned: {}", unassigned))
                            .size(11.0)
                            .color(COLOR_GRAY),
                    );
                });
            }
        }

        // Atmospheric Collector sub-section
        let collector_count = colony
            .buildings
            .iter()
            .filter(|b| b.building_type == BuildingType::AtmosphericCollector)
            .count();

        if collector_count > 0 {
            ui.add_space(4.0);
            ui.separator();
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(format!("Atmospheric Collectors ({}x)", collector_count))
                    .size(13.0)
                    .strong()
                    .color(egui::Color32::WHITE),
            );
            ui.add_space(2.0);

            let atmospheric = body_atmospheric
                .get(body_index)
                .cloned()
                .unwrap_or_default();

            let atmo_mult = TechTree::tier_multiplier(tech_tree.line_tier("atmospheric_science"));
            let mut assigned_counts: HashMap<ResourceType, u32> = HashMap::new();
            let mut collector_rates: HashMap<ResourceType, f64> = HashMap::new();
            let mut unassigned = 0u32;
            for b in &colony.buildings {
                if b.building_type != BuildingType::AtmosphericCollector {
                    continue;
                }
                match b.assigned_resource {
                    Some(res) => {
                        *assigned_counts.entry(res).or_insert(0) += 1;
                        if b.operational {
                            *collector_rates.entry(res).or_insert(0.0) +=
                                10_000.0 * (1.0 - b.degradation) * colony.other_power_fraction * atmo_mult;
                        }
                    }
                    None => unassigned += 1,
                }
            }

            egui::Grid::new("collector_assign_grid")
                .min_col_width(0.0)
                .spacing(egui::vec2(4.0, 2.0))
                .show(ui, |ui| {
                    for &res in &atmospheric {
                        let count = assigned_counts.get(&res).copied().unwrap_or(0);
                        if count == 0 && unassigned == 0 {
                            continue;
                        }
                        ui.add_space(16.0);
                        let label = if count > 0 {
                            let rate = collector_rates.get(&res).copied().unwrap_or(0.0);
                            format!("{}: {} ({}/d)", res.display_name(), count, format_colony_mass(rate))
                        } else {
                            format!("{}: {}", res.display_name(), count)
                        };
                        ui.add_sized(
                            [220.0, ui.spacing().interact_size.y],
                            egui::Label::new(
                                egui::RichText::new(label).size(11.0),
                            ),
                        );
                        if count > 0 {
                            if ui.small_button("\u{2212}").clicked() {
                                let n = if batch_size == 0 {
                                    count
                                } else {
                                    batch_size.min(count)
                                };
                                *action = ColonyScreenAction::RemoveCollectorAssignment(
                                    body_index, res, n,
                                );
                            }
                        } else {
                            ui.label("");
                        }
                        if unassigned > 0 {
                            let resp = ui.small_button("+");
                            if resp.clicked() {
                                let n = if batch_size == 0 {
                                    unassigned
                                } else {
                                    batch_size.min(unassigned)
                                };
                                *action = ColonyScreenAction::AddCollectorAssignment(
                                    body_index, res, n,
                                );
                            }
                            resp.on_hover_text("Produces 10,000 kg/day");
                        }
                        ui.end_row();
                    }
                });

            // Non-atmospheric resources with assignments (edge case)
            for (&res, &count) in &assigned_counts {
                if !atmospheric.contains(&res) && count > 0 {
                    ui.horizontal(|ui| {
                        ui.add_space(16.0);
                        ui.label(
                            egui::RichText::new(format!("{}: {}", res.display_name(), count))
                                .size(11.0),
                        );
                        if ui.small_button("\u{2212}").clicked() {
                            let n = if batch_size == 0 {
                                count
                            } else {
                                batch_size.min(count)
                            };
                            *action = ColonyScreenAction::RemoveCollectorAssignment(
                                body_index, res, n,
                            );
                        }
                    });
                }
            }

            if unassigned > 0 {
                ui.horizontal(|ui| {
                    ui.add_space(16.0);
                    ui.label(
                        egui::RichText::new(format!("Unassigned: {}", unassigned))
                            .size(11.0)
                            .color(COLOR_GRAY),
                    );
                });
            }
        }

        // Factory sub-section
        let factory_count = colony
            .buildings
            .iter()
            .filter(|b| b.building_type == BuildingType::Factory)
            .count();

        if factory_count > 0 {
            ui.add_space(4.0);
            ui.separator();
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(format!("Factories ({}x)", factory_count))
                    .size(13.0)
                    .strong()
                    .color(egui::Color32::WHITE),
            );
            ui.add_space(2.0);

            let mut recipe_counts: HashMap<FactoryRecipe, u32> = HashMap::new();
            let mut recipe_rates: HashMap<FactoryRecipe, f64> = HashMap::new();
            let mut unassigned_factories = 0u32;
            for b in &colony.buildings {
                if b.building_type != BuildingType::Factory {
                    continue;
                }
                match b.assigned_recipe {
                    Some(recipe) => {
                        *recipe_counts.entry(recipe).or_insert(0) += 1;
                        if b.operational {
                            let batches_per_day = 24.0 / recipe.batch_time_hours();
                            let factory_mult = TechTree::tier_multiplier(
                                tech_tree.line_tier(recipe.efficiency_line_id()),
                            );
                            let factor = batches_per_day
                                * (1.0 - b.degradation)
                                * colony.other_power_fraction
                                * factory_mult;
                            // Use the first (primary) output for the rate display
                            let outputs = recipe.outputs();
                            if let Some(&(_, amt)) = outputs.first() {
                                *recipe_rates.entry(recipe).or_insert(0.0) += amt * factor;
                            }
                        }
                    }
                    None => unassigned_factories += 1,
                }
            }

            egui::Grid::new("factory_assign_grid")
                .min_col_width(0.0)
                .spacing(egui::vec2(4.0, 2.0))
                .show(ui, |ui| {
                    for &recipe in ALL_RECIPES {
                        let count = recipe_counts.get(&recipe).copied().unwrap_or(0);
                        let recipe_unlocked = tech_tree.is_recipe_available(recipe.recipe_id());
                        // Skip recipes that are locked and have no factories assigned
                        if count == 0 && (!recipe_unlocked || unassigned_factories == 0) {
                            continue;
                        }
                        ui.add_space(16.0);
                        let label_color = if recipe_unlocked {
                            egui::Color32::WHITE
                        } else {
                            COLOR_GRAY
                        };
                        let label = if count > 0 {
                            let rate = recipe_rates.get(&recipe).copied().unwrap_or(0.0);
                            format!("{}: {} ({}/d)", recipe.display_name(), count, format_colony_mass(rate))
                        } else {
                            format!("{}: {}", recipe.display_name(), count)
                        };
                        ui.add_sized(
                            [240.0, ui.spacing().interact_size.y],
                            egui::Label::new(
                                egui::RichText::new(label)
                                    .size(11.0)
                                    .color(label_color),
                            ),
                        );
                        if count > 0 {
                            if ui.small_button("\u{2212}").clicked() {
                                let n = if batch_size == 0 {
                                    count
                                } else {
                                    batch_size.min(count)
                                };
                                *action = ColonyScreenAction::RemoveFactoryAssignment(
                                    body_index, recipe, n,
                                );
                            }
                        } else {
                            ui.label("");
                        }
                        if unassigned_factories > 0 && recipe_unlocked {
                            // Build tooltip
                            let inputs = recipe.inputs();
                            let outputs = recipe.outputs();
                            let input_str: String = inputs
                                .iter()
                                .map(|(r, a)| {
                                    format!("{} {}", format_colony_mass(*a), r.display_name())
                                })
                                .collect::<Vec<_>>()
                                .join(" + ");
                            let sail_tier = tech_tree.line_tier("sail_technology");
                            let output_str: String = outputs
                                .iter()
                                .map(|(r, a)| {
                                    if *r == ResourceType::MirrorSegment || *r == ResourceType::CollectorStation {
                                        let unit_mass = r.storage_mass_per_unit(sail_tier);
                                        format!("{} \u{00d7} {} {}", *a as u32, format_colony_mass(unit_mass), r.display_name())
                                    } else {
                                        format!("{} {}", format_colony_mass(*a), r.display_name())
                                    }
                                })
                                .collect::<Vec<_>>()
                                .join(" + ");
                            let tooltip = format!(
                                "{} \u{2192} {} ({}h, {})",
                                input_str,
                                output_str,
                                recipe.batch_time_hours() as u32,
                                format_power_kw(recipe.power_draw_kw()),
                            );
                            let resp = ui.small_button("+");
                            if resp.clicked() {
                                let n = if batch_size == 0 {
                                    unassigned_factories
                                } else {
                                    batch_size.min(unassigned_factories)
                                };
                                *action = ColonyScreenAction::AddFactoryAssignment(
                                    body_index, recipe, n,
                                );
                            }
                            resp.on_hover_text(tooltip);
                        }
                        ui.end_row();
                    }
                });

            if unassigned_factories > 0 {
                ui.horizontal(|ui| {
                    ui.add_space(16.0);
                    ui.label(
                        egui::RichText::new(format!("Unassigned: {}", unassigned_factories))
                            .size(11.0)
                            .color(COLOR_GRAY),
                    );
                });
            }
        }
    });
}

fn render_construction_card(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    colony: &Colony,
    body_index: usize,
    hab_score: u32,
    body_radius_m: f64,
    tech_tree: &crate::colony::TechTree,
    action: &mut ColonyScreenAction,
) {
    card_frame().show(ui, |ui| {
        section_heading(ui, "Construction");

        if colony.construction_queue.is_empty() {
            ui.label(
                egui::RichText::new("No active construction.")
                    .size(12.0)
                    .color(COLOR_GRAY),
            );
        } else {
            // Compute available construction capacity per day
            let construction_mult = TechTree::tier_multiplier(tech_tree.line_tier("construction"));
            let hab_mult = (200.0 - hab_score as f64) / 100.0;
            let mut robot_cap_per_day = 0.0_f64;
            for b in &colony.buildings {
                if !b.operational {
                    continue;
                }
                match b.building_type {
                    BuildingType::ConstructionRobot => robot_cap_per_day += 20_000.0 * construction_mult,
                    BuildingType::LightConstructionRobot => robot_cap_per_day += 5_000.0 * construction_mult,
                    _ => {}
                }
            }
            let mut maintenance_demand_per_day = 0.0_f64;
            for b in &colony.buildings {
                let costs = b.building_type.maintenance_cost_per_30d();
                let hab = if b.building_type.affected_by_habitability() {
                    hab_mult
                } else {
                    1.0
                };
                let mult = hab * b.building_type.size_multiplier(body_radius_m);
                let mass: f64 = costs.iter().map(|(_, amt)| amt * mult).sum();
                maintenance_demand_per_day += mass / 30.0;
            }
            let available_per_day = (robot_cap_per_day - maintenance_demand_per_day).max(0.0);

            let mut cumulative_days = 0.0_f64;
            for item in &colony.construction_queue {
                let progress = if item.total_mass > 0.0 {
                    (item.mass_assembled / item.total_mass) as f32
                } else {
                    0.0
                };
                let display_name = match item.effective_target() {
                    crate::colony::ConstructionTarget::Building(bt) => {
                        bt.display_name().to_string()
                    }
                    crate::colony::ConstructionTarget::Ship { name, .. } => {
                        format!("Ship: {}", name)
                    }
                };
                let remaining_units = item.count - item.completed;
                let remaining_mass = remaining_units as f64 * item.total_mass - item.mass_assembled;
                let item_days = if available_per_day > 0.0 {
                    remaining_mass / available_per_day
                } else {
                    f64::INFINITY
                };
                cumulative_days += item_days;

                let time_str = if available_per_day <= 0.0 {
                    " \u{2014} stalled".to_string()
                } else if cumulative_days < 1.0 {
                    format!(" \u{2014} ~{:.0}h", cumulative_days * 24.0)
                } else {
                    format!(" \u{2014} ~{:.1}d", cumulative_days)
                };

                let text = if remaining_units > 1 {
                    format!(
                        "{} \u{00d7}{} ({} / {}){}",
                        display_name,
                        remaining_units,
                        format_colony_mass(item.mass_assembled),
                        format_colony_mass(item.total_mass),
                        time_str,
                    )
                } else {
                    format!(
                        "{} ({} / {}){}",
                        display_name,
                        format_colony_mass(item.mass_assembled),
                        format_colony_mass(item.total_mass),
                        time_str,
                    )
                };
                ui.add(
                    egui::ProgressBar::new(progress.clamp(0.0, 1.0))
                        .text(text)
                        .fill(egui::Color32::from_rgb(80, 160, 80)),
                );
            }
        }

        ui.add_space(4.0);
        ui.separator();
        ui.add_space(4.0);

        // Batch size selector for construction queue
        let build_batch_id = egui::Id::new("colony_build_batch_size");
        let mut build_batch: u32 = ctx.data_mut(|d| *d.get_temp_mut_or(build_batch_id, 1u32));
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Queue:").size(12.0));
            for &(label, val) in
                &[("1", 1u32), ("10", 10), ("100", 100), ("All", 0)]
            {
                if ui
                    .selectable_label(build_batch == val, egui::RichText::new(label).size(11.0))
                    .clicked()
                {
                    build_batch = val;
                }
            }
        });
        ctx.data_mut(|d| d.insert_temp(build_batch_id, build_batch));

        ui.horizontal(|ui| {
            egui::ComboBox::from_id_source("cs_add_building_combo")
                .selected_text("Select building...")
                .width(200.0)
                .show_ui(ui, |ui| {
                    for &bt in BUILDABLE_BUILDINGS {
                        let tech_available = tech_tree.is_building_available(bt);
                        if !tech_available {
                            ui.add_enabled(
                                false,
                                egui::SelectableLabel::new(
                                    false,
                                    format!("{} [Locked]", bt.display_name()),
                                ),
                            );
                            continue;
                        }
                        let can_build = colony.can_queue_building(bt, hab_score, body_radius_m);
                        let costs = bt.build_cost();
                        // Display actual scaled cost: habitability for Habitat/Greenhouse,
                        // circumference for Mk IV accelerator.
                        let display_mult = {
                            let hab = if bt.affected_by_habitability() {
                                (200.0 - hab_score as f64) / 100.0
                            } else {
                                1.0
                            };
                            hab * bt.size_multiplier(body_radius_m)
                        };
                        let cost_str: String = costs
                            .iter()
                            .map(|(r, amt)| {
                                format!(
                                    "{} {}",
                                    format_colony_mass(*amt * display_mult),
                                    r.display_name()
                                )
                            })
                            .collect::<Vec<_>>()
                            .join(", ");
                        let label = format!("{} [{}]", bt.display_name(), cost_str);

                        if ui
                            .add_enabled(
                                can_build,
                                egui::SelectableLabel::new(false, &label),
                            )
                            .clicked()
                        {
                            *action = ColonyScreenAction::QueueBuilding(body_index, bt, build_batch);
                        }
                    }
                });
        });
    });
}

fn render_maintenance_card(ui: &mut egui::Ui, colony: &Colony, hab_mult: f64) {
    card_frame().show(ui, |ui| {
        section_heading(ui, "Maintenance");

        // Aggregate maintenance costs per 30 days
        let mut total_costs: BTreeMap<&str, (ResourceType, f64)> = BTreeMap::new();
        let mut total_mass_per_30d = 0.0_f64;
        for b in &colony.buildings {
            let costs = b.building_type.maintenance_cost_per_30d();
            let mult = if b.building_type.affected_by_habitability() {
                hab_mult
            } else {
                1.0
            };
            for &(res, amt) in &costs {
                let scaled = amt * mult;
                let entry = total_costs
                    .entry(res.display_name())
                    .or_insert((res, 0.0));
                entry.1 += scaled;
                total_mass_per_30d += scaled;
            }
        }

        if total_costs.is_empty() {
            ui.label(
                egui::RichText::new("No maintenance required.")
                    .size(12.0)
                    .color(COLOR_GRAY),
            );
            return;
        }

        egui::Grid::new("cs_maintenance_grid")
            .striped(true)
            .min_col_width(100.0)
            .show(ui, |ui| {
                ui.label(egui::RichText::new("Resource").size(11.0).strong());
                ui.label(egui::RichText::new("Per 30d").size(11.0).strong());
                ui.label(egui::RichText::new("In stock").size(11.0).strong());
                ui.label(egui::RichText::new("Days left").size(11.0).strong());
                ui.end_row();

                for (_name, (res, cost_30d)) in &total_costs {
                    let in_stock = colony.resources.get(*res);
                    let daily_rate = cost_30d / 30.0;
                    let days_left = if daily_rate > 0.0 {
                        in_stock / daily_rate
                    } else {
                        f64::INFINITY
                    };

                    ui.label(egui::RichText::new(res.display_name()).size(11.0));
                    ui.label(egui::RichText::new(format_colony_mass(*cost_30d)).size(11.0));
                    ui.label(egui::RichText::new(format_colony_mass(in_stock)).size(11.0));

                    let days_color = if days_left < 10.0 {
                        COLOR_RED
                    } else if days_left < 30.0 {
                        COLOR_YELLOW
                    } else {
                        egui::Color32::WHITE
                    };
                    ui.label(
                        egui::RichText::new(if days_left.is_infinite() {
                            "\u{221e}".to_string()
                        } else {
                            format!("{:.0}", days_left)
                        })
                        .size(11.0)
                        .color(days_color),
                    );
                    ui.end_row();
                }
            });

        ui.add_space(4.0);
        ui.separator();
        ui.add_space(4.0);

        ui.label(
            egui::RichText::new(format!(
                "Total: {} / 30 days",
                format_colony_mass(total_mass_per_30d)
            ))
            .size(11.0),
        );

        // Robot capacity
        let mut robot_capacity_per_day = 0.0_f64;
        for b in &colony.buildings {
            if !b.operational {
                continue;
            }
            match b.building_type {
                BuildingType::ConstructionRobot => robot_capacity_per_day += 20_000.0,
                BuildingType::LightConstructionRobot => robot_capacity_per_day += 5_000.0,
                _ => {}
            }
        }
        let daily_demand = total_mass_per_30d / 30.0;
        let robot_color = if robot_capacity_per_day < daily_demand {
            COLOR_RED
        } else {
            egui::Color32::WHITE
        };
        ui.label(
            egui::RichText::new(format!(
                "Robot capacity: {} / day (demand: {} / day)",
                format_colony_mass(robot_capacity_per_day),
                format_colony_mass(daily_demand),
            ))
            .size(11.0)
            .color(robot_color),
        );

        // Degraded buildings
        let mut deg_counts: BTreeMap<&str, (u32, f64)> = BTreeMap::new();
        for b in &colony.buildings {
            if b.degradation > 0.001 {
                let entry = deg_counts
                    .entry(b.building_type.display_name())
                    .or_insert((0, 0.0));
                entry.0 += 1;
                entry.1 = entry.1.max(b.degradation);
            }
        }
        if !deg_counts.is_empty() {
            ui.add_space(4.0);
            ui.separator();
            ui.add_space(4.0);
            for (name, (count, worst)) in &deg_counts {
                let label = if *count > 1 {
                    format!(
                        "{} ({}x) \u{2014} {:.0}% degraded",
                        name,
                        count,
                        worst * 100.0
                    )
                } else {
                    format!("{} \u{2014} {:.0}% degraded", name, worst * 100.0)
                };
                ui.label(
                    egui::RichText::new(label)
                        .size(11.0)
                        .color(COLOR_DEG_YELLOW),
                );
            }
        }
    });
}

fn render_resources_card(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    colony: &Colony,
    body_index: usize,
    rates: &ResourceRates,
    tech_tree: &crate::colony::TechTree,
    action: &mut ColonyScreenAction,
) {
    card_frame().show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        section_heading(ui, "Resources");

        let storage_cap = colony.storage_capacity();
        let sail_tier = tech_tree.line_tier("sail_technology");
        let storage_used = colony.resources.total_storage_mass(sail_tier);

        // Show total storage header
        ui.label(
            egui::RichText::new(format!(
                "Storage: {} / {}",
                format_colony_mass(storage_used),
                format_colony_mass(storage_cap),
            ))
            .size(12.0),
        );
        ui.add_space(4.0);

        // Build resource list: regular resources + food
        let mut resources: Vec<(ResourceType, f64)> = colony
            .resources
            .iter()
            .filter(|(_, &amt)| amt > 0.001)
            .map(|(&rt, &amt)| (rt, amt))
            .collect();
        resources.sort_by(|a, b| a.0.display_name().cmp(b.0.display_name()));

        // Add Food row
        let has_food_in_list = resources.iter().any(|(rt, _)| *rt == ResourceType::Food);
        if !has_food_in_list && (colony.food_stored > 0.001 || colony.crew > 0) {
            resources.push((ResourceType::Food, colony.food_stored));
            resources.sort_by(|a, b| a.0.display_name().cmp(b.0.display_name()));
        }

        // Add resources that have production/consumption but zero stock
        let rate_resources: Vec<ResourceType> = rates
            .production
            .keys()
            .chain(rates.consumption.keys())
            .copied()
            .collect();
        for rt in rate_resources {
            if rt == ResourceType::Food {
                continue;
            }
            if !resources.iter().any(|(r, _)| *r == rt) {
                resources.push((rt, 0.0));
            }
        }
        resources.sort_by(|a, b| a.0.display_name().cmp(b.0.display_name()));

        // Compute active resources for allocation display
        let active_resources = crate::colony::compute_active_resources(
            &colony.resources,
            &rates.production,
        );
        let alloc_pcts = colony.storage_allocation.effective_pcts(&active_resources);

        if resources.is_empty() {
            ui.label(
                egui::RichText::new("No resources in storage.")
                    .size(12.0)
                    .color(COLOR_GRAY),
            );
        } else {
            egui::Grid::new("cs_resources_grid")
                .striped(true)
                .num_columns(7)
                .min_col_width(60.0)
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("Resource").size(11.0).strong());
                    ui.label(egui::RichText::new("Amount").size(11.0).strong());
                    ui.label(egui::RichText::new("Cap").size(11.0).strong());
                    ui.label(egui::RichText::new("Production").size(11.0).strong());
                    ui.label(egui::RichText::new("Consumption").size(11.0).strong());
                    ui.label(egui::RichText::new("Days left").size(11.0).strong());
                    ui.label(egui::RichText::new("Alloc %").size(11.0).strong());
                    ui.end_row();

                    for (rt, amt) in &resources {
                        let is_food = *rt == ResourceType::Food;
                        let amount = if is_food {
                            colony.food_stored
                        } else {
                            *amt
                        };
                        let prod = rates.production.get(rt).copied().unwrap_or(0.0);
                        let cons = rates.consumption.get(rt).copied().unwrap_or(0.0);
                        let net = prod - cons;

                        let is_unit_counted = *rt == ResourceType::MirrorSegment
                            || *rt == ResourceType::CollectorStation;

                        // Resource name
                        ui.label(egui::RichText::new(rt.display_name()).size(11.0));

                        // Amount
                        if is_unit_counted {
                            let count = amount.round() as u64;
                            let unit_mass = if *rt == ResourceType::MirrorSegment {
                                let sail_tier = tech_tree.line_tier("sail_technology");
                                crate::colony::dyson_swarm::mirror_mass_at_tier(sail_tier)
                            } else {
                                crate::colony::dyson_swarm::COLLECTOR_MASS_KG
                            };
                            let total_kg = count as f64 * unit_mass;
                            ui.label(egui::RichText::new(format!(
                                "{} ({})", count, format_colony_mass(total_kg)
                            )).size(11.0));
                        } else {
                            ui.label(egui::RichText::new(format_colony_mass(amount)).size(11.0));
                        }

                        // Cap column (per-resource allocated capacity)
                        if is_food {
                            let food_cap = colony.food_capacity();
                            ui.label(egui::RichText::new(format_colony_mass(food_cap)).size(11.0).color(COLOR_GRAY));
                        } else {
                            let res_cap_kg = colony.storage_allocation.capacity_for(*rt, storage_cap, &active_resources);
                            if is_unit_counted {
                                // Show cap as unit count (cap_kg / mass_per_unit)
                                let unit_mass = rt.storage_mass_per_unit(sail_tier);
                                let cap_count = (res_cap_kg / unit_mass).floor() as u64;
                                let amount_kg = amount * unit_mass;
                                let cap_color = if amount_kg > res_cap_kg * 0.95 && res_cap_kg > 0.0 {
                                    COLOR_YELLOW
                                } else {
                                    COLOR_GRAY
                                };
                                ui.label(egui::RichText::new(format!("{}", cap_count)).size(11.0).color(cap_color));
                            } else {
                                let cap_color = if amount > res_cap_kg * 0.95 && res_cap_kg > 0.0 {
                                    COLOR_YELLOW
                                } else {
                                    COLOR_GRAY
                                };
                                ui.label(egui::RichText::new(format_colony_mass(res_cap_kg)).size(11.0).color(cap_color));
                            }
                        }

                        // Production
                        if prod > 0.001 {
                            if is_unit_counted {
                                ui.label(
                                    egui::RichText::new(format!("+{:.1}/day", prod))
                                        .size(11.0)
                                        .color(COLOR_GREEN),
                                );
                            } else {
                                ui.label(
                                    egui::RichText::new(format_rate(prod))
                                        .size(11.0)
                                        .color(COLOR_GREEN),
                                );
                            }
                        } else {
                            ui.label("");
                        }

                        // Consumption
                        if cons > 0.001 {
                            if is_unit_counted {
                                ui.label(
                                    egui::RichText::new(format!("\u{2212}{:.1}/day", cons))
                                        .size(11.0)
                                        .color(COLOR_ORANGE),
                                );
                            } else {
                                ui.label(
                                    egui::RichText::new(format_rate(-cons))
                                        .size(11.0)
                                        .color(COLOR_ORANGE),
                                );
                            }
                        } else {
                            ui.label("");
                        }

                        // Days left
                        if net < -0.001 && amount > 0.001 {
                            let days_left = amount / (-net);
                            let days_color = if days_left < 10.0 {
                                COLOR_RED
                            } else if days_left < 30.0 {
                                COLOR_YELLOW
                            } else {
                                egui::Color32::WHITE
                            };
                            ui.label(
                                egui::RichText::new(format!("{:.0}", days_left))
                                    .size(11.0)
                                    .color(days_color),
                            );
                        } else if net >= 0.001 {
                            ui.label(
                                egui::RichText::new("\u{221e}")
                                    .size(11.0)
                                    .color(COLOR_GREEN),
                            );
                        } else {
                            ui.label("");
                        }

                        // Alloc % column — editable for non-food resources
                        if is_food {
                            ui.label(egui::RichText::new("\u{2014}").size(11.0).color(COLOR_GRAY));
                        } else {
                            let pct = alloc_pcts.get(rt).copied().unwrap_or(0.0);
                            let is_pinned = colony.storage_allocation.is_pinned(*rt);
                            let id = egui::Id::new(("alloc_pct", *rt as u8));
                            let mut pct_str: String = ctx.data_mut(|d| {
                                d.get_temp(id).unwrap_or_else(|| format!("{:.0}", pct))
                            });

                            let pct_color = if is_pinned { COLOR_YELLOW } else { COLOR_GRAY };
                            let response = ui.add(
                                egui::TextEdit::singleline(&mut pct_str)
                                    .desired_width(35.0)
                                    .font(egui::TextStyle::Small)
                                    .text_color(pct_color),
                            );
                            if response.lost_focus() {
                                if let Ok(new_pct) = pct_str.trim().parse::<f64>() {
                                    *action = ColonyScreenAction::SetStorageAllocation {
                                        body_index,
                                        resource: *rt,
                                        percent: new_pct,
                                    };
                                }
                                // Reset temp to reflect new state next frame
                                ctx.data_mut(|d| d.remove::<String>(id));
                            } else if !response.has_focus() {
                                // Update display when not editing
                                ctx.data_mut(|d| d.insert_temp(id, format!("{:.0}", pct)));
                            } else {
                                ctx.data_mut(|d| d.insert_temp(id, pct_str));
                            }

                            // Right-click to unpin
                            if is_pinned {
                                response.context_menu(|ui| {
                                    if ui.button("Unpin (auto)").clicked() {
                                        *action = ColonyScreenAction::UnpinStorageAllocation {
                                            body_index,
                                            resource: *rt,
                                        };
                                        ui.close_menu();
                                    }
                                });
                            }
                        }

                        ui.end_row();
                    }
                });
        }
    });
}

// ============================================================
// Main render function
// ============================================================

/// Render the full-screen colony management screen.
/// Returns a ColonyScreenAction describing what the user wants to do.
fn render_hangar_card(
    ui: &mut egui::Ui,
    colony: &Colony,
    body_index: usize,
    action: &mut ColonyScreenAction,
) {
    if !colony.has_hangar() {
        return;
    }

    card_frame().show(ui, |ui| {
        section_heading(ui, "Ship Hangar");

        // Capacity bar
        let used = colony.hangar_used();
        let capacity = colony.hangar_capacity();
        let frac = if capacity > 0.0 {
            (used / capacity) as f32
        } else {
            0.0
        };
        let bar_color = if frac > 0.9 {
            egui::Color32::from_rgb(200, 80, 80)
        } else {
            egui::Color32::from_rgb(80, 140, 200)
        };
        ui.add(
            egui::ProgressBar::new(frac.clamp(0.0, 1.0))
                .text(format!(
                    "{} / {}",
                    format_colony_mass(used),
                    format_colony_mass(capacity),
                ))
                .fill(bar_color),
        );

        if colony.stored_ships.is_empty() {
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new("No ships stored.")
                    .size(12.0)
                    .color(COLOR_GRAY),
            );
        } else {
            ui.add_space(4.0);
            egui::Grid::new("cs_hangar_ships_grid")
                .striped(true)
                .min_col_width(60.0)
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("Ship").size(11.0).strong());
                    ui.label(egui::RichText::new("Blueprint").size(11.0).strong());
                    ui.label(egui::RichText::new("Dry Mass").size(11.0).strong());
                    ui.label(egui::RichText::new("\u{0394}v").size(11.0).strong());
                    ui.label(""); // Scrap button column
                    ui.end_row();

                    for ship in &colony.stored_ships {
                        ui.label(egui::RichText::new(&ship.name).size(11.0));
                        ui.label(
                            egui::RichText::new(
                                ship.blueprint_name.as_deref().unwrap_or("\u{2014}"),
                            )
                            .size(11.0)
                            .color(COLOR_GRAY),
                        );
                        ui.label(
                            egui::RichText::new(format_colony_mass(ship.dry_mass_kg)).size(11.0),
                        );
                        ui.label(
                            egui::RichText::new(format!("{:.0} m/s", ship.cached_delta_v))
                                .size(11.0),
                        );
                        if ui.small_button("Scrap").clicked() {
                            *action = ColonyScreenAction::ScrapShip(body_index, ship.id);
                        }
                        ui.end_row();
                    }
                });
        }
    });
}

fn render_mass_driver_card(
    ui: &mut egui::Ui,
    colony: &Colony,
    tech_tree: &crate::colony::TechTree,
) {
    card_frame().show(ui, |ui| {
        let driver = match colony.best_mass_driver() {
            Some(d) => d,
            None => return,
        };

        section_heading(ui, "Mass Driver");

        let sail_tier = tech_tree.line_tier("sail_technology");
        let mirror_mass = crate::colony::dyson_swarm::mirror_mass_at_tier(sail_tier);

        egui::Grid::new("mass_driver_grid")
            .striped(true)
            .num_columns(2)
            .show(ui, |ui| {
                ui.label("Type:");
                ui.label(
                    egui::RichText::new(driver.display_name())
                        .color(egui::Color32::WHITE),
                );
                ui.end_row();

                if let Some(track) = driver.mass_driver_track_m() {
                    ui.label("Track length:");
                    ui.label(format!("{:.0} km", track / 1000.0));
                    ui.end_row();
                }

                // Ship launch velocity
                if let Some(v) = driver.mass_driver_launch_velocity(1000.0, false) {
                    ui.label("Ship max velocity:");
                    ui.label(format!("{:.1} km/s", v / 1000.0));
                    ui.end_row();
                }

                // Mirror launch velocity
                if let Some(v) = driver.mass_driver_launch_velocity(mirror_mass, true) {
                    ui.label("Mirror max velocity:");
                    ui.label(format!("{:.1} km/s", v / 1000.0));
                    ui.end_row();
                }

                if let Some(max_payload) = driver.mass_driver_max_payload_kg() {
                    ui.label("Max payload:");
                    ui.label(format_colony_mass(max_payload));
                    ui.end_row();
                }

                ui.label("Power draw:");
                ui.label(format_power_kw(driver.power_draw_kw()));
                ui.end_row();

                // Energy capacity and stored
                if let Some(capacity) = driver.mass_driver_energy_capacity_j() {
                    ui.label("Energy capacity:");
                    ui.label(format_energy_j(capacity));
                    ui.end_row();

                    let energy_j = colony.mass_driver_energy_j;
                    let pct = if capacity > 0.0 { (energy_j / capacity * 100.0).min(100.0) } else { 0.0 };
                    ui.label("Energy stored:");
                    ui.label(format!("{} ({:.0}%)", format_energy_j(energy_j), pct));
                    ui.end_row();
                }

                ui.label("Mirrors launched:");
                ui.label(format!("{}", colony.mirrors_launched));
                ui.end_row();

                // Cadence estimate
                if let Some(v) = driver.mass_driver_launch_velocity(mirror_mass, true) {
                    let energy = BuildingType::mass_driver_launch_energy_j(mirror_mass, v);
                    let power_w = driver.power_draw_kw() * 1000.0;
                    let recharge_s = BuildingType::mass_driver_recharge_time_s(energy, power_w);
                    let per_day = if recharge_s > 0.0 { 86_400.0 / recharge_s } else { 0.0 };
                    ui.label("Mirror cadence:");
                    if per_day >= 1.0 {
                        ui.label(format!("{:.0}/day", per_day));
                    } else {
                        let hours = recharge_s / 3600.0;
                        ui.label(format!("1 per {:.1} hrs", hours));
                    }
                    ui.end_row();
                }
            });
    });
}

pub(super) fn render_dyson_swarm_card(
    ui: &mut egui::Ui,
    swarm: &crate::colony::DysonSwarm,
    tech_tree: &crate::colony::TechTree,
) {
    // Only show if there's any swarm activity
    if swarm.mirror_count == 0 && swarm.in_transit() == 0
        && swarm.collector_count == 0 && swarm.collectors_in_transit() == 0
    {
        return;
    }

    card_frame().show(ui, |ui| {
        section_heading(ui, "Dyson Swarm (0.1 AU)");

        let sail_tier = tech_tree.line_tier("sail_technology");
        let beta = crate::colony::dyson_swarm::lightness_number_at_tier(sail_tier);

        // === Section A: Swarm Status ===
        ui.label(
            egui::RichText::new("Swarm Status")
                .size(12.0)
                .strong()
                .color(COLOR_GRAY),
        );
        ui.add_space(2.0);

        egui::Grid::new("dyson_swarm_status_grid")
            .num_columns(2)
            .show(ui, |ui| {
                let area = swarm.total_area_km2();
                let area_str = if area >= 1_000_000.0 {
                    format!("{:.2} M km\u{00B2}", area / 1_000_000.0)
                } else if area >= 1_000.0 {
                    format!("{:.0}k km\u{00B2}", area / 1000.0)
                } else {
                    format!("{:.0} km\u{00B2}", area)
                };
                ui.label("Mirrors:");
                ui.label(
                    egui::RichText::new(format!("{} ({})", swarm.mirror_count, area_str))
                        .color(COLOR_GREEN),
                );
                ui.end_row();

                ui.label("Collectors:");
                ui.label(
                    egui::RichText::new(format!("{}", swarm.collector_count))
                        .color(if swarm.collector_count > 0 { COLOR_GREEN } else { COLOR_GRAY }),
                );
                ui.end_row();

                let mirrors_transit = swarm.in_transit();
                let collectors_transit = swarm.collectors_in_transit();
                if mirrors_transit > 0 || collectors_transit > 0 {
                    ui.label("In transit:");
                    let mut parts = Vec::new();
                    if mirrors_transit > 0 {
                        parts.push(format!("{} mirrors", mirrors_transit));
                    }
                    if collectors_transit > 0 {
                        parts.push(format!("{} collectors", collectors_transit));
                    }
                    ui.label(
                        egui::RichText::new(parts.join(", "))
                            .color(COLOR_YELLOW),
                    );
                    ui.end_row();
                }
            });

        // === Section B: Power Chain ===
        if swarm.collector_count > 0 || swarm.collectors_in_transit() > 0 {
            ui.add_space(4.0);
            ui.separator();
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new("Power Chain")
                    .size(12.0)
                    .strong()
                    .color(COLOR_GRAY),
            );
            ui.add_space(2.0);

            egui::Grid::new("dyson_swarm_power_grid")
                .num_columns(2)
                .show(ui, |ui| {
                    ui.label("Reflected power:");
                    ui.label(
                        egui::RichText::new(format_power_w(swarm.total_power_w()))
                            .color(COLOR_GREEN),
                    );
                    ui.end_row();

                    let eta = crate::colony::dyson_swarm::DysonSwarm::collection_efficiency(beta);
                    let efficiency_tooltip = format!(
                        "Mirror reflectivity: {:.0}%\nCollector PV+laser: {:.0}%\nReceiver conversion: {:.0}%",
                        crate::colony::dyson_swarm::MIRROR_EFFICIENCY * 100.0,
                        crate::colony::dyson_swarm::COLLECTOR_EFFICIENCY * 100.0,
                        crate::colony::dyson_swarm::RECEIVER_EFFICIENCY * 100.0,
                    );
                    ui.label("Collection efficiency:");
                    let eff_resp = ui.label(format!("{:.0}%", eta * 100.0));
                    eff_resp.on_hover_text(efficiency_tooltip);
                    ui.end_row();

                    let laser_w = swarm.available_laser_power(beta);
                    ui.label("Laser power:");
                    ui.label(
                        egui::RichText::new(format_power_w(laser_w))
                            .color(if laser_w > 0.0 { COLOR_GREEN } else { COLOR_GRAY }),
                    );
                    ui.end_row();
                });
        }

        // === Section C: Technology ===
        ui.add_space(4.0);
        ui.separator();
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new("Technology")
                .size(12.0)
                .strong()
                .color(COLOR_GRAY),
        );
        ui.add_space(2.0);

        egui::Grid::new("dyson_swarm_tech_grid")
            .num_columns(2)
            .show(ui, |ui| {
                let sigma = crate::colony::dyson_swarm::sail_loading_at_tier(sail_tier);
                ui.label("Sail loading:");
                ui.label(format!("{:.2} g/m\u{00B2}  |  \u{03B2}: {:.2}", sigma, beta));
                ui.end_row();

                if beta >= 1.0 {
                    ui.label("Status:");
                    ui.label(
                        egui::RichText::new("Statite capable")
                            .color(COLOR_GREEN),
                    );
                    ui.end_row();
                }
            });
    });
}

pub fn render_colony_screen(
    ctx: &egui::Context,
    body_index: usize,
    colony_manager: &ColonyManager,
    body_names: &[String],
    body_habitability: &[u32],
    body_radii: &[f64],
    body_mineable: &[Vec<ResourceType>],
    body_atmospheric: &[Vec<ResourceType>],
    warp_levels: &[f64],
    current_warp_index: usize,
    date_str: &str,
    paused: bool,
    can_return_to_flight: bool,
    active_toasts: &[(String, web_time::Instant)],
    solar_power_factor: f64,
    tech_tree: &crate::colony::TechTree,
    fleet: &FleetManager,
    earth_index: usize,
) -> ColonyScreenAction {
    let mut action = ColonyScreenAction::None;

    let colony = match colony_manager.get_by_body(body_index) {
        Some(c) => c,
        None => {
            return ColonyScreenAction::GoToTrackingStation;
        }
    };

    let hab_score = body_habitability.get(body_index).copied().unwrap_or(0);
    let body_radius_m = body_radii.get(body_index).copied().unwrap_or(0.0);
    let hab_mult = (200.0 - hab_score as f64) / 100.0;

    // Pre-compute resource rates
    let rates = compute_resource_rates(colony, hab_mult, body_radius_m, tech_tree);

    // === Top panel: colony name, selector, date, time warp ===
    egui::TopBottomPanel::top("colony_top_panel").show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.heading(
                egui::RichText::new(&colony.name)
                    .size(18.0)
                    .color(egui::Color32::WHITE),
            );

            ui.separator();

            // Colony selector ComboBox
            if colony_manager.colonies.len() > 1 {
                let selected_name = &colony.name;
                egui::ComboBox::from_id_source("colony_screen_selector")
                    .selected_text(selected_name.as_str())
                    .width(160.0)
                    .show_ui(ui, |ui| {
                        for c in &colony_manager.colonies {
                            let name = &c.name;
                            if ui
                                .selectable_label(c.body_index == body_index, name.as_str())
                                .clicked()
                            {
                                action = ColonyScreenAction::SwitchColony(c.body_index);
                            }
                        }
                    });

                ui.separator();
            }

            // Time warp buttons
            ui.label("Time Warp:");
            for (i, &warp) in warp_levels.iter().enumerate() {
                let label = if warp >= 1_000_000_000.0 {
                    format!("{}B", (warp / 1_000_000_000.0) as i32)
                } else if warp >= 1_000_000.0 {
                    format!("{}M", (warp / 1_000_000.0) as i32)
                } else if warp >= 1000.0 {
                    format!("{}K", (warp / 1000.0) as i32)
                } else {
                    format!("{}x", warp as i32)
                };
                let is_selected = i == current_warp_index;
                if ui.selectable_label(is_selected, &label).clicked() {
                    action = ColonyScreenAction::ChangeWarp(i);
                }
            }
            ui.separator();
            let current_warp = warp_levels[current_warp_index];
            ui.label(format!("Current: {}x", current_warp as i64));

            ui.separator();
            ui.label(date_str);
        });
    });

    // === Pause overlay ===
    if paused {
        egui::Area::new(egui::Id::new("colony_pause_overlay"))
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::none()
                    .fill(egui::Color32::from_rgba_unmultiplied(0, 0, 0, 180))
                    .inner_margin(egui::Margin::same(40.0))
                    .rounding(egui::Rounding::same(8.0))
                    .show(ui, |ui| {
                        ui.vertical_centered(|ui| {
                            ui.heading(
                                egui::RichText::new("Paused")
                                    .size(32.0)
                                    .color(egui::Color32::WHITE),
                            );
                            ui.add_space(20.0);
                            if can_return_to_flight {
                                if ui
                                    .button(egui::RichText::new("Return to Flight").size(18.0))
                                    .clicked()
                                {
                                    action = ColonyScreenAction::ReturnToFlight;
                                }
                                ui.add_space(8.0);
                            }
                            if ui
                                .button(egui::RichText::new("Tracking Station").size(18.0))
                                .clicked()
                            {
                                action = ColonyScreenAction::GoToTrackingStation;
                            }
                            ui.add_space(8.0);
                            if ui
                                .button(egui::RichText::new("Colony Overview").size(18.0))
                                .clicked()
                            {
                                action = ColonyScreenAction::GoToColonyOverview;
                            }
                            ui.add_space(8.0);
                            if ui
                                .button(egui::RichText::new("Main Menu").size(18.0))
                                .clicked()
                            {
                                action = ColonyScreenAction::GoToMainMenu;
                            }
                        });
                    });
            });

        super::flight::render_toasts(ctx, active_toasts);

        return action;
    }

    // === Central panel: card-based colony content ===
    egui::CentralPanel::default().show(ctx, |ui| {
        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                let body_name = body_names
                    .get(body_index)
                    .map(|s| s.as_str())
                    .unwrap_or("Unknown");

                // 1. Overview card
                render_overview_card(ui, colony, body_name, tech_tree);

                // 2. Power card
                render_power_card(ui, colony, solar_power_factor);

                // 3. Buildings card
                render_buildings_card(
                    ui,
                    ctx,
                    colony,
                    body_index,
                    body_mineable,
                    body_atmospheric,
                    &mut action,
                    tech_tree,
                );

                // 4. Construction card
                render_construction_card(
                    ui,
                    ctx,
                    colony,
                    body_index,
                    hab_score,
                    body_radius_m,
                    tech_tree,
                    &mut action,
                );

                // 5. Maintenance card
                render_maintenance_card(ui, colony, hab_mult);

                // 6. Resources card
                render_resources_card(ui, ctx, colony, body_index, &rates, tech_tree, &mut action);

                // 7. Trade routes section (read-only)
                {
                    ui.add_space(4.0);
                    let trade_action = super::trade_ui::render_colony_trade_section(
                        ui, body_index, fleet, body_names, earth_index,
                    );
                    if trade_action != TradeAction::None {
                        action = ColonyScreenAction::Trade(trade_action);
                    }
                }

                // 8. Hangar card
                render_hangar_card(ui, colony, body_index, &mut action);

                // 9. Mass Driver card (if colony has one)
                if colony.has_mass_driver() {
                    render_mass_driver_card(ui, colony, tech_tree);
                }

                // 10. Debug section (no card frame)
                ui.add_space(4.0);
                egui::CollapsingHeader::new("Debug")
                    .default_open(false)
                    .show(ui, |ui| {
                        // Add Resource
                        ui.horizontal(|ui| {
                            ui.label("Add Resource:");
                            let res_id = egui::Id::new("cs_debug_resource_idx");
                            let mut selected_idx: usize =
                                ui.data(|d| d.get_temp(res_id).unwrap_or(0));
                            let all_resources = ResourceType::all();
                            let selected_rt =
                                all_resources[selected_idx.min(all_resources.len() - 1)];

                            egui::ComboBox::from_id_source("cs_debug_resource")
                                .selected_text(selected_rt.display_name())
                                .width(160.0)
                                .show_ui(ui, |ui| {
                                    for (i, rt) in all_resources.iter().enumerate() {
                                        if ui
                                            .selectable_label(
                                                i == selected_idx,
                                                rt.display_name(),
                                            )
                                            .clicked()
                                        {
                                            selected_idx = i;
                                        }
                                    }
                                });
                            ui.data_mut(|d| d.insert_temp(res_id, selected_idx));

                            let amt_id = egui::Id::new("cs_debug_resource_amt");
                            let mut amount: f64 =
                                ui.data(|d| d.get_temp(amt_id).unwrap_or(10_000.0));
                            ui.add(
                                egui::DragValue::new(&mut amount)
                                    .clamp_range(100.0..=1_000_000.0)
                                    .speed(100.0)
                                    .suffix(" kg"),
                            );
                            ui.data_mut(|d| d.insert_temp(amt_id, amount));

                            if ui.button("Add").clicked() {
                                action = ColonyScreenAction::DebugAddResource(
                                    body_index,
                                    selected_rt,
                                    amount,
                                );
                            }
                        });

                        // Add Building
                        ui.horizontal(|ui| {
                            ui.label("Add Building:");
                            let bld_id = egui::Id::new("cs_debug_building_idx");
                            let mut selected_idx: usize =
                                ui.data(|d| d.get_temp(bld_id).unwrap_or(0));
                            let all_buildings = BuildingType::all();
                            let selected_bt =
                                all_buildings[selected_idx.min(all_buildings.len() - 1)];

                            egui::ComboBox::from_id_source("cs_debug_building")
                                .selected_text(selected_bt.display_name())
                                .width(200.0)
                                .show_ui(ui, |ui| {
                                    for (i, bt) in all_buildings.iter().enumerate() {
                                        if ui
                                            .selectable_label(
                                                i == selected_idx,
                                                bt.display_name(),
                                            )
                                            .clicked()
                                        {
                                            selected_idx = i;
                                        }
                                    }
                                });
                            ui.data_mut(|d| d.insert_temp(bld_id, selected_idx));

                            if ui.button("Add").clicked() {
                                action = ColonyScreenAction::DebugAddBuilding(
                                    body_index,
                                    selected_bt,
                                );
                            }
                        });

                        // Add Crew
                        ui.horizontal(|ui| {
                            ui.label("Add Crew:");
                            let crew_id = egui::Id::new("cs_debug_crew_amt");
                            let mut count: u32 =
                                ui.data(|d| d.get_temp(crew_id).unwrap_or(10));
                            ui.add(
                                egui::DragValue::new(&mut count)
                                    .clamp_range(1..=1000)
                                    .speed(1.0),
                            );
                            ui.data_mut(|d| d.insert_temp(crew_id, count));

                            if ui.button("Add").clicked() {
                                action =
                                    ColonyScreenAction::DebugAddCrew(body_index, count);
                            }
                        });
                    });
            });
    });

    // Toast notifications
    super::flight::render_toasts(ctx, active_toasts);

    action
}
