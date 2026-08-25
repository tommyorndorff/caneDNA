//! RodDNA GUI — browse and compare bamboo rod tapers.
//!
//! Multi-select the ~619 tapers from the library, overlay their profiles on a
//! single plot to compare patterns, and inspect specs. Foundation for the taper
//! explorer (travel rods, spey rods, etc.).

use eframe::egui;
use egui_plot::{Bar, BarChart, Legend, Line, Plot, PlotPoint, PlotPoints, Text, VLine};
use roddna_core::{CastingKb, Library, Taper};

// Bundle the data into the binary so the app is a single self-contained file.
const TAPERS_JSON: &str = include_str!("../../../data/tapers.json");
const CASTING_JSON: &str = include_str!("../../../data/kb/casting_kb.json");

/// Native desktop entry point.
#[cfg(not(target_arch = "wasm32"))]
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

/// Web (WASM) entry point — mounts the same App onto a browser canvas.
#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast as _;

    eframe::WebLogger::init(log::LevelFilter::Debug).ok();
    let web_options = eframe::WebOptions::default();

    wasm_bindgen_futures::spawn_local(async {
        let document = web_sys::window()
            .expect("no window")
            .document()
            .expect("no document");
        let canvas = document
            .get_element_by_id("canedna_canvas")
            .expect("missing canvas element")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("element is not a canvas");

        let result = eframe::WebRunner::new()
            .start(
                canvas,
                web_options,
                Box::new(|_cc| Ok(Box::new(App::new()))),
            )
            .await;

        // Remove the loading text once running (or show an error).
        if let Some(el) = document.get_element_by_id("loading_text") {
            match result {
                Ok(_) => el.remove(),
                Err(e) => {
                    el.set_inner_html(&format!("<p style='color:#b00'>Failed to start: {e:?}</p>"))
                }
            }
        }
    });
}

/// Which central-panel view is active for the current selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PanelView {
    Chart,
    StationData,
    MillSettings,
    DeltaChart,
    Stress,
    PlaningForm,
}

/// Which view is active while editing a taper in design mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DesignView {
    Editor,
    Profile,
    Stress,
    DeltaChart,
    MillSettings,
}

/// An in-memory, unsaved taper edit session: a seed taper (cloned so the
/// original library record is untouched) plus the scale/insert-station
/// controls used to reshape it. Nothing here persists past the app session —
/// export (a later roadmap item) is how a design leaves the app.
struct DesignState {
    taper: Taper,
    /// Name of the library rod this design started from, for display only.
    seed_name: String,
    view: DesignView,
    scale_multiplier: f64,
    scale_bias: f64,
    /// Pending station value for the "insert station" control.
    new_station: f64,
}

impl DesignState {
    fn new(seed: &Taper) -> Self {
        Self {
            taper: seed.clone(),
            seed_name: seed.name.clone().unwrap_or_else(|| "(unnamed)".to_string()),
            view: DesignView::Editor,
            scale_multiplier: 1.0,
            scale_bias: 0.0,
            new_station: 0.0,
        }
    }
}

struct App {
    lib: Library,
    kb: CastingKb,
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
    view: PanelView,
    rough_oversize: f64,
    finish_oversize: f64,
    split_by_piece: bool,
    /// Active taper design/edit session, if any. When set, the central panel
    /// shows the design UI instead of the browse/compare view.
    design: Option<DesignState>,
}

