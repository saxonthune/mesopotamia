use bevy::prelude::*;
use bevy::camera::Viewport;
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};

use crate::elk::abundance::AbundanceParams;
use crate::elk::{Decomposable, Decision, Elk, ElkParams, Herds, LastDecision, DriveSamples, RatioControls};
use crate::events::EventLog;
use crate::grid::{Fertility, Grid, GrowthRate};
use crate::history::History;
use crate::render::{cell_world_pos, CameraSettings, WorldCamera};

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiState>()
            .init_resource::<History>()
            // pick_herd (world click → select) runs before the panel draws it.
            .add_systems(Update, (pick_herd, crate::history::sample_history))
            .add_systems(
                EguiPrimaryContextPass,
                // graphs_bar (top) and control_panel (bottom) both claim screen
                // edges; set_camera_viewport then reads the leftover rect, so it
                // must run last — hence .chain().
                (graphs_bar, control_panel, set_camera_viewport).chain(),
            );
    }
}

/// A toggleable field overlay rendered in the world view. `ALL` drives the
/// toggle row; `HashSet<Overlay>` on `UiState` is the source of truth for
/// which overlays are active. Off by default — pure view state, no sim effect.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum Overlay {
    GrassGradient,
    WaterPenalty,
    DeathSites,
}

impl Overlay {
    pub const ALL: [Overlay; 3] = [Overlay::GrassGradient, Overlay::WaterPenalty, Overlay::DeathSites];

    pub fn label(self) -> &'static str {
        match self {
            Overlay::GrassGradient => "grass gradient",
            Overlay::WaterPenalty => "water penalty",
            Overlay::DeathSites => "death sites",
        }
    }
}

#[derive(Resource, Default)]
pub struct UiState {
    tab: Tab,
    selected: Option<u32>, // hex code of the herd shown in the details pane;
    // persists on a dead/migrated cohort until the user picks another.
    selected_elk: Option<Entity>, // the specific elk entity last picked by world click
    visible: std::collections::HashSet<Graph>, // overview graphs toggled on in the top bar
    pub overlays: std::collections::HashSet<Overlay>, // field overlays active in the world view
    histogram_metric: usize, // index into ELK_METRICS for the distribution histogram
}

#[derive(Default, PartialEq, Clone, Copy)]
enum Tab {
    #[default]
    Sliders,
    Herds,
}

/// A toggleable overview graph shown in the top bar. `ALL` drives both the
/// toggle row and the render loop, so adding a variant adds its button and plot
/// in one place. Visibility is pure view state (lives on `UiState`); the sampler
/// always records, regardless of what's shown.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Graph {
    Biomass,
    Histogram,
    Abundance,
}

impl Graph {
    const ALL: [Graph; 3] = [Graph::Biomass, Graph::Histogram, Graph::Abundance];

    /// The toggle-button label.
    fn label(self) -> &'static str {
        match self {
            Graph::Biomass => "biomass",
            Graph::Histogram => "histogram",
            Graph::Abundance => "abundance",
        }
    }
}

/// Standard labelled `f32` slider — the one-liner every control page uses.
fn slider(ui: &mut egui::Ui, value: &mut f32, range: std::ops::RangeInclusive<f32>, label: &str) {
    ui.add(egui::Slider::new(value, range).text(label));
}

/// A labelled set of time-series read from `History` for line-graph rendering.
struct PlotSpec<'a> {
    /// `egui_plot::Plot` id — must be unique within the panel.
    id: &'a str,
    /// Plot height in points. Inline plots use a fixed value; a graph filling a
    /// resizable window passes the window's available height.
    height: f32,
    /// Each entry is (display name, ring-buffer reference).
    series: Vec<(&'a str, &'a std::collections::VecDeque<f32>)>,
}

/// One slice of a pie chart: short display label, hover explanation, magnitude, fill colour.
struct PieSpec<'a> {
    slices: Vec<(&'a str, &'a str, f32, egui::Color32)>,
}

