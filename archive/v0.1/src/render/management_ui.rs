use crate::colony::{Company, ContractManager, ContractKind, GovernmentMilestone, ScienceState};
use super::types::ManagementAction;

/// Render the management screen. Returns the action and the updated R&D budget.
pub fn render_management_screen(
    ctx: &egui::Context,
    company: &Company,
    science: &ScienceState,
    contracts: &ContractManager,
    rd_budget: f64,
    warp_levels: &[f64],
    current_warp_index: usize,
    date_str: &str,
    paused: bool,
    active_toasts: &[(String, web_time::Instant)],
) -> (ManagementAction, f64) {
    let mut action = ManagementAction::None;
    let mut budget = rd_budget;

    // === Top panel: time warp bar + date ===
    egui::TopBottomPanel::top("management_top_panel").show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.heading(
                egui::RichText::new("Management")
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
                    action = ManagementAction::ChangeWarp(i);
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
        egui::Area::new(egui::Id::new("management_pause_overlay"))
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
                                .button(egui::RichText::new("Main Menu").size(18.0))
                                .clicked()
                            {
                                action = ManagementAction::GoToMainMenu;
                            }
                        });
                    });
            });

        super::flight::render_toasts(ctx, active_toasts);
        return (action, budget);
    }

    // === Central panel ===
    egui::CentralPanel::default().show(ctx, |ui| {
        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                ui.add_space(16.0);

                // === Finances ===
                ui.heading("Finances");
                ui.label(
                    egui::RichText::new(crate::colony::format_money(company.money))
                        .size(24.0)
                        .color(egui::Color32::from_rgb(100, 200, 100)),
                );

                ui.add_space(12.0);
                ui.separator();

                // === R&D ===
                ui.heading("Research & Development");
                ui.horizontal(|ui| {
                    ui.label("R&D Budget:");
                    let mut budget_m = budget / 1_000_000.0;
                    let drag = egui::DragValue::new(&mut budget_m)
                        .speed(0.5)
                        .clamp_range(0.0..=1000.0)
                        .suffix(" M/yr");
                    if ui.add(drag).changed() {
                        budget = budget_m * 1_000_000.0;
                        action = ManagementAction::SetRdBudget(budget);
                    }
                });

                ui.add_space(12.0);
                ui.separator();

                // === Science ===
                ui.heading("Science");
                ui.label(
                    egui::RichText::new(format!("Available: {:.0}", science.available))
                        .size(18.0)
                        .color(egui::Color32::from_rgb(100, 180, 255)),
                );
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(format!(
                        "Discovery: {:.0}  |  R&D: {:.0}  |  Lab: {:.0}",
                        science.cumulative_discovery, science.cumulative_rd, science.cumulative_lab
                    ))
                    .size(12.0)
                    .color(egui::Color32::from_rgb(140, 140, 140)),
                );

                ui.add_space(12.0);

                // Open Tech Tree button
                if ui
                    .button(
                        egui::RichText::new("Open Tech Tree")
                            .size(16.0),
                    )
                    .clicked()
                {
                    action = ManagementAction::OpenTechTree;
                }

                ui.add_space(12.0);
                ui.separator();

                // === Contracts ===
                render_contracts_section(ui, contracts, &mut action);
            });
    });

    // Toast notifications
    super::flight::render_toasts(ctx, active_toasts);

    (action, budget)
}