impl App {
    fn new() -> Self {
        let lib = Library::from_json(TAPERS_JSON).expect("bundled tapers.json is valid");
        let kb = CastingKb::from_json(CASTING_JSON).expect("bundled casting_kb.json is valid");
        let rod_types = lib.rod_types();
        let line_weights = distinct(lib.models.iter().filter_map(|m| m.line_weight));
        let piece_counts = distinct(lib.models.iter().filter_map(|m| m.pieces));
        Self {
            lib,
            kb,
            search: String::new(),
            type_filter: String::new(),
            line_weight_filter: None,
            pieces_filter: None,
            selected: Vec::new(),
            rod_types,
            line_weights,
            piece_counts,
            view: PanelView::Chart,
            rough_oversize: 0.07,
            finish_oversize: 0.03,
            split_by_piece: false,
            design: None,
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

    /// Render cited casting feedback for a taper (model-level if available, else
    /// maker-level), with action tags, if the KB has any.
    fn casting_notes(&self, ui: &mut egui::Ui, taper: &Taper) {
        let Some((label, mc)) = self.kb.for_taper(taper) else {
            return;
        };
        ui.separator();
        egui::CollapsingHeader::new(format!(
            "Casting notes — {label} ({} mentions)",
            mc.mentions_with_casting
        ))
        .default_open(true)
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(format!(
                    "How “{label}” is described in the Rodmakers listserv \
                     (1995–2004). Showing {} of {} casting mentions.",
                    mc.snippets_shown, mc.mentions_with_casting
                ))
                .weak()
                .small(),
            );
            // Action summary as chips (most common first).
            if !mc.action_counts.is_empty() {
                let mut actions: Vec<(&String, &u64)> = mc.action_counts.iter().collect();
                actions.sort_by(|a, b| b.1.cmp(a.1));
                ui.horizontal_wrapped(|ui| {
                    ui.label(egui::RichText::new("Action:").small().weak());
                    for (tag, n) in actions {
                        action_chip(ui, &format!("{tag} {n}"));
                    }
                });
            }
            ui.add_space(4.0);
            for s in &mc.snippets {
                ui.label(egui::RichText::new(format!("“{}”", s.quote)).italics());
                ui.horizontal_wrapped(|ui| {
                    let who = s.author.as_deref().unwrap_or("unknown");
                    let year = s.year.map(|y| y.to_string()).unwrap_or_default();
                    ui.label(
                        egui::RichText::new(format!("— {who}, {year}"))
                            .weak()
                            .small(),
                    );
                    for tag in &s.actions {
                        action_chip(ui, tag);
                    }
                });
                ui.add_space(6.0);
            }
        });
    }

    /// Render ferrule size/type/location info for a taper, if it has any
    /// (unused ferrule slots are placeholder `0.0`/`"None"` and are skipped
    /// by `Taper::ferrules()`).
    fn ferrules_section(&self, ui: &mut egui::Ui, taper: &Taper) {
        let ferrules = taper.ferrules();
        if ferrules.is_empty() {
            return;
        }
        ui.separator();
        ui.label(egui::RichText::new("Ferrules").strong());
        for f in ferrules {
            let mut line = format!(
                "Ferrule {}: {}, {} @ {:.2}\" from tip — rod {:.3}\" ({})",
                f.index,
                f.size,
                f.ferrule_type.as_deref().unwrap_or("unknown type"),
                f.location,
                f.dimension_at_location,
                format_64ths(f.dimension_at_location),
            );
            if let Some(od) = f.outside_diameter_apexes {
                line.push_str(&format!(
                    ", OD around apexes {:.3}\" ({})",
                    od,
                    format_64ths(od)
                ));
            }
            ui.label(line);
        }
    }

    /// Toggle a rod in/out of the selection, preserving click order.
    fn toggle(&mut self, i: usize) {
        if let Some(pos) = self.selected.iter().position(|&x| x == i) {
            self.selected.remove(pos);
        } else {
            self.selected.push(i);
        }
    }

    /// Renders the taper design/edit session in place of the browse view.
    fn design_panel(&mut self, ui: &mut egui::Ui) {
        let mut discard = false;
        if let Some(design) = self.design.as_mut() {
            ui.horizontal(|ui| {
                ui.heading(format!("Designing (seed: {})", design.seed_name));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Discard design").clicked() {
                        discard = true;
                    }
                });
            });
            ui.horizontal(|ui| {
                ui.label("Name:");
                ui.text_edit_singleline(design.taper.name.get_or_insert_with(String::new));
            });
            ui.separator();

            ui.horizontal(|ui| {
                ui.selectable_value(&mut design.view, DesignView::Editor, "Station Editor");
                ui.selectable_value(&mut design.view, DesignView::Profile, "Profile");
                ui.selectable_value(&mut design.view, DesignView::Stress, "Stress");
                ui.selectable_value(&mut design.view, DesignView::DeltaChart, "Dimension Changes");
                ui.selectable_value(&mut design.view, DesignView::MillSettings, "Mill Settings");
            });
            ui.add_space(4.0);

            match design.view {
                DesignView::Editor => design_editor(ui, design),
                DesignView::Profile => {
                    let points: PlotPoints = design.taper.profile().into_iter().collect();
                    Plot::new("design_profile")
                        .x_axis_label("Station (in from tip)")
                        .y_axis_label("Flat-to-flat (in)")
                        .height(ui.available_height() * 0.7)
                        .show(ui, |plot_ui| {
                            plot_ui.line(Line::new(points).name("Design"));
                        });
                }
                DesignView::Stress => {
                    let curve = design.taper.stress_curve();
                    if curve.is_empty() {
                        ui.label(
                            egui::RichText::new(
                                "No stress curve — this design is missing a required input \
                                 (line length/cast, impact factor, bamboo density, tip weight).",
                            )
                            .weak(),
                        );
                        ui.add_space(4.0);
                    }
                    let points: PlotPoints = curve.into_iter().collect();
                    Plot::new("design_stress")
                        .x_axis_label("Station (in from tip)")
                        .y_axis_label("Stress (psi)")
                        .height(ui.available_height() * 0.7)
                        .show(ui, |plot_ui| {
                            plot_ui.line(Line::new(points).name("Design"));
                        });
                }
                DesignView::DeltaChart => {
                    let deltas = design.taper.dimension_deltas();
                    let bars: Vec<Bar> = deltas
                        .iter()
                        .map(|d| Bar::new(d.station, d.delta))
                        .collect();
                    let line_points: PlotPoints =
                        deltas.iter().map(|d| [d.station, d.delta]).collect();
                    let ferrule_locations: Vec<f64> =
                        design.taper.ferrules().iter().map(|f| f.location).collect();
                    Plot::new("design_delta")
                        .x_axis_label("Station (in from tip)")
                        .y_axis_label("Dimension change (in)")
                        .height(ui.available_height() * 0.7)
                        .show(ui, |plot_ui| {
                            plot_ui.bar_chart(BarChart::new(bars).name("Δ dimension").width(4.0));
                            plot_ui.line(Line::new(line_points).name("Δ dimension"));
                            for loc in ferrule_locations {
                                plot_ui.vline(
                                    VLine::new(loc)
                                        .name("Ferrule")
                                        .color(egui::Color32::from_rgb(200, 80, 80)),
                                );
                            }
                        });
                }
                DesignView::MillSettings => {
                    egui::ScrollArea::vertical()
                        .id_salt("design_mill_settings_scroll")
                        .max_height(ui.available_height() * 0.7)
                        .show(ui, |ui| {
                            mill_settings_grid(
                                ui,
                                "design_mill_settings",
                                &design
                                    .taper
                                    .mill_settings(self.rough_oversize, self.finish_oversize),
                            );
                        });
                }
            }
        }
        if discard {
            self.design = None;
        }
    }
}

