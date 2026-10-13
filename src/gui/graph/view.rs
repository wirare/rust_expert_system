//! Explanation-first, read-only proof canvas with a contained inspector.
//!
//! Unlike the old search graph, only final causal evidence is displayed.
//! Nodes never have drag handlers. Zoom and scroll affect the viewport only.

use std::collections::HashMap;

use eframe::egui;

use crate::report::ExecutionReport;

use super::details;
use super::layout::{GraphLayout, COLUMN_GAP, NODE_HEIGHT, NODE_WIDTH};
use super::model::{
    DisplayNodeId, ExplanationGraph, ExplanationLink, LinkKind, NodeKind,
    outcome_name,
};

struct CachedDiagram
{
    revision: u64,
    query: usize,
    graph: ExplanationGraph,
    layout: GraphLayout,
}

pub struct GraphViewer
{
    query_index: usize,
    selection: Option<DisplayNodeId>,
    cached: Option<CachedDiagram>,
    zoom: f32,
}

impl Default for GraphViewer
{
    fn default() -> Self
    {
        Self {
            query_index: 0,
            selection: None,
            cached: None,
            zoom: 0.9,
        }
    }
}

fn fill(kind: NodeKind) -> egui::Color32
{
    match kind
    {
        NodeKind::Answer => egui::Color32::from_rgb(24, 112, 123),
        NodeKind::Fact => egui::Color32::from_rgb(31, 118, 86),
        NodeKind::Rule => egui::Color32::from_rgb(57, 87, 155),
        NodeKind::Clause => egui::Color32::from_rgb(150, 103, 39),
        NodeKind::Derived => egui::Color32::from_rgb(103, 76, 163),
        NodeKind::Notice => egui::Color32::from_rgb(94, 98, 108),
    }
}

fn edge_color(link: LinkKind) -> egui::Color32
{
    match link
    {
        LinkKind::RulesOut => egui::Color32::from_rgb(236, 156, 91),
        LinkKind::Unresolved => egui::Color32::from_rgb(212, 177, 98),
        LinkKind::Answer => egui::Color32::from_rgb(95, 194, 195),
        _ => egui::Color32::from_rgb(147, 159, 176),
    }
}

fn wrap_lines(value: &str, width: usize, limit: usize) -> String
{
    let mut lines = Vec::<String>::new();
    let mut current = String::new();
    let mut remaining = value.split_whitespace().peekable();

    while let Some(word) = remaining.next()
    {
        let current_len = current.chars().count();
        let word_len = word.chars().count();
        if current_len > 0 && current_len + 1 + word_len > width
        {
            lines.push(current);
            current = String::new();
            if lines.len() == limit
            {
                let last = lines.last_mut().unwrap();
                if !last.ends_with('…')
                {
                    last.push('…');
                }
                return lines.join("\n");
            }
        }

        if !current.is_empty()
        {
            current.push(' ');
        }

        // Avoid rendering beyond the card even for expressions without
        // whitespace (e.g. an unusually long conjunction).
        if word_len > width
        {
            let clipped = word.chars().take(width.saturating_sub(1))
                .collect::<String>();
            current.push_str(&clipped);
            current.push('…');
            if remaining.peek().is_some()
            {
                lines.push(current);
                current = String::new();
                if lines.len() == limit
                {
                    return lines.join("\n");
                }
            }
        }
        else
        {
            current.push_str(word);
        }
    }

    if !current.is_empty()
    {
        lines.push(current);
    }
    if lines.is_empty()
    {
        return String::new();
    }
    lines.truncate(limit);
    lines.join("\n")
}

/// Route edges through free corridors instead of drawing across nodes.
/// Adjacent stages use a short elbow. Long-range dependencies go through
/// the reserved top gutter, so they cannot cross intermediate card text.
fn route_points(
    source: egui::Rect,
    target: egui::Rect,
    scene_top: f32,
    edge_index: usize,
    zoom: f32,
) -> Vec<egui::Pos2>
{
    let start = source.right_center();
    let end = target.left_center();
    let gap = end.x - start.x;
    let adjacent = gap > 10.0 * zoom
        && gap < COLUMN_GAP * zoom * 1.5;

    if adjacent
    {
        let shift = (edge_index % 5) as f32 - 2.0;
        let bend = ((start.x + end.x) * 0.5 + shift * 9.0 * zoom)
            .clamp(start.x + 12.0 * zoom, end.x - 12.0 * zoom);
        return vec![
            start,
            egui::pos2(bend, start.y),
            egui::pos2(bend, end.y),
            end,
        ];
    }

    // Skip-layer, same-column, and backwards connections are sent around
    // the cards instead of through their bounding rectangles.
    let departure = start.x + COLUMN_GAP * zoom * 0.32;
    let approach = end.x - COLUMN_GAP * zoom * 0.32;
    let lane_y = scene_top + (20.0 + 13.0 * (edge_index % 5) as f32) * zoom;
    vec![
        start,
        egui::pos2(departure, start.y),
        egui::pos2(departure, lane_y),
        egui::pos2(approach, lane_y),
        egui::pos2(approach, end.y),
        end,
    ]
}