/// Declarative panel content. Add new variants here before `Custom` so the
/// match in `render_items` stays exhaustive and easy to extend.
#[allow(dead_code)]
enum Item<'a> {
    Slider { value: &'a mut f32, range: std::ops::RangeInclusive<f32>, label: &'a str },
    Label(String),
    Separator,
    /// Line graph via egui_plot.
    Plot(PlotSpec<'a>),
    /// Pie chart drawn with egui's Painter.
    Pie(PieSpec<'a>),
    /// Progressive-disclosure group: a `CollapsingHeader` wrapping nested items.
    Section { title: &'a str, items: Vec<Item<'a>>, default_open: bool },
    /// Escape hatch for complex bodies that can't yet be expressed as items.
    Custom(Box<dyn FnOnce(&mut egui::Ui) + 'a>),
}

/// Walk an item list and render each to `ui`. Recursive through `Section`.
fn render_items(ui: &mut egui::Ui, items: Vec<Item>) {
    for item in items {
        match item {
            Item::Slider { value, range, label } => slider(ui, value, range, label),
            Item::Label(text) => { ui.label(text); }
            Item::Separator => { ui.separator(); }
            Item::Plot(spec) => render_plot(ui, spec),
            Item::Pie(spec) => render_pie(ui, &spec.slices),
            Item::Section { title, items, default_open } => {
                egui::CollapsingHeader::new(title)
                    .default_open(default_open)
                    .show(ui, |ui| render_items(ui, items));
            }
            Item::Custom(f) => f(ui),
        }
    }
}

fn render_plot(ui: &mut egui::Ui, spec: PlotSpec) {
    use egui_plot::{Line, Plot, PlotPoints};
    // Lock the view: the plot auto-fits its data each frame, and the user can't
    // pan or zoom it out of frame. All declarative builder flags on egui_plot.
    Plot::new(spec.id)
        .height(spec.height)
        .allow_scroll(false)
        .allow_drag(false)
        .allow_zoom(false)
        .allow_boxed_zoom(false)
        .show(ui, |plot_ui| {
            for (name, data) in spec.series {
                let points: PlotPoints = data
                    .iter()
                    .enumerate()
                    .map(|(i, &v)| [i as f64, v as f64])
                    .collect();
                plot_ui.line(Line::new(name, points));
            }
        });
}

fn render_pie(ui: &mut egui::Ui, slices: &[(&str, &str, f32, egui::Color32)]) {
    let total: f32 = slices.iter().map(|(_, _, v, _)| *v).sum();
    if total < 1e-6 {
        ui.label("no drive data");
        return;
    }

    let size = 120.0f32;
    let (resp, painter) = ui.allocate_painter(egui::vec2(size, size), egui::Sense::hover());
    let center = resp.rect.center();
    let radius = size * 0.45;

    let mut start = -std::f32::consts::FRAC_PI_2;
    for &(_, _, value, color) in slices {
        let sweep = value / total * std::f32::consts::TAU;
        let steps = ((sweep * radius / 2.0) as usize).max(3);
        let mut pts: Vec<egui::Pos2> = Vec::with_capacity(steps + 2);
        pts.push(center);
        for k in 0..=steps {
            let a = start + sweep * k as f32 / steps as f32;
            pts.push(center + egui::vec2(a.cos(), a.sin()) * radius);
        }
        painter.add(egui::Shape::convex_polygon(pts, color, egui::Stroke::NONE));
        start += sweep;
    }

    // Legend: labelled colour swatches, one per row. Hover for the full
    // explanation and weight — progressive disclosure, visible label first.
    for &(label, explain, value, color) in slices {
        let row_resp = ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
            ui.painter().rect_filled(rect, 2.0, color);
            ui.label(label);
        }).response;
        row_resp.on_hover_text(format!("{explain}\n(weight {value:.2})"));
    }
}

/// One self-contained control group: a heading and an ordered list of items.
struct Panel<'a> {
    title: &'a str,
    /// Target column width — the flex-basis. Panels wrap to a new row when the
    /// current row can't fit another at its width.
    width: f32,
    items: Vec<Item<'a>>,
}

impl<'a> Panel<'a> {
    const DEFAULT_WIDTH: f32 = 240.0;

    fn new(title: &'a str, items: Vec<Item<'a>>) -> Self {
        Self { title, width: Self::DEFAULT_WIDTH, items }
    }

    /// Override the flex-basis for a wider/narrower group.
    fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }
}

/// Lay panels out left-to-right, wrapping to a new row when the row fills —
/// `flex-wrap: wrap`. Each panel is boxed to its own `width` so it forms a
/// column rather than spreading to fill the row.
fn panel_flow(ui: &mut egui::Ui, panels: Vec<Panel>) {
    ui.horizontal_wrapped(|ui| {
        for p in panels {
            ui.allocate_ui_with_layout(
                egui::vec2(p.width, ui.available_height()),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        ui.set_width(p.width);
                        ui.heading(p.title);
                        render_items(ui, p.items);
                    });
                },
            );
        }
    });
}

