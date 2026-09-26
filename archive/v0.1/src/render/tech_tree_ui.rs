use std::collections::HashMap;
use crate::colony::{ScienceState, TechTree};
use crate::parts::PartDefinitions;
use super::types::TechTreeScreenAction;

const NODE_W: f32 = 160.0;
const NODE_H: f32 = 35.0;
const COL_SPACING: f32 = 285.0;
const ROW_SPACING: f32 = 50.0;
const PADDING: f32 = 20.0;

/// Description of what each efficiency line improves.
fn line_affects(line_id: &str) -> &'static [&'static str] {
    match line_id {
        "mining" => &["Mine output (all resources)", "Base: 2,000 kg/day per mine"],
        "metallurgy" => &["Metal Smelting throughput", "Alloy Forging throughput (unlocked at T5)"],
        "electronics_mfg" => &[
            "Electronics Manufacturing throughput",
            "Superconductor Fabrication throughput (unlocked at T6)",
        ],
        "agriculture" => &["Greenhouse food output", "Basic: 0.5 kg/day, Advanced: 2.5 kg/day"],
        "chemical_processing" => &[
            "Electrolysis throughput",
            "Sabatier Reaction throughput (unlocked at T3)",
            "Kerosene Refining throughput (unlocked at T3)",
            "Deuterium Extraction throughput (unlocked at T8)",
        ],
        "atmospheric_science" => &["Atmospheric Collector output", "Base: 10,000 kg/day per collector"],
        "nuclear_engineering" => &[
            "Uranium Enrichment throughput (unlocked at T3)",
            "Tritium Breeding throughput",
            "Nuclear Pulse Unit Assembly throughput (unlocked at T5)",
        ],
        "isotope_extraction" => &[
            "Regolith He-3 Extraction throughput",
            "Gas Giant He-3 Separation throughput (unlocked at T5)",
        ],
        "construction" => &[
            "Robot assembly rate",
            "Robot maintenance capacity",
            "Ship Part Manufacturing speed (unlocked at T8)",
        ],
        "precision_mfg" => &["Precision Instruments manufacturing throughput"],
        "life_support" => &["Habitat maintenance cost reduction", "Greenhouse maintenance cost reduction"],
        _ => &[],
    }
}

fn era_label(era: u32) -> &'static str {
    match era {
        1 => "Era 1: Early Chemical",
        2 => "Era 2: Advanced Chemical",
        3 => "Era 3: Nuclear & Electric",
        4 => "Era 4: Interplanetary",
        5 => "Era 5: Nuclear Pulse",
        6 => "Era 6: Fusion",
        7 => "Era 7: Antimatter Research",
        8 => "Era 8: Antimatter Engines",
        9 => "Era 9: Endgame",
        _ => "Unknown Era",
    }
}