fn draw_arrow(
    painter: &egui::Painter,
    source: egui::Rect,
    target: egui::Rect,
    link: &ExplanationLink,
    selected: bool,
    scene_top: f32,
    edge_index: usize,
    zoom: f32,
)
{
    let stroke = egui::Stroke::new(
        if selected { 2.5 } else { 1.35 },
        if selected
        {
            egui::Color32::from_rgb(248, 206, 105)
        }
        else
        {
            edge_color(link.kind)
        },
    );

    let points = route_points(source, target, scene_top, edge_index, zoom);

    if link.kind == LinkKind::Unresolved
    {
        // Dashed wires mark ambiguity; they never count as proof edges.
        for segment in points.windows(2)
        {
            let delta = segment[1] - segment[0];
            let length = delta.length();
            if length <= 0.1
            {
                continue;
            }
            let direction = delta / length;
            let mut offset = 0.0;
            while offset < length
            {
                let finish = (offset + 6.0).min(length);
                painter.line_segment(
                    [
                        segment[0] + direction * offset,
                        segment[0] + direction * finish,
                    ],
                    stroke,
                );
                offset += 11.0;
            }
        }
    }
    else
    {
        painter.add(egui::Shape::line(points.clone(), stroke));
    }

    let end = *points.last().unwrap();
    let previous = points[points.len() - 2];
    let back = end - previous;
    if back.length_sq() > 0.001
    {
        let direction = back.normalized();
        let normal = egui::vec2(-direction.y, direction.x);
        painter.line_segment(
            [end - direction * 11.0 + normal * 5.0, end],
            stroke,
        );
        painter.line_segment(
            [end - direction * 11.0 - normal * 5.0, end],
            stroke,
        );
    }

    // Edge labels intentionally live in the inspector, which has a bounded
    // width. Floating them over the narrow routing corridors previously
    // hid arrows and even obscured text inside nearby cards.
}

fn draw_card(
    painter: &egui::Painter,
    rect: egui::Rect,
    node: &super::model::ExplanationNode,
    zoom: f32,
    selected: bool,
    hovered: bool,
)
{
    let border = if selected
    {
        egui::Stroke::new(3.1, egui::Color32::from_rgb(251, 204, 95))
    }
    else if hovered
    {
        egui::Stroke::new(2.1, egui::Color32::WHITE)
    }
    else
    {
        egui::Stroke::new(1.0, egui::Color32::from_rgb(137, 148, 167))
    };

    painter.rect_filled(rect, 10.0 * zoom, fill(node.kind));
    painter.rect_stroke(
        rect, 10.0 * zoom, border, egui::StrokeKind::Inside,
    );

    let left = rect.left() + 14.0 * zoom;
    let top = rect.top() + 10.0 * zoom;

    painter.text(
        egui::pos2(left, top),
        egui::Align2::LEFT_TOP,
        node.kind.name(),
        egui::FontId::proportional(10.5 * zoom),
        egui::Color32::from_rgb(219, 231, 238),
    );

    painter.text(
        egui::pos2(left, top + 23.0 * zoom),
        egui::Align2::LEFT_TOP,
        wrap_lines(&node.title, 32, 2),
        egui::FontId::proportional(16.0 * zoom),
        egui::Color32::WHITE,
    );

    painter.text(
        egui::pos2(left, top + 89.0 * zoom),
        egui::Align2::LEFT_TOP,
        wrap_lines(&node.subtitle, 44, 2),
        egui::FontId::proportional(11.0 * zoom),
        egui::Color32::from_rgb(224, 230, 234),
    );
}