/// The top bar: a row of toggles that show or hide overview graphs. Graphs start
/// hidden; toggling one opens a draggable, resizable window floating over the
/// world (an egui `Window`, not a panel — so it never shrinks the viewport).
fn graphs_bar(
    mut contexts: EguiContexts,
    mut state: ResMut<UiState>,
    history: Res<History>,
    elk: Query<&Elk>,
    event_log: Res<EventLog>,
) -> Result {
    let ctx = contexts.ctx_mut()?;

    egui::TopBottomPanel::top("graphs_bar").show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.label("graphs:");
            for g in Graph::ALL {
                let on = state.visible.contains(&g);
                if ui.selectable_label(on, g.label()).clicked() {
                    if on { state.visible.remove(&g); } else { state.visible.insert(g); }
                }
            }
            ui.separator();
            ui.label("overlays:");
            for ov in Overlay::ALL {
                let on = state.overlays.contains(&ov);
                if ui.selectable_label(on, ov.label()).clicked() {
                    if on { state.overlays.remove(&ov); } else { state.overlays.insert(ov); }
                }
            }
        });
    });

    // Time-series graphs: live in their own floating windows.
    for g in Graph::ALL {
        if state.visible.contains(&g) && matches!(g, Graph::Biomass | Graph::Abundance) {
            egui::Window::new(g.label())
                .default_size([360.0, 200.0])
                // The top-bar toggle already shows/hides the graph, so the
                // window's own collapse arrow is redundant.
                .collapsible(false)
                .show(ctx, |ui| {
                    render_plot(ui, graph_plot(g, &history, ui.available_height()));
                });
        }
    }

    // Histogram window: needs live elk data and mutable metric selection.
    if state.visible.contains(&Graph::Histogram) {
        let mut sel = state.histogram_metric;
        egui::Window::new(Graph::Histogram.label())
            .default_size([360.0, 200.0])
            .collapsible(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    for (i, m) in crate::metrics::ELK_METRICS.iter().enumerate() {
                        ui.selectable_value(&mut sel, i, m.name);
                    }
                });
                render_histogram(ui, &elk, sel);
            });
        state.histogram_metric = sel;
    }

    // Recent-events window: visible when the DeathSites overlay is active.
    if state.overlays.contains(&Overlay::DeathSites) {
        egui::Window::new("recent deaths")
            .default_size([300.0, 200.0])
            .collapsible(false)
            .show(ctx, |ui| {
                render_event_list(ui, &event_log);
            });
    }

    Ok(())
}

/// The plot for one overview graph, sized to `height`. Biomass plots grass and
/// shrubs as two series so the riparian and steppe compartments read separately
/// rather than collapsing into one summed line.
fn graph_plot<'a>(graph: Graph, history: &'a History, height: f32) -> PlotSpec<'a> {
    match graph {
        Graph::Biomass => PlotSpec {
            id: "biomass",
            height,
            series: vec![
                ("grass", &history.grass_mass),
                ("shrubs", &history.shrub_mass),
            ],
        },
        // Forage-per-elk and the regrowth÷drain ratio over time. When the ratio
        // line sits above 1, herds' patches refill faster than they graze them —
        // the quantitative reason they camp instead of migrating.
        Graph::Abundance => PlotSpec {
            id: "abundance",
            height,
            series: vec![
                ("forage per elk", &history.abundance_per_elk),
                ("regrowth ÷ drain", &history.regrowth_drain_ratio),
            ],
        },
        Graph::Histogram => unreachable!("Histogram is rendered by render_histogram, not graph_plot"),
    }
}

/// Draw a bar-chart histogram of `metric_idx` across all live elk.
/// Bins the selected metric into 20 equal-width buckets over [0, 1].
fn render_histogram(ui: &mut egui::Ui, elk: &Query<&Elk>, metric_idx: usize) {
    use egui_plot::{Bar, BarChart, Plot};

    let Some(metric) = crate::metrics::ELK_METRICS.get(metric_idx) else { return; };

    const BINS: usize = 20;
    let mut counts = [0u32; BINS];
    let mut n = 0u32;
    for e in elk {
        let v = (metric.extract)(e).clamp(0.0, 1.0);
        let bin = ((v * BINS as f32) as usize).min(BINS - 1);
        counts[bin] += 1;
        n += 1;
    }

    if n == 0 {
        ui.label("no elk");
        return;
    }

    let bin_w = 1.0 / BINS as f64;
    let bars: Vec<Bar> = counts
        .iter()
        .enumerate()
        .map(|(i, &c)| {
            Bar::new(i as f64 * bin_w + bin_w / 2.0, c as f64).width(bin_w)
        })
        .collect();

    Plot::new("elk_histogram")
        .height(ui.available_height().max(80.0))
        .allow_scroll(false)
        .allow_drag(false)
        .allow_zoom(false)
        .allow_boxed_zoom(false)
        .show(ui, |plot_ui| {
            plot_ui.bar_chart(BarChart::new(metric.name, bars));
        });
}

/// Show a scrollable list of the most recent starvation events: tick, cell, energy.
fn render_event_list(ui: &mut egui::Ui, event_log: &EventLog) {
    if event_log.recent.is_empty() {
        ui.weak("no starvation events yet");
        return;
    }
    egui::ScrollArea::vertical().show(ui, |ui| {
        for event in event_log.recent.iter().rev().take(50) {
            let kind = match event.kind {
                crate::events::EventKind::Starved => "starved",
            };
            let step = event.chosen_step
                .map(|(dx, dy)| format!(" step({dx:+},{dy:+})"))
                .unwrap_or_default();
            ui.label(format!(
                "tick {}  cell {}  {kind}  e={:.3}{step}",
                event.tick, event.cell, event.energy
            ));
        }
    });
}

