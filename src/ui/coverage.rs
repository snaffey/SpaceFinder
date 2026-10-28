use super::*;
use spacefinder::scanner::IssueKind;

impl SpaceFinder {
    pub(super) fn coverage(&mut self, ctx: &egui::Context) {
        if !self.show_issues {
            return;
        }
        let mut open = true;
        egui::Window::new("Coverage and permissions").open(&mut open).default_width(780.0).min_width(520.0).max_height(640.0).show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.strong(format!("{} folders with access denied", count(self.progress.denied)));
                ui.separator(); ui.label(format!("{} other errors", count(self.progress.errors.saturating_sub(self.progress.denied))));
                ui.separator(); ui.label(format!("{} links skipped", count(self.progress.skipped)));
            });
            ui.add_space(8.0);
            egui::Frame::new().fill(CARD).corner_radius(8).inner_margin(12).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new(if self.elevated { "The app is running as administrator." } else { "Administrator is optional and may allow more folders to be scanned." }).strong().color(ACCENT));
                ui.label("Some Windows folders remain protected even as administrator. Inaccessible does not mean empty: their sizes cannot be measured.");
                if !self.elevated && ui.add_enabled(self.events.is_none() && self.operation.is_none(), egui::Button::new("Run as administrator…")).clicked() { self.elevate(ctx); }
            });
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                ui.add(egui::TextEdit::singleline(&mut self.issue_filter).hint_text("Filter by path or reason…").desired_width(320.0));
                ui.checkbox(&mut self.denied_only, "Access denied only");
                if ui.button("Copy report").clicked() {
                    let mut text = format!("SpaceFinder · {} access denied · {} errors\n", self.progress.denied, self.progress.errors);
                    if let Some(scan) = &self.scan { for issue in &scan.issues { text.push_str(&format!("{}: {}\n", platform::display_path(&issue.path), issue.message)); } }
                    ctx.copy_text(text);
                }
            });
            ui.separator();
            let query = self.issue_filter.to_lowercase();
            let issues: Vec<_> = self.scan.as_ref().map(|s| s.issues.iter().filter(|issue| (!self.denied_only || issue.kind == IssueKind::AccessDenied) && (query.is_empty() || platform::display_path(&issue.path).to_lowercase().contains(&query) || issue.message.to_lowercase().contains(&query))).collect()).unwrap_or_default();
            if issues.is_empty() { ui.label(RichText::new("No issues match this filter.").color(MUTED)); }
            egui::ScrollArea::vertical().id_salt("coverage-list").max_height(280.0).show(ui, |ui| {
                for issue in issues {
                    egui::Frame::new().fill(CARD).corner_radius(4).inner_margin(8).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.add(egui::Label::new(RichText::new(platform::display_path(&issue.path)).strong()).wrap());
                        let reason = match issue.kind { IssueKind::AccessDenied => "Permission denied when reading this folder.", IssueKind::NotFound => "The item was removed or moved during the scan.", IssueKind::Other => &issue.message };
                        ui.label(RichText::new(reason).small().color(MUTED)).on_hover_text(&issue.message);
                    });
                }
            });
            ui.label(RichText::new("The first 100 issues are stored; counts include all errors.").small().color(MUTED));
            ui.add_space(8.0);
            ui.collapsing("Understanding the results", |ui| {
                ui.label("Coverage: hidden and system files are included when access is permitted. Links, junctions and other reparse points are not followed, avoiding cycles and crossing into other volumes.");
                ui.label("Size: values are logical sizes per path. Hard links, compression and sparse files may cause totals to differ from physical usage. Moving items to the Recycle Bin does not immediately free that space.");
                ui.label("Filters change visible rows only, not scan totals.");
                ui.label("Dates: folder modified and accessed dates include the latest value from scanned descendants. Windows may delay or disable last-access updates.");
            });
        });
        self.show_issues = open;
    }

    pub(super) fn permission_banner(&mut self, ui: &mut egui::Ui) {
        if self.scan.is_none() || self.progress.errors == 0 {
            return;
        }
        egui::Frame::new()
            .fill(Color32::from_rgb(49, 39, 30))
            .stroke(Stroke::new(1.0, Color32::from_rgb(95, 72, 48)))
            .corner_radius(6)
            .inner_margin(10)
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new("Partial scan").strong().color(COLORS[3]));
                    ui.label(format!(
                        "{} folders with access denied · {} other errors",
                        count(self.progress.denied),
                        count(self.progress.errors.saturating_sub(self.progress.denied))
                    ));
                    if ui.small_button("View details").clicked() {
                        self.show_issues = true;
                    }
                });
                if !self.elevated {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(
                            RichText::new(
                                "Administrator is optional and may grant access to more folders.",
                            )
                            .small()
                            .color(MUTED),
                        );
                        if ui
                            .add_enabled(
                                self.operation.is_none() && self.events.is_none(),
                                egui::Button::new("Run as administrator…").small(),
                            )
                            .clicked()
                        {
                            self.elevate(ui.ctx());
                        }
                    });
                } else {
                    ui.label(
                        RichText::new("Some folders remain protected even as administrator.")
                            .small()
                            .color(MUTED),
                    );
                }
            });
        ui.add_space(8.0);
    }
}
