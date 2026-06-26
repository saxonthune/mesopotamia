use std::time::Duration;

use bevy::prelude::*;
use bevy::camera::Viewport;
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};

use crate::elk::abundance::AbundanceParams;
use crate::elk::{Elk, ElkParams, Herding, Herds, RatioControls, Score};
use crate::droppings::Fertility;
use crate::events::EventLog;
use crate::grid::{GreenWave, Grid};
use crate::history::History;
use crate::render::{cell_world_pos, WorldCamera};
use crate::unit_select::{draw_elk_silhouette, herd_color32, SelectionParams, UnitSelectState};

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiState>()
            .init_resource::<History>()
            .init_resource::<WorldViewRect>()
            // pick_herd (world click → select) runs before the panel draws it.
            .add_systems(Update, (pick_herd, keyboard_speed, crate::history::sample_history))
            // must be in EguiPrimaryContextPass — context must exist when glyphs first render
            .add_systems(EguiPrimaryContextPass, install_icon_font)
            .add_systems(
                EguiPrimaryContextPass,
                // .chain(): set_camera_viewport must run after graphs_bar and control_panel
                (graphs_bar, control_panel, set_camera_viewport).chain(),
            );
    }
}

/// Rect (logical pts) the world camera fills: top-bar bottom → window bottom, ignoring dock height.
#[derive(Resource, Default)]
struct WorldViewRect {
    min: Vec2,
    size: Vec2,
}

/// Field overlays rendered in the world view. `ALL` drives the toggle row.
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

#[derive(Resource)]
pub struct UiState {
    tab: Tab,
    selected: Option<u32>, // persists on a dead/migrated cohort until the user picks another
    visible: std::collections::HashSet<Graph>,
    pub overlays: std::collections::HashSet<Overlay>,
    histogram_metric: usize, // index into ELK_METRICS
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            tab: Tab::default(),
            selected: None,
            visible: std::collections::HashSet::from([Graph::SurvivalScore]),
            overlays: std::collections::HashSet::new(),
            histogram_metric: 0,
        }
    }
}

#[derive(Default, PartialEq, Clone, Copy)]
enum Tab {
    #[default]
    Sliders,
    Herds,
    /// Auto-shown when a selection exists; hidden when empty.
    Selection,
}

/// `ALL` drives both the toggle row and the render loop — add a variant here to wire button + plot.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Graph {
    SurvivalScore,
    Biomass,
    Histogram,
    Abundance,
}

impl Graph {
    const ALL: [Graph; 4] = [Graph::SurvivalScore, Graph::Biomass, Graph::Histogram, Graph::Abundance];

    fn label(self) -> &'static str {
        match self {
            Graph::SurvivalScore => "survival score",
            Graph::Biomass => "biomass",
            Graph::Histogram => "histogram",
            Graph::Abundance => "abundance",
        }
    }
}

fn slider(ui: &mut egui::Ui, value: &mut f32, range: std::ops::RangeInclusive<f32>, label: &str) {
    ui.add(egui::Slider::new(value, range).text(label));
}

struct PlotSpec<'a> {
    /// must be unique within the panel (egui_plot requirement)
    id: &'a str,
    height: f32,
    /// (display name, ring-buffer)
    series: Vec<(&'a str, &'a std::collections::VecDeque<f32>)>,
}

