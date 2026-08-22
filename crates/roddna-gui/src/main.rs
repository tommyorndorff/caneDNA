//! RodDNA GUI — browse and plot bamboo rod tapers.
//!
//! v0: searchable/filterable list of the ~619 tapers from the RodDNA library,
//! with a taper-profile plot for the selected rod. Foundation for the taper
//! explorer (travel rods, spey rods, etc.).

use eframe::egui;
use egui_plot::{Legend, Line, Plot, PlotPoints};
use roddna_core::{Library, Taper};

// Bundle the data into the binary so the app is a single self-contained file.
const TAPERS_JSON: &str = include_str!("../../../data/tapers.json");

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 720.0])
            .with_title("RodDNA — Taper Explorer"),
        ..Default::default()
    };
    eframe::run_native(
        "roddna",
        native_options,
        Box::new(|_cc| Ok(Box::new(App::new()))),
    )
}

struct App {
    lib: Library,
    search: String,
    /// Filter by rod type; empty string == "All".
    type_filter: String,
    line_weight_filter: Option<f64>,
    pieces_filter: Option<f64>,
    selected: Option<usize>,
    rod_types: Vec<String>,
    line_weights: Vec<f64>,
    piece_counts: Vec<f64>,
}

impl App {
    fn new() -> Self {
        let lib = Library::from_json(TAPERS_JSON).expect("bundled tapers.json is valid");
        let rod_types = lib.rod_types();
        let line_weights = distinct(lib.models.iter().filter_map(|m| m.line_weight));
        let piece_counts = distinct(lib.models.iter().filter_map(|m| m.pieces));
        Self {
            lib,
            search: String::new(),
            type_filter: String::new(),
            line_weight_filter: None,
            pieces_filter: None,
            selected: None,
            rod_types,
            line_weights,
            piece_counts,
        }
    }

    fn matches(&self, t: &Taper) -> bool {
        if !self.search.is_empty() {
            let q = self.search.to_lowercase();
            let name = t.name.as_deref().unwrap_or("").to_lowercase();
            if !name.contains(&q) {
                return false;
            }
        }
        if !self.type_filter.is_empty() && t.rod_type.as_deref() != Some(&self.type_filter) {
            return false;
        }
        if let Some(lw) = self.line_weight_filter {
            if t.line_weight != Some(lw) {
                return false;
            }
        }
        if let Some(p) = self.pieces_filter {
            if t.pieces != Some(p) {
                return false;
            }
        }
        true
    }
}

/// Distinct, sorted values from an iterator of f64.
fn distinct(iter: impl Iterator<Item = f64>) -> Vec<f64> {
    let mut v: Vec<f64> = iter.collect();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v.dedup();
    v
}

fn fmt_len(inches: Option<f64>) -> String {
    match inches {
        Some(i) => {
            let ft = (i / 12.0).floor() as i64;
            let rem = i - (ft as f64) * 12.0;
            format!("{}' {:.0}\"", ft, rem)
        }
        None => "—".into(),
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Left panel: filters + list.
        egui::SidePanel::left("list")
            .resizable(true)
            .default_width(360.0)
            .show(ctx, |ui| {
                ui.add_space(4.0);
                ui.heading("Tapers");
                ui.horizontal(|ui| {
                    ui.label("Search:");
                    ui.text_edit_singleline(&mut self.search);
                });

                egui::ComboBox::from_label("Type")
                    .selected_text(if self.type_filter.is_empty() {
                        "All".to_string()
                    } else {
                        self.type_filter.clone()
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.type_filter, String::new(), "All");
                        for t in &self.rod_types {
                            ui.selectable_value(&mut self.type_filter, t.clone(), t);
                        }
                    });

                ui.horizontal(|ui| {
                    combo_opt(ui, "Line wt", &mut self.line_weight_filter, &self.line_weights);
                    combo_opt(ui, "Pieces", &mut self.pieces_filter, &self.piece_counts);
                });

                ui.separator();

                let indices: Vec<usize> = self
                    .lib
                    .models
                    .iter()
                    .enumerate()
                    .filter(|(_, t)| self.matches(t))
                    .map(|(i, _)| i)
                    .collect();

                ui.label(format!("{} of {} rods", indices.len(), self.lib.models.len()));

                egui::ScrollArea::vertical().show(ui, |ui| {
                    for i in indices {
                        let name = self.lib.models[i]
                            .name
                            .clone()
                            .unwrap_or_else(|| format!("model {i}"));
                        let selected = self.selected == Some(i);
                        if ui.selectable_label(selected, name).clicked() {
                            self.selected = Some(i);
                        }
                    }
                });
            });

        // Central panel: details + taper plot.
        egui::CentralPanel::default().show(ctx, |ui| match self.selected {
            None => {
                ui.centered_and_justified(|ui| {
                    ui.label("Select a rod to view its taper.");
                });
            }
            Some(i) => {
                let t = &self.lib.models[i];
                ui.heading(t.name.as_deref().unwrap_or("(unnamed)"));
                ui.horizontal_wrapped(|ui| {
                    chip(ui, "Type", t.rod_type.as_deref().unwrap_or("—"));
                    chip(ui, "Const", t.const_type.as_deref().unwrap_or("—"));
                    chip(ui, "Length", &fmt_len(t.length));
                    chip(ui, "Line wt", &opt_num(t.line_weight));
                    chip(ui, "Pieces", &opt_num(t.pieces));
                    chip(ui, "Points", &t.point_count().to_string());
                });
                ui.separator();

                let points: PlotPoints = t.profile().into_iter().collect();
                Plot::new("taper")
                    .legend(Legend::default())
                    .x_axis_label("Station (in from tip)")
                    .y_axis_label("Flat-to-flat (in)")
                    .height(ui.available_height() * 0.62)
                    .show(ui, |plot_ui| {
                        plot_ui.line(Line::new(points).name("taper"));
                    });

                if let Some(notes) = t.notes.as_deref() {
                    if !notes.is_empty() {
                        ui.separator();
                        ui.label(egui::RichText::new("Notes").strong());
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            ui.label(notes);
                        });
                    }
                }
            }
        });
    }
}

fn opt_num(v: Option<f64>) -> String {
    match v {
        Some(x) if x.fract() == 0.0 => format!("{}", x as i64),
        Some(x) => format!("{x}"),
        None => "—".into(),
    }
}

fn chip(ui: &mut egui::Ui, k: &str, v: &str) {
    ui.label(egui::RichText::new(format!("{k}: ")).weak());
    ui.label(egui::RichText::new(v).strong());
    ui.add_space(8.0);
}

/// A combo box over an Option<f64> filter with an "Any" entry.
fn combo_opt(ui: &mut egui::Ui, label: &str, sel: &mut Option<f64>, values: &[f64]) {
    let text = match sel {
        Some(v) => opt_num(Some(*v)),
        None => "Any".to_string(),
    };
    egui::ComboBox::from_label(label)
        .selected_text(text)
        .show_ui(ui, |ui| {
            ui.selectable_value(sel, None, "Any");
            for &v in values {
                ui.selectable_value(sel, Some(v), opt_num(Some(v)));
            }
        });
}
