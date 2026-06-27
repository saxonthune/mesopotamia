//! Declarative UI vocabulary over egui.
//!
//! Three tiers: a **Surface** is a top-level overlay region (a docked panel or a floating
//! `window`); a **Group** is a bounded cluster of primitives (the "div"); the primitives
//! themselves are the methods on `Group`. The method set is the closed vocabulary — there is
//! deliberately no reified widget tree (an `Item` enum), because nothing consumes the UI as data
//! and egui is immediate-mode. Adding a backend or serialization later would promote these methods
//! into data; until then the builder keeps declaration simple without the lifetime cost of a tree.

use std::ops::RangeInclusive;

use bevy_egui::egui;

/// A bounded cluster of primitives — the "div". Built imperatively against the themed vocabulary.
pub struct Group<'u> {
    ui: &'u mut egui::Ui,
}

impl<'u> Group<'u> {
    pub fn new(ui: &'u mut egui::Ui) -> Self {
        Self { ui }
    }

    pub fn slider(&mut self, value: &mut f32, range: RangeInclusive<f32>, label: &str) {
        self.ui.add(egui::Slider::new(value, range).text(label));
    }

    pub fn label(&mut self, text: impl Into<egui::WidgetText>) {
        self.ui.label(text);
    }

    pub fn separator(&mut self) {
        self.ui.separator();
    }

    pub fn plot(&mut self, spec: PlotSpec) {
        render_plot(self.ui, spec);
    }

    /// Collapsible nested group.
    pub fn section(&mut self, title: &str, default_open: bool, build: impl FnOnce(&mut Group)) {
        egui::CollapsingHeader::new(title)
            .default_open(default_open)
            .show(self.ui, |ui| build(&mut Group::new(ui)));
    }

    /// Escape hatch for one-off egui the vocabulary doesn't cover. Keep these rare — a recurring
    /// `ui()` call is a sign the vocabulary should grow a method.
    pub fn ui(&mut self, build: impl FnOnce(&mut egui::Ui)) {
        build(self.ui);
    }
}

/// A titled, framed group with a flex-basis width, for `group_row`.
pub struct TitledGroup<'a> {
    title: &'a str,
    width: f32,
    build: Box<dyn FnOnce(&mut Group) + 'a>,
}

impl<'a> TitledGroup<'a> {
    pub const DEFAULT_WIDTH: f32 = 240.0;

    pub fn new(title: &'a str, build: impl FnOnce(&mut Group) + 'a) -> Self {
        Self { title, width: Self::DEFAULT_WIDTH, build: Box::new(build) }
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }
}

/// Flex-wrapped row of titled, framed groups — wraps to a new line when the row fills.
pub fn group_row(ui: &mut egui::Ui, groups: Vec<TitledGroup>) {
    ui.horizontal_wrapped(|ui| {
        for g in groups {
            ui.allocate_ui_with_layout(
                egui::vec2(g.width, ui.available_height()),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        ui.set_width(g.width);
                        ui.heading(g.title);
                        (g.build)(&mut Group::new(ui));
                    });
                },
            );
        }
    });
}

/// A floating overlay window. A screen-space surface — it never reserves world-camera viewport.
pub fn window(ctx: &egui::Context, title: &str, default_size: [f32; 2], add: impl FnOnce(&mut egui::Ui)) {
    egui::Window::new(title)
        .default_size(default_size)
        // the caller's toggle shows/hides the window; the collapse arrow would be redundant
        .collapsible(false)
        .show(ctx, add);
}

/// A line plot of one or more named series, locked to auto-fit (no pan/zoom). Series are owned —
/// callers hand over `MetricLog::column` results directly without keeping a binding alive.
pub struct PlotSpec<'a> {
    /// must be unique within the surface (egui_plot requirement)
    pub id: &'a str,
    pub height: f32,
    pub series: Vec<(&'a str, Vec<f32>)>,
}

pub fn render_plot(ui: &mut egui::Ui, spec: PlotSpec) {
    use egui_plot::{Line, Plot, PlotPoints};
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