/// The docked bottom panel: a tab bar with always-visible speed controls, and a
/// scrolling content area paged by the selected tab.
#[allow(clippy::too_many_arguments)]
fn control_panel(
    mut contexts: EguiContexts,
    mut state: ResMut<UiState>,
    mut elk_params: ResMut<ElkParams>,
    mut ratio_controls: ResMut<RatioControls>,
    mut growth: ResMut<GrowthRate>,
    mut fertility: ResMut<Fertility>,
    mut ab_params: ResMut<AbundanceParams>,
    mut camera: ResMut<CameraSettings>,
    mut time: ResMut<Time<Virtual>>,
    mut world_seed: ResMut<crate::worldgen::WorldSeed>,
    mut next_state: ResMut<NextState<crate::sim::Sim>>,
    herds: Res<Herds>,
    history: Res<History>,
    drive_samples: Res<DriveSamples>,
    last_decisions: Query<&LastDecision>,
) -> Result {
    egui::TopBottomPanel::bottom("control_panel")
        .resizable(true)
        .default_height(280.0)
        .min_height(120.0)
        .show(contexts.ctx_mut()?, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut state.tab, Tab::Sliders, "Sliders");
                ui.selectable_value(&mut state.tab, Tab::Herds, "Herds");
                ui.separator();
                speed_inline(ui, time.as_mut());
                ui.separator();
                if ui.button("⟳ regenerate").on_hover_text(format!("seed {:#018x}", world_seed.0)).clicked() {
                    world_seed.0 = rand::random();
                    next_state.set(crate::sim::Sim::Generating);
                }
            });
            ui.separator();
            // The panel height is user-set by dragging its top border; the
            // scroll area fills whatever is left below the tab bar, so a taller
            // panel reveals more rows and a short one scrolls.
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| match state.tab {
                Tab::Sliders => {
                    let rc = ratio_controls.as_mut();
                    let regrow_ratio = &mut rc.regrow_ratio;
                    let bite_ratio = &mut rc.bite_ratio;
                    let cross_ratio = &mut rc.cross_ratio;
                    panel_flow(ui, vec![
                        Panel::new("Grass", grass_items(growth.as_mut(), fertility.as_mut(), regrow_ratio)),
                        // Behaviour carries the most rows, so give it a wider column.
                        Panel::new("Behaviour", vec![Item::Custom(Box::new(|ui| behaviour_tab(ui, elk_params.as_mut(), bite_ratio, cross_ratio)))]).width(300.0),
                        Panel::new("Abundance (measure)", abundance_items(ab_params.as_mut())),
                        Panel::new("View", vec![Item::Custom(Box::new(|ui| view_tab(ui, camera.as_mut())))]),
                    ])
                }
                Tab::Herds => herds_view(ui, state.as_mut(), &herds, &history, &drive_samples, &last_decisions),
            });
        });
    Ok(())
}

/// The Herds tab: a scrollable list of herd cards on the left; clicking one
/// shows its full details in the scrollable pane on the right. The selection
/// persists on a dead/migrated cohort until the user picks another.
fn herds_view(ui: &mut egui::Ui, state: &mut UiState, herds: &Herds, history: &History, drive_samples: &DriveSamples, last_decisions: &Query<&LastDecision>) {
    let alive: Vec<u32> = herds
        .order
        .iter()
        .copied()
        .filter(|c| herds.cohorts.get(c).is_some_and(|co| co.alive > 0))
        .collect();

    ui.columns(2, |cols| {
        egui::ScrollArea::vertical()
            .id_salt("herd_cards")
            .show(&mut cols[0], |ui| {
                if alive.is_empty() {
                    ui.weak("no herds");
                }
                for &code in &alive {
                    if let Some(co) = herds.cohorts.get(&code) {
                        herd_card(ui, code, co, state);
                    }
                }
            });

        egui::ScrollArea::vertical()
            .id_salt("herd_details")
            .show(&mut cols[1], |ui| match state
                .selected
                .and_then(|c| herds.cohorts.get(&c).map(|co| (c, co)))
            {
                Some((code, co)) => herd_details(ui, code, co, history, drive_samples, state.selected_elk, last_decisions),
                None => {
                    ui.weak("select a herd");
                }
            });
    });
}

fn herd_health(co: &crate::elk::Cohort) -> f32 {
    if co.alive > 0 {
        co.energy_sum / co.alive as f32
    } else {
        0.0
    }
}