/// The Station Editor tab body: scale/insert-station controls, ferrule
/// slots, and an editable station/dimension grid.
fn design_editor(ui: &mut egui::Ui, design: &mut DesignState) {
    ui.horizontal(|ui| {
        ui.label("Scale multiplier:");
        ui.add(
            egui::DragValue::new(&mut design.scale_multiplier)
                .speed(0.01)
                .range(0.1..=5.0),
        );
        ui.label("bias:");
        ui.add(
            egui::DragValue::new(&mut design.scale_bias)
                .speed(0.001)
                .fixed_decimals(4),
        );
        if ui.button("Apply scale").clicked() {
            design.taper = design.taper.scaled(design.scale_multiplier, design.scale_bias);
            design.scale_multiplier = 1.0;
            design.scale_bias = 0.0;
        }
    });
    ui.horizontal(|ui| {
        ui.label("Insert station at:");
        ui.add(
            egui::DragValue::new(&mut design.new_station)
                .speed(0.5)
                .fixed_decimals(2),
        );
        if ui.button("Insert").clicked() {
            design.taper.insert_station(design.new_station);
        }
    });
    ui.horizontal(|ui| {
        ui.label("Pieces:");
        let mut pieces = design.taper.pieces.unwrap_or(1.0);
        if ui
            .add(egui::DragValue::new(&mut pieces).range(1.0..=6.0))
            .changed()
        {
            design.taper.pieces = Some(pieces);
        }
    });

    ui.add_space(4.0);
    ui.label(egui::RichText::new("Ferrules (location 0 = none)").strong());
    ferrule_row(ui, &mut design.taper, 1);
    ferrule_row(ui, &mut design.taper, 2);
    ferrule_row(ui, &mut design.taper, 3);

    ui.add_space(8.0);
    ui.label(egui::RichText::new("Stations").strong());
    egui::ScrollArea::vertical()
        .id_salt("design_editor_scroll")
        .max_height(ui.available_height())
        .show(ui, |ui| {
            egui::Grid::new("design_editor_grid")
                .striped(true)
                .num_columns(2)
                .show(ui, |ui| {
                    for h in ["Station (in)", "Dimension (in)"] {
                        ui.label(egui::RichText::new(h).strong());
                    }
                    ui.end_row();
                    for (station, dimension) in design
                        .taper
                        .stations
                        .iter_mut()
                        .zip(design.taper.dimensions.iter_mut())
                    {
                        ui.add(egui::DragValue::new(station).speed(0.5).fixed_decimals(2));
                        ui.add(
                            egui::DragValue::new(dimension)
                                .speed(0.001)
                                .fixed_decimals(4),
                        );
                        ui.end_row();
                    }
                });
        });
}

