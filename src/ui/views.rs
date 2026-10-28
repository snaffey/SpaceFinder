use super::*;
use egui::{FontId, Pos2, Rect, Sense};
use egui_extras::{Column, TableBuilder};

impl SpaceFinder {
    pub(super) fn files_table(&mut self, ui: &mut egui::Ui) {
        let Some(scan) = self.scan.clone() else {
            return;
        };
        let mut sort_clicked = None;
        let mut selection = None;
        let mut toggle = None;
        let mut menu_action = None;
        let height = if self.compact { 28.0 } else { 38.0 };
        let scroll_to = self
            .pending_scroll_to
            .take()
            .and_then(|index| self.rows.iter().position(|row| row.index == index));
        let extra = usize::from(self.columns.accessed) * 148
            + usize::from(self.columns.created) * 148
            + usize::from(self.columns.attributes) * 180
            + usize::from(self.columns.path) * 360;
        if self.rows.is_empty() {
            empty_result(
                ui,
                "Sem correspondências",
                "Experimenta outro nome ou remove os filtros da tabela.",
            );
            return;
        }
        egui::ScrollArea::horizontal().id_salt("files-horizontal").auto_shrink([false, false]).show(ui, |ui| {
            // Labels must not consume row clicks for text selection.
            ui.style_mut().interaction.selectable_labels = false;
            ui.set_min_width(820.0 + extra as f32);
            let mut table = TableBuilder::new(ui)
                .id_salt(("files", self.browser.scope, self.browser.mode as u8))
                .striped(true).sense(Sense::click()).resizable(true).auto_shrink([false, false])
                .cell_layout(Layout::left_to_right(Align::Center))
                .column(Column::remainder().at_least(224.0).clip(true))
                .column(Column::initial(112.0).at_least(88.0).clip(true))
                .column(Column::initial(108.0).at_least(88.0).clip(true))
                .column(Column::initial(72.0).at_least(56.0).clip(true))
                .column(Column::initial(64.0).at_least(52.0).clip(true))
                .column(Column::initial(142.0).at_least(120.0).clip(true));
            if self.columns.accessed { table = table.column(Column::initial(148.0).clip(true)); }
            if self.columns.created { table = table.column(Column::initial(148.0).clip(true)); }
            if self.columns.attributes { table = table.column(Column::initial(180.0).clip(true)); }
            if self.columns.path { table = table.column(Column::initial(360.0).clip(true)); }
            if let Some(row) = scroll_to { table = table.scroll_to_row(row, Some(Align::Center)); }
            table.header(32.0, |mut header| {
                for (title, sort) in [("Nome", Some(Sort::Name)), ("Tamanho", Some(Sort::Size)), ("% da pasta", None), ("Ficheiros", Some(Sort::Files)), ("Pastas", Some(Sort::Folders)), ("Alteração", Some(Sort::Modified))] {
                    header.col(|ui| { if sort_header(ui, title, sort, &self.browser) { sort_clicked = sort; } });
                }
                if self.columns.accessed { header.col(|ui| { if sort_header(ui, "Último acesso", Some(Sort::Accessed), &self.browser) { sort_clicked = Some(Sort::Accessed); } }); }
                if self.columns.created { header.col(|ui| { sort_header(ui, "Criação", None, &self.browser); }); }
                if self.columns.attributes { header.col(|ui| { sort_header(ui, "Atributos", None, &self.browser); }); }
                if self.columns.path { header.col(|ui| { sort_header(ui, "Caminho completo", None, &self.browser); }); }
            }).body(|body| {
                body.rows(height, self.rows.len(), |mut row| {
                    let visible = self.rows[row.index()]; let index = visible.index; let node = &scan.nodes[index];
                    let selected = self.selected == index && self.selected_extension.is_none();
                    row.set_selected(selected);
                    row.col(|ui| {
                        if selected {
                            let left = ui.max_rect().left_top();
                            ui.painter().rect_filled(Rect::from_min_size(left + Vec2::new(0.0, 4.0), Vec2::new(2.0, height - 8.0)), 1.0, ACCENT);
                        }
                        ui.add_space(visible.depth.min(12) as f32 * 14.0 + 4.0);
                        if self.browser.mode == Mode::Tree && node.directory && !node.children.is_empty() {
                            let expanded = self.browser.expanded.contains(&index) || !self.browser.query.is_empty() || self.browser.extension.is_some();
                            if disclosure(ui, expanded).clicked() { toggle = Some(index); }
                        } else { ui.add_space(18.0); }
                        file_icon(ui, node.directory, if node.directory { COLORS[3] } else { COLORS[1] });
                        if node.incomplete {
                            let (rect, response) = ui.allocate_exact_size(Vec2::splat(6.0), Sense::hover());
                            ui.painter().circle_filled(rect.center(), 2.5, COLORS[3]);
                            response.on_hover_text("Cobertura parcial: este ramo inclui itens sem acesso ou por analisar.");
                        }
                        let name = if index == 0 { platform::display_path(&scan.root) } else { node.name.to_string_lossy().into_owned() };
                        let text = RichText::new(name).size(13.0).color(if selected { TEXT } else if node.incomplete { COLORS[3] } else { TEXT });
                        ui.add(egui::Label::new(if node.directory { text.strong() } else { text }).truncate());
                    });
                    row.col(|ui| number(ui, format!("{}{}", self.unit.format(node.bytes), if node.incomplete { " *" } else { "" })));
                    row.col(|ui| bar(ui, percentage(node.bytes, node.parent.map_or(node.bytes, |p| scan.nodes[p].bytes)), if node.incomplete { COLORS[3] } else { ACCENT }));
                    row.col(|ui| number(ui, count(node.files)));
                    row.col(|ui| number(ui, if node.directory { count(node.folders) } else { "—".into() }));
                    row.col(|ui| { ui.label(RichText::new(date(node.details.modified)).size(12.0).color(MUTED)); });
                    if self.columns.accessed { row.col(|ui| { ui.label(RichText::new(date(node.details.accessed)).size(12.0).color(MUTED)); }); }
                    if self.columns.created { row.col(|ui| { ui.label(RichText::new(date(node.details.created)).size(12.0).color(MUTED)); }); }
                    if self.columns.attributes { row.col(|ui| { ui.add(egui::Label::new(RichText::new(platform::attribute_names(node.details.attributes)).color(MUTED)).truncate()); }); }
                    if self.columns.path { row.col(|ui| { ui.add(egui::Label::new(RichText::new(platform::display_path(&scan.path(index))).color(MUTED)).truncate()); }); }
                    let response = row.response();
                    if response.clicked() || response.secondary_clicked() { selection = Some(index); }
                    if response.double_clicked() && node.directory { toggle = Some(index); }
                    response.clone().on_hover_text(format!("{}\n{} bytes{}\nAlterado: {}\n{}", platform::display_path(&scan.path(index)), count(node.bytes), if node.incomplete { " · valor parcial" } else { "" }, date(node.details.modified), if node.directory { "Duplo clique para expandir/recolher · botão direito para ações" } else { "Seleciona para consultar os detalhes · botão direito para ações" }));
                    response.context_menu(|ui| { if let Some(action) = self.item_menu(ui, index) { menu_action = Some((index, action)); } });
                });
            });
        });
        let redraw = sort_clicked.is_some() || selection.is_some() || toggle.is_some();
        if let Some(sort) = sort_clicked {
            self.browser.set_sort(sort);
            self.refresh();
        }
        if let Some(index) = selection {
            self.select(index);
        }
        if let Some(index) = toggle {
            if self.browser.query.is_empty() && self.browser.extension.is_none() {
                if !self.browser.expanded.remove(&index) {
                    self.browser.expanded.insert(index);
                }
                self.select(index);
                self.refresh();
            } else {
                self.notice = Some("Limpa a pesquisa para recolher ramos; os antepassados dos resultados ficam expandidos.".into());
            }
        }
        if let Some((index, action)) = menu_action {
            self.execute_item(ui.ctx(), index, action);
        }
        if redraw {
            ui.ctx().request_repaint();
        }
    }