/// Render the full-screen tech tree. Mutates tech_tree and science directly for unlock/upgrade.
/// Returns only navigation actions.
pub fn render_tech_tree_screen(
    ctx: &egui::Context,
    tech_tree: &mut TechTree,
    science: &mut ScienceState,
    warp_levels: &[f64],
    current_warp_index: usize,
    date_str: &str,
    paused: bool,
    active_toasts: &[(String, web_time::Instant)],
    part_defs: &PartDefinitions,
) -> TechTreeScreenAction {
    let mut action = TechTreeScreenAction::None;

    // === Top panel ===
    egui::TopBottomPanel::top("tech_tree_top_panel").show(ctx, |ui| {
        ui.horizontal(|ui| {
            if ui.button("Back").clicked() {
                action = TechTreeScreenAction::Back;
            }
            ui.separator();

            ui.heading(
                egui::RichText::new("Tech Tree")
                    .size(18.0)
                    .color(egui::Color32::WHITE),
            );
            ui.separator();

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
                    action = TechTreeScreenAction::ChangeWarp(i);
                }
            }
            ui.separator();
            ui.label(date_str);

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new(format!("Science: {:.0}", science.available))
                        .size(16.0)
                        .color(egui::Color32::from_rgb(100, 180, 255)),
                );
            });
        });
    });

    // === Pause overlay ===
    if paused {
        egui::Area::new(egui::Id::new("tech_tree_pause_overlay"))
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
                            if ui
                                .button(egui::RichText::new("Back").size(18.0))
                                .clicked()
                            {
                                action = TechTreeScreenAction::Back;
                            }
                        });
                    });
            });

        super::flight::render_toasts(ctx, active_toasts);
        return action;
    }

    // Get selected node/line/part from egui temp data
    let selected_node_key = egui::Id::new("tech_tree_selected_node");
    let selected_line_key = egui::Id::new("tech_tree_selected_line");
    let selected_part_key = egui::Id::new("tech_tree_selected_part");
    let selected_node_id: Option<String> =
        ctx.data_mut(|d| d.get_temp::<Option<String>>(selected_node_key).flatten());
    let selected_line_id: Option<String> =
        ctx.data_mut(|d| d.get_temp::<Option<String>>(selected_line_key).flatten());
    let selected_part_name: Option<String> =
        ctx.data_mut(|d| d.get_temp::<Option<String>>(selected_part_key).flatten());

    // === Detail side panel (right) — tech node ===
    if let Some(ref sel_id) = selected_node_id {
        if let Some(node) = tech_tree.nodes.iter().find(|n| n.id == *sel_id).cloned() {
            let is_unlocked = tech_tree.is_unlocked(&node.id);
            let can_unlock = tech_tree.can_unlock(&node.id);
            let affordable = science.available >= node.cost;

            egui::SidePanel::right("tech_detail_panel")
                .exact_width(210.0)
                .resizable(false)
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        ui.heading(
                            egui::RichText::new(&node.name)
                                .size(18.0)
                                .color(egui::Color32::WHITE),
                        );
                        ui.label(
                            egui::RichText::new(format!("{} | {}", era_label(node.era), node.id))
                                .size(11.0)
                                .color(egui::Color32::GRAY),
                        );
                        ui.add_space(8.0);

                        // Status badge
                        let (status_text, status_color) = if is_unlocked {
                            ("Unlocked", egui::Color32::from_rgb(80, 180, 80))
                        } else if can_unlock && affordable {
                            ("Available", egui::Color32::from_rgb(80, 140, 220))
                        } else if can_unlock {
                            ("Need Science", egui::Color32::from_rgb(180, 140, 60))
                        } else {
                            ("Locked", egui::Color32::from_rgb(120, 120, 120))
                        };
                        ui.label(
                            egui::RichText::new(status_text)
                                .color(status_color)
                                .strong(),
                        );
                        ui.add_space(4.0);

                        ui.label(format!("Cost: {:.0} Science", node.cost));
                        ui.add_space(8.0);

                        // Prerequisites
                        if !node.prerequisites.is_empty() {
                            ui.label(
                                egui::RichText::new("Prerequisites:")
                                    .size(13.0)
                                    .strong(),
                            );
                            for pre in &node.prerequisites {
                                let met = tech_tree.is_unlocked(pre);
                                let marker = if met { "\u{2713}" } else { "\u{1f512}" };
                                let color = if met {
                                    egui::Color32::from_rgb(80, 180, 80)
                                } else {
                                    egui::Color32::from_rgb(180, 80, 80)
                                };
                                let display = tech_tree
                                    .nodes
                                    .iter()
                                    .find(|n| n.id == *pre)
                                    .map(|n| n.name.as_str())
                                    .unwrap_or(pre.as_str());
                                ui.colored_label(color, format!("  {} {}", marker, display));
                            }
                            ui.add_space(8.0);
                        }

                        // Unlocks Parts (clickable for info)
                        if !node.unlocks_parts.is_empty() {
                            ui.label(
                                egui::RichText::new("Unlocks Parts:")
                                    .size(13.0)
                                    .strong(),
                            );
                            for part in &node.unlocks_parts {
                                let is_selected_part = selected_part_name.as_ref() == Some(part);
                                let color = if is_selected_part {
                                    egui::Color32::from_rgb(100, 180, 255)
                                } else {
                                    egui::Color32::from_rgb(160, 200, 255)
                                };
                                if ui.link(
                                    egui::RichText::new(format!("  {}", part))
                                        .color(color)
                                        .size(12.0),
                                ).clicked() {
                                    let new_val = if is_selected_part { None } else { Some(part.clone()) };
                                    ctx.data_mut(|d| {
                                        d.insert_temp::<Option<String>>(selected_part_key, new_val);
                                    });
                                }
                            }
                            ui.add_space(8.0);
                        }

                        // Part info sub-panel (when a part is clicked)
                        if let Some(ref part_name) = selected_part_name {
                            if node.unlocks_parts.contains(part_name) {
                                if let Some(def) = part_defs.find_by_name(part_name) {
                                    ui.separator();
                                    crate::editor::render_part_info(ui, def, "tech_tree");
                                }
                            }
                        }

                        // Unlocks Buildings
                        if !node.unlocks_buildings.is_empty() {
                            ui.label(
                                egui::RichText::new("Unlocks Buildings:")
                                    .size(13.0)
                                    .strong(),
                            );
                            for bt in &node.unlocks_buildings {
                                ui.label(format!("  {}", bt.display_name()));
                            }
                            ui.add_space(8.0);
                        }

                        // Unlock button
                        if !is_unlocked && can_unlock && affordable {
                            ui.add_space(8.0);
                            if ui
                                .button(
                                    egui::RichText::new(format!(
                                        "Unlock ({:.0} sci)",
                                        node.cost
                                    ))
                                    .size(14.0),
                                )
                                .clicked()
                            {
                                let cost = node.cost;
                                if tech_tree.unlock(&node.id) {
                                    science.available -= cost;
                                }
                            }
                        }

                        ui.add_space(12.0);
                        if ui
                            .link(
                                egui::RichText::new("Close")
                                    .size(12.0)
                                    .color(egui::Color32::GRAY),
                            )
                            .clicked()
                        {
                            ctx.data_mut(|d| {
                                d.insert_temp::<Option<String>>(selected_node_key, None)
                            });
                        }
                    });
                });
        }
    }

    // === Detail side panel (right) — efficiency line ===
    if selected_node_id.is_none() {
        if let Some(ref sel_id) = selected_line_id {
            if let Some(line) = tech_tree.efficiency_lines.iter().find(|l| l.id == *sel_id).cloned() {
                let current_tier = tech_tree.line_tier(&line.id);
                let prereq_met = tech_tree.is_unlocked(&line.prerequisite_node);
                let line_prereq_met = line
                    .prerequisite_line_tier
                    .as_ref()
                    .map_or(true, |(pre_line, pre_tier)| {
                        tech_tree.line_tier(pre_line) >= *pre_tier
                    });
                let available = prereq_met && line_prereq_met;

                egui::SidePanel::right("tech_detail_panel")
                    .exact_width(210.0)
                    .resizable(false)
                    .show(ctx, |ui| {
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            // Line name
                            ui.heading(
                                egui::RichText::new(&line.name)
                                    .size(18.0)
                                    .color(egui::Color32::WHITE),
                            );
                            ui.label(
                                egui::RichText::new(format!("Efficiency Line | {}", line.id))
                                    .size(11.0)
                                    .color(egui::Color32::GRAY),
                            );
                            ui.add_space(8.0);

                            // Status badge
                            let (status_text, status_color) = if current_tier >= 15 {
                                ("Maxed", egui::Color32::from_rgb(80, 180, 80))
                            } else if current_tier > 0 {
                                ("In Progress", egui::Color32::from_rgb(80, 140, 220))
                            } else if available {
                                ("Available", egui::Color32::from_rgb(80, 140, 220))
                            } else {
                                ("Locked", egui::Color32::from_rgb(120, 120, 120))
                            };
                            ui.label(
                                egui::RichText::new(if current_tier > 0 && current_tier < 15 {
                                    format!("Tier {}/15", current_tier)
                                } else {
                                    status_text.to_string()
                                })
                                .color(status_color)
                                .strong(),
                            );
                            ui.add_space(4.0);

                            // Current efficiency
                            let mult = TechTree::tier_multiplier(current_tier);
                            ui.label(
                                egui::RichText::new(format!("{:.0}% efficiency", mult * 100.0))
                                    .color(egui::Color32::from_rgb(100, 200, 100)),
                            );
                            ui.add_space(8.0);

                            // Affects section
                            let affects = line_affects(&line.id);
                            if !affects.is_empty() {
                                ui.label(
                                    egui::RichText::new("Affects:")
                                        .size(13.0)
                                        .strong(),
                                );
                                for desc in affects {
                                    ui.label(
                                        egui::RichText::new(format!("  {}", desc))
                                            .size(12.0)
                                            .color(egui::Color32::from_rgb(180, 180, 180)),
                                    );
                                }
                                ui.add_space(8.0);
                            }

                            // Next upgrade section
                            if current_tier < 15 {
                                let next_mult = TechTree::tier_multiplier(current_tier + 1);
                                ui.label(format!("Next tier: {:.0}% efficiency", next_mult * 100.0));
                                if let Some(cost) = tech_tree.tier_cost(&line.id) {
                                    ui.label(format!("Cost: {:.0} Science", cost));
                                    ui.add_space(4.0);

                                    let affordable = science.available >= cost;
                                    if available && affordable {
                                        if ui
                                            .button(
                                                egui::RichText::new(format!(
                                                    "Upgrade ({:.0} sci)",
                                                    cost
                                                ))
                                                .size(14.0),
                                            )
                                            .clicked()
                                        {
                                            if tech_tree.upgrade_line(&line.id) {
                                                science.available -= cost;
                                            }
                                        }
                                    }
                                }
                                ui.add_space(8.0);
                            }

                            // Prerequisites
                            ui.label(
                                egui::RichText::new("Prerequisites:")
                                    .size(13.0)
                                    .strong(),
                            );
                            let prereq_display = tech_tree
                                .nodes
                                .iter()
                                .find(|n| n.id == line.prerequisite_node)
                                .map(|n| n.name.as_str())
                                .unwrap_or(line.prerequisite_node.as_str());
                            let marker = if prereq_met { "\u{2713}" } else { "\u{1f512}" };
                            let color = if prereq_met {
                                egui::Color32::from_rgb(80, 180, 80)
                            } else {
                                egui::Color32::from_rgb(180, 80, 80)
                            };
                            ui.colored_label(color, format!("  {} {}", marker, prereq_display));

                            if let Some((ref pre_line_id, pre_tier)) = line.prerequisite_line_tier {
                                let pre_line_name = tech_tree
                                    .efficiency_lines
                                    .iter()
                                    .find(|l| l.id == *pre_line_id)
                                    .map(|l| l.name.as_str())
                                    .unwrap_or(pre_line_id.as_str());
                                let met = tech_tree.line_tier(pre_line_id) >= pre_tier;
                                let marker = if met { "\u{2713}" } else { "\u{1f512}" };
                                let color = if met {
                                    egui::Color32::from_rgb(80, 180, 80)
                                } else {
                                    egui::Color32::from_rgb(180, 80, 80)
                                };
                                ui.colored_label(
                                    color,
                                    format!("  {} {} Tier {}", marker, pre_line_name, pre_tier),
                                );
                            }
                            ui.add_space(8.0);

                            // Recipe gates
                            if !line.recipe_gates.is_empty() {
                                ui.label(
                                    egui::RichText::new("Unlocked Recipes:")
                                        .size(13.0)
                                        .strong(),
                                );
                                for (tier, recipe) in &line.recipe_gates {
                                    let unlocked = current_tier >= *tier;
                                    let marker = if unlocked { "\u{2713}" } else { "\u{25cb}" };
                                    let color = if unlocked {
                                        egui::Color32::from_rgb(80, 180, 80)
                                    } else {
                                        egui::Color32::from_rgb(180, 80, 80)
                                    };
                                    ui.colored_label(
                                        color,
                                        format!("  {} T{}: {}", marker, tier, recipe),
                                    );
                                }
                            } else {
                                ui.label(
                                    egui::RichText::new("No recipe gates")
                                        .size(12.0)
                                        .color(egui::Color32::GRAY)
                                        .italics(),
                                );
                            }

                            ui.add_space(12.0);
                            if ui
                                .link(
                                    egui::RichText::new("Close")
                                        .size(12.0)
                                        .color(egui::Color32::GRAY),
                                )
                                .clicked()
                            {
                                ctx.data_mut(|d| {
                                    d.insert_temp::<Option<String>>(selected_line_key, None)
                                });
                            }
                        });
                    });
            }
        }
    }

    // === Central panel: graph ===
    egui::CentralPanel::default().show(ctx, |ui| {
        egui::ScrollArea::both()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                render_graph(
                    ui,
                    ctx,
                    tech_tree,
                    science,
                    selected_node_key,
                    selected_line_key,
                    &selected_node_id,
                    &selected_line_id,
                );
            });
    });

    // Toast notifications
    super::flight::render_toasts(ctx, active_toasts);

    action
}

