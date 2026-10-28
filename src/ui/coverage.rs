use super::*;
use spacefinder::scanner::IssueKind;

impl SpaceFinder {
    pub(super) fn coverage(&mut self, ctx: &egui::Context) {
        if !self.show_issues {
            return;
        }
        let mut open = true;
        egui::Window::new("Cobertura e permissões").open(&mut open).default_width(780.0).min_width(520.0).max_height(640.0).show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.strong(format!("{} pastas sem permissão", count(self.progress.denied)));
                ui.separator(); ui.label(format!("{} outros erros", count(self.progress.errors.saturating_sub(self.progress.denied))));
                ui.separator(); ui.label(format!("{} ligações ignoradas", count(self.progress.skipped)));
            });
            ui.add_space(8.0);
            egui::Frame::new().fill(CARD).corner_radius(8).inner_margin(12).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new(if self.elevated { "A app está a executar como administrador." } else { "Administrador é opcional e pode permitir analisar mais pastas." }).strong().color(ACCENT));
                ui.label("Algumas pastas do Windows continuam protegidas mesmo com administrador. Não ter acesso não significa que estejam vazias: os respetivos tamanhos ficam por medir.");
                if !self.elevated && ui.add_enabled(self.events.is_none() && self.operation.is_none(), egui::Button::new("Abrir como administrador…")).clicked() { self.elevate(ctx); }
            });
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                ui.add(egui::TextEdit::singleline(&mut self.issue_filter).hint_text("Filtrar por caminho ou motivo…").desired_width(320.0));
                ui.checkbox(&mut self.denied_only, "Só sem permissão");
                if ui.button("Copiar relatório").clicked() {
                    let mut text = format!("SpaceFinder · {} sem permissão · {} erros\n", self.progress.denied, self.progress.errors);
                    if let Some(scan) = &self.scan { for issue in &scan.issues { text.push_str(&format!("{}: {}\n", platform::display_path(&issue.path), issue.message)); } }
                    ctx.copy_text(text);
                }
            });
            ui.separator();
            let query = self.issue_filter.to_lowercase();
            let issues: Vec<_> = self.scan.as_ref().map(|s| s.issues.iter().filter(|issue| (!self.denied_only || issue.kind == IssueKind::AccessDenied) && (query.is_empty() || platform::display_path(&issue.path).to_lowercase().contains(&query) || issue.message.to_lowercase().contains(&query))).collect()).unwrap_or_default();
            if issues.is_empty() { ui.label(RichText::new("Sem ocorrências para este filtro.").color(MUTED)); }
            egui::ScrollArea::vertical().id_salt("coverage-list").max_height(280.0).show(ui, |ui| {
                for issue in issues {
                    egui::Frame::new().fill(CARD).corner_radius(4).inner_margin(8).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.add(egui::Label::new(RichText::new(platform::display_path(&issue.path)).strong()).wrap());
                        let reason = match issue.kind { IssueKind::AccessDenied => "Sem permissão para ler esta pasta.", IssueKind::NotFound => "O item foi removido ou movido durante a análise.", IssueKind::Other => &issue.message };
                        ui.label(RichText::new(reason).small().color(MUTED)).on_hover_text(&issue.message);
                    });
                }
            });
            ui.label(RichText::new("São guardadas as primeiras 100 ocorrências; as contagens incluem todos os erros.").small().color(MUTED));
            ui.add_space(8.0);
            ui.collapsing("Como interpretar os resultados", |ui| {
                ui.label("Cobertura: ficheiros ocultos e de sistema são incluídos quando há permissão. Ligações, junctions e outros reparse points não são seguidos, evitando ciclos e atravessar outros volumes.");
                ui.label("Tamanho: os valores são lógicos, por caminho. Hard links, compressão e ficheiros sparse podem fazer os totais diferir do espaço físico. Enviar para a Reciclagem não liberta imediatamente esse espaço.");
                ui.label("Filtros: alteram apenas as linhas visíveis, não os totais da análise.");
                ui.label("Datas: a alteração e o acesso das pastas incluem o valor mais recente dos descendentes analisados. O Windows pode atrasar ou desativar a atualização do último acesso.");
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
                    ui.label(RichText::new("Análise parcial").strong().color(COLORS[3]));
                    ui.label(format!(
                        "{} pastas sem permissão · {} outros erros",
                        count(self.progress.denied),
                        count(self.progress.errors.saturating_sub(self.progress.denied))
                    ));
                    if ui.small_button("Ver detalhes").clicked() {
                        self.show_issues = true;
                    }
                });
                if !self.elevated {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(
                            RichText::new(
                                "Administrador é opcional e pode dar acesso a mais pastas.",
                            )
                            .small()
                            .color(MUTED),
                        );
                        if ui
                            .add_enabled(
                                self.operation.is_none() && self.events.is_none(),
                                egui::Button::new("Abrir como administrador…").small(),
                            )
                            .clicked()
                        {
                            self.elevate(ui.ctx());
                        }
                    });
                } else {
                    ui.label(
                        RichText::new(
                            "Algumas pastas continuam protegidas mesmo com administrador.",
                        )
                        .small()
                        .color(MUTED),
                    );
                }
            });
        ui.add_space(8.0);
    }
}
