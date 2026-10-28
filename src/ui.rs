mod coverage;
mod dashboard;
mod operations;
mod views;
mod widgets;

use crossbeam_channel::Receiver;
use dashboard::navigation;
use eframe::egui::{self, Align, Color32, Layout, RichText, Stroke, Vec2};
use operations::{DeleteDialog, ItemAction, Operation};
use spacefinder::{
    human_size,
    platform::{self, Volume},
    recommendations::{self, Report},
    scanner::{self, Event, Progress, Scan},
    view::{self, Browser, Mode, Row, Sort},
};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use widgets::*;

const BG: Color32 = Color32::from_rgb(13, 17, 23);
const PANEL: Color32 = Color32::from_rgb(19, 25, 34);
const CARD: Color32 = Color32::from_rgb(26, 34, 45);
const LINE: Color32 = Color32::from_rgb(40, 51, 66);
const TEXT: Color32 = Color32::from_rgb(234, 240, 248);
const MUTED: Color32 = Color32::from_rgb(149, 164, 185);
const ACCENT: Color32 = Color32::from_rgb(86, 218, 182);
const COLORS: [Color32; 8] = [
    ACCENT,
    Color32::from_rgb(115, 166, 245),
    Color32::from_rgb(179, 147, 236),
    Color32::from_rgb(232, 181, 115),
    Color32::from_rgb(228, 135, 160),
    Color32::from_rgb(101, 193, 219),
    Color32::from_rgb(167, 192, 120),
    Color32::from_rgb(180, 172, 219),
];

#[derive(Default)]
struct Columns {
    accessed: bool,
    created: bool,
    attributes: bool,
    path: bool,
}

type AnalysisResult = Result<(Arc<Scan>, Report), String>;

pub struct SpaceFinder {
    path: String,
    volumes: Vec<Volume>,
    scan: Option<Arc<Scan>>,
    events: Option<Receiver<Event>>,
    cancel: Option<Arc<AtomicBool>>,
    progress: Progress,
    browser: Browser,
    rows: Vec<Row>,
    extension_rows: Vec<String>,
    selected: usize,
    selected_extension: Option<String>,
    map_focus: Option<usize>,
    map_nodes: Vec<(Option<usize>, u64)>,
    unit: Unit,
    columns: Columns,
    show_map: bool,
    show_details: bool,
    compact: bool,
    workers: usize,
    notice: Option<String>,
    picker: Option<Receiver<Option<PathBuf>>>,
    exporter: Option<Receiver<Result<bool, String>>>,
    show_issues: bool,
    history: Vec<usize>,
    history_at: usize,
    initial: Option<PathBuf>,
    operation: Option<Operation>,
    delete_dialog: Option<DeleteDialog>,
    elevated: bool,
    issue_filter: String,
    denied_only: bool,
    resume_after_scan: Option<PathBuf>,
    report: Option<Arc<Report>>,
    analysis: Option<Receiver<AnalysisResult>>,
    recommendation_query: String,
    recommendation_category: Option<recommendations::Category>,
    overview_children: Vec<usize>,
    recommendation_selected: Option<usize>,
    pending_scroll_to: Option<usize>,
    elevation_ready: Option<(std::ffi::OsString, u64)>,
}