fn render_graph(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    tech_tree: &mut TechTree,
    _science: &mut ScienceState,
    selected_node_key: egui::Id,
    selected_line_key: egui::Id,
    selected_node_id: &Option<String>,
    selected_line_id: &Option<String>,
) {
    // Compute max col/row from both nodes and efficiency lines
    let max_col = tech_tree
        .nodes
        .iter()
        .map(|n| n.col)
        .chain(tech_tree.efficiency_lines.iter().map(|l| l.col))
        .max()
        .unwrap_or(0);
    let max_row = tech_tree
        .nodes
        .iter()
        .map(|n| n.row)
        .chain(tech_tree.efficiency_lines.iter().map(|l| l.row))
        .max()
        .unwrap_or(0);

    // Build tech node rects
    let mut node_rects: HashMap<String, egui::Rect> = HashMap::new();
    for node in &tech_tree.nodes {
        let x = node.col as f32 * COL_SPACING + PADDING;
        let y = node.row as f32 * ROW_SPACING + PADDING;
        let rect = egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(NODE_W, NODE_H));
        node_rects.insert(node.id.clone(), rect);
    }

    // Build efficiency line rects
    let mut line_rects: HashMap<String, egui::Rect> = HashMap::new();
    for line in &tech_tree.efficiency_lines {
        let x = line.col as f32 * COL_SPACING + PADDING;
        let y = line.row as f32 * ROW_SPACING + PADDING;
        let rect = egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(NODE_W, NODE_H));
        line_rects.insert(line.id.clone(), rect);
    }

    // Allocate painter area
    let canvas_w = (max_col + 1) as f32 * COL_SPACING + NODE_W + 2.0 * PADDING;
    let canvas_h = (max_row + 1) as f32 * ROW_SPACING + NODE_H + 2.0 * PADDING;
    let (response, painter) =
        ui.allocate_painter(egui::vec2(canvas_w, canvas_h), egui::Sense::click());
    let origin = response.rect.min;

    // === Draw arrows for tech nodes ===
    for node in &tech_tree.nodes {
        if let Some(dest_rect) = node_rects.get(&node.id) {
            for pre_id in &node.prerequisites {
                if let Some(src_rect) = node_rects.get(pre_id) {
                    let met = tech_tree.is_unlocked(pre_id);
                    let color = if met {
                        egui::Color32::from_rgb(80, 160, 80)
                    } else {
                        egui::Color32::from_rgb(80, 80, 80)
                    };
                    draw_arrow(&painter, origin, src_rect, dest_rect, color);
                }
            }
        }
    }

    // === Draw arrows for efficiency line nodes ===
    for line in &tech_tree.efficiency_lines {
        if let Some(dest_rect) = line_rects.get(&line.id) {
            // Arrow from prerequisite_node -> line
            if let Some(src_rect) = node_rects.get(&line.prerequisite_node) {
                let met = tech_tree.is_unlocked(&line.prerequisite_node);
                let color = if met {
                    egui::Color32::from_rgb(80, 160, 80)
                } else {
                    egui::Color32::from_rgb(80, 80, 80)
                };
                draw_arrow(&painter, origin, src_rect, dest_rect, color);
            }

            // Arrow from prerequisite line -> this line (if any)
            if let Some((ref pre_line_id, pre_tier)) = line.prerequisite_line_tier {
                if let Some(src_rect) = line_rects.get(pre_line_id) {
                    let met = tech_tree.line_tier(pre_line_id) >= pre_tier;
                    let color = if met {
                        egui::Color32::from_rgb(80, 160, 80)
                    } else {
                        egui::Color32::from_rgb(80, 80, 80)
                    };
                    draw_arrow(&painter, origin, src_rect, dest_rect, color);
                }
            }
        }
    }

    // === Build connected set for selection highlighting ===
    let mut connected_ids: std::collections::HashSet<String> = std::collections::HashSet::new();

    if let Some(ref sel_id) = selected_node_id {
        // Prerequisites of selected tech node
        if let Some(sel_node) = tech_tree.nodes.iter().find(|n| &n.id == sel_id) {
            for pre in &sel_node.prerequisites {
                connected_ids.insert(pre.clone());
            }
        }
        // Tech nodes that depend on selected
        for node in &tech_tree.nodes {
            if node.prerequisites.iter().any(|p| p == sel_id) {
                connected_ids.insert(node.id.clone());
            }
        }
        // Efficiency lines that depend on selected tech node
        for line in &tech_tree.efficiency_lines {
            if line.prerequisite_node == *sel_id {
                connected_ids.insert(line.id.clone());
            }
        }
    }

    if let Some(ref sel_id) = selected_line_id {
        // Prerequisite tech node of selected line
        if let Some(sel_line) = tech_tree.efficiency_lines.iter().find(|l| &l.id == sel_id) {
            connected_ids.insert(sel_line.prerequisite_node.clone());
            // Prerequisite line
            if let Some((ref pre_line_id, _)) = sel_line.prerequisite_line_tier {
                connected_ids.insert(pre_line_id.clone());
            }
        }
        // Lines that depend on selected line via prerequisite_line_tier
        for line in &tech_tree.efficiency_lines {
            if let Some((ref pre_line_id, _)) = line.prerequisite_line_tier {
                if pre_line_id == sel_id {
                    connected_ids.insert(line.id.clone());
                }
            }
        }
    }

    // === Draw tech nodes ===
    let mut clicked_node: Option<String> = None;
    let mut clicked_line: Option<String> = None;

    for node in &tech_tree.nodes {
        if let Some(rect) = node_rects.get(&node.id) {
            let screen_rect = rect.translate(origin.to_vec2());

            let is_unlocked = tech_tree.is_unlocked(&node.id);
            let can_unlock = tech_tree.can_unlock(&node.id);
            let is_selected = selected_node_id.as_ref() == Some(&node.id);
            let is_connected = connected_ids.contains(&node.id);

            let fill = if is_unlocked {
                egui::Color32::from_rgb(40, 100, 40)
            } else if can_unlock {
                egui::Color32::from_rgb(30, 60, 120)
            } else {
                egui::Color32::from_rgb(40, 40, 45)
            };

            let (stroke_width, stroke_color) = if is_selected {
                (2.0, egui::Color32::from_rgb(80, 140, 220))
            } else if is_connected {
                (2.0, egui::Color32::WHITE)
            } else if response.hover_pos().map_or(false, |p| screen_rect.contains(p)) {
                (1.0, egui::Color32::from_rgb(200, 200, 200))
            } else {
                (1.0, egui::Color32::from_rgb(70, 70, 80))
            };

            painter.rect(
                screen_rect,
                egui::Rounding::same(4.0),
                fill,
                egui::Stroke::new(stroke_width, stroke_color),
            );

            // Node name (truncate if too long)
            let name = if node.name.len() > 22 {
                format!("{}..", &node.name[..20])
            } else {
                node.name.clone()
            };
            painter.text(
                screen_rect.center(),
                egui::Align2::CENTER_CENTER,
                &name,
                egui::FontId::proportional(12.0),
                egui::Color32::WHITE,
            );

            // Click detection
            if response.clicked() {
                if let Some(pos) = response.interact_pointer_pos() {
                    if screen_rect.contains(pos) {
                        clicked_node = Some(node.id.clone());
                    }
                }
            }
        }
    }

    // === Draw efficiency line nodes ===
    for line in &tech_tree.efficiency_lines {
        if let Some(rect) = line_rects.get(&line.id) {
            let screen_rect = rect.translate(origin.to_vec2());

            let current_tier = tech_tree.line_tier(&line.id);
            let prereq_met = tech_tree.is_unlocked(&line.prerequisite_node);
            let line_prereq_met = line
                .prerequisite_line_tier
                .as_ref()
                .map_or(true, |(pre_line, pre_tier)| {
                    tech_tree.line_tier(pre_line) >= *pre_tier
                });
            let available = prereq_met && line_prereq_met;

            let is_selected = selected_line_id.as_ref() == Some(&line.id);
            let is_connected = connected_ids.contains(&line.id);

            // Distinct fill colors for line nodes
            let fill = if current_tier >= 15 {
                egui::Color32::from_rgb(40, 100, 40) // Maxed — dark green
            } else if current_tier > 0 {
                egui::Color32::from_rgb(30, 80, 80) // In progress — dark teal
            } else if available {
                egui::Color32::from_rgb(30, 60, 120) // Available — dark blue
            } else {
                egui::Color32::from_rgb(40, 40, 45) // Locked — dark gray
            };

            let (stroke_width, stroke_color) = if is_selected {
                (2.0, egui::Color32::from_rgb(80, 140, 220))
            } else if is_connected {
                (2.0, egui::Color32::WHITE)
            } else if response.hover_pos().map_or(false, |p| screen_rect.contains(p)) {
                (1.0, egui::Color32::from_rgb(200, 200, 200))
            } else {
                (1.0, egui::Color32::from_rgb(70, 70, 80))
            };

            painter.rect(
                screen_rect,
                egui::Rounding::same(4.0),
                fill,
                egui::Stroke::new(stroke_width, stroke_color),
            );

            // Label: always show "Name (tier/15)"
            let label = format!("{} ({}/15)", line.name, current_tier);
            let label = if label.len() > 22 {
                format!("{}..", &label[..20])
            } else {
                label
            };
            painter.text(
                screen_rect.center(),
                egui::Align2::CENTER_CENTER,
                &label,
                egui::FontId::proportional(12.0),
                egui::Color32::WHITE,
            );

            // Click detection
            if response.clicked() {
                if let Some(pos) = response.interact_pointer_pos() {
                    if screen_rect.contains(pos) {
                        clicked_line = Some(line.id.clone());
                    }
                }
            }
        }
    }

    // Apply clicks — clicking a node clears line selection and vice versa
    if let Some(id) = clicked_node {
        ctx.data_mut(|d| {
            d.insert_temp::<Option<String>>(selected_node_key, Some(id));
            d.insert_temp::<Option<String>>(selected_line_key, None);
        });
    } else if let Some(id) = clicked_line {
        ctx.data_mut(|d| {
            d.insert_temp::<Option<String>>(selected_line_key, Some(id));
            d.insert_temp::<Option<String>>(selected_node_key, None);
        });
    }
}

/// Draw an arrow from the right edge of src_rect to the left edge of dest_rect.
fn draw_arrow(
    painter: &egui::Painter,
    origin: egui::Pos2,
    src_rect: &egui::Rect,
    dest_rect: &egui::Rect,
    color: egui::Color32,
) {
    let from = egui::pos2(
        origin.x + src_rect.max.x,
        origin.y + src_rect.center().y,
    );
    let to = egui::pos2(
        origin.x + dest_rect.min.x,
        origin.y + dest_rect.center().y,
    );

    painter.line_segment([from, to], egui::Stroke::new(1.5, color));

    // Arrowhead
    let dir = (to - from).normalized();
    let perp = egui::vec2(-dir.y, dir.x);
    let arrow_size = 6.0;
    let p1 = to - dir * arrow_size + perp * (arrow_size * 0.5);
    let p2 = to - dir * arrow_size - perp * (arrow_size * 0.5);
    painter.add(egui::Shape::convex_polygon(
        vec![to, p1, p2],
        color,
        egui::Stroke::NONE,
    ));
}