/// A clickable summary card for one herd. Highlights when selected.
fn herd_card(ui: &mut egui::Ui, code: u32, co: &crate::elk::Cohort, state: &mut UiState) {
    let mut frame = egui::Frame::group(ui.style());
    if state.selected == Some(code) {
        frame = frame.fill(ui.visuals().selection.bg_fill);
    }
    let resp = frame
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.strong(format!("{code:06x}"));
                if co.alive == 0 {
                    ui.weak("· no survivors");
                }
            });
            ui.add(
                egui::ProgressBar::new(herd_health(co))
                    .desired_width(ui.available_width())
                    .text(format!("health {:.0}%", herd_health(co) * 100.0)),
            );
            ui.label(format!("alive: {}  peak: {}  cohort slot: {}", co.alive, co.peak, co.slot));
        })
        .response
        .interact(egui::Sense::click());

    if resp.clicked() {
        state.selected = Some(code);
    }
}

const DRIVE_COLORS: [egui::Color32; 5] = [
    egui::Color32::from_rgb(70, 130, 180),  // sep   — steel blue
    egui::Color32::from_rgb(60, 179, 113),  // coh   — medium sea green
    egui::Color32::from_rgb(34, 139, 34),   // grass — forest green
    egui::Color32::from_rgb(255, 165, 0),   // social — orange
    egui::Color32::from_rgb(180, 60, 80),   // migration — crimson
];

/// Short display labels per drive, in the same order as `DRIVE_COLORS` and
/// `Drives::contributions`. Shown as visible text beside the colour swatch in the
/// drive pie legend; hover reveals the full `DRIVE_EXPLAIN` text.
const DRIVE_LABELS: [&str; 5] = [
    "Separation",
    "Cohesion",
    "Forage",
    "Foraging cue",
    "Migration",
];

/// Plain-language explanation per drive, in the same order as `DRIVE_COLORS` and
/// `Drives::contributions`. Shown on hover as the deeper-disclosure tier beneath
/// the visible `DRIVE_LABELS`.
const DRIVE_EXPLAIN: [&str; 5] = [
    "Separation — spacing out so the herd doesn't pile onto one cell",
    "Cohesion — drifting back toward packmates",
    "Forage — pull up the grass/shrub gradient toward food",
    "Foraging cue — drawn toward others already grazing",
    "Migration — the slow push toward the far side of the map",
];

fn herd_details(
    ui: &mut egui::Ui,
    code: u32,
    co: &crate::elk::Cohort,
    history: &History,
    drive_samples: &DriveSamples,
    selected_elk: Option<Entity>,
    last_decisions: &Query<&LastDecision>,
) {
    ui.heading(format!("pack {code:06x}"));
    ui.label(format!("status: {}", if co.alive > 0 { "alive" } else { "no survivors" }));
    ui.label(format!("cohort slot: {}", co.slot));
    ui.separator();
    ui.add(
        egui::ProgressBar::new(herd_health(co))
            .text(format!("avg energy {:.0}%", herd_health(co) * 100.0)),
    );
    ui.label(format!("alive: {}", co.alive));
    ui.label(format!("peak: {}", co.peak));
    ui.label(format!("starved: {}", co.deaths));
    ui.label(format!("migrated out: {}", co.departures));
    ui.separator();

    // Drive-split pie for this herd's slot.
    let slot = co.slot as usize;
    if let Some(ds) = drive_samples.per_slot.get(slot) {
        let mags = [ds.sep, ds.coh, ds.grass, ds.social, ds.migration];
        let pie_slices: Vec<(&str, &str, f32, egui::Color32)> = (0..5)
            .map(|i| (DRIVE_LABELS[i], DRIVE_EXPLAIN[i], mags[i], DRIVE_COLORS[i]))
            .collect();
        ui.label("drive composition:");
        render_items(ui, vec![Item::Pie(PieSpec { slices: pie_slices })]);
    }

    ui.separator();

    // Population over time (all elk).
    render_items(ui, vec![
        Item::Label("population".to_string()),
        Item::Plot(PlotSpec {
            id: "pop_plot",
            height: 80.0,
            series: vec![("population", &history.population)],
        }),
    ]);

    // Migration share for this slot over time.
    if let Some(share_buf) = history.migration_share.get(slot) {
        render_items(ui, vec![
            Item::Label("migration share".to_string()),
            Item::Plot(PlotSpec {
                id: &format!("mig_share_{slot}"),
                height: 80.0,
                series: vec![("migration share", share_buf)],
            }),
        ]);
    }

    ui.separator();
    match selected_elk.and_then(|e| last_decisions.get(e).ok()) {
        Some(ld) => elk_decision_panel(ui, &ld.0),
        None => { ui.weak("no elk selected — click one to inspect its decision"); }
    }
}

