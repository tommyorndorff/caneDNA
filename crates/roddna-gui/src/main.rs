//! RodDNA GUI — browse and compare bamboo rod tapers.
//!
//! Multi-select the ~619 tapers from the library, overlay their profiles on a
//! single plot to compare patterns, and inspect specs. Foundation for the taper
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
            .with_title("caneDNA — Taper Explorer"),
        ..Default::default()
    };
    eframe::run_native(
        "canedna",
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
    /// Selected rods, in click order (drives legend/color stability).
    selected: Vec<usize>,
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
            selected: Vec::new(),
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

    /// Toggle a rod in/out of the selection, preserving click order.
    fn toggle(&mut self, i: usize) {
        if let Some(pos) = self.selected.iter().position(|&x| x == i) {
            self.selected.remove(pos);
        } else {
            self.selected.push(i);
        }
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
                ui.horizontal(|ui| {
                    ui.heading("Tapers");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Clear").clicked() {
                            self.selected.clear();
                        }
                        ui.label(format!("{} selected", self.selected.len()));
                    });
                });
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
                        let selected = self.selected.contains(&i);
                        if ui.selectable_label(selected, name).clicked() {
                            self.toggle(i);
                        }
                    }
                });
            });

        // Central panel: comparison table + overlaid taper plot.
        egui::CentralPanel::default().show(ctx, |ui| {
            if self.selected.is_empty() {
                ui.centered_and_justified(|ui| {
                    ui.label("Select one or more rods to overlay their tapers.");
                });
                return;
            }

            ui.heading(if self.selected.len() == 1 {
                "Taper".to_string()
            } else {
                format!("Comparing {} tapers", self.selected.len())
            });

            // Compact spec table for the current selection.
            egui::Grid::new("specs")
                .striped(true)
                .num_columns(6)
                .show(ui, |ui| {
                    for h in ["Rod", "Type", "Const", "Length", "Line", "Pieces"] {
                        ui.label(egui::RichText::new(h).strong());
                    }
                    ui.end_row();
                    for &i in &self.selected {
                        let t = &self.lib.models[i];
                        ui.label(t.name.as_deref().unwrap_or("(unnamed)"));
                        ui.label(t.rod_type.as_deref().unwrap_or("—"));
                        ui.label(t.const_type.as_deref().unwrap_or("—"));
                        ui.label(fmt_len(t.length));
                        ui.label(opt_num(t.line_weight));
                        ui.label(opt_num(t.pieces));
                        ui.end_row();
                    }
                });

            ui.separator();

            // Overlaid taper profiles. egui_plot auto-assigns a stable color per
            // named line, and the legend lets you toggle individual rods.
            Plot::new("taper")
                .legend(Legend::default())
                .x_axis_label("Station (in from tip)")
                .y_axis_label("Flat-to-flat (in)")
                .height(ui.available_height() * 0.7)
                .show(ui, |plot_ui| {
                    for &i in &self.selected {
                        let t = &self.lib.models[i];
                        let name = t.name.clone().unwrap_or_else(|| format!("model {i}"));
                        let points: PlotPoints = t.profile().into_iter().collect();
                        plot_ui.line(Line::new(points).name(name));
                    }
                });

            // Notes only make sense for a single rod.
            if self.selected.len() == 1 {
                if let Some(notes) = self.lib.models[self.selected[0]].notes.as_deref() {
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