    pub(super) fn extensions_table(&mut self, ui: &mut egui::Ui) {
        let Some(scan) = self.scan.clone() else {
            return;
        };
        let mut sort_clicked = None;
        let mut selected = None;
        let mut open = None;
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!(
                    "{} tipos encontrados",
                    count(self.extension_rows.len() as u64)
                ))
                .size(12.0)
                .color(MUTED),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    RichText::new("Duplo clique para explorar os ficheiros")
                        .size(11.0)
                        .color(MUTED),
                );
            });
        });
        ui.add_space(4.0);
        if self.extension_rows.is_empty() {
            empty_result(
                ui,
                "Sem tipos correspondentes",
                "Limpa o filtro para voltar a ver todas as extensões.",
            );
            return;
        }
        egui::ScrollArea::horizontal()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.style_mut().interaction.selectable_labels = false;
                ui.set_min_width(670.0);
                TableBuilder::new(ui)
                    .id_salt("extensions")
                    .striped(true)
                    .sense(Sense::click())
                    .resizable(true)
                    .auto_shrink([false, false])
                    .cell_layout(Layout::left_to_right(Align::Center))
                    .column(Column::remainder().at_least(130.0).clip(true))
                    .column(Column::initial(124.0).clip(true))
                    .column(Column::initial(152.0).clip(true))
                    .column(Column::initial(90.0).clip(true))
                    .column(Column::initial(128.0).clip(true))
                    .header(32.0, |mut header| {
                        for (title, sort) in [
                            ("Extensão", Some(Sort::Name)),
                            ("Tamanho total", Some(Sort::Size)),
                            ("% da análise", None),
                            ("Ficheiros", Some(Sort::Files)),
                            ("Média por ficheiro", None),
                        ] {
                            header.col(|ui| {
                                if sort_header(ui, title, sort, &self.browser) {
                                    sort_clicked = sort;
                                }
                            });
                        }
                    })
                    .body(|body| {
                        body.rows(
                            if self.compact { 30.0 } else { 40.0 },
                            self.extension_rows.len(),
                            |mut row| {
                                let rank = row.index();
                                let ext = &self.extension_rows[rank];
                                let stats = &scan.extensions[ext];
                                let color = extension_color(ext);
                                row.set_selected(self.selected_extension.as_ref() == Some(ext));
                                row.col(|ui| {
                                    let (rect, _) =
                                        ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
                                    ui.painter().rect_filled(rect, 2.0, color);
                                    ui.label(
                                        RichText::new(extension_label(ext)).size(13.0).strong(),
                                    );
                                });
                                row.col(|ui| number(ui, self.unit.format(stats.bytes)));
                                row.col(|ui| {
                                    bar(ui, percentage(stats.bytes, scan.nodes[0].bytes), color)
                                });
                                row.col(|ui| number(ui, count(stats.files)));
                                row.col(|ui| {
                                    number(ui, self.unit.format(stats.bytes / stats.files.max(1)))
                                });
                                if row.response().clicked() {
                                    selected = Some(ext.clone());
                                }
                                if row.response().double_clicked() {
                                    open = Some(ext.clone());
                                }
                            },
                        );
                    });
            });
        let redraw = sort_clicked.is_some() || selected.is_some() || open.is_some();
        if let Some(sort) = sort_clicked {
            self.browser.set_sort(sort);
            self.refresh();
        }
        if let Some(ext) = selected {
            self.selected_extension = Some(ext);
        }
        if let Some(ext) = open {
            self.open_extension(ext);
        }
        if redraw {
            ui.ctx().request_repaint();
        }
    }

    fn open_extension(&mut self, ext: String) {
        self.browser.mode = Mode::Largest;
        self.browser.extension = Some(ext);
        self.browser.query.clear();
        if self.browser.sort != Sort::Size || !self.browser.descending {
            self.browser.set_sort(Sort::Size);
        }
        self.selected_extension = None;
        self.refresh();
        if let Some(row) = self.rows.first() {
            self.select(row.index);
        }
    }

    pub(super) fn properties(&mut self, ui: &mut egui::Ui) {
        let Some(scan) = self.scan.clone() else {
            return;
        };
        section_title(ui, "DETALHES DA SELEÇÃO");
        ui.add_space(8.0);
        egui::ScrollArea::vertical()
            .id_salt("properties-scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if self.browser.mode == Mode::Extensions
                    && let Some(ext) = self.selected_extension.clone()
                    && let Some(stats) = scan.extensions.get(&ext)
                {
                    let color = extension_color(&ext);
                    ui.label(RichText::new(extension_label(&ext)).size(22.0).strong());
                    ui.label(RichText::new("Tipo de ficheiro").size(12.0).color(MUTED));
                    metric(
                        ui,
                        human_size(stats.bytes),
                        "tamanho lógico agregado",
                        color,
                    );
                    detail_row(ui, "Ficheiros", count(stats.files));
                    detail_row(ui, "Média", human_size(stats.bytes / stats.files.max(1)));
                    detail_row(ui, "Bytes exatos", count(stats.bytes));
                    ui.add_space(8.0);
                    bar(ui, percentage(stats.bytes, scan.nodes[0].bytes), color);
                    ui.label(RichText::new("do espaço medido").size(11.0).color(MUTED));
                    ui.add_space(16.0);
                    if ui
                        .add_sized(
                            [ui.available_width(), 34.0],
                            egui::Button::new("Explorar estes ficheiros"),
                        )
                        .clicked()
                    {
                        self.open_extension(ext);
                    }
                    return;
                }
                let node = &scan.nodes[self.selected];
                let path = scan.path(self.selected);
                ui.horizontal(|ui| {
                    file_icon(
                        ui,
                        node.directory,
                        if node.directory { COLORS[3] } else { COLORS[1] },
                    );
                    ui.label(
                        RichText::new(if node.directory { "PASTA" } else { "FICHEIRO" })
                            .size(10.0)
                            .strong()
                            .color(MUTED),
                    );
                });
                ui.add_space(4.0);
                ui.add(
                    egui::Label::new(
                        RichText::new(if self.selected == 0 {
                            platform::display_path(&path)
                        } else {
                            node.name.to_string_lossy().into_owned()
                        })
                        .size(17.0)
                        .strong(),
                    )
                    .wrap(),
                );
                if !node.directory {
                    ui.label(
                        RichText::new(extension_label(&view::extension(node)))
                            .size(11.0)
                            .color(MUTED),
                    );
                }
                if node.incomplete {
                    ui.add_space(8.0);
                    egui::Frame::new()
                        .fill(COLORS[3].gamma_multiply(0.09))
                        .corner_radius(5)
                        .inner_margin(8)
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.label(
                                RichText::new("Cobertura parcial")
                                    .size(12.0)
                                    .strong()
                                    .color(COLORS[3]),
                            );
                            ui.label(
                                RichText::new("Inclui apenas o que foi possível medir.")
                                    .size(11.0)
                                    .color(MUTED),
                            );
                        });
                }
                metric(
                    ui,
                    human_size(node.bytes),
                    "tamanho lógico",
                    if node.incomplete { COLORS[3] } else { ACCENT },
                );
                detail_row(ui, "Bytes exatos", count(node.bytes));
                if node.directory {
                    detail_row(ui, "Ficheiros", count(node.files));
                    detail_row(ui, "Subpastas", count(node.folders));
                    detail_row(ui, "Itens diretos", count(node.children.len() as u64));
                    detail_row(
                        ui,
                        "Média / ficheiro",
                        human_size(node.bytes / node.files.max(1)),
                    );
                }
                ui.add_space(8.0);
                section_title(ui, "OCUPAÇÃO");
                detail_row(
                    ui,
                    "Da pasta superior",
                    format!(
                        "{:.2}%",
                        percentage(
                            node.bytes,
                            node.parent.map_or(node.bytes, |i| scan.nodes[i].bytes)
                        )
                    ),
                );
                detail_row(
                    ui,
                    "Da análise",
                    format!("{:.2}%", percentage(node.bytes, scan.nodes[0].bytes)),
                );
                ui.add_space(12.0);
                section_title(ui, "DATAS E ATRIBUTOS");
                detail_row(
                    ui,
                    if node.directory {
                        "Alteração do ramo"
                    } else {
                        "Alteração"
                    },
                    date(node.details.modified),
                );
                detail_row(
                    ui,
                    if node.directory {
                        "Acesso do ramo"
                    } else {
                        "Último acesso"
                    },
                    date(node.details.accessed),
                );
                detail_row(ui, "Criação", date(node.details.created));
                property(
                    ui,
                    "Atributos",
                    platform::attribute_names(node.details.attributes),
                );
                ui.add_space(4.0);
                section_title(ui, "LOCALIZAÇÃO");
                ui.add_space(4.0);
                ui.add(
                    egui::Label::new(
                        RichText::new(platform::display_path(&path))
                            .size(11.5)
                            .color(MUTED),
                    )
                    .wrap(),
                );
                ui.add_space(12.0);
                let idle = self.operation.is_none() && self.events.is_none();
                ui.horizontal_wrapped(|ui| {
                    if ui.button("Copiar caminho").clicked() {
                        ui.ctx().copy_text(platform::display_path(&path));
                    }
                    if ui
                        .add_enabled(idle, egui::Button::new("Explorador"))
                        .clicked()
                    {
                        self.execute_item(ui.ctx(), self.selected, ItemAction::Reveal);
                    }
                    if node.directory && ui.button("Focar pasta").clicked() {
                        self.focus(self.selected, true);
                    }
                    if ui
                        .add_enabled(idle, egui::Button::new("Propriedades do Windows"))
                        .clicked()
                    {
                        self.execute_item(ui.ctx(), self.selected, ItemAction::Properties);
                    }
                });
                ui.add_space(12.0);
                ui.separator();
                if ui
                    .add_enabled(
                        idle && self.selected != 0,
                        egui::Button::new(
                            RichText::new("Enviar para a Reciclagem…")
                                .color(Color32::from_rgb(242, 155, 151)),
                        )
                        .min_size(Vec2::new(ui.available_width(), 32.0)),
                    )
                    .clicked()
                {
                    self.execute_item(ui.ctx(), self.selected, ItemAction::Recycle);
                }
                if let Some(volume) = self.volumes.iter().find(|v| {
                    platform::display_path(&scan.root).starts_with(&platform::display_path(&v.path))
                }) {
                    ui.add_space(16.0);
                    section_title(ui, "UNIDADE");
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.strong(platform::display_path(&volume.path));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(
                                RichText::new(format!("{} livres", human_size(volume.free)))
                                    .size(11.0)
                                    .color(MUTED),
                            );
                        });
                    });
                    ui.add(
                        egui::ProgressBar::new(
                            1.0 - volume.free as f32 / volume.total.max(1) as f32,
                        )
                        .fill(ACCENT)
                        .desired_height(5.0),
                    );
                }
            });
    }

    pub(super) fn map(&mut self, ui: &mut egui::Ui) {
        let Some(scan) = self.scan.clone() else {
            return;
        };
        let focus = self.map_focus.unwrap_or(0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Distribuição do espaço").size(13.0).strong());
            ui.add(
                egui::Label::new(
                    RichText::new(format!(
                        "{}{}",
                        platform::display_path(&scan.path(focus)),
                        if scan.nodes[focus].incomplete {
                            " · parcial"
                        } else {
                            ""
                        }
                    ))
                    .size(11.0)
                    .color(MUTED),
                )
                .truncate(),
            );
        });
        ui.add_space(4.0);
        let (rect, _) = ui.allocate_exact_size(
            Vec2::new(
                ui.available_width(),
                (ui.available_height() - 28.0).max(24.0),
            ),
            Sense::hover(),
        );
        ui.painter().rect_filled(rect, 5.0, CARD);
        let mut navigate = None;
        let mut menu_action = None;
        let mut selection = None;
        for (rank, (index, bytes, area)) in treemap(&self.map_nodes, rect.shrink(1.0))
            .into_iter()
            .enumerate()
        {
            if area.width() < 2.0 || area.height() < 2.0 {
                continue;
            }
            let response = ui.interact(area, ui.id().with(("map", rank)), Sense::click());
            if let Some(index) = index {
                if response.secondary_clicked() {
                    selection = Some(index);
                }
                response.clone().context_menu(|ui| {
                    if let Some(action) = self.item_menu(ui, index) {
                        menu_action = Some((index, action));
                    }
                });
            }
            let name = index
                .map(|i| scan.nodes[i].name.to_string_lossy().into_owned())
                .unwrap_or_else(|| "Outros itens".into());
            let color = map_color(&scan, index, rank);
            let selected = index == Some(self.selected);
            let inner = area.shrink(1.5);
            ui.painter().rect_filled(
                inner,
                3.0,
                color.gamma_multiply(if response.hovered() || selected {
                    0.44
                } else {
                    0.24
                }),
            );
            if response.hovered() || selected {
                ui.painter().rect_stroke(
                    inner,
                    3.0,
                    Stroke::new(1.0, color),
                    egui::StrokeKind::Inside,
                );
            }
            if area.width() > 72.0 && area.height() > 42.0 {
                let p = ui.painter().with_clip_rect(area.shrink(8.0));
                p.text(
                    area.min + Vec2::new(10.0, 9.0),
                    egui::Align2::LEFT_TOP,
                    &name,
                    FontId::proportional(12.0),
                    TEXT,
                );
                p.text(
                    area.min + Vec2::new(10.0, 28.0),
                    egui::Align2::LEFT_TOP,
                    self.unit.format(bytes),
                    FontId::monospace(11.0),
                    color,
                );
            }
            if response.clicked()
                && let Some(index) = index
            {
                navigate = Some(index);
            }
            response.on_hover_text(format!(
                "{name}\n{} · {:.2}% desta pasta\nClica para explorar · botão direito para ações",
                human_size(bytes),
                percentage(bytes, scan.nodes[focus].bytes)
            ));
        }
        if self.map_nodes.is_empty() {
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "Esta pasta não contém ficheiros com tamanho medido.",
                FontId::proportional(12.0),
                MUTED,
            );
        }
        let legend_count = ((ui.available_width() / 180.0) as usize)
            .clamp(1, 3)
            .min(self.map_nodes.len());
        if legend_count > 0 {
            ui.add_space(6.0);
            let legend_width =
                (ui.available_width() - 8.0 * (legend_count - 1) as f32) / legend_count as f32;
            ui.horizontal(|ui| {
                for (rank, (index, bytes)) in self.map_nodes.iter().take(legend_count).enumerate() {
                    let (area, response) =
                        ui.allocate_exact_size(Vec2::new(legend_width, 16.0), Sense::hover());
                    let color = map_color(&scan, *index, rank);
                    ui.painter().circle_filled(
                        area.left_center() + Vec2::new(4.0, 0.0),
                        3.0,
                        color,
                    );
                    let name = index
                        .map(|i| scan.nodes[i].name.to_string_lossy().into_owned())
                        .unwrap_or_else(|| "Outros itens".into());
                    let painter = ui.painter().with_clip_rect(Rect::from_min_max(
                        area.min + Vec2::new(13.0, 0.0),
                        Pos2::new(area.max.x - 74.0, area.max.y),
                    ));
                    painter.text(
                        area.left_center() + Vec2::new(13.0, 0.0),
                        egui::Align2::LEFT_CENTER,
                        &name,
                        FontId::proportional(11.0),
                        MUTED,
                    );
                    ui.painter().text(
                        area.right_center(),
                        egui::Align2::RIGHT_CENTER,
                        human_size(*bytes),
                        FontId::monospace(10.5),
                        TEXT,
                    );
                    response.on_hover_text(name);
                }
            });
        }
        if let Some(index) = navigate {
            if scan.nodes[index].directory {
                self.focus(index, true);
            } else {
                self.select(index);
            }
            ui.ctx().request_repaint();
        }
        if let Some(index) = selection {
            self.select(index);
            ui.ctx().request_repaint();
        }
        if let Some((index, action)) = menu_action {
            self.execute_item(ui.ctx(), index, action);
        }
    }
}