/// Compass glyph for a unit step in grid space (world +y is up, so +row points up).
fn step_arrow(step: (isize, isize)) -> &'static str {
    match step {
        (1, 0) => "→",
        (-1, 0) => "←",
        (0, 1) => "↑",
        (0, -1) => "↓",
        _ => "•",
    }
}

fn elk_decision_panel(ui: &mut egui::Ui, decision: &Decision) {
    ui.label("selected elk — last decision");

    // Drive decomposition: labelled swatches with hover for full explanation.
    // `contributions` is in DRIVE_LABELS/DRIVE_COLORS/DRIVE_EXPLAIN order.
    let contributions = decision.drives.contributions();
    let pie_slices: Vec<(&str, &str, f32, egui::Color32)> = contributions
        .iter()
        .enumerate()
        .map(|(i, (_, vec))| (DRIVE_LABELS[i], DRIVE_EXPLAIN[i], vec.length(), DRIVE_COLORS[i]))
        .collect();
    ui.label("drive breakdown:");
    render_items(ui, vec![Item::Pie(PieSpec { slices: pie_slices })]);

    // Step options as direction arrows — the chosen one tinted, the rest dimmed.
    // Hover any arrow for its score / water penalty / pick chance, so there are no
    // coordinate tuples or column headers to decode.
    let total_weight: f32 = decision.options.iter().map(|e| e.weight).sum();
    if decision.options.is_empty() {
        ui.weak("hemmed in — nowhere to step");
    } else {
        ui.label("step options (hover for scores):");
        ui.horizontal(|ui| {
            for (idx, eval) in decision.options.iter().enumerate() {
                let prob = if total_weight > 1e-6 { eval.weight / total_weight } else { 0.0 };
                let chosen = decision.chosen == Some(idx);
                let mut text = egui::RichText::new(step_arrow(eval.step)).size(20.0);
                text = if chosen {
                    text.strong().color(egui::Color32::from_rgb(120, 200, 120))
                } else {
                    text.weak()
                };
                ui.label(text).on_hover_text(format!(
                    "score {:.2} · water penalty {:.2} · pick chance {:.0}%",
                    eval.score, eval.penalty, prob * 100.0
                ));
            }
        });
    }
}

/// A left-click in the world selects the nearest elk's herd and opens the Herds
/// tab on it. Clicks over the egui panel are ignored. The cursor is unprojected
/// through the world camera, so it respects the viewport clip above the dock.
fn pick_herd(
    mouse: Res<ButtonInput<MouseButton>>,
    mut contexts: EguiContexts,
    window: Single<&Window>,
    camera: Single<(&Camera, &GlobalTransform), With<WorldCamera>>,
    grid: Res<Grid>,
    elk: Query<(Entity, &Elk)>,
    mut state: ResMut<UiState>,
) -> Result {
    if !mouse.just_pressed(MouseButton::Left) {
        return Ok(());
    }
    if contexts.ctx_mut()?.wants_pointer_input() {
        return Ok(()); // the click landed on the UI
    }
    let Some(cursor) = window.cursor_position() else {
        return Ok(());
    };
    // Let the camera do the unprojection — it knows its own viewport, so a click
    // maps to the right world point even though the world is clipped to the area
    // above the dock. Outside the viewport (e.g. over the panel) this errors.
    let (cam, cam_transform) = *camera;
    let Ok(world) = cam.viewport_to_world_2d(cam_transform, cursor) else {
        return Ok(());
    };

    // Nearest elk within a generous world-space radius (~6 tiles) picks the herd.
    const PICK_RADIUS: f32 = 96.0;
    let tol2 = PICK_RADIUS * PICK_RADIUS;
    let mut best: Option<(Entity, u32, f32)> = None;
    for (entity, elk) in &elk {
        let d2 = (cell_world_pos(&grid, elk.cell) - world).length_squared();
        if d2 <= tol2 && best.is_none_or(|(_, _, b)| d2 < b) {
            best = Some((entity, elk.code, d2));
        }
    }
    if let Some((entity, code, _)) = best {
        state.selected = Some(code);
        state.selected_elk = Some(entity);
        state.tab = Tab::Herds;
    }
    Ok(())
}

/// Map the egui logical rect left free above the bottom dock to a physical-pixel
/// camera viewport, so the world renders above the panel rather than behind it.
///
/// Pure so it can be unit-tested without a window: `min`/`size` are the egui
/// available rect in logical points, `scale` is the window's scale factor, and
/// `target` is the render target's physical size. Returns `None` when the free
/// area collapses (the panel fills the window) so the caller clears the
/// viewport and the camera falls back to the whole window.
fn world_viewport(min: Vec2, size: Vec2, scale: f32, target: UVec2) -> Option<Viewport> {
    let pos = (min * scale).max(Vec2::ZERO).as_uvec2();
    let px = (size * scale).max(Vec2::ZERO).as_uvec2();
    // Clamp so position + size never exceed the target (an over-large scissor
    // rect is a hard wgpu crash, not a clip).
    let w = px.x.min(target.x.saturating_sub(pos.x));
    let h = px.y.min(target.y.saturating_sub(pos.y));
    if w == 0 || h == 0 {
        return None;
    }
    Some(Viewport {
        physical_position: pos,
        physical_size: UVec2::new(w, h),
        ..default()
    })
}

