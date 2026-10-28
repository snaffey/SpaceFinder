use super::*;
use chrono::TimeZone;
use egui::{FontId, Pos2, Rect, Sense};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    Auto,
    Bytes,
    KiB,
    MiB,
    GiB,
}
impl Unit {
    pub const ALL: [Self; 5] = [Self::Auto, Self::Bytes, Self::KiB, Self::MiB, Self::GiB];
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "Auto",
            Self::Bytes => "Bytes",
            Self::KiB => "KiB",
            Self::MiB => "MiB",
            Self::GiB => "GiB",
        }
    }
    pub fn format(self, bytes: u64) -> String {
        match self {
            Self::Auto => human_size(bytes),
            Self::Bytes => count(bytes),
            Self::KiB => format!("{:.2} KiB", bytes as f64 / 1024.0),
            Self::MiB => format!("{:.2} MiB", bytes as f64 / 1_048_576.0),
            Self::GiB => format!("{:.2} GiB", bytes as f64 / 1_073_741_824.0),
        }
    }
}

pub fn count(value: u64) -> String {
    let text = value.to_string();
    let mut result = String::new();
    for (i, c) in text.chars().enumerate() {
        if i > 0 && (text.len() - i).is_multiple_of(3) {
            result.push(' ');
        }
        result.push(c);
    }
    result
}

pub fn date(seconds: Option<i64>) -> String {
    seconds
        .and_then(|s| chrono::Local.timestamp_opt(s, 0).single())
        .map(|d| d.format("%d/%m/%Y %H:%M").to_string())
        .unwrap_or_else(|| "—".into())
}

pub fn percentage(bytes: u64, total: u64) -> f32 {
    if total == 0 {
        0.0
    } else {
        (bytes as f64 / total as f64 * 100.0) as f32
    }
}

pub fn bar(ui: &mut egui::Ui, value: f32, color: Color32) {
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 22.0), Sense::hover());
    let width = (rect.width() - 48.0).max(0.0);
    let base = Rect::from_min_size(rect.min + Vec2::new(0.0, 8.0), Vec2::new(width, 5.0));
    ui.painter()
        .rect_filled(base, 3.0, LINE.gamma_multiply(0.65));
    let filled = if value > 0.0 {
        (width * (value / 100.0).clamp(0.0, 1.0))
            .max(1.0)
            .min(width)
    } else {
        0.0
    };
    ui.painter().rect_filled(
        Rect::from_min_size(base.min, Vec2::new(filled, 5.0)),
        3.0,
        color,
    );
    ui.painter().text(
        rect.right_center(),
        egui::Align2::RIGHT_CENTER,
        if value > 0.0 && value < 0.1 {
            "<0.1%".into()
        } else {
            format!("{value:.1}%")
        },
        FontId::proportional(12.0),
        TEXT,
    );
    response.on_hover_text(format!("{value:.3}%"));
}

pub fn brand(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::hover());
    ui.painter().rect_filled(rect, 7.0, ACCENT);
    for (x, y, w, h) in [
        (6.0, 6.0, 7.0, 16.0),
        (16.0, 6.0, 6.0, 7.0),
        (16.0, 16.0, 6.0, 6.0),
    ] {
        ui.painter().rect_filled(
            Rect::from_min_size(rect.min + Vec2::new(x, y), Vec2::new(w, h)),
            1.0,
            BG,
        );
    }
    ui.label(RichText::new("SpaceFinder").size(18.0).strong());
}

pub fn file_icon(ui: &mut egui::Ui, directory: bool, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(16.0, 18.0), Sense::hover());
    if directory {
        ui.painter().rect_filled(
            Rect::from_min_size(rect.min + Vec2::new(0.0, 2.0), Vec2::new(7.0, 5.0)),
            2.0,
            color,
        );
        ui.painter().rect_filled(
            Rect::from_min_size(rect.min + Vec2::new(0.0, 5.0), Vec2::new(16.0, 11.0)),
            2.0,
            color,
        );
    } else {
        ui.painter().rect_stroke(
            Rect::from_min_size(rect.min + Vec2::new(1.0, 1.0), Vec2::new(12.0, 16.0)),
            2.0,
            Stroke::new(1.0, color),
            egui::StrokeKind::Inside,
        );
        ui.painter().line_segment(
            [
                rect.min + Vec2::new(4.0, 7.0),
                rect.min + Vec2::new(10.0, 7.0),
            ],
            Stroke::new(1.0, color),
        );
    }
}

pub fn disclosure(ui: &mut egui::Ui, expanded: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(18.0), Sense::click());
    let c = rect.center();
    let points = if expanded {
        vec![
            c + Vec2::new(-4.0, -2.0),
            c + Vec2::new(4.0, -2.0),
            c + Vec2::new(0.0, 3.0),
        ]
    } else {
        vec![
            c + Vec2::new(-2.0, -4.0),
            c + Vec2::new(3.0, 0.0),
            c + Vec2::new(-2.0, 4.0),
        ]
    };
    ui.painter().add(egui::Shape::convex_polygon(
        points,
        if response.hovered() { ACCENT } else { MUTED },
        Stroke::NONE,
    ));
    response
}

pub fn number(ui: &mut egui::Ui, text: impl Into<String>) {
    let text = text.into();
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        ui.add(egui::Label::new(RichText::new(&text).monospace().size(12.5)).truncate())
            .on_hover_text(text);
    });
}

pub fn property(ui: &mut egui::Ui, label: &str, value: impl Into<String>) {
    ui.label(RichText::new(label).size(11.0).color(MUTED));
    ui.add(egui::Label::new(RichText::new(value.into()).size(13.0)).wrap());
    ui.add_space(6.0);
}

pub fn treemap(nodes: &[(Option<usize>, u64)], rect: Rect) -> Vec<(Option<usize>, u64, Rect)> {
    fn split(
        nodes: &[(Option<usize>, u64)],
        rect: Rect,
        output: &mut Vec<(Option<usize>, u64, Rect)>,
    ) {
        if nodes.len() == 1 {
            output.push((nodes[0].0, nodes[0].1, rect));
            return;
        }
        if nodes.is_empty() {
            return;
        }
        let total: u64 = nodes.iter().map(|n| n.1).sum();
        let mut sum = 0;
        let mut pivot = 1;
        for (i, node) in nodes.iter().enumerate().take(nodes.len() - 1) {
            sum += node.1;
            pivot = i + 1;
            if sum >= total / 2 {
                break;
            }
        }
        let fraction = sum as f32 / total.max(1) as f32;
        let (a, b) = if rect.width() >= rect.height() {
            let x = rect.min.x + rect.width() * fraction;
            (
                Rect::from_min_max(rect.min, Pos2::new(x, rect.max.y)),
                Rect::from_min_max(Pos2::new(x, rect.min.y), rect.max),
            )
        } else {
            let y = rect.min.y + rect.height() * fraction;
            (
                Rect::from_min_max(rect.min, Pos2::new(rect.max.x, y)),
                Rect::from_min_max(Pos2::new(rect.min.x, y), rect.max),
            )
        };
        split(&nodes[..pivot], a, output);
        split(&nodes[pivot..], b, output);
    }
    let mut output = Vec::new();
    split(nodes, rect, &mut output);
    output
}