fn extension_label(ext: &str) -> String {
    if ext == "sem extensão" {
        ext.into()
    } else {
        format!(".{ext}")
    }
}

fn extension_color(ext: &str) -> Color32 {
    let hash = ext.bytes().fold(0_usize, |value, byte| {
        value.wrapping_mul(31).wrapping_add(byte as usize)
    });
    COLORS[hash % COLORS.len()]
}

fn map_color(scan: &Scan, index: Option<usize>, rank: usize) -> Color32 {
    match index {
        Some(index) if !scan.nodes[index].directory => {
            extension_color(&view::extension(&scan.nodes[index]))
        }
        Some(_) => COLORS[rank % COLORS.len()],
        None => MUTED,
    }
}

fn section_title(ui: &mut egui::Ui, title: &str) {
    ui.add_space(2.0);
    ui.label(RichText::new(title).size(10.0).strong().color(MUTED));
    ui.add_space(2.0);
    ui.separator();
}

fn metric(ui: &mut egui::Ui, value: String, caption: &str, color: Color32) {
    ui.add_space(14.0);
    ui.label(RichText::new(value).size(27.0).strong().color(color));
    ui.label(RichText::new(caption).size(11.0).color(MUTED));
    ui.add_space(12.0);
}

fn detail_row(ui: &mut egui::Ui, label: &str, value: impl Into<String>) {
    let value = value.into();
    let width = ui
        .available_width()
        .min(ui.clip_rect().right() - ui.cursor().left())
        .max(0.0);
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 22.0), Sense::hover());
    let split = rect.min.x + rect.width() * 0.42;
    let labels = ui.painter().with_clip_rect(egui::Rect::from_min_max(
        rect.min,
        egui::Pos2::new(split, rect.max.y),
    ));
    labels.text(
        rect.left_center(),
        egui::Align2::LEFT_CENTER,
        label,
        FontId::proportional(11.0),
        MUTED,
    );
    let values = ui.painter().with_clip_rect(egui::Rect::from_min_max(
        egui::Pos2::new(split + 4.0, rect.min.y),
        rect.max,
    ));
    values.text(
        rect.right_center() - Vec2::new(2.0, 0.0),
        egui::Align2::RIGHT_CENTER,
        &value,
        FontId::proportional(12.0),
        TEXT,
    );
    response.on_hover_text(format!("{label}: {value}"));
}