/// Confine the *world* camera to the rect egui leaves above the dock. The egui
/// camera is separate and full-window, so clipping here never touches the UI.
fn set_camera_viewport(
    mut contexts: EguiContexts,
    window: Single<&Window>,
    camera: Single<&mut Camera, With<WorldCamera>>,
) -> Result {
    let avail = contexts.ctx_mut()?.available_rect(); // free area above the panel
    let target = UVec2::new(window.physical_width(), window.physical_height());
    camera.into_inner().viewport = world_viewport(
        Vec2::new(avail.min.x, avail.min.y),
        Vec2::new(avail.width(), avail.height()),
        window.scale_factor(),
        target,
    );
    Ok(())
}

fn speed_inline(ui: &mut egui::Ui, time: &mut Time<Virtual>) {
    const PRESETS: [(&str, f32); 5] =
        [("1x", 1.0), ("2x", 2.0), ("3x", 3.0), ("4x", 4.0), (">>", 64.0)];
    let current = time.relative_speed();
    if ui.selectable_label(time.is_paused(), "‖").clicked() {
        if time.is_paused() {
            time.unpause();
        } else {
            time.pause();
        }
    }
    for (label, mult) in PRESETS {
        if ui.selectable_label(current == mult, label).clicked() {
            time.set_relative_speed(mult);
        }
    }
    let mut custom = current;
    if ui
        .add(egui::DragValue::new(&mut custom).range(0.0..=256.0).speed(0.1))
        .changed()
    {
        time.set_relative_speed(custom);
    }
}

fn grass_items<'a>(growth: &'a mut GrowthRate, fertility: &'a mut Fertility, regrow_ratio: &'a mut f32) -> Vec<Item<'a>> {
    vec![
        Item::Slider { value: regrow_ratio, range: 0.0..=1.0, label: "regrowth ÷ drain" },
        Item::Slider { value: &mut growth.spread, range: 0.0..=0.3, label: "spread from neighbours" },
        Item::Section {
            title: "Fertility",
            items: vec![
                Item::Slider { value: &mut fertility.rate, range: 0.0..=0.5, label: "poop → grass / tick" },
                Item::Slider { value: &mut fertility.efficiency, range: 0.0..=1.0, label: "conversion efficiency" },
            ],
            default_open: false,
        },
    ]
}

/// Sliders for the herd-abundance measurement. Pure view state — these only
/// reshape the `abundance` graph, never the simulation.
fn abundance_items(p: &mut AbundanceParams) -> Vec<Item<'_>> {
    vec![
        Item::Slider { value: &mut p.radius, range: 1.0..=20.0, label: "nearby radius (cells)" },
        Item::Slider { value: &mut p.energy_weight, range: 0.0..=1.0, label: "grass ↔ energy weight" },
    ]
}

