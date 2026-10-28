use super::operations::OperationDone;
use super::*;
use egui::{FontId, Pos2, Rect, Sense};
use spacefinder::{
    actions::{self, CleanupTool},
    recommendations::{Action, Category},
};

pub(super) fn navigation(
    ui: &mut egui::Ui,
    mode: Mode,
    title: &str,
    caption: &str,
    selected: bool,
) -> egui::Response {
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 52.0), Sense::click());
    let painter = ui.painter().with_clip_rect(rect);
    if selected || response.hovered() {
        painter.rect_filled(
            rect,
            8.0,
            if selected {
                Color32::from_rgb(27, 58, 55)
            } else {
                CARD
            },
        );
    }
    let color = if selected { ACCENT } else { MUTED };
    let center = rect.min + Vec2::new(20.0, 24.0);
    match mode {
        Mode::Overview => {
            for (x, y) in [(-7.0, -7.0), (2.0, -7.0), (-7.0, 2.0), (2.0, 2.0)] {
                painter.rect_stroke(
                    Rect::from_min_size(center + Vec2::new(x, y), Vec2::splat(5.0)),
                    1.0,
                    Stroke::new(1.2, color),
                    egui::StrokeKind::Inside,
                );
            }
        }
        Mode::Tree => {
            painter.rect_stroke(
                Rect::from_center_size(center, Vec2::new(17.0, 13.0)),
                2.0,
                Stroke::new(1.4, color),
                egui::StrokeKind::Inside,
            );
            painter.line_segment(
                [
                    center + Vec2::new(-7.0, -8.0),
                    center + Vec2::new(-1.0, -8.0),
                ],
                Stroke::new(2.0, color),
            );
        }
        Mode::Largest => {
            for (i, h) in [6.0, 11.0, 16.0].into_iter().enumerate() {
                painter.rect_filled(
                    Rect::from_min_size(
                        center + Vec2::new(i as f32 * 6.0 - 8.0, 8.0 - h),
                        Vec2::new(4.0, h),
                    ),
                    1.0,
                    color,
                );
            }
        }
        Mode::Extensions => {
            for y in [-6.0, 0.0, 6.0] {
                painter.circle_filled(center + Vec2::new(-7.0, y), 1.5, color);
                painter.line_segment(
                    [center + Vec2::new(-2.0, y), center + Vec2::new(8.0, y)],
                    Stroke::new(1.4, color),
                );
            }
        }
        Mode::Recommendations => {
            painter.circle_stroke(center, 9.0, Stroke::new(1.4, color));
            painter.line_segment(
                [center + Vec2::new(-4.0, 0.0), center + Vec2::new(-1.0, 3.0)],
                Stroke::new(1.6, color),
            );
            painter.line_segment(
                [center + Vec2::new(-1.0, 3.0), center + Vec2::new(5.0, -4.0)],
                Stroke::new(1.6, color),
            );
        }
    }
    painter.text(
        rect.min + Vec2::new(40.0, 12.0),
        egui::Align2::LEFT_TOP,
        title,
        FontId::proportional(13.0),
        if selected { TEXT } else { MUTED },
    );
    painter.text(
        rect.min + Vec2::new(40.0, 31.0),
        egui::Align2::LEFT_TOP,
        caption,
        FontId::proportional(9.5),
        MUTED,
    );
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn category(category: Category) -> (&'static str, Color32) {
    match category {
        Category::WindowsCleanup => ("MANAGE WITH WINDOWS", COLORS[1]),
        Category::Regenerable => ("CAN BE RECREATED", ACCENT),
        Category::Review => ("PERSONAL REVIEW", COLORS[3]),
    }
}

fn frame() -> egui::Frame {
    egui::Frame::new()
        .fill(CARD)
        .stroke(Stroke::new(1.0, LINE))
        .corner_radius(12)
        .inner_margin(20)
}

fn label(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).size(10.0).strong().color(MUTED));
}

fn stat(ui: &mut egui::Ui, width: f32, title: &str, value: String, caption: &str, color: Color32) {
    frame().show(ui, |ui| {
        ui.vertical(|ui| {
            ui.set_width((width - 42.0).max(100.0));
            label(ui, title);
            ui.add_space(8.0);
            ui.label(RichText::new(value).size(26.0).strong().color(color));
            ui.add(egui::Label::new(RichText::new(caption).size(11.0).color(MUTED)).wrap());
        });
    });
}