impl SpaceFinder {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        initial: Option<PathBuf>,
        elevation_ready: Option<std::ffi::OsString>,
    ) -> Self {
        let ctx = &cc.egui_ctx;
        ctx.set_theme(egui::Theme::Dark);
        let mut style = (*ctx.global_style()).clone();
        style.visuals = egui::Visuals::dark();
        style.visuals.panel_fill = BG;
        style.visuals.window_fill = PANEL;
        style.visuals.extreme_bg_color = BG;
        style.visuals.override_text_color = Some(TEXT);
        style.visuals.selection.bg_fill = Color32::from_rgb(36, 76, 77);
        style.visuals.selection.stroke = Stroke::new(1.0, ACCENT);
        style.visuals.widgets.inactive.weak_bg_fill = CARD;
        style.visuals.widgets.inactive.bg_fill = CARD;
        style.visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, LINE);
        style.visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(48, 65, 77);
        style.spacing.item_spacing = Vec2::new(8.0, 8.0);
        style.spacing.button_padding = Vec2::new(12.0, 8.0);
        style.spacing.slider_width = 120.0;
        style.animation_time = 0.16;
        style
            .text_styles
            .insert(egui::TextStyle::Body, egui::FontId::proportional(13.0));
        style
            .text_styles
            .insert(egui::TextStyle::Button, egui::FontId::proportional(13.0));
        ctx.set_global_style(style);
        let volumes = platform::volumes();
        let path = initial
            .as_ref()
            .map(|p| platform::display_path(p))
            .unwrap_or_else(|| {
                volumes
                    .first()
                    .map(|v| platform::display_path(&v.path))
                    .unwrap_or_default()
            });
        Self {
            path,
            volumes,
            scan: None,
            events: None,
            cancel: None,
            progress: Progress::default(),
            browser: Browser::with_mode(Mode::Overview),
            rows: Vec::new(),
            extension_rows: Vec::new(),
            selected: 0,
            selected_extension: None,
            map_focus: None,
            map_nodes: Vec::new(),
            unit: Unit::Auto,
            columns: Columns::default(),
            show_map: true,
            show_details: true,
            compact: true,
            workers: scanner::default_workers(),
            notice: None,
            picker: None,
            exporter: None,
            show_issues: false,
            history: vec![0],
            history_at: 0,
            initial,
            operation: None,
            delete_dialog: None,
            elevated: spacefinder::actions::is_elevated(),
            issue_filter: String::new(),
            denied_only: false,
            resume_after_scan: None,
            report: None,
            analysis: None,
            recommendation_query: String::new(),
            recommendation_category: None,
            overview_children: Vec::new(),
            recommendation_selected: None,
            pending_scroll_to: None,
            elevation_ready: elevation_ready.map(|name| (name, ctx.cumulative_frame_nr())),
        }
    }

    fn start(&mut self) {
        if self.events.is_some()
            || self.operation.is_some()
            || self.delete_dialog.is_some()
            || self.path.trim().is_empty()
        {
            return;
        }
        let (events, cancel) = scanner::start(PathBuf::from(self.path.trim()), self.workers);
        self.events = Some(events);
        self.cancel = Some(cancel);
        self.scan = None;
        self.progress = Progress::default();
        self.browser = Browser::with_mode(Mode::Overview);
        self.rows.clear();
        self.extension_rows.clear();
        self.selected = 0;
        self.selected_extension = None;
        self.map_focus = None;
        self.map_nodes.clear();
        self.history = vec![0];
        self.history_at = 0;
        self.notice = None;
        self.resume_after_scan = None;
        // Dropping the old receiver prevents reports from a previous scan being used.
        self.analysis = None;
        self.report = None;
        self.overview_children.clear();
        self.recommendation_query.clear();
        self.recommendation_selected = None;
        self.pending_scroll_to = None;
        self.volumes = platform::volumes();
    }

    fn poll(&mut self, ctx: &egui::Context) {
        self.poll_operation(ctx);
        let mut finished = false;
        if let Some(events) = &self.events {
            while let Ok(event) = events.try_recv() {
                match event {
                    Event::Progress(progress) => self.progress = progress,
                    Event::Finished(result) => {
                        match result {
                            Ok(scan) => {
                                self.progress = scan.progress.clone();
                                self.scan = Some(Arc::new(scan));
                            }
                            Err(e) => self.notice = Some(e),
                        }
                        finished = true;
                    }
                }
            }
        }
        if finished {
            self.events = None;
            self.cancel = None;
            self.refresh();
            self.prepare_map();
            if let Some(scan) = self.scan.clone() {
                self.overview_children = scan.sorted_children(0).into_iter().take(8).collect();
                let (tx, rx) = crossbeam_channel::bounded(1);
                let wake = ctx.clone();
                std::thread::spawn(move || {
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map_or(0, |v| v.as_secs() as i64);
                        let report = recommendations::analyze(&scan, now);
                        (scan, report)
                    }))
                    .map_err(|_| {
                        "Não foi possível preparar as recomendações. Tenta reanalisar.".into()
                    });
                    let _ = tx.send(result);
                    wake.request_repaint();
                });
                self.analysis = Some(rx);
            }
            if let Some(path) = self.resume_after_scan.take()
                && let Some(scan) = &self.scan
            {
                let mut index = 0;
                if let Ok(relative) = path.strip_prefix(&scan.root) {
                    for part in relative.components() {
                        let Some(child) = scan.nodes[index].children.iter().copied().find(|i| {
                            scan.nodes[*i].directory && scan.nodes[*i].name == part.as_os_str()
                        }) else {
                            break;
                        };
                        index = child;
                    }
                }
                self.focus(index, false);
                self.history = vec![index];
                self.history_at = 0;
            }
        }
        if let Some(analysis) = &self.analysis
            && let Ok(result) = analysis.try_recv()
        {
            self.analysis = None;
            match result {
                Ok((source, report))
                    if self.scan.as_ref().is_some_and(|s| Arc::ptr_eq(s, &source)) =>
                {
                    self.recommendation_selected = (!report.items.is_empty()).then_some(0);
                    self.report = Some(Arc::new(report));
                }
                Ok(_) => {}
                Err(error) => self.notice = Some(error),
            }
        }
        if let Some(picker) = &self.picker
            && let Ok(path) = picker.try_recv()
        {
            if let Some(path) = path {
                self.path = platform::display_path(&path);
                self.start();
            }
            self.picker = None;
        }
        if let Some(exporter) = &self.exporter
            && let Ok(result) = exporter.try_recv()
        {
            self.notice = Some(match result {
                Ok(true) => "Relatório CSV exportado.".into(),
                Ok(false) => "Exportação cancelada.".into(),
                Err(e) => e,
            });
            self.exporter = None;
        }
        if self.events.is_some()
            || self.picker.is_some()
            || self.exporter.is_some()
            || self.operation.is_some()
            || self.analysis.is_some()
        {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }

    fn refresh(&mut self) {
        if let Some(scan) = &self.scan {
            self.rows = self.browser.rows(scan);
            let query = self.browser.query.to_lowercase();
            self.extension_rows = scan
                .extensions
                .keys()
                .filter(|ext| ext.contains(&query))
                .cloned()
                .collect();
            self.extension_rows.sort_unstable_by(|a, b| {
                let (left, right) = (&scan.extensions[a], &scan.extensions[b]);
                let order = match self.browser.sort {
                    Sort::Name => a.cmp(b),
                    Sort::Files => left.files.cmp(&right.files),
                    _ => left.bytes.cmp(&right.bytes),
                };
                (if self.browser.descending {
                    order.reverse()
                } else {
                    order
                })
                .then_with(|| a.cmp(b))
            });
            if self.browser.mode == Mode::Extensions
                && self
                    .selected_extension
                    .as_ref()
                    .is_none_or(|ext| !self.extension_rows.contains(ext))
            {
                self.selected_extension = self.extension_rows.first().cloned();
            }
        }
    }

    fn select(&mut self, index: usize) {
        self.selected = index;
        self.selected_extension = None;
        self.prepare_map();
    }

    fn prepare_map(&mut self) {
        let Some(scan) = &self.scan else {
            return;
        };
        let index = if scan.nodes[self.selected].directory {
            self.selected
        } else {
            scan.nodes[self.selected].parent.unwrap_or(0)
        };
        if self.map_focus == Some(index) {
            return;
        }
        self.map_focus = Some(index);
        self.map_nodes.clear();
        let mut rest = 0;
        for child in scan.sorted_children(index) {
            let bytes = scan.nodes[child].bytes;
            if bytes == 0 {
                continue;
            }
            if self.map_nodes.len() < 47 {
                self.map_nodes.push((Some(child), bytes));
            } else {
                rest += bytes;
            }
        }
        if rest > 0 {
            self.map_nodes.push((None, rest));
        }
    }

    fn focus(&mut self, index: usize, remember: bool) {
        self.browser.scope = index;
        self.browser.mode = Mode::Tree;
        self.browser.expanded.insert(index);
        self.browser.query.clear();
        self.browser.extension = None;
        if remember && self.history.get(self.history_at) != Some(&index) {
            self.history.truncate(self.history_at + 1);
            self.history.push(index);
            self.history_at += 1;
        }
        self.select(index);
        self.refresh();
    }

    fn choose_folder(&mut self) {
        if self.picker.is_some()
            || self.operation.is_some()
            || self.events.is_some()
            || self.delete_dialog.is_some()
        {
            return;
        }
        let (tx, rx) = crossbeam_channel::bounded(1);
        std::thread::spawn(move || {
            let _ = tx.send(
                rfd::FileDialog::new()
                    .set_title("Escolher pasta para analisar")
                    .pick_folder(),
            );
        });
        self.picker = Some(rx);
    }

    fn export(&mut self) {
        let Some(scan) = self.scan.clone() else {
            return;
        };
        let (tx, rx) = crossbeam_channel::bounded(1);
        std::thread::spawn(move || {
            let result = if let Some(path) = rfd::FileDialog::new()
                .set_file_name("spacefinder.csv")
                .add_filter("CSV", &["csv"])
                .save_file()
            {
                use std::io::Write;
                std::fs::File::create(path)
                    .and_then(|file| {
                        let mut writer = std::io::BufWriter::new(file);
                        scan.write_csv(&mut writer)?;
                        writer.flush()
                    })
                    .map(|()| true)
                    .map_err(|e| format!("Falha ao exportar: {e}"))
            } else {
                Ok(false)
            };
            let _ = tx.send(result);
        });
        self.exporter = Some(rx);
    }

    fn change_mode(&mut self, mode: Mode) {
        if self.browser.mode == mode {
            return;
        }
        self.browser.mode = mode;
        self.browser.query.clear();
        self.browser.extension = None;
        if mode == Mode::Extensions
            && !matches!(self.browser.sort, Sort::Name | Sort::Size | Sort::Files)
        {
            self.browser.set_sort(Sort::Size);
        }
        self.refresh();
    }

    fn sidebar(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        ui.horizontal(brand);
        ui.label(
            RichText::new("ESPAÇO PARA O QUE IMPORTA")
                .size(9.5)
                .color(MUTED),
        );
        ui.add_space(24.0);
        let idle =
            self.events.is_none() && self.operation.is_none() && self.delete_dialog.is_none();
        egui::ScrollArea::vertical()
            .id_salt("sidebar-content")
            .max_height((ui.available_height() - 96.0).max(200.0))
            .show(ui, |ui| {
                ui.label(RichText::new("ANÁLISE").size(10.0).strong().color(MUTED));
                ui.add_space(8.0);
                for (mode, title, caption) in [
                    (Mode::Overview, "Visão geral", "O disco num relance"),
                    (Mode::Tree, "Explorador", "Pastas e ficheiros"),
                    (Mode::Largest, "Maiores ficheiros", "Onde está o espaço"),
                    (
                        Mode::Extensions,
                        "Tipos de ficheiro",
                        "Distribuição por extensão",
                    ),
                    (
                        Mode::Recommendations,
                        "Recomendações",
                        "O que vale a pena rever",
                    ),
                ] {
                    if navigation(ui, mode, title, caption, self.browser.mode == mode).clicked() {
                        self.change_mode(mode);
                    }
                }
                ui.add_space(24.0);
                ui.label(
                    RichText::new("UNIDADES LOCAIS")
                        .size(10.0)
                        .strong()
                        .color(MUTED),
                );
                ui.add_space(8.0);
                let mut chosen = None;
                for volume in &self.volumes {
                    let used = volume.total.saturating_sub(volume.free);
                    let response = egui::Frame::new()
                        .fill(CARD)
                        .corner_radius(8)
                        .inner_margin(12)
                        .show(ui, |ui| {
                            ui.set_width((ui.available_width()).max(120.0));
                            ui.horizontal(|ui| {
                                ui.strong(platform::display_path(&volume.path));
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    ui.label(
                                        RichText::new(format!(
                                            "{:.0}%",
                                            percentage(used, volume.total)
                                        ))
                                        .small()
                                        .color(MUTED),
                                    );
                                });
                            });
                            ui.add(
                                egui::ProgressBar::new(used as f32 / volume.total.max(1) as f32)
                                    .fill(if volume.free < volume.total / 10 {
                                        COLORS[3]
                                    } else {
                                        ACCENT
                                    })
                                    .desired_height(4.0),
                            );
                            ui.label(
                                RichText::new(format!("{} livres", human_size(volume.free)))
                                    .size(11.0)
                                    .color(MUTED),
                            );
                        })
                        .response;
                    let click = ui.interact(
                        response.rect,
                        response.id.with("choose-volume"),
                        egui::Sense::click(),
                    );
                    if idle && click.clicked() {
                        chosen = Some(volume.path.clone());
                    }
                    click.on_hover_text("Analisar esta unidade");
                    ui.add_space(8.0);
                }
                if let Some(path) = chosen {
                    self.path = platform::display_path(&path);
                    self.start();
                }
                if ui
                    .add_enabled(
                        idle && self.picker.is_none(),
                        egui::Button::new("Escolher uma pasta…")
                            .min_size(Vec2::new(ui.available_width(), 40.0)),
                    )
                    .clicked()
                {
                    self.choose_folder();
                }
            });
        ui.with_layout(Layout::bottom_up(Align::LEFT), |ui| {
            ui.add_space(8.0);
            ui.label(
                RichText::new("Análise local · Rust · Windows")
                    .size(10.0)
                    .color(MUTED),
            );
            ui.label(
                RichText::new(if self.elevated {
                    "Modo administrador"
                } else {
                    "Modo normal"
                })
                .size(12.0)
                .color(ACCENT),
            );
            if ui.small_button("Permissões e cobertura").clicked() {
                self.show_issues = true;
            }
        });
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        let idle =
            self.events.is_none() && self.operation.is_none() && self.delete_dialog.is_none();
        let (title, subtitle) = match self.browser.mode {
            Mode::Overview => (
                "Visão geral",
                "Percebe o teu disco. Decide o que vale a pena manter.",
            ),
            Mode::Recommendations => (
                "Recomendações",
                "Evidência e contexto antes de remover qualquer ficheiro.",
            ),
            Mode::Tree => (
                "Explorador",
                "Navega pela árvore e vê o detalhe de cada pasta.",
            ),
            Mode::Largest => (
                "Maiores ficheiros",
                "Encontra os ficheiros que concentram mais espaço.",
            ),
            Mode::Extensions => (
                "Tipos de ficheiro",
                "Compara extensões, quantidades e tamanhos.",
            ),
        };
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(RichText::new(title).size(25.0).strong());
                ui.label(RichText::new(subtitle).size(12.0).color(MUTED));
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.add_enabled(self.scan.is_some() && self.exporter.is_none() && idle, egui::Button::new("Exportar CSV")).clicked() { self.export(); }
                ui.menu_button("Opções", |ui| {
                    ui.set_max_width(320.0);
                    ui.label(RichText::new(if self.elevated { "Administrador ativo" } else { "Administrador é opcional" }).strong().color(ACCENT));
                    ui.label("Pode permitir analisar mais pastas. Algumas continuam protegidas pelo Windows.");
                    if !self.elevated && ui.add_enabled(idle, egui::Button::new("Abrir como administrador…")).clicked() { self.elevate(ui.ctx()); ui.close(); }
                    ui.separator();
                    ui.label("DESEMPENHO");
                    ui.add_enabled(idle, egui::Slider::new(&mut self.workers, 1..=32).text("workers"));
                    ui.label(RichText::new("SSD: vários workers · HDD: experimenta 1–2").small().color(MUTED));
                    ui.separator();
                    ui.checkbox(&mut self.compact, "Linhas compactas");
                    ui.checkbox(&mut self.show_details, "Propriedades no explorador");
                    ui.checkbox(&mut self.show_map, "Mapa no explorador");
                    if ui.button("Erros e cobertura").clicked() { self.show_issues = true; ui.close(); }
                });
            });
        });
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            let width = (ui.available_width() - 120.0).max(160.0);
            let input = ui.add_enabled(
                idle,
                egui::TextEdit::singleline(&mut self.path)
                    .desired_width(width)
                    .margin(Vec2::new(12.0, 10.0))
                    .hint_text("Escolhe uma unidade ou escreve o caminho de uma pasta"),
            );
            let enter = input.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if self.events.is_some() {
                if ui
                    .add_sized([104.0, 36.0], egui::Button::new("Parar análise"))
                    .clicked()
                    && let Some(cancel) = &self.cancel
                {
                    cancel.store(true, Ordering::Relaxed);
                }
            } else if self.operation.is_some() {
                ui.add_enabled(false, egui::Button::new("Aguarda…"));
            } else if ui
                .add_sized(
                    [104.0, 36.0],
                    egui::Button::new(
                        RichText::new(if self.scan.is_some() {
                            "Reanalisar"
                        } else {
                            "Analisar"
                        })
                        .strong()
                        .color(BG),
                    )
                    .fill(ACCENT),
                )
                .clicked()
                || enter
            {
                self.start();
            }
        });
    }

    fn view_toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if self.browser.mode == Mode::Tree {
                if ui
                    .add_enabled(self.history_at > 0, egui::Button::new("Voltar"))
                    .clicked()
                {
                    self.history_at -= 1;
                    self.focus(self.history[self.history_at], false);
                }
                if ui
                    .add_enabled(
                        self.history_at + 1 < self.history.len(),
                        egui::Button::new("Avançar"),
                    )
                    .clicked()
                {
                    self.history_at += 1;
                    self.focus(self.history[self.history_at], false);
                }
                if ui.button("Raiz").clicked() {
                    self.focus(0, true);
                }
                if ui
                    .add_enabled(
                        self.scan
                            .as_ref()
                            .is_some_and(|s| s.nodes[self.selected].directory),
                        egui::Button::new("Focar seleção"),
                    )
                    .clicked()
                {
                    self.focus(self.selected, true);
                }
            } else {
                ui.label(
                    RichText::new(if self.browser.mode == Mode::Largest {
                        "1 000 maiores ficheiros correspondentes"
                    } else {
                        "Extensões · totais de toda a análise"
                    })
                    .strong(),
                );
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let search = ui.add(
                    egui::TextEdit::singleline(&mut self.browser.query)
                        .id(egui::Id::new("search"))
                        .hint_text(if self.browser.mode == Mode::Extensions {
                            "Filtrar extensão..."
                        } else {
                            "Procurar em todos os níveis..."
                        })
                        .desired_width(220.0),
                );
                if ui.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::F)) {
                    search.request_focus();
                }
                if search.changed() {
                    self.refresh();
                }
                if !self.browser.query.is_empty() && ui.small_button("Limpar").clicked() {
                    self.browser.query.clear();
                    self.refresh();
                }
            });
        });
        ui.horizontal(|ui| {
            let mut changed = false;
            if self.browser.mode == Mode::Tree {
                for level in 1..=3 {
                    if ui.small_button(format!("Nível {level}")).clicked()
                        && let Some(scan) = &self.scan
                    {
                        self.browser.expand_to(scan, level);
                        changed = true;
                    }
                }
                changed |= ui
                    .checkbox(&mut self.browser.folders_only, "Só pastas")
                    .changed();
            }
            if self.browser.mode != Mode::Extensions {
                ui.menu_button("Colunas", |ui| {
                    ui.label(
                        "Nome, tamanho, %, ficheiros, pastas e alteração estão sempre visíveis.",
                    );
                    ui.checkbox(&mut self.columns.accessed, "Último acesso");
                    ui.checkbox(&mut self.columns.created, "Data de criação");
                    ui.checkbox(&mut self.columns.attributes, "Atributos");
                    ui.checkbox(&mut self.columns.path, "Caminho completo");
                });
            }
            egui::ComboBox::from_id_salt("units")
                .selected_text(self.unit.label())
                .width(80.0)
                .show_ui(ui, |ui| {
                    for unit in Unit::ALL {
                        ui.selectable_value(&mut self.unit, unit, unit.label());
                    }
                });
            ui.checkbox(&mut self.show_map, "Mapa");
            ui.checkbox(&mut self.show_details, "Propriedades");
            if let Some(ext) = self.browser.extension.clone() {
                ui.label(RichText::new(format!("Extensão: {ext}")).color(ACCENT));
                if ui.small_button("Remover filtro").clicked() {
                    self.browser.extension = None;
                    self.refresh();
                }
            }
            if changed {
                self.refresh();
            }
        });
        if self.browser.mode == Mode::Tree
            && let Some(scan) = &self.scan
        {
            ui.add(
                egui::Label::new(
                    RichText::new(platform::display_path(&scan.path(self.browser.scope)))
                        .small()
                        .color(MUTED),
                )
                .truncate(),
            );
        }
        ui.separator();
    }
}