/// One editable ferrule slot: location, size, and a button to carve out an
/// explicit profile point there via `Taper::insert_station`. Reads/writes the
/// taper's fields by value around the UI closure (rather than holding field
/// borrows) so `insert_station`'s `&mut self` can still be called at the end.
fn ferrule_row(ui: &mut egui::Ui, taper: &mut Taper, index: usize) {
    let (mut loc_val, mut size_val) = match index {
        1 => (
            taper.ferrule1_loc.unwrap_or(0.0),
            taper.ferrule1_size.clone().unwrap_or_default(),
        ),
        2 => (
            taper.ferrule2_loc.unwrap_or(0.0),
            taper.ferrule2_size.clone().unwrap_or_default(),
        ),
        _ => (
            taper.ferrule3_loc.unwrap_or(0.0),
            taper.ferrule3_size.clone().unwrap_or_default(),
        ),
    };

    let (mut loc_changed, mut size_changed, mut insert_clicked) = (false, false, false);
    ui.horizontal(|ui| {
        ui.label(format!("Ferrule {index}:"));
        loc_changed = ui
            .add(
                egui::DragValue::new(&mut loc_val)
                    .speed(0.5)
                    .fixed_decimals(2),
            )
            .changed();
        size_changed = ui.text_edit_singleline(&mut size_val).changed();
        insert_clicked = loc_val != 0.0 && ui.button("Insert station here").clicked();
    });

    if loc_changed {
        match index {
            1 => taper.ferrule1_loc = Some(loc_val),
            2 => taper.ferrule2_loc = Some(loc_val),
            _ => taper.ferrule3_loc = Some(loc_val),
        }
    }
    if size_changed {
        match index {
            1 => taper.ferrule1_size = Some(size_val),
            2 => taper.ferrule2_size = Some(size_val),
            _ => taper.ferrule3_size = Some(size_val),
        }
    }
    if insert_clicked {
        taper.insert_station(loc_val);
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

/// Format inches as a 64ths-of-an-inch fraction, e.g. 0.208 -> "13.3/64".
fn format_64ths(inches: f64) -> String {
    format!("{:.1}/64", inches * 64.0)
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
                ui.hyperlink_to(
                    format!("v{}", env!("CARGO_PKG_VERSION")),
                    "https://github.com/tommyorndorff/caneDNA/blob/main/CHANGELOG.md",
                );
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
                    combo_opt(
                        ui,
                        "Line wt",
                        &mut self.line_weight_filter,
                        &self.line_weights,
                    );
                    combo_opt(ui, "Pieces", &mut self.pieces_filter, &self.piece_counts);
                });

                ui.separator();

                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(
                            self.selected.len() == 1,
                            egui::Button::new("New design from selection"),
                        )
                        .clicked()
                    {
                        self.design = Some(DesignState::new(&self.lib.models[self.selected[0]]));
                    }
                    // Seed preset for the decided first spey design target
                    // (docs/SPEY_DESIGN.md): an 11' 5/6 4-piece switch spey.
                    if let Some(seed) = self
                        .lib
                        .models
                        .iter()
                        .find(|m| m.name.as_deref() == Some("Zeitner T.M. 61105/6-4 Switch Spey"))
                    {
                        if ui.button("New: 11' 5/6 spey seed").clicked() {
                            self.design = Some(DesignState::new(seed));
                        }
                    }
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

                ui.label(format!(
                    "{} of {} rods",
                    indices.len(),
                    self.lib.models.len()
                ));

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
            if self.design.is_some() {
                self.design_panel(ui);
                return;
            }

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

            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.view, PanelView::Chart, "Chart");
                ui.selectable_value(&mut self.view, PanelView::StationData, "Station Data");
                ui.selectable_value(&mut self.view, PanelView::MillSettings, "Mill Settings");
                ui.selectable_value(&mut self.view, PanelView::DeltaChart, "Dimension Changes");
                ui.selectable_value(&mut self.view, PanelView::Stress, "Stress");
                ui.selectable_value(&mut self.view, PanelView::PlaningForm, "Planing Form");
            });
            ui.add_space(4.0);

            match self.view {
                PanelView::Chart => {
                    // Overlaid taper profiles. egui_plot auto-assigns a stable
                    // color per named line, and the legend lets you toggle
                    // individual rods.
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
                }
                PanelView::StationData => {
                    if self.selected.len() != 1 {
                        ui.label("Select exactly one rod to view its station data.");
                    } else {
                        let t = &self.lib.models[self.selected[0]];
                        egui::ScrollArea::vertical()
                            .id_salt("station_data_scroll")
                            .max_height(ui.available_height() * 0.7)
                            .show(ui, |ui| {
                                egui::Grid::new("station_data")
                                    .striped(true)
                                    .num_columns(2)
                                    .show(ui, |ui| {
                                        for h in ["Station (in)", "Dimension (in)"] {
                                            ui.label(egui::RichText::new(h).strong());
                                        }
                                        ui.end_row();
                                        for [station, dimension] in t.profile() {
                                            ui.label(format!("{station:.2}"));
                                            ui.label(format!("{dimension:.4}"));
                                            ui.end_row();
                                        }
                                    });
                            });
                    }
                }
                PanelView::MillSettings => {
                    if self.selected.len() != 1 {
                        ui.label("Select exactly one rod to view Morgan Hand Mill settings.");
                    } else {
                        let t = &self.lib.models[self.selected[0]];
                        let multi_piece = t.pieces.unwrap_or(1.0) > 1.0;
                        ui.horizontal(|ui| {
                            ui.label("Rough oversize:");
                            ui.add(
                                egui::DragValue::new(&mut self.rough_oversize)
                                    .speed(0.001)
                                    .fixed_decimals(3),
                            );
                            ui.label("Finish oversize:");
                            ui.add(
                                egui::DragValue::new(&mut self.finish_oversize)
                                    .speed(0.001)
                                    .fixed_decimals(3),
                            );
                            if multi_piece {
                                ui.checkbox(&mut self.split_by_piece, "Split by piece");
                            }
                        });
                        ui.add_space(4.0);
                        egui::ScrollArea::vertical()
                            .id_salt("mill_settings_scroll")
                            .max_height(ui.available_height() * 0.7)
                            .show(ui, |ui| {
                                if multi_piece && self.split_by_piece {
                                    for (k, section) in t
                                        .mill_sections(self.rough_oversize, self.finish_oversize)
                                        .into_iter()
                                        .enumerate()
                                    {
                                        let mut heading = section.label.clone();
                                        if section.approximate {
                                            heading.push_str(
                                                " (approx. split — no ferrule location on record)",
                                            );
                                        }
                                        ui.label(egui::RichText::new(heading).strong());
                                        mill_settings_grid(
                                            ui,
                                            &format!("mill_{k}"),
                                            &section.settings,
                                        );
                                        ui.add_space(8.0);
                                    }
                                } else {
                                    mill_settings_grid(
                                        ui,
                                        "mill_settings",
                                        &t.mill_settings(self.rough_oversize, self.finish_oversize),
                                    );
                                }
                            });
                    }
                }
                PanelView::DeltaChart => {
                    if self.selected.len() != 1 {
                        ui.label(
                            "Select exactly one rod to view station-to-station dimension changes.",
                        );
                    } else {
                        let t = &self.lib.models[self.selected[0]];
                        let deltas = t.dimension_deltas();
                        let bars: Vec<Bar> = deltas
                            .iter()
                            .map(|d| Bar::new(d.station, d.delta).name(format!("{:.2}\"", d.station)))
                            .collect();
                        let line_points: PlotPoints =
                            deltas.iter().map(|d| [d.station, d.delta]).collect();
                        let ferrule_locations: Vec<f64> =
                            t.ferrules().iter().map(|f| f.location).collect();
                        Plot::new("delta")
                            .legend(Legend::default())
                            .x_axis_label("Station (in from tip)")
                            .y_axis_label("Dimension change (in)")
                            .height(ui.available_height() * 0.7)
                            .show(ui, |plot_ui| {
                                plot_ui.bar_chart(BarChart::new(bars).name("Δ dimension").width(4.0));
                                plot_ui.line(Line::new(line_points).name("Δ dimension"));
                                for d in &deltas {
                                    plot_ui.text(Text::new(
                                        PlotPoint::new(d.station, d.delta),
                                        format!("{:.3}", d.delta),
                                    ));
                                }
                                for loc in ferrule_locations {
                                    plot_ui.vline(
                                        VLine::new(loc)
                                            .name("Ferrule")
                                            .color(egui::Color32::from_rgb(200, 80, 80)),
                                    );
                                }
                            });
                    }
                }
                PanelView::Stress => {
                    // Overlaid Garrison stress curves, same idiom as the
                    // Chart tab. Rods missing a required input (line
                    // weight/length/cast, impact factor, bamboo density, tip
                    // weight) contribute no line rather than erroring — flag
                    // them explicitly so an empty plot doesn't look broken.
                    let missing: Vec<&str> = self
                        .selected
                        .iter()
                        .map(|&i| &self.lib.models[i])
                        .filter(|t| t.stress_curve().is_empty())
                        .map(|t| t.name.as_deref().unwrap_or("(unnamed)"))
                        .collect();
                    if !missing.is_empty() {
                        ui.label(
                            egui::RichText::new(format!(
                                "No stress curve for: {} — this source library doesn't carry the \
                                 physics inputs (line length/cast, impact factor, bamboo density, \
                                 tip weight) the model needs. Only RodDNA-sourced records have them.",
                                missing.join(", ")
                            ))
                            .weak(),
                        );
                        ui.add_space(4.0);
                    }
                    Plot::new("stress")
                        .legend(Legend::default())
                        .x_axis_label("Station (in from tip)")
                        .y_axis_label("Stress (psi)")
                        .height(ui.available_height() * 0.7)
                        .show(ui, |plot_ui| {
                            for &i in &self.selected {
                                let t = &self.lib.models[i];
                                let curve = t.stress_curve();
                                if curve.is_empty() {
                                    continue;
                                }
                                let name = t.name.clone().unwrap_or_else(|| format!("model {i}"));
                                let points: PlotPoints = curve.into_iter().collect();
                                plot_ui.line(Line::new(points).name(name));
                            }
                        });
                }
                PanelView::PlaningForm => {
                    if self.selected.len() != 1 {
                        ui.label("Select exactly one rod to view planing-form settings.");
                    } else {
                        let t = &self.lib.models[self.selected[0]];
                        let settings = t.planing_form_depths();
                        if settings.is_empty() {
                            ui.label(
                                egui::RichText::new(
                                    "No planing-form settings — RodDNA only supports Hex, Quad, \
                                     and Penta geometries for this report.",
                                )
                                .weak(),
                            );
                        } else {
                            egui::ScrollArea::vertical()
                                .id_salt("planing_form_scroll")
                                .max_height(ui.available_height() * 0.7)
                                .show(ui, |ui| {
                                    egui::Grid::new("planing_form")
                                        .striped(true)
                                        .num_columns(3)
                                        .show(ui, |ui| {
                                            for h in ["Station (in)", "Dimension (in)", "Form depth (in)"]
                                            {
                                                ui.label(egui::RichText::new(h).strong());
                                            }
                                            ui.end_row();
                                            for s in &settings {
                                                ui.label(format!("{:.2}", s.station));
                                                ui.label(format!("{:.4}", s.dimension));
                                                ui.label(format!("{:.4}", s.depth));
                                                ui.end_row();
                                            }
                                        });
                                });
                        }
                    }
                }
            }

            // Notes + casting feedback only make sense for a single rod.
            if self.selected.len() == 1 {
                let taper = &self.lib.models[self.selected[0]];
                egui::ScrollArea::vertical()
                    .id_salt("notes_scroll")
                    .show(ui, |ui| {
                        if let Some(notes) = taper.notes.as_deref() {
                            if !notes.is_empty() {
                                ui.separator();
                                ui.label(egui::RichText::new("Notes").strong());
                                ui.label(notes);
                            }
                        }
                        self.ferrules_section(ui, taper);
                        self.casting_notes(ui, taper);
                    });
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

/// A small pill rendering an action tag.
fn action_chip(ui: &mut egui::Ui, text: &str) {
    egui::Frame::none()
        .fill(ui.visuals().faint_bg_color)
        .rounding(6.0)
        .inner_margin(egui::Margin::symmetric(5.0, 1.0))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(text).small());
        });
}

/// Render one Morgan Hand Mill settings table.
fn mill_settings_grid(ui: &mut egui::Ui, grid_id: &str, settings: &[roddna_core::MillSetting]) {
    egui::Grid::new(grid_id)
        .striped(true)
        .num_columns(8)
        .show(ui, |ui| {
            for h in [
                "Station",
                "Anvil #",
                "Dimension",
                "Half",
                "Rough Oversize",
                "Finish Oversize",
                "Finish+Enamel",
                "Total Increase",
            ] {
                ui.label(egui::RichText::new(h).strong());
            }
            ui.end_row();
            for m in settings {
                ui.label(format!("{:.2}", m.station));
                ui.label(format!("#{}", m.anvil_number));
                ui.label(format!("{:.4}", m.dimension));
                ui.label(format!("{:.4}", m.half_dimension));
                ui.label(format!("{:.4}", m.rough_oversize));
                ui.label(format!("{:.4}", m.finish_oversize));
                ui.label(format!("{:.4}", m.finish_enamel));
                ui.label(format!("{:.4}", m.total_increase));
                ui.end_row();
            }
        });
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