#[allow(dead_code)]
enum Item<'a> {
    Slider { value: &'a mut f32, range: std::ops::RangeInclusive<f32>, label: &'a str },
    Label(String),
    Separator,
    Plot(PlotSpec<'a>),
    Section { title: &'a str, items: Vec<Item<'a>>, default_open: bool },
    Custom(Box<dyn FnOnce(&mut egui::Ui) + 'a>),
}

fn render_items(ui: &mut egui::Ui, items: Vec<Item>) {
    for item in items {
        match item {
            Item::Slider { value, range, label } => slider(ui, value, range, label),
            Item::Label(text) => { ui.label(text); }
            Item::Separator => { ui.separator(); }
            Item::Plot(spec) => render_plot(ui, spec),
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
    // lock view: plot auto-fits each frame; no pan/zoom
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

/// Colour ramp ceiling — not a cap on the score.
const SCORE_BRIGHT: f32 = 120.0;

fn render_score_gauge(ui: &mut egui::Ui, score: &Score, history: &History) {
    use egui::{Color32, RichText};

    let lit = (score.current / SCORE_BRIGHT).clamp(0.0, 1.0);
    let number_color = Color32::from_rgb(
        (120.0 - 60.0 * lit) as u8,
        (120.0 + 110.0 * lit) as u8,
        90,
    );
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(format!("{:.0}", score.current.max(0.0)))
                .color(number_color)
                .size(34.0)
                .strong(),
        );
        ui.vertical(|ui| {
            ui.label(RichText::new("survival score").size(12.0).weak());
            ui.label(RichText::new(format!("high  {:.0}", score.high)).size(15.0));
        });
    });

    const LOOKBACK: usize = 300;
    const EPS_POP: f32 = 1.0;
    const EPS_ENERGY: f32 = 0.005;
    let pop = history.population.back().copied().unwrap_or(0.0);
    let pop_arr = trend_arrow(trend(&history.population, LOOKBACK, EPS_POP));
    let energy = history.avg_energy.back().copied().unwrap_or(0.0);
    let energy_arr = trend_arrow(trend(&history.avg_energy, LOOKBACK, EPS_ENERGY));
    ui.label(
        RichText::new(format!(
            "pop {pop:.0} {pop_arr}   energy {energy:.2} {energy_arr}"
        ))
        .size(11.0)
        .weak(),
    );

    ui.separator();
    render_difficulty(ui, score.difficulty);
}

fn difficulty_color(difficulty: f32) -> egui::Color32 {
    if difficulty < 0.34 {
        egui::Color32::from_rgb(70, 180, 90)
    } else if difficulty < 0.67 {
        egui::Color32::from_rgb(230, 150, 40)
    } else {
        egui::Color32::from_rgb(210, 65, 65)
    }
}

fn render_difficulty(ui: &mut egui::Ui, difficulty: f32) {
    use egui::{Color32, RichText};
    const BARS: i32 = 5;

    let frac = difficulty.clamp(0.0, 1.0);
    let color = difficulty_color(frac);
    let filled = (frac * BARS as f32).round() as i32;

    ui.label(RichText::new("DIFFICULTY").size(13.0).weak());
    ui.horizontal(|ui| {
        let (resp, painter) = ui.allocate_painter(egui::vec2(58.0, 34.0), egui::Sense::hover());
        let r = resp.rect;
        let slot = r.width() / BARS as f32;
        let bw = slot * 0.62;
        for i in 0..BARS {
            let h = r.height() * (0.32 + 0.68 * i as f32 / (BARS - 1) as f32);
            let x = r.left() + i as f32 * slot;
            let bar = egui::Rect::from_min_max(
                egui::pos2(x, r.bottom() - h),
                egui::pos2(x + bw, r.bottom()),
            );
            let c = if i < filled { color } else { Color32::from_gray(60) };
            painter.rect_filled(bar, 1.0, c);
        }
        resp.on_hover_text(
            "Difficulty — how scarce you've made the world (the regrowth slider). It scales \
             every point scored, so an easy world barely moves the score. Crank it up, then \
             keep the herd alive and pushing east, to drive your high score into the hundreds.",
        );
        ui.add_space(8.0);
        ui.label(RichText::new(format!("{:.0}%", frac * 100.0)).size(30.0).strong().color(color));
    });
}

struct Panel<'a> {
    title: &'a str,
    /// flex-basis — panels wrap to a new row when full
    width: f32,
    items: Vec<Item<'a>>,
}

impl<'a> Panel<'a> {
    const DEFAULT_WIDTH: f32 = 240.0;

    fn new(title: &'a str, items: Vec<Item<'a>>) -> Self {
        Self { title, width: Self::DEFAULT_WIDTH, items }
    }

    fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }
}

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