impl SpaceFinder {
    fn cleanup_tool(&mut self, ctx: &egui::Context, tool: CleanupTool) {
        self.start_operation(ctx, "Opening the Windows tool…", move || {
            actions::cleanup_tool(tool)?;
            Ok(OperationDone::Shell)
        });
    }

    fn inspect_recommendation(&mut self, index: usize) {
        let Some(scan) = self.scan.clone() else {
            return;
        };
        let scope = if scan.nodes[index].directory {
            index
        } else {
            scan.nodes[index].parent.unwrap_or(0)
        };
        self.focus(scope, true);
        self.select(index);
        self.pending_scroll_to = Some(index);
    }

    pub(super) fn overview(&mut self, ui: &mut egui::Ui) {
        let Some(scan) = self.scan.clone() else {
            self.welcome(ui);
            return;
        };
        let report = self.report.clone();
        egui::ScrollArea::vertical().id_salt("overview-scroll").show(ui, |ui| {
            let width = (ui.available_width() - 36.0) / 4.0;
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 12.0;
                stat(ui, width, "SCANNED SIZE", human_size(scan.progress.bytes), if scan.nodes[0].incomplete { "Partial coverage · logical sizes" } else { "Logical file size" }, TEXT);
                stat(ui, width, "FILES", count(scan.progress.files), &format!("{} folders scanned", count(scan.progress.folders)), TEXT);
                stat(ui, width, "CAN BE RECREATED", report.as_ref().map_or_else(|| "Scanning…".into(), |r| human_size(r.regenerable_bytes)), "Generated artifacts, review required", ACCENT);
                stat(ui, width, "SCAN TIME", format!("{:.2} s", scan.progress.seconds), "Native parallel scanning", COLORS[1]);
            });
            ui.add_space(16.0);
            ui.columns(2, |columns| {
                frame().show(&mut columns[0], |ui| {
                    ui.set_width(ui.available_width());
                    label(ui, "DRIVE CAPACITY"); ui.add_space(12.0);
                    let root = platform::display_path(&scan.root);
                    if let Some(volume) = self.volumes.iter().find(|v| root.starts_with(&platform::display_path(&v.path))) {
                        let used = volume.total.saturating_sub(volume.free);
                        ui.horizontal(|ui| {
                            let (rect, _) = ui.allocate_exact_size(Vec2::splat(144.0), Sense::hover());
                            let center = rect.center(); let radius = 56.0;
                            let ratio = (used as f32 / volume.total.max(1) as f32).clamp(0.0, 1.0);
                            let color = if ratio >= 0.9 { COLORS[3] } else { ACCENT };
                            ui.painter().circle_stroke(center, radius, Stroke::new(11.0, LINE));
                            let points: Vec<_> = (0..=96).map(|i| { let a = -std::f32::consts::FRAC_PI_2 + i as f32 / 96.0 * std::f32::consts::TAU * ratio; center + Vec2::new(a.cos(), a.sin()) * radius }).collect();
                            ui.painter().add(egui::Shape::line(points, Stroke::new(11.0, color)));
                            ui.painter().text(center - Vec2::new(0.0, 8.0), egui::Align2::CENTER_CENTER, format!("{:.0}%", ratio * 100.0), FontId::proportional(27.0), TEXT);
                            ui.painter().text(center + Vec2::new(0.0, 18.0), egui::Align2::CENTER_CENTER, "used", FontId::proportional(11.0), MUTED);
                            ui.vertical(|ui| {
                                ui.strong(format!("Drive {}", platform::display_path(&volume.path)));
                                property(ui, "Free", human_size(volume.free));
                                property(ui, "Capacity", human_size(volume.total));
                                ui.label(RichText::new("Windows information for the entire drive.").size(11.0).color(MUTED));
                            });
                        });
                    } else { ui.label("Capacity information is unavailable for this location."); }
                });
                frame().show(&mut columns[1], |ui| {
                    ui.set_width(ui.available_width());
                    label(ui, "A STARTING POINT FOR CLEANUP"); ui.add_space(12.0);
                    ui.label(RichText::new("Review first. Remove later.").size(21.0).strong());
                    ui.add_space(8.0);
                    if let Some(report) = &report {
                        ui.label(format!("{} {} in this scan.", report.items.len(), if report.items.len() == 1 { "suggestion found" } else { "suggestions found" }));
                        ui.label(RichText::new(format!("{} in recreatable artifacts · {} in files to review", human_size(report.regenerable_bytes), human_size(report.review_bytes))).size(12.0).color(MUTED));
                    } else { ui.horizontal(|ui| { ui.spinner(); ui.label("Preparing recommendations…"); }); }
                    ui.add_space(8.0);
                    ui.label(RichText::new("Values are observed logical sizes, not a promise of recovered space.").size(11.0).color(MUTED));
                    ui.add_space(16.0);
                    if ui.add_sized([ui.available_width(), 36.0], egui::Button::new(RichText::new("View recommendations").strong().color(BG)).fill(ACCENT)).clicked() { self.change_mode(Mode::Recommendations); }
                });
            });
            ui.add_space(20.0);
            ui.horizontal(|ui| { ui.label(RichText::new("Where the space goes").size(18.0).strong()); ui.with_layout(Layout::right_to_left(Align::Center), |ui| { if ui.small_button("Open explorer").clicked() { self.focus(0, true); } }); });
            ui.add_space(8.0);
            let mut selected = None;
            for index in &self.overview_children {
                let node = &scan.nodes[*index];
                let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 40.0), Sense::click());
                if response.hovered() { ui.painter().rect_filled(rect, 6.0, CARD); }
                let name = node.name.to_string_lossy();
                let text = ui.painter().with_clip_rect(Rect::from_min_max(rect.min, Pos2::new(rect.min.x + rect.width() * 0.48, rect.max.y)));
                text.text(rect.left_center() + Vec2::new(12.0, 0.0), egui::Align2::LEFT_CENTER, name, FontId::proportional(13.0), TEXT);
                let fraction = percentage(node.bytes, scan.nodes[0].bytes) / 100.0;
                let start = rect.min + Vec2::new(rect.width() * 0.50, 17.0); let width = rect.width() * 0.30;
                ui.painter().rect_filled(Rect::from_min_size(start, Vec2::new(width, 5.0)), 3.0, LINE);
                ui.painter().rect_filled(Rect::from_min_size(start, Vec2::new(width * fraction, 5.0)), 3.0, ACCENT);
                ui.painter().text(rect.right_center() - Vec2::new(12.0, 0.0), egui::Align2::RIGHT_CENTER, format!("{}{}", human_size(node.bytes), if node.incomplete { " *" } else { "" }), FontId::monospace(12.0), MUTED);
                if response.clicked() { selected = Some(*index); }
            }
            if let Some(index) = selected { self.inspect_recommendation(index); ui.ctx().request_repaint(); }
            ui.add_space(16.0);
            ui.label(RichText::new("Scan sizes and physical disk usage may differ due to compression, hard links, metadata and permissions.").size(11.0).color(MUTED));
        });
    }

    pub(super) fn welcome(&mut self, ui: &mut egui::Ui) {
        ui.add_space(32.0);
        frame().show(ui, |ui| {
            ui.set_width(ui.available_width());
            label(ui, "UNDERSTAND YOUR DISK"); ui.add_space(12.0);
            ui.label(RichText::new(if self.events.is_some() { "Finding where the space goes…" } else { "More clarity. More room to decide." }).size(28.0).strong());
            ui.add_space(12.0);
            ui.label(RichText::new("Choose a drive or folder. The scan reads metadata and prepares recommendations without deleting anything.").color(MUTED));
            ui.add_space(24.0);
            if self.events.is_some() {
                ui.horizontal(|ui| { ui.spinner(); ui.label(format!("{} files · {} · {:.1} s", count(self.progress.files), human_size(self.progress.bytes), self.progress.seconds)); });
                ui.add(egui::Label::new(RichText::new(&self.progress.current).size(11.0).color(MUTED)).truncate());
            } else if ui.add_sized([224.0, 40.0], egui::Button::new(RichText::new("Choose a folder").color(BG).strong()).fill(ACCENT)).clicked() { self.choose_folder(); }
        });
        ui.add_space(20.0);
        ui.columns(3, |columns| {
            for (ui, (title, body)) in columns.iter_mut().zip([
                (
                    "Explore",
                    "See folders, files and sizes in a detailed tree.",
                ),
                (
                    "Understand",
                    "Compare file types and find what takes up the most space.",
                ),
                (
                    "Decide",
                    "Review suggestions, evidence and cautions before removing anything.",
                ),
            ]) {
                frame().show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new(title).size(17.0).strong());
                    ui.add_space(8.0);
                    ui.label(RichText::new(body).size(12.0).color(MUTED));
                });
            }
        });
        ui.add_space(16.0);
        ui.label(RichText::new("Administrator is optional and may allow more folders to be read. Some remain protected.").size(11.0).color(MUTED));
    }

    pub(super) fn recommendations_page(&mut self, ui: &mut egui::Ui) {
        let Some(report) = self.report.clone() else {
            ui.add_space(32.0);
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Analyzing scan data to prepare recommendations…");
            });
            return;
        };
        let Some(scan) = self.scan.clone() else {
            return;
        };
        egui::Frame::new().fill(Color32::from_rgb(24, 43, 43)).corner_radius(8).inner_margin(14).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new("Suggestions to review, with context.").strong().color(ACCENT));
            ui.label(RichText::new("Scanning deletes nothing. Age, size or extension do not guarantee a file is no longer needed.").size(12.0).color(MUTED));
        });
        ui.add_space(12.0);
        ui.horizontal_wrapped(|ui| {
            for (value, title) in [
                (None, "All"),
                (Some(Category::WindowsCleanup), "Windows tools"),
                (Some(Category::Regenerable), "Recreatable"),
                (Some(Category::Review), "Personal review"),
            ] {
                ui.selectable_value(&mut self.recommendation_category, value, title);
            }
        });
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.recommendation_query)
                    .hint_text("Filter suggestions by name or path…")
                    .desired_width((ui.available_width() - 100.0).max(120.0)),
            );
            ui.label(
                RichText::new(format!(
                    "{} {}",
                    report.items.len(),
                    if report.items.len() == 1 {
                        "suggestion"
                    } else {
                        "suggestions"
                    }
                ))
                .size(11.0)
                .color(MUTED),
            );
        });
        ui.add_space(8.0);
        let query = self.recommendation_query.to_lowercase();
        let filtered: Vec<_> = report
            .items
            .iter()
            .enumerate()
            .filter(|(_, item)| {
                self.recommendation_category
                    .is_none_or(|c| c == item.category)
                    && (query.is_empty()
                        || item.title.to_lowercase().contains(&query)
                        || platform::display_path(&scan.path(item.node))
                            .to_lowercase()
                            .contains(&query))
            })
            .map(|(i, _)| i)
            .collect();
        if self
            .recommendation_selected
            .is_none_or(|i| !filtered.contains(&i))
        {
            self.recommendation_selected = filtered.first().copied();
            ui.ctx().request_repaint();
        }
        if report.truncated {
            ui.label(RichText::new("Up to 200 priority suggestions are shown. Scan a smaller folder for more detail.").size(11.0).color(COLORS[3]));
        }
        if filtered.is_empty() {
            ui.add_space(24.0);
            ui.label(RichText::new("No suggestions match this filter.").size(18.0));
            ui.label(RichText::new("This does not mean there is nothing to clean up; the engine only suggests items with enough context.").color(MUTED));
        }
        egui::ScrollArea::vertical()
            .id_salt("recommendation-list")
            .auto_shrink([false, false])
            .show_rows(ui, 96.0, filtered.len(), |ui, range| {
                for row in range {
                    let index = filtered[row];
                    let item = &report.items[index];
                    let selected = self.recommendation_selected == Some(index);
                    let (rect, response) = ui
                        .allocate_exact_size(Vec2::new(ui.available_width(), 96.0), Sense::click());
                    let p = ui.painter().with_clip_rect(rect);
                    p.rect_filled(
                        rect,
                        8.0,
                        if selected {
                            Color32::from_rgb(27, 52, 53)
                        } else {
                            CARD
                        },
                    );
                    if selected {
                        p.rect_filled(
                            Rect::from_min_size(rect.min, Vec2::new(3.0, rect.height())),
                            2.0,
                            ACCENT,
                        );
                    }
                    let (caption, color) = category(item.category);
                    p.text(
                        rect.min + Vec2::new(16.0, 13.0),
                        egui::Align2::LEFT_TOP,
                        caption,
                        FontId::proportional(9.5),
                        color,
                    );
                    p.text(
                        rect.min + Vec2::new(16.0, 36.0),
                        egui::Align2::LEFT_TOP,
                        &item.title,
                        FontId::proportional(14.0),
                        TEXT,
                    );
                    let path = platform::display_path(&scan.path(item.node));
                    let path_p = p.with_clip_rect(Rect::from_min_max(
                        rect.min + Vec2::new(16.0, 60.0),
                        rect.max - Vec2::new(16.0, 4.0),
                    ));
                    path_p.text(
                        rect.min + Vec2::new(16.0, 68.0),
                        egui::Align2::LEFT_TOP,
                        &path,
                        FontId::proportional(10.5),
                        MUTED,
                    );
                    p.text(
                        rect.right_top() + Vec2::new(-16.0, 14.0),
                        egui::Align2::RIGHT_TOP,
                        human_size(item.bytes),
                        FontId::proportional(17.0),
                        color,
                    );
                    if response.clicked() {
                        self.recommendation_selected = Some(index);
                        ui.ctx().request_repaint();
                    }
                    response
                        .on_hover_text(path)
                        .on_hover_cursor(egui::CursorIcon::PointingHand);
                }
            });
    }

    pub(super) fn recommendation_details(&mut self, ui: &mut egui::Ui) {
        let (Some(report), Some(scan)) = (self.report.clone(), self.scan.clone()) else {
            ui.label(RichText::new("Recommendations appear after scanning.").color(MUTED));
            return;
        };
        let Some(item) = self
            .recommendation_selected
            .and_then(|i| report.items.get(i))
        else {
            ui.label("Select a suggestion to see its context.");
            return;
        };
        egui::ScrollArea::vertical().id_salt("recommendation-details").show(ui, |ui| {
            let (caption, color) = category(item.category);
            ui.label(RichText::new(caption).size(10.0).strong().color(color)); ui.add_space(12.0);
            ui.label(RichText::new(&item.title).size(20.0).strong()); ui.add_space(12.0);
            ui.label(RichText::new(human_size(item.bytes)).size(29.0).strong().color(color));
            ui.label(RichText::new("observed logical size").size(11.0).color(MUTED)); ui.add_space(20.0);
            label(ui, "WHY THIS SUGGESTION"); ui.add_space(6.0); ui.label(&item.reason); ui.add_space(16.0);
            label(ui, "BEFORE REMOVING"); ui.add_space(6.0); ui.add(egui::Label::new(RichText::new(&item.caution).color(COLORS[3])).wrap()); ui.add_space(16.0);
            property(ui, "Location", platform::display_path(&scan.path(item.node)));
            property(ui, "Last modified", date(scan.nodes[item.node].details.modified));
            if report.incomplete { ui.label(RichText::new("The scan is incomplete; some data and opportunities may not have been measured.").size(11.0).color(MUTED)); }
            ui.add_space(12.0);
            let idle = self.operation.is_none() && self.events.is_none() && self.delete_dialog.is_none();
            let (button, tool) = match item.action { Action::StorageSettings => ("Open Windows cleanup", Some(CleanupTool::WindowsRecommendations)), Action::RecycleBin => ("Review Recycle Bin", Some(CleanupTool::RecycleBin)), Action::Inspect => ("Review in tree", None) };
            if ui.add_enabled(idle, egui::Button::new(RichText::new(button).color(BG).strong()).fill(ACCENT).min_size(Vec2::new(ui.available_width(), 40.0))).clicked() {
                if let Some(tool) = tool { self.cleanup_tool(ui.ctx(), tool); } else { self.inspect_recommendation(item.node); ui.ctx().request_repaint(); }
            }
            if ui.add_enabled(idle, egui::Button::new("Show in File Explorer").min_size(Vec2::new(ui.available_width(), 36.0))).clicked() { self.execute_item(ui.ctx(), item.node, ItemAction::Reveal); }
            ui.add_space(16.0);
            ui.label(RichText::new("This suggestion does not run or delete the file. Deletion still requires an explicit action and confirmation in the explorer.").size(11.0).color(MUTED));
            ui.add_space(12.0);
            ui.collapsing("Criteria and references", |ui| {
                ui.label("Duplicates are not identified without comparing content. Personal files, cloud locations and incomplete branches require review. Totals avoid counting a folder and its descendants twice.");
                ui.hyperlink_to("Microsoft · free up space", "https://support.microsoft.com/en-us/windows/experience/storage-filemanagement/free-up-drive-space-in-windows");
                ui.hyperlink_to("Rust · cargo clean", "https://doc.rust-lang.org/cargo/commands/cargo-clean.html");
                ui.hyperlink_to("Next.js · build directory", "https://nextjs.org/docs/app/api-reference/config/next-config-js/distDir");
            });
        });
    }
}