fn behaviour_tab(ui: &mut egui::Ui, p: &mut ElkParams, bite_ratio: &mut f32, cross_ratio: &mut f32) {
    ui.label("drive weights");
    slider(ui, &mut p.separation, 0.0..=3.0, "separation");
    slider(ui, &mut p.cohesion, 0.0..=3.0, "cohesion");
    slider(ui, &mut p.grass, 0.0..=3.0, "grass-seeking");
    slider(ui, &mut p.social, 0.0..=3.0, "social foraging");
    slider(ui, cross_ratio, 0.0..=2.0, "migration ÷ crossing cost");
    slider(ui, &mut p.quiet, 0.05..=3.0, "migration crossover (quiet)");
    slider(ui, &mut p.cross, 0.0..=4.0, "water crossing (× hunger)");
    ui.separator();
    ui.label("perception (cells)");
    slider(ui, &mut p.sep_radius, 1.0..=10.0, "separation radius");
    slider(ui, &mut p.coh_radius, 1.0..=20.0, "cohesion radius");
    slider(ui, &mut p.grass_radius, 1.0..=12.0, "grass radius");
    slider(ui, &mut p.social_radius, 1.0..=40.0, "social radius");
    slider(ui, &mut p.temperature, 0.1..=2.0, "step randomness");
    ui.separator();
    ui.label("metabolism");
    slider(ui, &mut p.bite, 0.0..=1.0, "bite / graze (max)");
    slider(ui, bite_ratio, 0.1..=20.0, "intake ÷ drain (bite ratio)");
    // Giving-up density: grass below this fraction of a cell's capacity isn't
    // worth biting, so a grazed-down patch can't sustain an elk.
    slider(ui, &mut p.graze_floor, 0.0..=0.9, "giving-up density (× capacity)");
    slider(ui, &mut p.energy_drain, 0.0..=0.02, "energy drain / tick");
    // The balance made legible: on a full bite, net energy/tick = graze_yield·bite·g
    // − energy_drain, so an elk lives only if it grazes (on full bites) at least
    // drain/(graze_yield·bite) of the time. As a patch thins toward the floor the
    // bite shrinks, so the real duty cycle a grazed-down cell demands is worse.
    let full_bite = p.graze_yield * p.bite;
    ui.label(if full_bite <= 0.0 {
        "break-even grazing: impossible (no energy/bite)".to_string()
    } else {
        let pct = p.energy_drain / full_bite * 100.0;
        if pct > 100.0 {
            format!("break-even grazing: {pct:.0}% — herds starve")
        } else {
            format!("break-even grazing: {pct:.0}% of ticks (on full bites)")
        }
    });
    ui.separator();
    ui.label("shrubs & crossing");
    slider(ui, &mut p.shrub_energy, 0.0..=0.1, "energy / shrub bite");
    slider(ui, &mut p.shrub_bite, 0.0..=1.0, "shrubs / bite");
    slider(ui, &mut p.water_cost, 0.0..=4.0, "water crossing cost");
    slider(ui, &mut p.ford_discount, 0.0..=1.0, "ford discount (0 = free)");
    slider(ui, &mut p.swim_drain, 0.0..=0.05, "swim energy drain");
    slider(ui, &mut p.mig_growth, 0.0..=0.01, "migration growth / tick");
    ui.separator();
    ui.label("patch leaving");
    slider(ui, &mut p.intake_smoothing, 0.001..=0.3, "intake smoothing (α)");
    slider(ui, &mut p.giving_up, 0.0..=0.99, "giving-up ratio (× habitat mean)");
    slider(ui, &mut p.leave_boost, 0.0..=4.0, "migration boost when leaving");
}

fn view_tab(ui: &mut egui::Ui, cam: &mut CameraSettings) {
    ui.add(
        egui::Slider::new(&mut cam.zoom, 0.1..=10.0)
            .logarithmic(true)
            .text("zoom"),
    );
    slider(ui, &mut cam.pan.x, -2000.0..=2000.0, "pan x");
    slider(ui, &mut cam.pan.y, -2000.0..=2000.0, "pan y");
    if ui.button("reset view").clicked() {
        cam.zoom = 1.0;
        cam.pan = Vec2::ZERO;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A 1280x960 window at scale 1 with no panel: the world fills it.
    #[test]
    fn full_window_when_no_panel() {
        let vp = world_viewport(Vec2::ZERO, Vec2::new(1280.0, 960.0), 1.0, UVec2::new(1280, 960))
            .expect("non-empty viewport");
        assert_eq!(vp.physical_position, UVec2::ZERO);
        assert_eq!(vp.physical_size, UVec2::new(1280, 960));
    }

    // A 240pt dock at the bottom shrinks only the height; the world stays pinned
    // to the top-left.
    #[test]
    fn bottom_dock_shrinks_height_only() {
        let vp = world_viewport(Vec2::ZERO, Vec2::new(1280.0, 720.0), 1.0, UVec2::new(1280, 960))
            .expect("non-empty viewport");
        assert_eq!(vp.physical_position, UVec2::ZERO);
        assert_eq!(vp.physical_size, UVec2::new(1280, 720));
    }

    // The Retina regression: logical points must be scaled to physical pixels
    // exactly once. A 640x360 logical region at scale 2 is 1280x720 physical —
    // not 2560x1440 (double-counted, which crashed wgpu with an oversized
    // scissor rect).
    #[test]
    fn retina_scale_counts_once() {
        let vp = world_viewport(Vec2::ZERO, Vec2::new(640.0, 360.0), 2.0, UVec2::new(1280, 720))
            .expect("non-empty viewport");
        assert_eq!(vp.physical_size, UVec2::new(1280, 720));
    }

    // A rect larger than the target is clamped, never allowed to exceed it.
    #[test]
    fn oversize_rect_clamps_to_target() {
        let vp = world_viewport(Vec2::ZERO, Vec2::new(4000.0, 4000.0), 1.0, UVec2::new(1280, 720))
            .expect("non-empty viewport");
        assert_eq!(vp.physical_size, UVec2::new(1280, 720));
    }

    // When the panel fills the window the free area collapses → no viewport, so
    // the camera falls back to the full window instead of a zero-size scissor.
    #[test]
    fn collapsed_area_yields_none() {
        assert!(world_viewport(Vec2::new(0.0, 960.0), Vec2::ZERO, 1.0, UVec2::new(1280, 960)).is_none());
    }
}