/// Top-bar graph toggles; each opens a floating egui `Window` (not a panel) so it never shrinks the viewport.
fn graphs_bar(
    mut contexts: EguiContexts,
    mut state: ResMut<UiState>,
    mut view_rect: ResMut<WorldViewRect>,
    history: Res<History>,
    elk: Query<&Elk>,
    event_log: Res<EventLog>,
    score: Res<crate::elk::Score>,
) -> Result {
    let ctx = contexts.ctx_mut()?;

    let top = egui::TopBottomPanel::top("graphs_bar").show(ctx, |ui| {
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

    // Write WorldViewRect for set_camera_viewport: top-bar bottom to window bottom, ignoring the dock.
    let screen = ctx.viewport_rect();
    let top_bottom = top.response.rect.bottom();
    view_rect.min = Vec2::new(screen.min.x, top_bottom);
    view_rect.size = Vec2::new(screen.width(), (screen.bottom() - top_bottom).max(0.0));

    if state.visible.contains(&Graph::SurvivalScore) {
        egui::Window::new(Graph::SurvivalScore.label())
            .default_size([340.0, 110.0])
            .collapsible(false)
            .show(ctx, |ui| render_score_gauge(ui, &score, &history));
    }

    for g in Graph::ALL {
        if state.visible.contains(&g) && matches!(g, Graph::Biomass | Graph::Abundance) {
            egui::Window::new(g.label())
                .default_size([360.0, 200.0])
                // top-bar toggle already shows/hides the window; collapse arrow is redundant
                .collapsible(false)
                .show(ctx, |ui| {
                    render_plot(ui, graph_plot(g, &history, ui.available_height()));
                });
        }
    }

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
        // ratio > 1: regrowth outpaces grazing — herds camp rather than migrate
        Graph::Abundance => PlotSpec {
            id: "abundance",
            height,
            series: vec![
                ("forage per elk", &history.abundance_per_elk),
                ("regrowth ÷ drain", &history.regrowth_drain_ratio),
            ],
        },
        Graph::Histogram => unreachable!("Histogram is rendered by render_histogram, not graph_plot"),
        Graph::SurvivalScore => unreachable!("SurvivalScore is rendered by render_score_gauge, not graph_plot"),
    }
}

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

fn render_event_list(ui: &mut egui::Ui, event_log: &EventLog) {
    if event_log.recent.is_empty() {
        ui.weak("no starvation events yet");
        return;
    }
    egui::ScrollArea::vertical().show(ui, |ui| {
        for event in event_log.recent.iter().rev().take(50) {
            let kind = match event.kind {
                crate::events::EventKind::Starved => "starved",
                crate::events::EventKind::Departed => "departed",
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

/// Bundled so `control_panel` stays under Bevy's 16-param system limit.
#[derive(bevy::ecs::system::SystemParam)]
struct WorldTunables<'w> {
    green_wave: ResMut<'w, GreenWave>,
    fertility: Option<ResMut<'w, Fertility>>,
    ab_params: ResMut<'w, AbundanceParams>,
}

#[allow(clippy::too_many_arguments)]
fn control_panel(
    mut contexts: EguiContexts,
    mut state: ResMut<UiState>,
    mut elk_params: ResMut<ElkParams>,
    mut ratio_controls: ResMut<RatioControls>,
    mut world: WorldTunables,
    mut time: ResMut<Time<Virtual>>,
    mut world_seed: ResMut<crate::worldgen::WorldSeed>,
    mut next_state: ResMut<NextState<crate::sim::Sim>>,
    herds: Res<Herds>,
    history: Res<History>,
    mut selection: SelectionParams,
    mut dock_settle_frames: Local<u32>,
) -> Result {
    // OS/WM resizes the window after creation; hold height to 25% for a few frames so the dock opens at the right size.
    const DOCK_SETTLE_FRAMES: u32 = 30;
    let target_height = contexts.ctx_mut()?.viewport_rect().height() * 0.25;
    let settling = *dock_settle_frames < DOCK_SETTLE_FRAMES;
    if settling {
        *dock_settle_frames += 1;
    }
    let mut panel = egui::TopBottomPanel::bottom("control_panel").resizable(true);
    panel = if settling {
        panel.min_height(target_height).max_height(target_height)
    } else {
        panel.min_height(120.0)
    };
    panel
        .show(contexts.ctx_mut()?, |ui| {
            {
                let elk = &selection.elk;
                selection.state.units.retain(|e| elk.get(*e).is_ok());
                if let Some(f) = selection.state.focused {
                    if elk.get(f).is_err() {
                        selection.state.focused = None;
                    }
                }
            }
            if selection.state.just_selected {
                state.tab = Tab::Selection;
                selection.state.just_selected = false;
            }
            let has_units = !selection.state.units.is_empty();
            if !has_units && state.tab == Tab::Selection {
                state.tab = Tab::Sliders;
            }
            ui.horizontal(|ui| {
                ui.selectable_value(&mut state.tab, Tab::Sliders, "Sliders");
                ui.selectable_value(&mut state.tab, Tab::Herds, "Herds");
                if has_units {
                    ui.selectable_value(&mut state.tab, Tab::Selection, "Selection");
                }
                ui.separator();
                speed_inline(ui, time.as_mut());
                ui.separator();
                if ui.button("⟳ regenerate").on_hover_text(format!("seed {:#018x}", world_seed.0)).clicked() {
                    world_seed.0 = rand::random();
                    next_state.set(crate::sim::Sim::Generating);
                }
            });
            ui.separator();
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| match state.tab {
                Tab::Sliders => {
                    // preset apply borrows resources before the per-slider reborrows below
                    ui.horizontal(|ui| {
                        ui.label("presets:");
                        for preset in &crate::elk::presets::PRESETS {
                            if ui.button(preset.name).on_hover_text(preset.description).clicked() {
                                crate::elk::presets::apply(
                                    preset,
                                    ratio_controls.as_mut(),
                                    world.green_wave.as_mut(),
                                    elk_params.as_mut(),
                                );
                            }
                        }
                    });
                    ui.separator();
                    let rc = ratio_controls.as_mut();
                    let grass_regrow = &mut rc.grass_regrow;
                    let shrub_regrow = &mut rc.shrub_regrow;
                    let feed_ratio = &mut rc.feed_ratio;
                    let cross_ratio = &mut rc.cross_ratio;
                    panel_flow(ui, vec![
                        Panel::new("Forage", grass_items(world.fertility.as_mut().map(|f| f.as_mut()), grass_regrow, shrub_regrow)),
                        Panel::new("Behaviour", vec![Item::Custom(Box::new(|ui| behaviour_tab(ui, elk_params.as_mut(), feed_ratio, cross_ratio)))]).width(300.0),
                        Panel::new("Abundance (measure)", abundance_items(world.ab_params.as_mut())),
                    ])
                }
                Tab::Herds => herds_view(ui, state.as_mut(), &herds, &history),
                Tab::Selection => selection_tab(ui, selection.state.as_mut(), &selection.elk),
            });
        });
    Ok(())
}

fn herds_view(ui: &mut egui::Ui, state: &mut UiState, herds: &Herds, history: &History) {
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
                Some((code, co)) => herd_details(ui, code, co, history),
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

fn herd_details(
    ui: &mut egui::Ui,
    code: u32,
    co: &crate::elk::Cohort,
    history: &History,
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

    render_items(ui, vec![
        Item::Label("population".to_string()),
        Item::Plot(PlotSpec {
            id: "pop_plot",
            height: 80.0,
            series: vec![("population", &history.population)],
        }),
    ]);
}

const THUMB: f32 = 58.0;
const PORTRAIT_H: f32 = 168.0;

/// Stale units are pruned by `control_panel` before this runs.
fn selection_tab(
    ui: &mut egui::Ui,
    sel: &mut UnitSelectState,
    elk: &Query<(Entity, &Elk, &Herding)>,
) {
    // Snapshot the order so the roster can iterate while clicks mutate `focused`.
    let units = sel.units.clone();
    let total = sel.captured_total;

    ui.columns(3, |cols| {
        let roster = &mut cols[0];
        roster.horizontal(|ui| {
            let header = if units.len() < total {
                format!("{} of {} selected", units.len(), total)
            } else {
                format!("{} selected", units.len())
            };
            ui.strong(header);
            if ui.button("clear").clicked() {
                sel.units.clear();
                sel.focused = None;
            }
        });
        roster.separator();
        egui::ScrollArea::vertical()
            .id_salt("unit_roster")
            .auto_shrink([false, false])
            .show(roster, |ui| {
            ui.horizontal_wrapped(|ui| {
            for &e in &units {
                let Ok((_, ec, _)) = elk.get(e) else { continue };
                let (rect, resp) = ui.allocate_exact_size(egui::vec2(THUMB, THUMB), egui::Sense::click());
                let painter = ui.painter_at(rect);
                draw_elk_silhouette(&painter, rect, herd_color32(ec.slot));
                let bar = egui::Rect::from_min_max(
                    egui::pos2(rect.left() + 3.0, rect.bottom() - 7.0),
                    egui::pos2(rect.right() - 3.0, rect.bottom() - 3.0),
                );
                painter.rect_filled(bar, 0.0, egui::Color32::from_gray(40));
                let fill = egui::Rect::from_min_max(
                    bar.min,
                    egui::pos2(bar.left() + bar.width() * ec.energy.clamp(0.0, 1.0), bar.bottom()),
                );
                painter.rect_filled(fill, 0.0, egui::Color32::from_rgb(120, 200, 120));
                painter.text(
                    rect.left_top() + egui::vec2(3.0, 2.0),
                    egui::Align2::LEFT_TOP,
                    format!("{:06x}", ec.code),
                    egui::FontId::monospace(9.0),
                    egui::Color32::WHITE,
                );
                if sel.focused == Some(e) {
                    painter.rect_stroke(
                        rect,
                        3.0,
                        egui::Stroke::new(2.0, egui::Color32::WHITE),
                        egui::StrokeKind::Inside,
                    );
                }
                if resp.clicked() {
                    sel.focused = Some(e);
                }
            }
            });
        });

        let stats = &mut cols[1];
        egui::ScrollArea::vertical()
            .id_salt("unit_stats")
            .auto_shrink([false, false])
            .show(stats, |ui| match sel.focused.and_then(|e| elk.get(e).ok()) {
            Some((e, ec, hc)) => {
                ui.heading(format!("elk {:06x}", ec.code));
                ui.horizontal(|ui| {
                    let idx = units.iter().position(|&x| x == e);
                    if ui.button("‹ prev").clicked() {
                        if let Some(i) = idx {
                            sel.focused = Some(units[(i + units.len() - 1) % units.len()]);
                        }
                    }
                    if ui.button("next ›").clicked() {
                        if let Some(i) = idx {
                            sel.focused = Some(units[(i + 1) % units.len()]);
                        }
                    }
                });
                ui.add(
                    egui::ProgressBar::new(ec.energy.clamp(0.0, 1.0))
                        .text(format!("energy {:.0}%", ec.energy * 100.0)),
                );
                ui.label(format!(
                    "{}  ·  {}",
                    if ec.grazing { "grazing" } else { "not grazing" },
                    if hc.is_traveling() { "traveling" } else { "foraging" },
                ));
                ui.label(format!("digesting {}", ec.digesting.len()));
                ui.label(format!("cell {}", ec.cell));
            }
            None => {
                ui.weak("select a unit");
            }
        });

        let portrait = &mut cols[2];
        let (rect, _) = portrait.allocate_exact_size(
            egui::vec2(portrait.available_width(), PORTRAIT_H),
            egui::Sense::hover(),
        );
        let painter = portrait.painter_at(rect);
        match sel.focused.and_then(|e| elk.get(e).ok()) {
            Some((_, ec, _)) => {
                draw_elk_silhouette(&painter, rect, herd_color32(ec.slot));
                painter.text(
                    rect.center_bottom() + egui::vec2(0.0, -8.0),
                    egui::Align2::CENTER_BOTTOM,
                    format!("{:06x}", ec.code),
                    egui::FontId::monospace(15.0),
                    egui::Color32::WHITE,
                );
            }
            None => {
                painter.rect_filled(rect, 4.0, egui::Color32::from_gray(24));
                painter.text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "no unit",
                    egui::FontId::proportional(12.0),
                    egui::Color32::from_gray(120),
                );
            }
        }
    });
}

/// World click → nearest herd's tab. Runs on release so box-select can claim drags; panel clicks are ignored.
pub(crate) fn pick_herd(
    mouse: Res<ButtonInput<MouseButton>>,
    mut contexts: EguiContexts,
    window: Single<&Window>,
    camera: Single<(&Camera, &GlobalTransform), With<WorldCamera>>,
    grid: Res<Grid>,
    elk: Query<(Entity, &Elk)>,
    mut state: ResMut<UiState>,
    sel: Res<UnitSelectState>,
) -> Result {
    if !mouse.just_released(MouseButton::Left) {
        return Ok(());
    }
    if sel.drag_was_box {
        return Ok(()); // the release closed a box drag, not a single pick
    }
    if contexts.ctx_mut()?.wants_pointer_input() {
        return Ok(()); // the click landed on the UI
    }
    let Some(cursor) = window.cursor_position() else {
        return Ok(());
    };
    // camera unprojection respects viewport clip; errors outside the viewport (→ Ok)
    let (cam, cam_transform) = *camera;
    let Ok(world) = cam.viewport_to_world_2d(cam_transform, cursor) else {
        return Ok(());
    };

    const PICK_RADIUS: f32 = 96.0;
    let tol2 = PICK_RADIUS * PICK_RADIUS;
    let mut best: Option<(Entity, u32, f32)> = None;
    for (entity, elk) in &elk {
        let d2 = (cell_world_pos(&grid, elk.cell) - world).length_squared();
        if d2 <= tol2 && best.is_none_or(|(_, _, b)| d2 < b) {
            best = Some((entity, elk.code, d2));
        }
    }
    if let Some((_entity, code, _)) = best {
        state.selected = Some(code);
        state.tab = Tab::Herds;
    }
    Ok(())
}

/// Convert egui logical rect → physical camera viewport. Returns `None` when the area collapses so the camera falls back to the full window.
fn world_viewport(min: Vec2, size: Vec2, scale: f32, target: UVec2) -> Option<Viewport> {
    let pos = (min * scale).max(Vec2::ZERO).as_uvec2();
    let px = (size * scale).max(Vec2::ZERO).as_uvec2();
    // wgpu crashes (not clips) on oversized scissor rects; clamp so pos + size never exceed target
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

/// Confine world camera to `WorldViewRect`. The dock is excluded from this rect, so resizing it never moves the map.
fn set_camera_viewport(
    view_rect: Res<WorldViewRect>,
    window: Single<&Window>,
    camera: Single<&mut Camera, With<WorldCamera>>,
) {
    let target = UVec2::new(window.physical_width(), window.physical_height());
    camera.into_inner().viewport = world_viewport(
        view_rect.min,
        view_rect.size,
        window.scale_factor(),
        target,
    );
}

/// Max virtual time `FixedUpdate` can advance per frame — caps steps/frame to prevent UI freezes at high speed.
const FRAME_SIM_BUDGET: Duration = Duration::from_millis(500);

/// Bevy's default raw-delta cap; `sim_max_delta` only ever tightens this.
const DEFAULT_MAX_DELTA: Duration = Duration::from_millis(250);

/// Returns max_delta so that max_delta × speed ≤ budget. Without this, Bevy's default 250 ms cap is useless at high multipliers and the UI freezes.
fn sim_max_delta(budget: Duration, speed: f32) -> Duration {
    budget.div_f32(speed.max(1.0)).min(DEFAULT_MAX_DELTA)
}

/// Every speed change goes through here to keep the per-frame budget in sync.
fn set_sim_speed(time: &mut Time<Virtual>, speed: f32) {
    time.set_relative_speed(speed);
    time.set_max_delta(sim_max_delta(FRAME_SIM_BUDGET, speed));
}

/// One-shot: `set_fonts` replaces the whole atlas, so re-running every frame would be wasteful.
fn install_icon_font(mut contexts: EguiContexts, mut installed: Local<bool>) -> Result {
    if *installed {
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?;
    let mut fonts = egui::FontDefinitions::default();
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
    ctx.set_fonts(fonts);
    *installed = true;
    Ok(())
}

/// Keyboard shortcuts (1–5 for speed, space for pause); suppressed while egui holds focus.
fn keyboard_speed(
    mut contexts: EguiContexts,
    keys: Res<ButtonInput<KeyCode>>,
    mut time: ResMut<Time<Virtual>>,
) -> Result {
    if contexts.ctx_mut()?.wants_keyboard_input() {
        return Ok(());
    }

    if keys.just_pressed(KeyCode::Space) {
        if time.is_paused() {
            time.unpause();
        } else {
            time.pause();
        }
    }

    const DIGITS: [KeyCode; 5] = [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
    ];
    for (i, key) in DIGITS.iter().enumerate() {
        if keys.just_pressed(*key) {
            // A speed key implies "run at this speed", so lift any pause first.
            if time.is_paused() {
                time.unpause();
            }
            set_sim_speed(time.as_mut(), SPEED_PRESETS[i].1);
        }
    }

    Ok(())
}

/// Index = digit-key shortcut; shared by `keyboard_speed` and `speed_inline`.
const SPEED_PRESETS: [(&str, f32); 5] =
    [("1x", 1.0), ("2x", 2.0), ("3x", 3.0), ("4x", 4.0), (">>", 64.0)];

/// Must match `Time::<Fixed>::from_hz(N)` in the binaries — ticks/s = FIXED_HZ × speed.
const FIXED_HZ: f32 = 10.0;

fn speed_inline(ui: &mut egui::Ui, time: &mut Time<Virtual>) {
    let current = time.relative_speed();
    let icon = if time.is_paused() {
        egui_phosphor::regular::PLAY
    } else {
        egui_phosphor::regular::PAUSE
    };
    if ui.button(icon).on_hover_text("toggle pause (space)").clicked() {
        if time.is_paused() {
            time.unpause();
        } else {
            time.pause();
        }
    }
    for (i, (label, mult)) in SPEED_PRESETS.iter().enumerate() {
        if ui
            .selectable_label(current == *mult, *label)
            .on_hover_text(format!("speed (key {})", i + 1))
            .clicked()
        {
            set_sim_speed(time, *mult);
        }
    }
    let mut custom = current;
    if ui
        .add(egui::DragValue::new(&mut custom).range(0.0..=256.0).speed(0.1))
        .changed()
    {
        set_sim_speed(time, custom);
    }
    let tps = if time.is_paused() { 0.0 } else { FIXED_HZ * current };
    ui.separator();
    ui.label(format!("{tps:.0} ticks/s")).on_hover_text("simulation ticks per second (10 Hz × speed)");
}

fn grass_items<'a>(fertility: Option<&'a mut Fertility>, grass_regrow: &'a mut f32, shrub_regrow: &'a mut f32) -> Vec<Item<'a>> {
    let mut items = vec![
        Item::Slider { value: grass_regrow, range: 0.0..=0.05, label: "grass regrowth / tick" },
        Item::Slider { value: shrub_regrow, range: 0.0..=0.02, label: "shrub regrowth / tick" },
    ];
    // Fertility resource is absent when DroppingsPlugin is omitted from the binary.
    if let Some(fertility) = fertility {
        items.push(Item::Section {
            title: "Fertility",
            items: vec![
                Item::Slider { value: &mut fertility.rate, range: 0.0..=0.5, label: "droppings → grass / tick" },
                Item::Slider { value: &mut fertility.efficiency, range: 0.0..=1.0, label: "conversion efficiency" },
            ],
            default_open: false,
        });
    }
    items
}

fn abundance_items(p: &mut AbundanceParams) -> Vec<Item<'_>> {
    vec![
        Item::Slider { value: &mut p.radius, range: 1.0..=20.0, label: "nearby radius (cells)" },
        Item::Slider { value: &mut p.energy_weight, range: 0.0..=1.0, label: "grass ↔ energy weight" },
    ]
}

fn behaviour_tab(ui: &mut egui::Ui, p: &mut ElkParams, feed_ratio: &mut f32, cross_ratio: &mut f32) {
    ui.label("forage perception");
    slider(ui, &mut p.grass_radius, 1.0..=12.0, "grass radius (cells)");
    slider(ui, &mut p.freshness_weight, 0.0..=6.0, "freshness weight (green-up front)");
    slider(ui, &mut p.sightline_range, 0.0..=32.0, "sightline range (cells)");
    slider(ui, &mut p.sightline_weight, 0.0..=5.0, "sightline weight (leapfrog)");
    ui.separator();
    ui.label("metabolism");
    slider(ui, feed_ratio, 0.5..=16.0, "ticks of life per bite (feed ratio)");
    // break-even: grazing (CHEW_TICKS+1)/feed_ratio of ticks sustains the herd
    let pct = if *feed_ratio > 0.0 {
        100.0 * (crate::elk::CHEW_TICKS + 1) as f32 / *feed_ratio
    } else {
        f32::INFINITY
    };
    ui.label(if pct > 100.0 {
        "break-even grazing: impossible — herds starve".to_string()
    } else {
        format!("break-even grazing: {pct:.0}% of ticks")
    });
    ui.separator();
    ui.label("crossing");
    slider(ui, &mut p.water_cost, 0.0..=4.0, "water crossing cost");
    slider(ui, &mut p.ford_discount, 0.0..=1.0, "ford discount (0 = free)");
    slider(ui, &mut p.swim_drain, 0.0..=0.05, "swim energy drain");
    slider(ui, &mut p.cross_peek, 1.0..=40.0, "cross peek (far-bank sight)");
    slider(ui, &mut p.swim_reluctance, 0.0..=2.0, "swim reluctance (decision cost)");
    // no longer steers movement — kept only as the score penalty
    slider(ui, cross_ratio, 0.0..=2.0, "crossing pull (score penalty)");
}

pub enum Trend {
    Up,
    Down,
    Flat,
}

pub fn trend(series: &std::collections::VecDeque<f32>, lookback: usize, eps: f32) -> Trend {
    if series.len() <= lookback {
        return Trend::Flat;
    }
    let latest = *series.back().unwrap();
    let back = series[series.len() - 1 - lookback];
    let delta = latest - back;
    if delta > eps {
        Trend::Up
    } else if delta < -eps {
        Trend::Down
    } else {
        Trend::Flat
    }
}

pub fn trend_arrow(t: Trend) -> &'static str {
    match t {
        Trend::Up => "^",
        Trend::Down => "v",
        Trend::Flat => "-",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fast_speeds_bound_scaled_advance_to_budget() {
        let budget = FRAME_SIM_BUDGET;
        for speed in [4.0_f32, 16.0, 64.0, 256.0] {
            let scaled = sim_max_delta(budget, speed).mul_f32(speed);
            // Allow a millisecond of rounding slack from the f32 divide/multiply.
            assert!(
                scaled <= budget + Duration::from_millis(1),
                "speed {speed}x scaled advance {scaled:?} exceeds budget {budget:?}",
            );
        }
    }

    #[test]
    fn max_delta_is_monotone_non_increasing_in_speed() {
        let budget = FRAME_SIM_BUDGET;
        let mut prev = sim_max_delta(budget, 1.0);
        for speed in [2.0_f32, 4.0, 8.0, 64.0, 256.0] {
            let next = sim_max_delta(budget, speed);
            assert!(next <= prev, "max_delta grew from {prev:?} to {next:?} at {speed}x");
            prev = next;
        }
    }

    #[test]
    fn slow_speeds_keep_engine_default() {
        assert_eq!(sim_max_delta(FRAME_SIM_BUDGET, 0.5), DEFAULT_MAX_DELTA);
        assert_eq!(sim_max_delta(FRAME_SIM_BUDGET, 1.0), DEFAULT_MAX_DELTA);
    }

    #[test]
    fn full_window_when_no_panel() {
        let vp = world_viewport(Vec2::ZERO, Vec2::new(1280.0, 960.0), 1.0, UVec2::new(1280, 960))
            .expect("non-empty viewport");
        assert_eq!(vp.physical_position, UVec2::ZERO);
        assert_eq!(vp.physical_size, UVec2::new(1280, 960));
    }

    #[test]
    fn bottom_dock_shrinks_height_only() {
        let vp = world_viewport(Vec2::ZERO, Vec2::new(1280.0, 720.0), 1.0, UVec2::new(1280, 960))
            .expect("non-empty viewport");
        assert_eq!(vp.physical_position, UVec2::ZERO);
        assert_eq!(vp.physical_size, UVec2::new(1280, 720));
    }

    // regression: scale must be applied once; double-counting crashed wgpu with an oversized scissor rect
    #[test]
    fn retina_scale_counts_once() {
        let vp = world_viewport(Vec2::ZERO, Vec2::new(640.0, 360.0), 2.0, UVec2::new(1280, 720))
            .expect("non-empty viewport");
        assert_eq!(vp.physical_size, UVec2::new(1280, 720));
    }

    #[test]
    fn oversize_rect_clamps_to_target() {
        let vp = world_viewport(Vec2::ZERO, Vec2::new(4000.0, 4000.0), 1.0, UVec2::new(1280, 720))
            .expect("non-empty viewport");
        assert_eq!(vp.physical_size, UVec2::new(1280, 720));
    }

    #[test]
    fn collapsed_area_yields_none() {
        assert!(world_viewport(Vec2::new(0.0, 960.0), Vec2::ZERO, 1.0, UVec2::new(1280, 960)).is_none());
    }

    fn deque(values: &[f32]) -> std::collections::VecDeque<f32> {
        values.iter().copied().collect()
    }

    #[test]
    fn trend_up_when_latest_exceeds_lookback() {
        let s = deque(&[1.0, 1.0, 1.0, 2.0]);
        assert!(matches!(trend(&s, 2, 0.05), Trend::Up));
    }

    #[test]
    fn trend_down_when_latest_below_lookback() {
        let s = deque(&[2.0, 2.0, 2.0, 1.0]);
        assert!(matches!(trend(&s, 2, 0.05), Trend::Down));
    }

    #[test]
    fn trend_flat_within_eps() {
        let s = deque(&[1.0, 1.01, 0.99, 1.02]);
        assert!(matches!(trend(&s, 2, 0.1), Trend::Flat));
    }

    #[test]
    fn trend_flat_when_series_too_short() {
        let s = deque(&[1.0, 2.0]);
        assert!(matches!(trend(&s, 5, 0.01), Trend::Flat));
    }

    #[test]
    fn trend_flat_on_empty_series() {
        let s = deque(&[]);
        assert!(matches!(trend(&s, 3, 0.01), Trend::Flat));
    }
}