/// Render the contracts section (reusable in both management screen and editor window).
pub fn render_contracts_section(
    ui: &mut egui::Ui,
    contracts: &ContractManager,
    action: &mut ManagementAction,
) {
    ui.heading("Contracts");
    ui.add_space(4.0);

    // Available contracts
    ui.label(
        egui::RichText::new("Available")
            .size(14.0)
            .strong()
            .color(egui::Color32::from_rgb(200, 200, 200)),
    );
    ui.add_space(2.0);

    if contracts.available.is_empty() {
        ui.label(
            egui::RichText::new("No contracts available yet")
                .size(12.0)
                .color(egui::Color32::from_rgb(140, 140, 140)),
        );
    }

    for contract in &contracts.available {
        egui::Frame::none()
            .fill(egui::Color32::from_rgba_unmultiplied(30, 35, 45, 220))
            .rounding(egui::Rounding::same(4.0))
            .inner_margin(egui::Margin::same(6.0))
            .outer_margin(egui::Margin::symmetric(0.0, 2.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(&contract.name).strong(),
                    );
                    ui.with_layout(
                        egui::Layout::right_to_left(egui::Align::Center),
                        |ui| {
                            ui.label(
                                egui::RichText::new(crate::colony::format_money(contract.payout))
                                    .color(egui::Color32::from_rgb(100, 200, 100))
                                    .size(12.0),
                            );
                            if ui.small_button("Accept").clicked() {
                                *action = ManagementAction::AcceptContract(contract.id);
                            }
                        },
                    );
                });
                ui.label(
                    egui::RichText::new(contract.description())
                        .size(11.0)
                        .color(egui::Color32::from_rgb(160, 160, 160)),
                );
            });
    }

    // Active contracts
    if !contracts.active.is_empty() {
        ui.add_space(10.0);
        ui.label(
            egui::RichText::new("Active")
                .size(14.0)
                .strong()
                .color(egui::Color32::from_rgb(200, 200, 200)),
        );
        ui.add_space(2.0);

        for contract in &contracts.active {
            egui::Frame::none()
                .fill(egui::Color32::from_rgba_unmultiplied(25, 40, 30, 220))
                .rounding(egui::Rounding::same(4.0))
                .inner_margin(egui::Margin::same(6.0))
                .outer_margin(egui::Margin::symmetric(0.0, 2.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(&contract.name).strong(),
                        );
                        ui.with_layout(
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                ui.label(
                                    egui::RichText::new(crate::colony::format_money(contract.payout))
                                        .color(egui::Color32::from_rgb(100, 200, 100))
                                        .size(12.0),
                                );
                                if ui.small_button("Cancel").clicked() {
                                    *action = ManagementAction::CancelContract(contract.id);
                                }
                            },
                        );
                    });
                    // Show status for active contracts
                    let status_text: String = match &contract.kind {
                        ContractKind::Tourism { destination_reached, .. } => {
                            if *destination_reached {
                                "Destination reached — return to Earth and recover vessel".to_string()
                            } else {
                                "Fly to destination".to_string()
                            }
                        }
                        ContractKind::Payload { .. } => "Load payload in cargo container and deliver".to_string(),
                        ContractKind::SuborbitalTest { part_name, target_altitude, .. } => {
                            format!("Build vessel with {} and reach {:.0} km", part_name, target_altitude / 1000.0)
                        }
                    };
                    ui.label(
                        egui::RichText::new(&status_text)
                            .size(11.0)
                            .color(egui::Color32::from_rgb(160, 160, 160)),
                    );
                });
        }
    }

    // Government Milestones
    ui.add_space(10.0);
    ui.label(
        egui::RichText::new("Government Milestones")
            .size(14.0)
            .strong()
            .color(egui::Color32::from_rgb(200, 200, 200)),
    );
    ui.add_space(2.0);

    for milestone in GovernmentMilestone::all() {
        let awarded = contracts.awarded_milestones.contains(milestone);
        let color = if awarded {
            egui::Color32::from_rgb(100, 200, 100)
        } else {
            egui::Color32::from_rgb(120, 120, 120)
        };
        let prefix = if awarded { "[x]" } else { "[ ]" };
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(format!("{} {}", prefix, milestone.display_name()))
                    .size(12.0)
                    .color(color),
            );
            ui.label(
                egui::RichText::new(crate::colony::format_money(milestone.payout()))
                    .size(11.0)
                    .color(if awarded {
                        egui::Color32::from_rgb(80, 160, 80)
                    } else {
                        egui::Color32::from_rgb(100, 100, 100)
                    }),
            );
        });
    }
}