fn canvas(
    ui: &mut egui::Ui,
    graph: &ExplanationGraph,
    layout: &GraphLayout,
    selection: &mut Option<DisplayNodeId>,
    zoom: f32,
    scroll_revision: (u64, usize),
)
{
    let available = ui.available_size();
    let width = available.x.max(200.0);
    let height = available.y.max(170.0);

    egui::Frame::group(ui.style()).show(ui, |ui|
    {
        egui::ScrollArea::both()
            .id_salt(("explanation_canvas", scroll_revision))
            .auto_shrink([false, false])
            .max_width(width)
            .max_height(height)
            .scroll_bar_visibility(
                egui::containers::scroll_area::ScrollBarVisibility::AlwaysVisible
            )
            .show(ui, |ui|
            {
                let canvas_size = egui::vec2(
                    (layout.width * zoom).max(width - 20.0),
                    (layout.height * zoom).max(height - 20.0),
                );
                let (response, painter) = ui.allocate_painter(
                    canvas_size, egui::Sense::hover(),
                );
                let origin = response.rect.min;

                let positions = layout.nodes.iter().map(|node|
                {
                    let position = origin + egui::vec2(
                        node.x * zoom,
                        node.y * zoom,
                    );
                    (
                        node.id,
                        egui::Rect::from_min_size(
                            position,
                            egui::vec2(NODE_WIDTH * zoom, NODE_HEIGHT * zoom),
                        ),
                    )
                }).collect::<HashMap<_, _>>();

                // Draw dependencies behind nodes and visually emphasize
                // only connections touching the selected proof step.
                for (edge_index, link) in graph.links.iter().enumerate()
                {
                    if let (Some(from), Some(to)) =
                        (positions.get(&link.from), positions.get(&link.to))
                    {
                        draw_arrow(
                            &painter,
                            *from,
                            *to,
                            link,
                            selection.is_some_and(|id|
                            {
                                id == link.from || id == link.to
                            }),
                            origin.y,
                            edge_index,
                            zoom,
                        );
                    }
                }

                for node in &graph.nodes
                {
                    let Some(rect) = positions.get(&node.id).copied() else
                    {
                        continue;
                    };
                    if !ui.is_rect_visible(rect)
                    {
                        continue;
                    }

                    // Click-only input: dragging is intentionally unavailable.
                    let hit = ui.interact(
                        rect,
                        ui.id().with(("explanation_node", node.id)),
                        egui::Sense::click(),
                    );
                    if hit.clicked()
                    {
                        *selection = Some(node.id);
                    }

                    draw_card(
                        &painter,
                        rect,
                        node,
                        zoom,
                        *selection == Some(node.id),
                        hit.hovered(),
                    );

                    hit.on_hover_text(format!(
                        "{}\n{}\nClick for a full explanation",
                        node.title, node.subtitle,
                    ));
                }
            });
    });
}

fn legend(ui: &mut egui::Ui)
{
    ui.horizontal_wrapped(|ui|
    {
        ui.small("Read left → right: evidence → rule → deduction → answer.");
        ui.separator();
        for kind in [
            NodeKind::Fact, NodeKind::Rule, NodeKind::Clause,
            NodeKind::Derived, NodeKind::Answer, NodeKind::Notice,
        ]
        {
            ui.colored_label(fill(kind), "●");
            ui.small(kind.name());
        }
    });
}

