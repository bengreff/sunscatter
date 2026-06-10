use crate::parts::{FuelType, PartCategory, PartDefinitions, PartSize, PlacedPartId};
use super::{EditorState, ShipStats};

/// Drag payload for staging panel: either a part or a whole stage being reordered
#[derive(Clone, Copy)]
enum StagingDrag {
    Part(PlacedPartId),
    Stage(usize),
}

/// Format mass - shows tonnes if >= 1000 kg, otherwise kg
fn format_mass(kg: f64) -> String {
    if kg >= 1000.0 {
        format!("{:.2} t", kg / 1000.0)
    } else {
        format!("{:.0} kg", kg)
    }
}

/// Format power with appropriate SI prefix (W, kW, MW, GW, TW)
fn format_power(watts: f64) -> String {
    if watts >= 1e12 {
        format!("{:.1} TW", watts / 1e12)
    } else if watts >= 1e9 {
        format!("{:.1} GW", watts / 1e9)
    } else if watts >= 1e6 {
        format!("{:.1} MW", watts / 1e6)
    } else if watts >= 1e3 {
        format!("{:.1} kW", watts / 1e3)
    } else {
        format!("{:.0} W", watts)
    }
}

/// Format seconds into a human-readable duration string (e.g., "1d 2h 3m 4s")
fn format_duration(seconds: f64) -> String {
    if !seconds.is_finite() || seconds < 0.0 {
        return "---".to_string();
    }
    let total = seconds as u64;
    let d = total / 86400;
    let h = (total % 86400) / 3600;
    let m = (total % 3600) / 60;
    let s = total % 60;
    if d > 0 {
        format!("{}d {}h {}m {}s", d, h, m, s)
    } else if h > 0 {
        format!("{}h {}m {}s", h, m, s)
    } else if m > 0 {
        format!("{}m {}s", m, s)
    } else {
        format!("{}s", s)
    }
}

/// Editor UI action that should be handled by the game
#[derive(Debug, Clone)]
pub enum EditorAction {
    None,
    Launch,
    SaveBlueprint(String),
    LoadBlueprint(String),
    DeleteBlueprint(String),
    /// Pick a blueprint `.ron` from disk and install it into the registry.
    ImportBlueprint,
    /// Save the blueprint with this stored name to a user-chosen file location.
    ExportBlueprint(String),
    NewVessel,
    OpenContracts,
}

/// Body info for TWR calculation
pub struct BodyInfo {
    pub name: String,
    pub surface_gravity: f64,
}

/// Format mass flow rate with appropriate units (g/s, kg/s, t/s)
fn format_mass_flow(kg_s: f64) -> String {
    if kg_s >= 1000.0 {
        format!("{:.2} t/s", kg_s / 1000.0)
    } else if kg_s >= 0.1 {
        format!("{:.2} kg/s", kg_s)
    } else if kg_s >= 0.001 {
        format!("{:.2} g/s", kg_s * 1000.0)
    } else if kg_s > 0.0 {
        format!("{:.3} g/s", kg_s * 1000.0)
    } else {
        "0 kg/s".to_string()
    }
}

/// Format delta-v for display
fn format_delta_v(dv: f64) -> String {
    if dv >= 1000.0 {
        format!("{:.1} km/s", dv / 1000.0)
    } else {
        format!("{:.0} m/s", dv)
    }
}

/// Render part definition info (name, stats, engine/tank/pod details).
/// Reusable across editor info panel and tech tree detail panel.
/// `id_prefix` is used to disambiguate egui indent IDs across call sites.
pub fn render_part_info(ui: &mut egui::Ui, def: &crate::parts::PartDefinition, id_prefix: &str) {
    use crate::colony::economy::{material_breakdown_lines, part_dry_earth_cost};
    use crate::colony::format_money;

    ui.heading(&def.name);
    ui.label(&def.description);

    ui.separator();
    ui.label(format!("Size: {}", def.size.display_name()));
    ui.label(format!("Mass: {:.3} t ({:.0} kg)", def.mass, def.mass * 1000.0));
    ui.label(format!("Cost: {}", format_money(part_dry_earth_cost(def))));
    for line in material_breakdown_lines(def) {
        ui.label(line);
    }
    ui.label(format!("Dimensions: {}x{} grid", def.grid_width, def.grid_height));

    // Engine info
    if let Some(ref engine) = def.engine {
        ui.separator();
        ui.heading("Engine Stats");

        if let Some(secondary) = engine.secondary_propellant {
            ui.label(format!("Propellant: {} + {}", engine.propellant.display_name(), secondary.display_name()));
        } else {
            ui.label(format!("Propellant: {}", engine.propellant.display_name()));
        }

        ui.label("Thrust:");
        ui.indent(format!("{}_thrust_indent", id_prefix), |ui| {
            ui.label(format!("Vacuum: {:.1} kN", engine.thrust_vac));
            ui.label(format!("Sea Level: {:.1} kN", engine.thrust_asl));
        });

        ui.label("Specific Impulse:");
        ui.indent(format!("{}_isp_indent", id_prefix), |ui| {
            ui.label(format!("Vacuum: {:.0} s", engine.isp_vac));
            ui.label(format!("Sea Level: {:.0} s", engine.isp_asl));
        });

        ui.label("Mass Flow (vacuum):");
        ui.indent(format!("{}_flow_indent", id_prefix), |ui| {
            for (name, rate) in engine.fuel_flows_display() {
                ui.label(format!("{}: {}", name, format_mass_flow(rate)));
            }
        });

        ui.label("Gimbal:");
        ui.indent(format!("{}_gimbal_indent", id_prefix), |ui| {
            if engine.gimbal_range > 0.0 {
                ui.label(format!("Range: ±{:.1}°", engine.gimbal_range));
            } else {
                ui.label("Fixed (no gimbal)");
            }
        });

        if engine.throttleable {
            ui.label("Throttleable: Yes");
        } else {
            ui.label("Throttleable: No");
        }

        if engine.waste_heat_watts > 0.0 {
            ui.label(format!("Waste heat: {} (full throttle)", format_power(engine.waste_heat_watts)));
        }

        // TWR calculation for this engine alone
        ui.separator();
        ui.label("Single Engine TWR:");
        let engine_twr = engine.thrust_vac / (def.mass * 9.81);
        ui.label(format!("  {:.1} (vacuum, Earth)", engine_twr));
    }

    // Tank info
    if let Some(ref tank) = def.tank {
        ui.separator();
        ui.heading("Tank Stats");
        ui.label(format!("Dry Mass: {:.0} kg", def.mass * 1000.0));

        ui.separator();

        if let Some(locked) = tank.fixed_fuel_type {
            // Specialized tank: show capacity for its locked fuel only.
            let (ox, fuel) = tank.propellant_capacity(locked);
            ui.label(format!("Fuel: {} (locked)", locked.display_name()));
            ui.label(format!("Capacity: {}", format_mass(ox + fuel)));
        } else {
            // Standard tank: show capacities for the three common chemical propellants.
            ui.label("Propellant Capacity:");
            let (ox, fuel) = tank.propellant_capacity(FuelType::Rp1);
            ui.label(format!("  RP-1: {}", format_mass(ox + fuel)));
            let (ox, fuel) = tank.propellant_capacity(FuelType::Methane);
            ui.label(format!("  CH4: {}", format_mass(ox + fuel)));
            let (ox, fuel) = tank.propellant_capacity(FuelType::Hydrogen);
            ui.label(format!("  LH2: {}", format_mass(ox + fuel)));
        }
    }

    // Pod info
    if let Some(ref pod) = def.pod {
        ui.separator();
        ui.heading("Pod Stats");
        ui.label(format!("Crew Capacity: {}", pod.crew_capacity));
    }

    // RCS info
    if let Some(ref rcs) = def.rcs {
        ui.separator();
        ui.heading("RCS Stats");
        ui.label(format!("Thrust: {:.1} kN", rcs.thrust));
        ui.label(format!("Isp: {:.0} s", rcs.isp));
        ui.label("Fuel: Monopropellant");
    }

    // Reactor info
    if let Some(ref reactor) = def.reactor {
        ui.separator();
        ui.heading("Reactor Stats");
        ui.label(format!("Output: {}", format_power(reactor.output_watts)));
        if reactor.waste_heat_watts > 0.0 {
            ui.label(format!("Waste heat: {}", format_power(reactor.waste_heat_watts)));
        }
    }

    // Radiator info
    if let Some(ref radiator) = def.radiator {
        ui.separator();
        ui.heading("Radiator");
        let tier = match radiator.tier {
            crate::parts::RadiatorTier::HeatPipe => "Heat Pipe (1,200 K)",
            crate::parts::RadiatorTier::Droplet  => "Liquid Droplet (2,500 K)",
            crate::parts::RadiatorTier::Phononic => "Phononic Metamaterial (6,000 K)",
        };
        ui.label(format!("Tier: {}", tier));
        ui.label(format!("Rejection: {} (deployed)", format_power(radiator.rejection_watts)));
        ui.label(format!("Deploy time: {:.1} s", radiator.deploy_time_sec));
        ui.label(egui::RichText::new("Deployable wing — stows for launch / atmospheric flight.")
            .color(egui::Color32::from_rgb(180, 180, 180))
            .small());
    }

    // Shield info
    if let Some(ref shield) = def.shield {
        ui.separator();
        ui.heading("Shield Stats");
        ui.label(format!("Type: {:?}", shield.shield_type));
        ui.label(format!("Max Velocity: {:.0}% c", shield.max_velocity_c * 100.0));
        if shield.power_base_watts > 0.0 {
            ui.label(format!("Power Draw: {}", format_power(shield.power_base_watts)));
        } else {
            ui.label("Power Draw: None (passive)");
        }
    }

    // Parachute info
    if let Some(ref chute) = def.parachute {
        ui.separator();
        ui.heading("Parachute");
        let width_m = chute.deployed_width * crate::parts::GRID_SQUARE_SIZE;
        ui.label(format!("Deployed Width: {:.1} m", width_m));
    }

    // Cargo info
    if let Some(ref cargo) = def.cargo {
        ui.separator();
        ui.heading("Cargo");
        ui.label(format!("Capacity: {}", format_mass(cargo.capacity_kg)));
    }
}