impl eframe::App for SpaceFinder {
    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = root.ctx().clone();
        self.poll(&ctx);
        if let Some(path) = self.initial.take() {
            self.path = platform::display_path(&path);
            self.start();
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape))
            && let Some(cancel) = &self.cancel
        {
            cancel.store(true, Ordering::Relaxed);
        }
        if ctx.input(|i| i.key_pressed(egui::Key::F5)) && self.events.is_none() {
            self.start();
        }
        egui::Panel::left("navigation")
            .exact_size(224.0)
            .resizable(false)
            .frame(egui::Frame::new().fill(PANEL).inner_margin(16))
            .show(root, |ui| self.sidebar(ui));
        egui::Panel::top("toolbar")
            .frame(egui::Frame::new().fill(BG).inner_margin(20))
            .show(root, |ui| self.toolbar(ui));
        egui::Panel::bottom("status")
            .frame(egui::Frame::new().fill(PANEL).inner_margin(8))
            .show(root, |ui| {
                ui.horizontal(|ui| {
                    let text = if let Some(operation) = &self.operation {
                        operation.label.clone()
                    } else if self.events.is_some() {
                        self.progress.current.clone()
                    } else if self.analysis.is_some() {
                        "A preparar recomendações a partir dos metadados…".into()
                    } else {
                        format!(
                            "{} ficheiros · {} pastas · {:.2}s · dados locais",
                            count(self.progress.files),
                            count(self.progress.folders),
                            self.progress.seconds
                        )
                    };
                    ui.add_sized(
                        [(ui.available_width() - 300.0).max(80.0), 18.0],
                        egui::Label::new(RichText::new(text).small().color(MUTED)).truncate(),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .small_button(format!(
                                "{} sem acesso · {} outros · {} ignorados",
                                self.progress.denied,
                                self.progress.errors.saturating_sub(self.progress.denied),
                                self.progress.skipped
                            ))
                            .clicked()
                        {
                            self.show_issues = true;
                        }
                    });
                });
            });
        if self.scan.is_some() && self.browser.mode == Mode::Recommendations {
            egui::Panel::right("recommendation-details")
                .default_size(336.0)
                .min_size(280.0)
                .max_size(440.0)
                .resizable(true)
                .frame(egui::Frame::new().fill(PANEL).inner_margin(20))
                .show(root, |ui| self.recommendation_details(ui));
        } else if self.scan.is_some()
            && self.show_details
            && matches!(
                self.browser.mode,
                Mode::Tree | Mode::Largest | Mode::Extensions
            )
        {
            egui::Panel::right("properties")
                .default_size(272.0)
                .min_size(232.0)
                .max_size(420.0)
                .resizable(true)
                .frame(egui::Frame::new().fill(PANEL).inner_margin(16))
                .show(root, |ui| self.properties(ui));
        }
        if self.scan.is_some()
            && self.show_map
            && matches!(self.browser.mode, Mode::Tree | Mode::Largest)
        {
            egui::Panel::bottom("map")
                .default_size(196.0)
                .min_size(120.0)
                .max_size(360.0)
                .resizable(true)
                .frame(egui::Frame::new().fill(BG).inner_margin(12))
                .show(root, |ui| self.map(ui));
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(BG).inner_margin(20))
            .show(root, |ui| {
                if let Some(notice) = self.notice.clone() {
                    ui.horizontal(|ui| {
                        ui.add_sized(
                            [(ui.available_width() - 72.0).max(80.0), 20.0],
                            egui::Label::new(RichText::new(notice).color(COLORS[3])).wrap(),
                        );
                        if ui.small_button("Fechar").clicked() {
                            self.notice = None;
                        }
                    });
                }
                self.permission_banner(ui);
                if self.scan.is_some() && self.browser.mode == Mode::Overview {
                    self.overview(ui);
                } else if self.scan.is_some() && self.browser.mode == Mode::Recommendations {
                    self.recommendations_page(ui);
                } else if self.scan.is_some() {
                    self.view_toolbar(ui);
                    if self.browser.mode == Mode::Extensions {
                        self.extensions_table(ui);
                    } else {
                        self.files_table(ui);
                    }
                } else {
                    self.welcome(ui);
                }
            });
        self.coverage(&ctx);
        self.confirmation(&ctx);
        // Signal only on a later frame, so a failed GUI initialization never
        // dismisses the original window. Scanning need not finish for handover.
        if let Some((_, first_frame)) = &self.elevation_ready {
            if ctx.cumulative_frame_nr() > *first_frame {
                let (name, _) = self.elevation_ready.take().unwrap();
                if let Err(error) = spacefinder::actions::acknowledge_elevation_ready(&name) {
                    self.notice = Some(format!(
                        "Não foi possível confirmar a abertura à janela anterior: {error}"
                    ));
                }
            }
            ctx.request_repaint();
        }
    }
}

impl Drop for SpaceFinder {
    fn drop(&mut self) {
        if let Some(cancel) = &self.cancel {
            cancel.store(true, Ordering::Relaxed);
        }
    }
}