fn empty_result(ui: &mut egui::Ui, title: &str, help: &str) {
    ui.add_space(40.0);
    ui.vertical_centered(|ui| {
        ui.label(RichText::new(title).size(18.0).strong());
        ui.add_space(6.0);
        ui.label(RichText::new(help).size(12.0).color(MUTED));
    });
}

fn sort_header(ui: &mut egui::Ui, title: &str, key: Option<Sort>, browser: &Browser) -> bool {
    let active = key == Some(browser.sort);
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), ui.available_height().max(28.0)),
        if key.is_some() {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    if response.hovered() && key.is_some() {
        ui.painter().rect_filled(rect, 3.0, CARD);
    }
    if active {
        ui.painter().line_segment(
            [rect.left_bottom(), rect.right_bottom()],
            Stroke::new(2.0, ACCENT),
        );
    }
    let right_padding = if active { 16.0 } else { 2.0 };
    let painter = ui.painter().with_clip_rect(Rect::from_min_max(
        rect.min,
        Pos2::new(rect.max.x - right_padding, rect.max.y),
    ));
    painter.text(
        rect.left_center(),
        egui::Align2::LEFT_CENTER,
        title,
        FontId::proportional(12.0),
        if active { ACCENT } else { MUTED },
    );
    if active {
        let center = rect.right_center() - Vec2::new(6.0, 0.0);
        let points = if browser.descending {
            vec![
                center + Vec2::new(-3.0, -1.5),
                center + Vec2::new(3.0, -1.5),
                center + Vec2::new(0.0, 2.5),
            ]
        } else {
            vec![
                center + Vec2::new(-3.0, 1.5),
                center + Vec2::new(3.0, 1.5),
                center + Vec2::new(0.0, -2.5),
            ]
        };
        ui.painter()
            .add(egui::Shape::convex_polygon(points, ACCENT, Stroke::NONE));
    }
    key.is_some()
        && response
            .on_hover_text("Clica para ordenar; volta a clicar para inverter.")
            .clicked()
}