impl GraphViewer
{
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        report: &ExecutionReport,
        revision: u64,
    )
    {
        if report.queries.is_empty()
        {
            ui.weak("No queries were evaluated.");
            return;
        }

        self.query_index = self.query_index.min(report.queries.len() - 1);
        let mut next_query = self.query_index;

        ui.horizontal_wrapped(|ui|
        {
            ui.heading("Why this result?");
            ui.separator();
            egui::ComboBox::from_id_salt("explanation_query_selector")
                .selected_text(format!(
                    "Query {} · {}",
                    report.queries[next_query].symbol,
                    outcome_name(&report.queries[next_query].outcome)
                ))
                .show_ui(ui, |ui|
                {
                    for (index, query) in report.queries.iter().enumerate()
                    {
                        ui.selectable_value(
                            &mut next_query,
                            index,
                            format!(
                                "{} — {}",
                                query.symbol,
                                outcome_name(&query.outcome)
                            ),
                        );
                    }
                });

            ui.separator();
            ui.label("Zoom");
            if ui.small_button("−").clicked()
            {
                self.zoom = (self.zoom - 0.10).max(0.70);
            }
            ui.add(egui::Slider::new(&mut self.zoom, 0.70..=1.45)
                .show_value(false));
            if ui.small_button("+").clicked()
            {
                self.zoom = (self.zoom + 0.10).min(1.45);
            }
            if ui.small_button("100%").clicked()
            {
                self.zoom = 1.0;
            }
        });

        if next_query != self.query_index
        {
            self.query_index = next_query;
            self.cached = None;
            self.selection = None;
        }

        if self.cached.as_ref().is_none_or(|cache|
        {
            cache.revision != revision || cache.query != self.query_index
        })
        {
            let graph = ExplanationGraph::new(
                report, &report.queries[self.query_index],
            );
            let layout = GraphLayout::new(&graph);
            self.selection = Some(graph.root);
            self.cached = Some(CachedDiagram {
                revision,
                query: self.query_index,
                graph,
                layout,
            });
        }

        let cache = self.cached.as_ref().unwrap();
        let query = &report.queries[self.query_index];

        legend(ui);
        ui.small(format!(
            "Focused explanation: {} steps, {} connections · {} trace events recorded. Click a card for details.",
            cache.graph.nodes.len(),
            cache.graph.links.len(),
            query.trace.events.len(),
        ));

        if cache.graph.limited
        {
            ui.colored_label(
                egui::Color32::from_rgb(219, 164, 95),
                "This proof is very large: the canvas shows the first 280 causal steps. The complete trace remains available.",
            );
        }

        ui.separator();

        // Use an actual constrained side panel. The former allocate_ui()
        // arrangement let long Debug strings increase the inspector width
        // and render off the right edge of the window.
        egui::Panel::right("proof_explanation_inspector")
            .resizable(true)
            .default_size(370.0)
            .min_size(285.0)
            .show(ui, |ui|
            {
                details::show(
                    ui,
                    report,
                    query,
                    &cache.graph,
                    &mut self.selection,
                );
            });

        canvas(
            ui,
            &cache.graph,
            &cache.layout,
            &mut self.selection,
            self.zoom,
            (revision, self.query_index),
        );
    }
}

#[cfg(test)]
mod tests
{
    use super::*;

    #[test]
    fn text_wrapping_is_bounded_and_unicode_safe()
    {
        assert_eq!(wrap_lines("A => B", 30, 2), "A => B");
        let long = wrap_lines(
            "A + B + C + D + E + F + G + H + I + J => K + L", 15, 2,
        );
        assert!(long.lines().count() <= 2);
        assert!(long.contains('…'));
        assert!(wrap_lines("Réécriture extrêmement longue", 12, 2)
            .lines().count() <= 2);
    }

    #[test]
    fn long_range_wire_uses_top_gutter_instead_of_crossing_cards()
    {
        let source = egui::Rect::from_min_size(
            egui::pos2(80.0, 200.0),
            egui::vec2(NODE_WIDTH, NODE_HEIGHT),
        );
        let target = egui::Rect::from_min_size(
            egui::pos2(80.0 + 3.0 * (NODE_WIDTH + COLUMN_GAP), 420.0),
            egui::vec2(NODE_WIDTH, NODE_HEIGHT),
        );
        let points = route_points(source, target, 50.0, 0, 1.0);
        assert_eq!(points.first().copied(), Some(source.right_center()));
        assert_eq!(points.last().copied(), Some(target.left_center()));
        assert!(points.len() > 4);
        assert!(points.iter().any(|point| point.y < source.top()));
        assert!(points.iter().any(|point| point.y < target.top()));
    }

    #[test]
    fn neighboring_stages_route_through_the_column_gap()
    {
        let source = egui::Rect::from_min_size(
            egui::pos2(80.0, 210.0),
            egui::vec2(NODE_WIDTH, NODE_HEIGHT),
        );
        let target = egui::Rect::from_min_size(
            egui::pos2(80.0 + NODE_WIDTH + COLUMN_GAP, 410.0),
            egui::vec2(NODE_WIDTH, NODE_HEIGHT),
        );
        let points = route_points(source, target, 0.0, 2, 1.0);
        assert_eq!(points.len(), 4);
        assert!(points[1].x > source.right());
        assert!(points[1].x < target.left());
        assert_eq!(points[1].x, points[2].x);
    }

    #[test]
    fn viewer_is_not_initially_bound_to_old_query_results()
    {
        let viewer = GraphViewer::default();
        assert!(viewer.cached.is_none());
        assert!(viewer.selection.is_none());
    }
}