/// Render the editor UI using egui
pub fn render_editor_ui(
    ctx: &egui::Context,
    editor: &mut EditorState,
    part_defs: &PartDefinitions,
    blueprint_names: &[&str],
    locked_blueprints: &std::collections::HashSet<String>,
    stats: &ShipStats,
    bodies: &[BodyInfo],
    stage_delta_vs: &[f64],
    stage_burn_times: &[f64],
    company_money: f64,
    vessel_cost: f64,
    contracts: &crate::colony::ContractManager,
    tech_tree: &crate::colony::TechTree,
) -> EditorAction {
    let mut action = EditorAction::None;

    // Top toolbar
    egui::TopBottomPanel::top("editor_toolbar").show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.heading("Vehicle Editor");
            ui.separator();

            // New button
            if ui.button("New").clicked() {
                action = EditorAction::NewVessel;
            }

            // Save button
            if ui.button("Save").clicked() {
                editor.show_save_dialog = true;
            }

            // Load button
            if ui.button("Load").clicked() {
                editor.show_load_dialog = true;
            }

            ui.separator();

            // Symmetry mode
            ui.label("Symmetry:");
            if ui.button(editor.symmetry_mode.display()).clicked() {
                editor.symmetry_mode = editor.symmetry_mode.cycle_next();
            }

            ui.separator();

            // Vessel name + Launch button
            ui.horizontal(|ui| {
                ui.label("Name:");
                let name_field = ui.add(egui::TextEdit::singleline(&mut editor.vessel_name)
                    .desired_width(120.0));
                if name_field.changed() && editor.vessel_name.is_empty() {
                    editor.vessel_name = "Untitled Vessel".to_string();
                }
            });
            let can_launch = editor.can_launch();
            ui.add_enabled_ui(can_launch, |ui| {
                if ui.button("🚀 Launch").clicked() {
                    action = EditorAction::Launch;
                }
            });

            // Launch warnings
            if can_launch {
                let has_control = editor.parts.values().any(|p| {
                    part_defs.get(&p.definition_id)
                        .and_then(|d| d.pod.as_ref())
                        .map_or(false, |pod| pod.can_control)
                });
                let has_engine = editor.parts.values().any(|p| {
                    part_defs.get(&p.definition_id)
                        .map_or(false, |d| d.engine.is_some())
                });
                let has_fuel = editor.parts.values().any(|p| {
                    p.fuel_type != crate::parts::FuelType::Empty && p.fill_fraction > 0.0
                });
                let warn_color = egui::Color32::from_rgb(220, 180, 60);
                if !has_control {
                    ui.label(egui::RichText::new("⚠ No command pod").size(10.0).color(warn_color));
                }
                if !has_engine {
                    ui.label(egui::RichText::new("⚠ No engines").size(10.0).color(warn_color));
                }
                if has_engine && !has_fuel {
                    ui.label(egui::RichText::new("⚠ No fuel loaded").size(10.0).color(warn_color));
                }
            }

            ui.separator();

            // Contracts button
            if ui.button("Contracts").clicked() {
                action = EditorAction::OpenContracts;
            }

            // Right-aligned info
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(format!("Parts: {}", editor.part_count()));
                ui.separator();
                ui.label(egui::RichText::new(crate::colony::format_money(company_money))
                    .color(egui::Color32::from_rgb(100, 200, 100)));
            });
        });
    });

    // Stats bar (below toolbar)
    egui::TopBottomPanel::top("stats_bar")
        .frame(egui::Frame::none()
            .fill(egui::Color32::from_rgba_unmultiplied(25, 30, 40, 240))
            .inner_margin(egui::Margin::symmetric(8.0, 6.0)))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                // Large stats: Mass, Thrust, TWR
                ui.style_mut().override_text_style = Some(egui::TextStyle::Heading);

                // Mass (wet/dry)
                ui.label(format!("Mass: {:.2} t", stats.wet_mass))
                    .on_hover_text(format!("Wet: {:.2} t / Dry: {:.2} t", stats.wet_mass, stats.dry_mass));
                ui.separator();

                // Thrust
                let thrust = if editor.twr_settings.show_asl {
                    stats.thrust_asl
                } else {
                    stats.thrust_vac
                };
                ui.label(format!("Thrust: {:.1} kN", thrust));
                ui.separator();

                // TWR with body selector
                let gravity = bodies.get(editor.twr_settings.body_index)
                    .map(|b| b.surface_gravity)
                    .unwrap_or(9.81);
                let twr = if editor.twr_settings.show_asl {
                    stats.twr_asl(gravity)
                } else {
                    stats.twr_vac(gravity)
                };
                ui.label(format!("TWR: {:.2}", twr));

                // Body dropdown
                ui.style_mut().override_text_style = Some(egui::TextStyle::Body);
                let current_body = bodies.get(editor.twr_settings.body_index)
                    .map(|b| b.name.as_str())
                    .unwrap_or("Unknown");

                egui::ComboBox::from_id_source("twr_body")
                    .selected_text(current_body)
                    .show_ui(ui, |ui: &mut egui::Ui| {
                        for (i, body) in bodies.iter().enumerate() {
                            ui.selectable_value(
                                &mut editor.twr_settings.body_index,
                                i,
                                &body.name,
                            );
                        }
                    });

                // ASL/Vacuum toggle
                let atmo_label = if editor.twr_settings.show_asl { "ASL" } else { "Vac" };
                if ui.button(atmo_label).clicked() {
                    editor.twr_settings.show_asl = !editor.twr_settings.show_asl;
                }

                // Delta-v + total burn time
                let total_dv: f64 = stage_delta_vs.iter().sum();
                if total_dv > 0.0 {
                    ui.separator();
                    let total_burn: f64 = stage_burn_times.iter().sum();
                    let dv_label = ui.label(format!("Δv: {}", format_delta_v(total_dv)));
                    if total_burn > 0.0 {
                        dv_label.on_hover_text(format!("Total burn: {}", format_duration(total_burn)));
                    }
                }

                // Vessel cost
                if vessel_cost > 0.0 {
                    ui.separator();
                    let cost_color = if vessel_cost <= company_money {
                        egui::Color32::from_rgb(100, 200, 100) // green = affordable
                    } else {
                        egui::Color32::from_rgb(220, 80, 80) // red = too expensive
                    };
                    ui.label(egui::RichText::new(format!("Cost: {}", crate::colony::format_money(vessel_cost)))
                        .color(cost_color));
                }

                // Thermal balance: waste heat vs radiator rejection
                if stats.waste_heat_gen > 0.0 || stats.waste_heat_reject > 0.0 {
                    ui.separator();
                    let gen = stats.waste_heat_gen;
                    let rej = stats.waste_heat_reject;
                    let color = if rej >= gen {
                        egui::Color32::from_rgb(100, 200, 100) // green: balanced
                    } else if rej >= gen * 0.9 {
                        egui::Color32::from_rgb(220, 200, 80) // yellow: marginal
                    } else {
                        egui::Color32::from_rgb(220, 80, 80)  // red: deficit
                    };
                    ui.label(egui::RichText::new(format!(
                        "Thermal: {} / {}",
                        format_power(gen),
                        format_power(rej),
                    )).color(color))
                    .on_hover_text(
                        "Waste heat generated (engines + reactors at full throttle) vs. radiator rejection capacity\n\
                         (sum of all radiators when fully deployed). Exceeding capacity in flight will trip reactors\n\
                         at 1200 K and start melting parts past 1500 K."
                    );
                }

                ui.separator();

                // Resources (smaller text)
                ui.style_mut().override_text_style = Some(egui::TextStyle::Body);

                // Show resources in a consistent order
                if let Some(ox) = stats.resources.get("oxygen") {
                    ui.label(format!("O2: {}", format_mass(ox.current)));
                }
                if let Some(rp1) = stats.resources.get("rp1") {
                    ui.label(format!("RP1: {}", format_mass(rp1.current)));
                }
                if let Some(ch4) = stats.resources.get("methane") {
                    ui.label(format!("CH4: {}", format_mass(ch4.current)));
                }
                if let Some(lh2) = stats.resources.get("hydrogen") {
                    ui.label(format!("LH2: {}", format_mass(lh2.current)));
                }
                if let Some(mp) = stats.resources.get("monopropellant") {
                    ui.label(format!("MP: {}", format_mass(mp.current)));
                }

                // Power stats
                if stats.electricity_capacity > 0.0 || stats.power_generation > 0.0 || stats.power_consumption > 0.0 {
                    ui.separator();
                    if stats.electricity_capacity > 0.0 {
                        if stats.electricity_capacity >= 1000.0 {
                            ui.label(format!("EC: {:.1}k Wh", stats.electricity_capacity / 1000.0));
                        } else {
                            ui.label(format!("EC: {:.0} Wh", stats.electricity_capacity));
                        }
                    }
                    if stats.power_generation > 0.0 || stats.power_consumption > 0.0 {
                        ui.label(format!("Power: +{:.0}W / -{:.0}W", stats.power_generation, stats.power_consumption));
                        let net = stats.power_generation - stats.power_consumption;
                        if net < 0.0 && stats.electricity_capacity > 0.0 {
                            let seconds = (stats.electricity_capacity / net.abs()) * 3600.0;
                            ui.label(format!("Duration: {}", format_duration(seconds)));
                        }
                    }
                }
            });
        });

    // Left panel - Parts palette
    egui::SidePanel::left("parts_palette")
        .default_width(200.0)
        .show(ctx, |ui| {
            ui.heading("Parts");
            ui.separator();

            // Category tabs
            ui.horizontal_wrapped(|ui| {
                for category in PartCategory::all() {
                    let selected = editor.selected_category == *category;
                    if ui.selectable_label(selected, category.display_name()).clicked() {
                        editor.selected_category = *category;
                    }
                }
            });

            ui.separator();

            // Parts list for selected category
            egui::ScrollArea::vertical().show(ui, |ui| {
                let category = editor.selected_category;

                if category == PartCategory::Heat || category == PartCategory::Interstellar {
                    // Flat list for Interstellar (no size sub-grouping)
                    let parts: Vec<_> = part_defs.by_category(category)
                        .into_iter()
                        .filter(|p| tech_tree.is_part_available(&p.name))
                        .collect();
                    if parts.is_empty() {
                        ui.label("No parts in this category");
                    } else {
                        for part in parts {
                            let is_selected = editor.selected_part_def.as_ref() == Some(&part.id);
                            if ui.selectable_label(is_selected, &part.name).clicked() {
                                if is_selected {
                                    editor.deselect();
                                } else {
                                    editor.select_part_def(&part.id);
                                }
                            }
                        }
                    }
                } else {
                    // Grouped by size for all other categories
                    let mut any_parts = false;

                    for size in PartSize::all() {
                        let parts: Vec<_> = part_defs.by_category_and_size(category, *size)
                            .into_iter()
                            .filter(|p| tech_tree.is_part_available(&p.name))
                            .collect();

                        if parts.is_empty() {
                            continue;
                        }
                        any_parts = true;

                        egui::CollapsingHeader::new(size.display_name())
                            .default_open(true)
                            .show(ui, |ui| {
                                for part in parts {
                                    let is_selected = editor.selected_part_def.as_ref() == Some(&part.id);
                                    if ui.selectable_label(is_selected, &part.name).clicked() {
                                        if is_selected {
                                            editor.deselect();
                                        } else {
                                            editor.select_part_def(&part.id);
                                        }
                                    }
                                }
                            });
                    }

                    if !any_parts {
                        ui.label("No parts in this category");
                    }
                }
            });
        });

    // Right panel - Staging (always visible, far right)
    egui::SidePanel::right("staging_panel")
        .default_width(150.0)
        .show(ctx, |ui| {
            ui.heading("Staging");

            // Total Δv
            let total_dv: f64 = stage_delta_vs.iter().sum();
            if total_dv > 0.0 {
                ui.label(egui::RichText::new(format!("Total Δv: {}", format_delta_v(total_dv)))
                    .size(12.0).strong());
            }

            ui.separator();

            egui::ScrollArea::vertical().show(ui, |ui| {
                // Track deferred actions
                let mut insert_stage_at: Option<usize> = None;
                let mut delete_stage_at: Option<usize> = None;
                let mut move_stage_to: Option<(usize, usize)> = None; // (from, to_insert_pos)
                let mut drop_action: Option<(StagingDrag, usize)> = None;
                let mut staging_select: Option<PlacedPartId> = None;

                // Helper: render a "+" gap that is also a drop zone for stage reordering.
                // `insert_pos` is where a new/moved stage would be inserted.
                let plus_gap = |ui: &mut egui::Ui, insert_pos: usize,
                                     insert_out: &mut Option<usize>,
                                     move_out: &mut Option<(usize, usize)>| {
                    let frame = egui::Frame::none();
                    let (inner, dropped) = ui.dnd_drop_zone::<StagingDrag, ()>(frame, |ui| {
                        if ui.small_button("+").on_hover_text("Insert stage here").clicked() {
                            *insert_out = Some(insert_pos);
                        }
                    });
                    // Highlight when a stage is hovering
                    if inner.response.hovered() && egui::DragAndDrop::has_any_payload(ui.ctx()) {
                        inner.response.highlight();
                    }
                    if let Some(payload) = dropped {
                        if let StagingDrag::Stage(from_idx) = *payload {
                            *move_out = Some((from_idx, insert_pos));
                        }
                    }
                };

                // Render stages in reverse order (highest stage number at top)
                for stage_idx in (0..editor.stages.len()).rev() {
                    // "+" gap above this stage
                    plus_gap(ui, stage_idx + 1, &mut insert_stage_at, &mut move_stage_to);

                    let frame = egui::Frame::group(ui.style());
                    let (_, dropped_payload) = ui.dnd_drop_zone::<StagingDrag, ()>(frame, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            let stage_drag_id = egui::Id::new(("staging_stage", stage_idx));
                            ui.dnd_drag_source(stage_drag_id, StagingDrag::Stage(stage_idx), |ui| {
                                ui.label(format!("{}", stage_idx + 1));
                            });
                            // Per-stage Δv
                            let stage_dv = stage_delta_vs.get(stage_idx).copied().unwrap_or(0.0);
                            if stage_dv > 0.0 {
                                ui.label(egui::RichText::new(format_delta_v(stage_dv))
                                    .size(10.0).color(egui::Color32::from_rgb(120, 200, 120)));
                            }
                            // Per-stage burn time
                            let burn_time = stage_burn_times.get(stage_idx).copied().unwrap_or(0.0);
                            if burn_time > 0.0 {
                                ui.label(egui::RichText::new(format_duration(burn_time))
                                    .size(10.0).color(egui::Color32::from_rgb(120, 200, 120)));
                            }
                            // Delete button for empty stages
                            if editor.stages[stage_idx].is_empty() {
                                if ui.small_button("\u{2715}").on_hover_text("Delete empty stage").clicked() {
                                    delete_stage_at = Some(stage_idx);
                                }
                            }
                        });

                        if editor.stages[stage_idx].is_empty() {
                            ui.weak("(empty)");
                        }

                        // Track which partner IDs have been rendered to avoid duplicates
                        let mut rendered_partners: Vec<PlacedPartId> = Vec::new();

                        for &part_id in &editor.stages[stage_idx] {
                            // Skip if this part was already rendered as part of a mirrored pair
                            if rendered_partners.contains(&part_id) {
                                continue;
                            }

                            let part = editor.parts.get(&part_id);
                            let name = part
                                .and_then(|p| part_defs.get(&p.definition_id))
                                .map(|d| d.name.clone())
                                .unwrap_or_else(|| format!("Part {}", part_id));

                            // Check if this part has a mirror partner in the same stage
                            let mirror_in_same_stage = part
                                .and_then(|p| p.mirror_partner)
                                .filter(|mid| editor.stages[stage_idx].contains(mid));

                            let display_name = if let Some(mid) = mirror_in_same_stage {
                                rendered_partners.push(mid);
                                format!("{} x2", name)
                            } else {
                                name
                            };

                            let is_selected = editor.selected_placed_part == Some(part_id)
                                || mirror_in_same_stage.is_some()
                                    && editor.selected_placed_part == mirror_in_same_stage;

                            let item_id = egui::Id::new(("staging_item", part_id));
                            let resp = ui.dnd_drag_source(item_id, StagingDrag::Part(part_id), |ui| {
                                let text = egui::RichText::new(&display_name);
                                let text = if is_selected {
                                    text.color(egui::Color32::from_rgb(128, 179, 255))
                                } else {
                                    text
                                };
                                ui.label(text);
                            });
                            if resp.response.clicked() || resp.response.drag_started() {
                                staging_select = Some(part_id);
                            }
                        }
                    });

                    if let Some(payload) = dropped_payload {
                        drop_action = Some((*payload, stage_idx));
                    }
                }

                // "+" gap below the bottom stage
                plus_gap(ui, 0, &mut insert_stage_at, &mut move_stage_to);

                // Apply deferred actions (only one action per frame)
                if let Some((from_idx, to_pos)) = move_stage_to {
                    if from_idx < editor.stages.len() {
                        let stage = editor.stages.remove(from_idx);
                        // Adjust insertion index after removal
                        let insert_at = if from_idx < to_pos {
                            (to_pos - 1).min(editor.stages.len())
                        } else {
                            to_pos.min(editor.stages.len())
                        };
                        editor.stages.insert(insert_at, stage);
                    }
                } else if let Some(idx) = delete_stage_at {
                    if idx < editor.stages.len() && editor.stages[idx].is_empty() {
                        editor.stages.remove(idx);
                    }
                } else if let Some(idx) = insert_stage_at {
                    editor.stages.insert(idx, Vec::new());
                } else if let Some((drag, target_idx)) = drop_action {
                    match drag {
                        StagingDrag::Part(part_id) => {
                            // Also move mirror partner if linked
                            let mirror_id = editor.parts.get(&part_id).and_then(|p| p.mirror_partner);
                            for stage in &mut editor.stages {
                                stage.retain(|&id| id != part_id && Some(id) != mirror_id);
                            }
                            if target_idx < editor.stages.len() {
                                editor.stages[target_idx].push(part_id);
                                if let Some(mid) = mirror_id {
                                    editor.stages[target_idx].push(mid);
                                }
                            }
                        }
                        StagingDrag::Stage(from_idx) => {
                            if from_idx != target_idx && from_idx < editor.stages.len() {
                                let stage = editor.stages.remove(from_idx);
                                let insert_at = if from_idx < target_idx {
                                    (target_idx - 1).min(editor.stages.len())
                                } else {
                                    target_idx.min(editor.stages.len())
                                };
                                editor.stages.insert(insert_at, stage);
                            }
                        }
                    }
                }

                // Apply staging → grid selection
                if let Some(part_id) = staging_select {
                    editor.selected_placed_part = Some(part_id);
                    editor.selected_part_def = None;
                    editor.ghost_position = None;
                    editor.ghost_valid = false;
                    editor.mirror_ghost_position = None;
                    editor.mirror_ghost_def_id = None;
                }
            });
        });

    // Right panel - Part info (only when a part is selected, renders left of staging)
    let show_info_panel = editor.selected_part_def.is_some() || editor.selected_placed_part.is_some();
    if show_info_panel {
        egui::SidePanel::right("info_panel")
            .default_width(200.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                // Show part definition info when selected from palette
                if let Some(ref def_id) = editor.selected_part_def {
                    if let Some(def) = part_defs.get(def_id) {
                        render_part_info(ui, def, "palette");
                    }
                }
                // Show placed part info when selected
                else if let Some(part_id) = editor.selected_placed_part {
                    if let Some(part) = editor.parts.get(&part_id).cloned() {
                        if let Some(def) = part_defs.get(&part.definition_id) {
                            use crate::colony::economy::{
                                fuel_blended_cost_per_kg, material_breakdown_lines,
                                part_dry_earth_cost, part_filled_fuel_cost,
                            };
                            use crate::colony::format_money;

                            ui.heading(&def.name);
                            ui.label(&def.description);

                            ui.separator();
                            ui.label(format!("Size: {}", def.size.display_name()));
                            ui.label(format!("Mass: {:.3} t ({:.0} kg)", def.mass, def.mass * 1000.0));
                            let dry_cost = part_dry_earth_cost(def);
                            let fuel_cost = part_filled_fuel_cost(def, part.fuel_type, part.fill_fraction);
                            if fuel_cost > 0.0 {
                                let unit = fuel_blended_cost_per_kg(part.fuel_type);
                                ui.label(format!("Cost: {} (dry {} + fuel {} @ ${:.2}/kg)",
                                    format_money(dry_cost + fuel_cost),
                                    format_money(dry_cost),
                                    format_money(fuel_cost),
                                    unit));
                            } else {
                                ui.label(format!("Cost: {}", format_money(dry_cost)));
                            }
                            for line in material_breakdown_lines(def) {
                                ui.label(line);
                            }
                            ui.label(format!("Dimensions: {}x{} grid", def.grid_width, def.grid_height));

                            // Engine info
                            if let Some(ref engine) = def.engine {
                                ui.separator();
                                ui.heading("Engine Stats");

                                if let Some(secondary) = engine.secondary_propellant {
                                    ui.label(format!("Propellant: {} + {}", engine.propellant.display_name(), secondary.display_name()));
                                } else {
                                    ui.label(format!("Propellant: {}", engine.propellant.display_name()));
                                }

                                ui.label("Thrust:");
                                ui.indent("placed_thrust_indent", |ui| {
                                    ui.label(format!("Vacuum: {:.1} kN", engine.thrust_vac));
                                    ui.label(format!("Sea Level: {:.1} kN", engine.thrust_asl));
                                });

                                ui.label("Specific Impulse:");
                                ui.indent("placed_isp_indent", |ui| {
                                    ui.label(format!("Vacuum: {:.0} s", engine.isp_vac));
                                    ui.label(format!("Sea Level: {:.0} s", engine.isp_asl));
                                });

                                ui.label("Mass Flow (vacuum):");
                                ui.indent("placed_flow_indent", |ui| {
                                    for (name, rate) in engine.fuel_flows_display() {
                                        ui.label(format!("{}: {}", name, format_mass_flow(rate)));
                                    }
                                });

                                ui.label("Gimbal:");
                                ui.indent("placed_gimbal_indent", |ui| {
                                    if engine.gimbal_range > 0.0 {
                                        ui.label(format!("Range: ±{:.1}°", engine.gimbal_range));
                                    } else {
                                        ui.label("Fixed (no gimbal)");
                                    }
                                });

                                if engine.throttleable {
                                    ui.label("Throttleable: Yes");
                                } else {
                                    ui.label("Throttleable: No");
                                }

                                // TWR calculation for this engine alone
                                ui.separator();
                                ui.label("Single Engine TWR:");
                                let engine_twr = engine.thrust_vac / (def.mass * 9.81);
                                ui.label(format!("  {:.1} (vacuum, Earth)", engine_twr));
                            }

                            // Tank info with controls
                            if let Some(ref tank) = def.tank {
                                ui.separator();
                                ui.heading("Tank Stats");
                                ui.label(format!("Dry Mass: {:.0} kg", def.mass * 1000.0));

                                ui.separator();

                                let mirror_id = part.mirror_partner;

                                if let Some(locked) = tank.fixed_fuel_type {
                                    // Specialized tank: fuel type is locked; no selector shown.
                                    let unit = crate::colony::economy::fuel_blended_cost_per_kg(locked);
                                    let price_str = if unit > 0.0 {
                                        format!(" — ${:.2}/kg", unit)
                                    } else {
                                        String::new()
                                    };
                                    ui.label(format!("Fuel Type: {} (locked){}",
                                        locked.display_name(), price_str));
                                } else {
                                    ui.label("Fuel Type:");

                                    // Fuel type selector buttons — standard tanks only.
                                    // Specialized fuels (Xenon, Antimatter, Pulse Units) are excluded;
                                    // they require dedicated tank parts.
                                    ui.horizontal_wrapped(|ui| {
                                        for fuel_type in FuelType::all() {
                                            if !fuel_type.is_standard_tank_compatible() {
                                                continue;
                                            }
                                            let selected = part.fuel_type == *fuel_type;
                                            let unit = crate::colony::economy::fuel_blended_cost_per_kg(*fuel_type);
                                            let label = if *fuel_type == FuelType::Empty || unit == 0.0 {
                                                fuel_type.display_name().to_string()
                                            } else if unit < 0.1 {
                                                format!("{} (${:.2}/kg)", fuel_type.display_name(), unit)
                                            } else if unit < 100.0 {
                                                format!("{} (${:.2}/kg)", fuel_type.display_name(), unit)
                                            } else {
                                                format!("{} ({}/kg)",
                                                    fuel_type.display_name(),
                                                    crate::colony::format_money(unit))
                                            };
                                            if ui.selectable_label(selected, label).clicked() {
                                                let new_fill = if *fuel_type != FuelType::Empty { 1.0 } else { 0.0 };
                                                if let Some(p) = editor.parts.get_mut(&part_id) {
                                                    p.fuel_type = *fuel_type;
                                                    p.fill_fraction = new_fill;
                                                }
                                                // Apply to mirror partner
                                                if let Some(mid) = mirror_id {
                                                    if let Some(mp) = editor.parts.get_mut(&mid) {
                                                        mp.fuel_type = *fuel_type;
                                                        mp.fill_fraction = new_fill;
                                                    }
                                                }
                                            }
                                        }
                                    });
                                }

                                // Draggable fuel bars (only if fuel type selected)
                                if part.fuel_type != FuelType::Empty {
                                    ui.separator();
                                    let (ox_cap, fuel_cap) = tank.propellant_capacity(part.fuel_type);
                                    let frac = part.fill_fraction as f32;
                                    let mut new_frac: Option<f64> = None;

                                    // Oxidizer bar (only for fuels that have oxidizer)
                                    if ox_cap > 0.0 {
                                        let ox_current = format_mass(ox_cap * part.fill_fraction);
                                        let ox_total = format_mass(ox_cap);
                                        let (rect, response) = ui.allocate_exact_size(
                                            egui::vec2(ui.available_width(), 20.0),
                                            egui::Sense::click_and_drag(),
                                        );
                                        if ui.is_rect_visible(rect) {
                                            let painter = ui.painter();
                                            painter.rect_filled(rect, 2.0, egui::Color32::from_rgb(40, 50, 60));
                                            let fill_rect = egui::Rect::from_min_max(
                                                rect.min,
                                                egui::pos2(rect.min.x + rect.width() * frac, rect.max.y),
                                            );
                                            painter.rect_filled(fill_rect, 2.0, egui::Color32::from_rgb(80, 140, 200));
                                            painter.text(
                                                rect.center(),
                                                egui::Align2::CENTER_CENTER,
                                                format!("O2: {}/{}", ox_current, ox_total),
                                                egui::FontId::proportional(12.0),
                                                egui::Color32::WHITE,
                                            );
                                        }
                                        if response.clicked() || response.dragged() {
                                            if let Some(pos) = response.interact_pointer_pos() {
                                                let f = ((pos.x - rect.min.x) / rect.width()).clamp(0.0, 1.0) as f64;
                                                new_frac = Some(f);
                                            }
                                        }
                                    }

                                    // Fuel bar
                                    if let Some(fuel_name) = part.fuel_type.fuel_resource_name() {
                                        let fuel_current = format_mass(fuel_cap * part.fill_fraction);
                                        let fuel_total = format_mass(fuel_cap);
                                        let (rect, response) = ui.allocate_exact_size(
                                            egui::vec2(ui.available_width(), 20.0),
                                            egui::Sense::click_and_drag(),
                                        );
                                        if ui.is_rect_visible(rect) {
                                            let painter = ui.painter();
                                            painter.rect_filled(rect, 2.0, egui::Color32::from_rgb(40, 50, 60));
                                            let fill_rect = egui::Rect::from_min_max(
                                                rect.min,
                                                egui::pos2(rect.min.x + rect.width() * frac, rect.max.y),
                                            );
                                            painter.rect_filled(fill_rect, 2.0, egui::Color32::from_rgb(200, 160, 60));
                                            painter.text(
                                                rect.center(),
                                                egui::Align2::CENTER_CENTER,
                                                format!("{}: {}/{}", fuel_name.to_uppercase(), fuel_current, fuel_total),
                                                egui::FontId::proportional(12.0),
                                                egui::Color32::WHITE,
                                            );
                                        }
                                        if response.clicked() || response.dragged() {
                                            if let Some(pos) = response.interact_pointer_pos() {
                                                let f = ((pos.x - rect.min.x) / rect.width()).clamp(0.0, 1.0) as f64;
                                                new_frac = Some(f);
                                            }
                                        }
                                    }

                                    // Apply dragged fill fraction
                                    if let Some(f) = new_frac {
                                        if let Some(p) = editor.parts.get_mut(&part_id) {
                                            p.fill_fraction = f;
                                        }
                                        if let Some(mid) = mirror_id {
                                            if let Some(mp) = editor.parts.get_mut(&mid) {
                                                mp.fill_fraction = f;
                                            }
                                        }
                                    }

                                    // Fill/Empty convenience buttons
                                    ui.horizontal(|ui| {
                                        if ui.button("Fill").clicked() {
                                            if let Some(p) = editor.parts.get_mut(&part_id) {
                                                p.fill_fraction = 1.0;
                                            }
                                            if let Some(mid) = mirror_id {
                                                if let Some(mp) = editor.parts.get_mut(&mid) {
                                                    mp.fill_fraction = 1.0;
                                                }
                                            }
                                        }
                                        if ui.button("Empty").clicked() {
                                            if let Some(p) = editor.parts.get_mut(&part_id) {
                                                p.fill_fraction = 0.0;
                                            }
                                            if let Some(mid) = mirror_id {
                                                if let Some(mp) = editor.parts.get_mut(&mid) {
                                                    mp.fill_fraction = 0.0;
                                                }
                                            }
                                        }
                                    });
                                }

                                // Show total mass
                                ui.separator();
                                let dry_mass = def.mass;
                                let prop_mass = if part.fill_fraction > 0.0 && part.fuel_type != FuelType::Empty {
                                    let (ox, fuel) = tank.propellant_capacity(part.fuel_type);
                                    (ox + fuel) * part.fill_fraction / 1000.0  // Convert kg to tonnes
                                } else {
                                    0.0
                                };
                                ui.label(format!("Dry: {:.3} t", dry_mass));
                                ui.label(format!("Prop: {:.3} t", prop_mass));
                                ui.label(format!("Total: {:.3} t", dry_mass + prop_mass));
                            }

                            // Pod info
                            if let Some(ref pod) = def.pod {
                                ui.separator();
                                ui.heading("Pod Stats");
                                ui.label(format!("Crew Capacity: {}", pod.crew_capacity));
                            }

                            // RCS info
                            if let Some(ref rcs) = def.rcs {
                                ui.separator();
                                ui.heading("RCS Stats");
                                ui.label(format!("Thrust: {:.1} kN", rcs.thrust));
                                ui.label(format!("Isp: {:.0} s", rcs.isp));
                                ui.label("Fuel: Monopropellant");
                            }

                            // Battery info
                            if let Some(ref battery) = def.battery {
                                ui.separator();
                                ui.heading("Battery");
                                ui.label(format!("Capacity: {:.0} Wh", battery.capacity_wh));
                            }

                            // Solar panel info
                            if let Some(ref solar) = def.solar_panel {
                                ui.separator();
                                ui.heading("Solar Panel");
                                ui.label(format!("Output at Earth: {:.0} W", solar.output_1au));
                                let label = if part.deployed { "Retract" } else { "Extend" };
                                if ui.button(label).clicked() {
                                    let new_state = !part.deployed;
                                    if let Some(p) = editor.parts.get_mut(&part_id) {
                                        p.deployed = new_state;
                                    }
                                    if let Some(mid) = part.mirror_partner {
                                        if let Some(mp) = editor.parts.get_mut(&mid) {
                                            mp.deployed = new_state;
                                        }
                                    }
                                }
                            }

                            // RTG info
                            if let Some(ref rtg) = def.rtg {
                                ui.separator();
                                ui.heading("RTG");
                                ui.label(format!("Output: {:.0} W", rtg.output_watts));
                            }

                            // Radiator deploy/retract in editor
                            if def.radiator.is_some() {
                                let label = if part.deployed { "Retract" } else { "Deploy" };
                                if ui.button(label).clicked() {
                                    let new_state = !part.deployed;
                                    if let Some(p) = editor.parts.get_mut(&part_id) {
                                        p.deployed = new_state;
                                    }
                                    if let Some(mid) = part.mirror_partner {
                                        if let Some(mp) = editor.parts.get_mut(&mid) {
                                            mp.deployed = new_state;
                                        }
                                    }
                                }
                            }

                            // Decoupler info
                            if def.decoupler.is_some() {
                                ui.separator();
                                ui.heading("Decoupler");
                                let mut crossfeed = part.crossfeed_enabled;
                                if ui.checkbox(&mut crossfeed, "Fuel Crossfeed").changed() {
                                    if let Some(p) = editor.parts.get_mut(&part_id) {
                                        p.crossfeed_enabled = crossfeed;
                                    }
                                    if let Some(mid) = part.mirror_partner {
                                        if let Some(mp) = editor.parts.get_mut(&mid) {
                                            mp.crossfeed_enabled = crossfeed;
                                        }
                                    }
                                }
                            }

                            // Cargo container configuration
                            if let Some(ref cargo_data) = def.cargo {
                                ui.separator();
                                ui.heading("Cargo");

                                let capacity_kg = cargo_data.capacity_kg;

                                // Calculate used mass
                                let resource_mass: f64 = part.cargo_resources.iter().map(|(_, kg)| *kg).sum();
                                let building_mass: f64 = part.cargo_buildings.iter()
                                    .filter_map(|name| crate::colony::BuildingType::from_display_name(name))
                                    .map(|bt| bt.total_build_mass())
                                    .sum();
                                let payload_mass: f64 = part.cargo_payloads.iter().map(|p| p.mass_kg).sum();
                                let used_kg = resource_mass + building_mass + payload_mass;
                                let remaining_kg = (capacity_kg - used_kg).max(0.0);

                                // Capacity bar
                                let fraction = (used_kg / capacity_kg).min(1.0);
                                let bar_color = if fraction > 0.95 {
                                    egui::Color32::from_rgb(200, 60, 60)
                                } else {
                                    egui::Color32::from_rgb(60, 140, 200)
                                };
                                let bar = egui::ProgressBar::new(fraction as f32)
                                    .text(format!("{:.0} / {:.0} kg", used_kg, capacity_kg))
                                    .fill(bar_color);
                                ui.add(bar);

                                // Editable resource list with amounts and remove buttons
                                let cargo_res = part.cargo_resources.clone();
                                let mut resource_to_remove: Option<usize> = None;
                                for (idx, (name, kg)) in cargo_res.iter().enumerate() {
                                    let mut amount = *kg;
                                    let max_for_this = kg + remaining_kg;
                                    ui.horizontal(|ui| {
                                        if ui.small_button("\u{00d7}").clicked() {
                                            resource_to_remove = Some(idx);
                                        }
                                        ui.label(format!("{}:", name));
                                        let drag = egui::DragValue::new(&mut amount)
                                            .clamp_range(0.0..=max_for_this)
                                            .speed(10.0)
                                            .suffix(" kg");
                                        if ui.add(drag).changed() {
                                            if let Some(p) = editor.parts.get_mut(&part_id) {
                                                if let Some(entry) = p.cargo_resources.get_mut(idx) {
                                                    entry.1 = amount;
                                                }
                                            }
                                            if let Some(mid) = part.mirror_partner {
                                                if let Some(mp) = editor.parts.get_mut(&mid) {
                                                    if let Some(entry) = mp.cargo_resources.get_mut(idx) {
                                                        entry.1 = amount;
                                                    }
                                                }
                                            }
                                        }
                                    });
                                }
                                if let Some(idx) = resource_to_remove {
                                    if let Some(p) = editor.parts.get_mut(&part_id) {
                                        p.cargo_resources.remove(idx);
                                    }
                                    if let Some(mid) = part.mirror_partner {
                                        if let Some(mp) = editor.parts.get_mut(&mid) {
                                            if idx < mp.cargo_resources.len() {
                                                mp.cargo_resources.remove(idx);
                                            }
                                        }
                                    }
                                }

                                // Buildings list with remove buttons
                                let mut building_to_remove: Option<usize> = None;
                                for (idx, name) in part.cargo_buildings.iter().enumerate() {
                                    let mass = crate::colony::BuildingType::from_display_name(name)
                                        .map(|bt| bt.total_build_mass())
                                        .unwrap_or(0.0);
                                    ui.horizontal(|ui| {
                                        if ui.small_button("\u{00d7}").clicked() {
                                            building_to_remove = Some(idx);
                                        }
                                        ui.label(format!("{} ({:.0} kg)", name, mass));
                                    });
                                }
                                if let Some(idx) = building_to_remove {
                                    if let Some(p) = editor.parts.get_mut(&part_id) {
                                        p.cargo_buildings.remove(idx);
                                    }
                                    if let Some(mid) = part.mirror_partner {
                                        if let Some(mp) = editor.parts.get_mut(&mid) {
                                            if idx < mp.cargo_buildings.len() {
                                                mp.cargo_buildings.remove(idx);
                                            }
                                        }
                                    }
                                }

                                // Add Resource combo
                                if remaining_kg > 0.0 {
                                    ui.horizontal(|ui| {
                                        ui.label("Add:");
                                        egui::ComboBox::from_id_source("cargo_add_resource")
                                            .selected_text("Resource...")
                                            .width(120.0)
                                            .show_ui(ui, |ui: &mut egui::Ui| {
                                                for rt in crate::colony::ResourceType::all() {
                                                    if rt.is_ship_fuel() {
                                                        continue;
                                                    }
                                                    let name = rt.display_name();
                                                    let default_amount = remaining_kg.min(1000.0);
                                                    if ui.selectable_label(false, name).clicked() {
                                                        if let Some(p) = editor.parts.get_mut(&part_id) {
                                                            p.cargo_resources.push((name.to_string(), default_amount));
                                                        }
                                                        if let Some(mid) = part.mirror_partner {
                                                            if let Some(mp) = editor.parts.get_mut(&mid) {
                                                                mp.cargo_resources.push((name.to_string(), default_amount));
                                                            }
                                                        }
                                                    }
                                                }
                                            });
                                    });

                                    // Add Building combo
                                    ui.horizontal(|ui| {
                                        ui.label("Add:");
                                        egui::ComboBox::from_id_source("cargo_add_building")
                                            .selected_text("Building...")
                                            .width(120.0)
                                            .show_ui(ui, |ui: &mut egui::Ui| {
                                                for bt in crate::colony::BuildingType::all() {
                                                    let name = bt.display_name();
                                                    let mass = bt.total_build_mass();
                                                    if mass <= remaining_kg {
                                                        let label = format!("{} ({:.0} kg)", name, mass);
                                                        if ui.selectable_label(false, label).clicked() {
                                                            if let Some(p) = editor.parts.get_mut(&part_id) {
                                                                p.cargo_buildings.push(name.to_string());
                                                            }
                                                            if let Some(mid) = part.mirror_partner {
                                                                if let Some(mp) = editor.parts.get_mut(&mid) {
                                                                    mp.cargo_buildings.push(name.to_string());
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            });
                                    });
                                }

                                // Contract Payloads section
                                let payload_list = part.cargo_payloads.clone();
                                if !payload_list.is_empty() {
                                    ui.add_space(4.0);
                                    ui.label(egui::RichText::new("Payloads").size(12.0).strong());
                                }
                                let mut payload_to_remove: Option<usize> = None;
                                for (idx, payload) in payload_list.iter().enumerate() {
                                    ui.horizontal(|ui| {
                                        if ui.small_button("\u{00d7}").clicked() {
                                            payload_to_remove = Some(idx);
                                        }
                                        ui.label(format!("{} ({:.0} kg)", payload.name, payload.mass_kg));
                                    });
                                }
                                if let Some(idx) = payload_to_remove {
                                    if let Some(p) = editor.parts.get_mut(&part_id) {
                                        if idx < p.cargo_payloads.len() {
                                            p.cargo_payloads.remove(idx);
                                        }
                                    }
                                    if let Some(mid) = part.mirror_partner {
                                        if let Some(mp) = editor.parts.get_mut(&mid) {
                                            if idx < mp.cargo_payloads.len() {
                                                mp.cargo_payloads.remove(idx);
                                            }
                                        }
                                    }
                                }

                                // Recalculate remaining after potential payload removal
                                let payload_mass: f64 = if let Some(p) = editor.parts.get(&part_id) {
                                    p.cargo_payloads.iter().map(|pl| pl.mass_kg).sum()
                                } else {
                                    0.0
                                };
                                let resource_mass_now: f64 = if let Some(p) = editor.parts.get(&part_id) {
                                    p.cargo_resources.iter().map(|(_, kg)| *kg).sum()
                                } else {
                                    0.0
                                };
                                let building_mass_now: f64 = if let Some(p) = editor.parts.get(&part_id) {
                                    p.cargo_buildings.iter()
                                        .filter_map(|name| crate::colony::BuildingType::from_display_name(name))
                                        .map(|bt| bt.total_build_mass())
                                        .sum()
                                } else {
                                    0.0
                                };
                                let remaining_for_payloads = (capacity_kg - resource_mass_now - building_mass_now - payload_mass).max(0.0);

                                // Add Payload combo — show available payloads from active contracts
                                // that haven't been placed in any cargo container yet
                                if remaining_for_payloads > 0.0 {
                                    let available_payloads = contracts.active_payload_contracts();
                                    // Collect all payload contract IDs already placed in any part
                                    let placed_ids: std::collections::HashSet<u64> = editor.parts.values()
                                        .flat_map(|p| p.cargo_payloads.iter().map(|pl| pl.contract_id))
                                        .collect();
                                    let unplaced: Vec<_> = available_payloads.into_iter()
                                        .filter(|p| !placed_ids.contains(&p.contract_id) && p.mass_kg <= remaining_for_payloads)
                                        .collect();

                                    if !unplaced.is_empty() {
                                        ui.horizontal(|ui| {
                                            ui.label("Add:");
                                            egui::ComboBox::from_id_source("cargo_add_payload")
                                                .selected_text("Payload...")
                                                .width(160.0)
                                                .show_ui(ui, |ui: &mut egui::Ui| {
                                                    for payload in &unplaced {
                                                        let label = format!("{} ({:.0} kg)", payload.name, payload.mass_kg);
                                                        if ui.selectable_label(false, label).clicked() {
                                                            if let Some(p) = editor.parts.get_mut(&part_id) {
                                                                p.cargo_payloads.push(payload.clone());
                                                            }
                                                            if let Some(mid) = part.mirror_partner {
                                                                if let Some(mp) = editor.parts.get_mut(&mid) {
                                                                    mp.cargo_payloads.push(payload.clone());
                                                                }
                                                            }
                                                        }
                                                    }
                                                });
                                        });
                                    }
                                }
                            }

                            ui.separator();

                            let delete_label = if part.mirror_partner.is_some() {
                                "Delete Parts (x2)"
                            } else {
                                "Delete Part"
                            };
                            if ui.button(delete_label).clicked() {
                                editor.part_to_delete = Some(part_id);
                            }
                        }
                    }
                }
                }); // ScrollArea
            });
    }

    // Bottom panel - Instructions
    egui::TopBottomPanel::bottom("editor_instructions")
        .frame(egui::Frame::none().fill(egui::Color32::from_rgba_unmultiplied(20, 20, 30, 200)))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Click part to select • Click build area to place • Right-click to deselect • Scroll to zoom • Drag to pan");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("?").on_hover_text("Keyboard shortcuts").clicked() {
                        editor.show_shortcuts_help = !editor.show_shortcuts_help;
                    }
                });
            });
        });

    // Keyboard shortcuts popup
    if editor.show_shortcuts_help {
        egui::Window::new("Keyboard Shortcuts")
            .id(egui::Id::new("editor_shortcuts_help"))
            .collapsible(false)
            .resizable(false)
            .default_width(280.0)
            .show(ctx, |ui| {
                ui.heading("Editor");
                ui.label("Arrow Keys — Pan camera");
                ui.label("Scroll — Zoom in/out");
                ui.label("R — Rotate part 90°");
                ui.label("Delete / Backspace — Delete selected part");
                ui.label("Ctrl+Z — Undo last action");
                ui.label("Right-click — Deselect / cancel placement");
                ui.label("Click part in palette — Select for placement");
                ui.label("Click placed part — Select for info/editing");
                ui.separator();
                ui.heading("Flight");
                ui.label("Space — Activate next stage");
                ui.label("Shift / Ctrl — Throttle up / down");
                ui.label("Z / X — Full throttle / zero throttle");
                ui.label("W/A/S/D — RCS translation");
                ui.label("Q / E — Rotate left / right");
                ui.label("R — Toggle RCS");
                ui.label("` (backtick) — Focus on ship");
                ui.label("[ / ] — Previous / next vessel");
                ui.separator();
                if ui.button("Close").clicked() {
                    editor.show_shortcuts_help = false;
                }
            });
    }

    // Save dialog
    if editor.show_save_dialog {
        egui::Window::new("Save Blueprint")
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Name:");
                    ui.text_edit_singleline(&mut editor.vessel_name);
                });

                ui.horizontal(|ui| {
                    if ui.button("Save").clicked() {
                        action = EditorAction::SaveBlueprint(editor.vessel_name.clone());
                        editor.show_save_dialog = false;
                    }
                    if ui.button("Cancel").clicked() {
                        editor.show_save_dialog = false;
                    }
                });
            });
    }

    // Load dialog
    if editor.show_load_dialog {
        egui::Window::new("Load Blueprint")
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                let import_btn = ui.button("Import Blueprint File\u{2026}");
                if import_btn.clicked() {
                    action = EditorAction::ImportBlueprint;
                }
                import_btn.on_hover_text(
                    "Pick a .ron blueprint from disk (e.g. one downloaded from a community share)",
                );
                ui.add_space(8.0);

                if blueprint_names.is_empty() {
                    ui.label("No saved blueprints");
                } else {
                    egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                        for name in blueprint_names {
                            let is_locked = locked_blueprints.contains(*name);
                            ui.horizontal(|ui| {
                                if is_locked {
                                    let btn = ui.add_enabled(false,
                                        egui::Button::new(format!("\u{1f512} {}", name)));
                                    btn.on_disabled_hover_text("Contains locked parts");
                                } else if ui.button(*name).clicked() {
                                    action = EditorAction::LoadBlueprint(name.to_string());
                                    editor.show_load_dialog = false;
                                }
                                if ui.small_button("\u{2913}").on_hover_text("Export as .ron").clicked() {
                                    action = EditorAction::ExportBlueprint(name.to_string());
                                }
                                if ui.small_button("🗑").on_hover_text("Delete").clicked() {
                                    editor.confirm_delete_blueprint = Some(name.to_string());
                                }
                            });
                        }
                    });
                }

                if ui.button("Cancel").clicked() {
                    editor.show_load_dialog = false;
                    editor.confirm_delete_blueprint = None;
                }
            });

        // Delete confirmation overlay — sits on top of the load dialog so
        // the user has to acknowledge before the blueprint is removed.
        if let Some(delete_name) = editor.confirm_delete_blueprint.clone() {
            let confirm_label = format!("Delete \"{}\"?", delete_name);
            egui::Area::new(egui::Id::new("delete_blueprint_confirm_overlay"))
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    egui::Frame::none()
                        .fill(egui::Color32::from_rgba_unmultiplied(0, 0, 0, 220))
                        .inner_margin(egui::Margin::same(20.0))
                        .rounding(egui::Rounding::same(6.0))
                        .show(ui, |ui| {
                            ui.vertical_centered(|ui| {
                                ui.label(egui::RichText::new(&confirm_label).size(18.0).color(egui::Color32::WHITE));
                                ui.add_space(4.0);
                                ui.label(egui::RichText::new("This cannot be undone.").size(13.0).color(egui::Color32::from_rgb(200, 150, 150)));
                                ui.add_space(12.0);
                                ui.horizontal(|ui| {
                                    if ui.button(egui::RichText::new("Delete").color(egui::Color32::from_rgb(220, 80, 80))).clicked() {
                                        action = EditorAction::DeleteBlueprint(delete_name.clone());
                                        editor.confirm_delete_blueprint = None;
                                    }
                                    ui.add_space(8.0);
                                    if ui.button("Cancel").clicked() {
                                        editor.confirm_delete_blueprint = None;
                                    }
                                });
                            });
                        });
                });
        }
    }

    // Alert message overlay (shown temporarily after errors)
    if let Some(ref msg) = editor.alert_message {
        egui::Area::new(egui::Id::new("editor_alert"))
            .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 60.0))
            .show(ctx, |ui| {
                egui::Frame::none()
                    .fill(egui::Color32::from_rgba_unmultiplied(180, 40, 40, 230))
                    .rounding(egui::Rounding::same(6.0))
                    .inner_margin(egui::Margin::symmetric(16.0, 10.0))
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new(msg)
                            .color(egui::Color32::WHITE)
                            .size(14.0));
                    });
            });
    }

    action
}

/// Check if the mouse is over any UI element
pub fn is_mouse_over_ui(ctx: &egui::Context) -> bool {
    ctx.is_pointer_over_area()
}
