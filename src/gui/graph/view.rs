//! Static egui canvas for reasoning graphs. Nodes support click/inspect,
//! but have no drag, edit, or reposition interaction.

use std::collections::HashMap;

use eframe::egui;

use crate::report::{
    EdgeKind, ExecutionReport, GraphNode, GraphNodeKind, NodeId, QueryExecution, QueryOutcome,
    ReasoningGraph,
};

use super::details;
use super::layout::{GraphLayout, NODE_HEIGHT, NODE_WIDTH};

const QUERY_NODE: NodeId = 5_000_000_000;

struct CachedLayout
{
    revision: u64,
    query: char,
    layout: GraphLayout,
}

pub struct GraphViewer
{
    query_index: usize,
    selected_node: Option<NodeId>,
    layout: Option<CachedLayout>,
    zoom: f32,
    show_search_links: bool,
}

impl Default for GraphViewer
{
    fn default() -> Self
    {
        Self {
            query_index: 0,
            selected_node: None,
            layout: None,
            zoom: 0.90,
            show_search_links: false,
        }
    }
}

fn category_fill(kind: &GraphNodeKind) -> egui::Color32
{
    match kind
    {
        GraphNodeKind::InitialFact { .. } => egui::Color32::from_rgb(43, 113, 85),
        GraphNodeKind::Rule { .. } => egui::Color32::from_rgb(49, 91, 147),
        GraphNodeKind::Constraint { .. } => egui::Color32::from_rgb(144, 99, 41),
        GraphNodeKind::Deduction { .. } => egui::Color32::from_rgb(91, 69, 148),
        GraphNodeKind::Query { .. } => egui::Color32::from_rgb(39, 110, 126),
        GraphNodeKind::SearchGoal { .. } => egui::Color32::from_rgb(77, 83, 97),
        _ => egui::Color32::from_rgb(83, 83, 97),
    }
}

fn edge_is_evidence(kind: EdgeKind) -> bool
{
    matches!(
        kind,
        EdgeKind::Justification
            | EdgeKind::PremiseDependency
            | EdgeKind::ConstraintActivation
            | EdgeKind::UnitPropagation
    )
}

fn query_status(outcome: &QueryOutcome) -> String
{
    match outcome
    {
        QueryOutcome::Value(value) => format!("{value:?}"),
        QueryOutcome::Error { .. } => "Error".to_owned(),
    }
}

fn short_label(label: &str) -> String
{
    const MAX_CHARS: usize = 21;
    let mut chars = label.chars();
    let text = chars.by_ref().take(MAX_CHARS).collect::<String>();
    if chars.next().is_some()
    {
        format!("{text}…")
    }
    else
    {
        text
    }
}

fn draw_arrow(
    painter: &egui::Painter,
    source: egui::Rect,
    target: egui::Rect,
    stroke: egui::Stroke,
)
{
    let (start, end, horizontal) = if source.center().x + 25.0 < target.center().x
    {
        (source.right_center(), target.left_center(), true)
    }
    else if target.center().x + 25.0 < source.center().x
    {
        (source.left_center(), target.right_center(), true)
    }
    else if source.center().y < target.center().y
    {
        (source.center_bottom(), target.center_top(), false)
    }
    else
    {
        (source.center_top(), target.center_bottom(), false)
    };

    let points = if horizontal
    {
        let halfway = (start.x + end.x) * 0.5;
        vec![
            start,
            egui::pos2(halfway, start.y),
            egui::pos2(halfway, end.y),
            end,
        ]
    }
    else
    {
        let halfway = (start.y + end.y) * 0.5;
        vec![
            start,
            egui::pos2(start.x, halfway),
            egui::pos2(end.x, halfway),
            end,
        ]
    };

    painter.add(egui::Shape::line(points.clone(), stroke));

    let previous = points[points.len() - 2];
    let displacement = end - previous;
    if displacement.length_sq() > 0.1
    {
        let direction = displacement.normalized();
        let perpendicular = egui::vec2(-direction.y, direction.x);
        let left = end - direction * 9.0 + perpendicular * 4.0;
        let right = end - direction * 9.0 - perpendicular * 4.0;
        painter.line_segment([left, end], stroke);
        painter.line_segment([right, end], stroke);
    }
}

fn draw_canvas(
    ui: &mut egui::Ui,
    data: &ReasoningGraph,
    layout: &GraphLayout,
    selected_node: &mut Option<NodeId>,
    zoom: f32,
    show_search_links: bool,
    query_symbol: char,
)
{
    let canvas_size = egui::vec2(
        (layout.width * zoom).max(ui.available_width()),
        (layout.height * zoom).max(ui.available_height()),
    );

    egui::Frame::group(ui.style()).show(ui, |ui|
    {
        egui::ScrollArea::both()
            .id_salt("read_only_reasoning_canvas")
            .auto_shrink([false, false])
            .scroll_bar_visibility(
                egui::containers::scroll_area::ScrollBarVisibility::AlwaysVisible
            )
            .show(ui, |ui|
            {
                let (allocated, painter) = ui.allocate_painter(
                    canvas_size,
                    egui::Sense::hover(),
                );
                let origin = allocated.rect.min;
                let node_by_id: HashMap<NodeId, &GraphNode> =
                    data.nodes.iter().map(|node| (node.id, node)).collect();

                let node_rects: HashMap<NodeId, egui::Rect> = layout.nodes.iter()
                    .map(|position|
                    {
                        let min = origin + egui::vec2(
                            position.x * zoom,
                            position.y * zoom,
                        );
                        let rect = egui::Rect::from_min_size(
                            min,
                            egui::vec2(NODE_WIDTH * zoom, NODE_HEIGHT * zoom),
                        );
                        (position.id, rect)
                    })
                    .collect();

                // Edges must render first so they cannot obscure clickable
                // nodes. Search/return paths are optional for readability.
                for edge in &data.edges
                {
                    if !show_search_links && !edge_is_evidence(edge.kind)
                    {
                        continue;
                    }

                    let (Some(&start), Some(&end)) =
                        (node_rects.get(&edge.from), node_rects.get(&edge.to))
                    else
                    {
                        continue;
                    };

                    let selected = selected_node.is_some_and(|id|
                    {
                        edge.from == id || edge.to == id
                    });
                    let color = if selected
                    {
                        egui::Color32::from_rgb(245, 186, 91)
                    }
                    else
                    {
                        ui.visuals().weak_text_color().gamma_multiply(0.60)
                    };

                    let stroke = egui::Stroke::new(
                        if selected { 2.2 } else { 1.0 },
                        color,
                    );
                    draw_arrow(&painter, start, end, stroke);
                }

                for node in &layout.nodes
                {
                    let Some(&rect) = node_rects.get(&node.id) else
                    {
                        continue;
                    };
                    if !ui.is_rect_visible(rect)
                    {
                        continue;
                    }
                    let Some(record) = node_by_id.get(&node.id) else
                    {
                        continue;
                    };

                    let hit = ui.interact(
                        rect,
                        ui.id().with(("graph_node", query_symbol, node.id)),
                        egui::Sense::click(),
                    );
                    if hit.clicked()
                    {
                        *selected_node = Some(node.id);
                    }

                    let is_selected = *selected_node == Some(node.id);
                    let fill = category_fill(&record.kind);
                    let border = if is_selected
                    {
                        egui::Stroke::new(
                            3.0,
                            egui::Color32::from_rgb(245, 186, 91),
                        )
                    }
                    else if hit.hovered()
                    {
                        egui::Stroke::new(2.0, egui::Color32::WHITE)
                    }
                    else
                    {
                        egui::Stroke::new(
                            1.0,
                            egui::Color32::from_rgb(153, 156, 166),
                        )
                    };

                    painter.rect_filled(rect, 8.0, fill);
                    painter.rect_stroke(
                        rect, 8.0, border, egui::StrokeKind::Outside,
                    );

                    let label = short_label(&record.label);
                    painter.text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        label,
                        egui::FontId::proportional((14.0 * zoom).max(10.0)),
                        egui::Color32::WHITE,
                    );
                    hit.on_hover_text(&record.label);
                }
            });
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
            ui.weak("The current run contains no queries.");
            return;
        }

        self.query_index = self.query_index.min(report.queries.len() - 1);
        let mut chosen = self.query_index;

        ui.horizontal_wrapped(|ui|
        {
            ui.heading("Reasoning graph");
            ui.separator();
            ui.label("Query");

            egui::ComboBox::from_id_salt("graph_query_selector")
                .selected_text(
                    format!(
                        "{} — {}",
                        report.queries[chosen].symbol,
                        query_status(&report.queries[chosen].outcome)
                    )
                )
                .show_ui(ui, |ui|
                {
                    for (index, query) in report.queries.iter().enumerate()
                    {
                        ui.selectable_value(
                            &mut chosen, index,
                            format!("{} — {}", query.symbol, query_status(&query.outcome)),
                        );
                    }
                });

            ui.separator();

            if ui.small_button("−").clicked()
            {
                self.zoom = (self.zoom - 0.15).max(0.40);
            }
            ui.add(
                egui::Slider::new(&mut self.zoom, 0.40..=1.60)
                    .text("Zoom")
                    .fixed_decimals(2)
            );
            if ui.small_button("+").clicked()
            {
                self.zoom = (self.zoom + 0.15).min(1.60);
            }
            ui.checkbox(&mut self.show_search_links, "Search links");
        });

        if chosen != self.query_index
        {
            self.query_index = chosen;
            self.layout = None;
            self.selected_node = None;
        }

        let query = &report.queries[self.query_index];
        if self.layout.as_ref().is_none_or(|cache|
            cache.revision != revision || cache.query != query.symbol)
        {
            self.layout = Some(CachedLayout {
                revision,
                query: query.symbol,
                layout: GraphLayout::new(&query.graph),
            });
            self.selected_node = query.graph.nodes.iter()
                .find(|node| matches!(node.kind, GraphNodeKind::Query { .. }))
                .map(|node| node.id)
                .or(Some(QUERY_NODE));
        }

        let layout = &self.layout.as_ref().unwrap().layout;
        ui.small(format!(
            "{} nodes · {} connections · Click a node to inspect · Layout is fixed",
            query.graph.nodes.len(),
            query.graph.edges.len(),
        ));
        ui.separator();

        // The inspector is right-aligned on desktop widths and below the
        // graph on smaller windows. Only the viewport scrolls; nodes do not
        // support dragging or editing.
        let available = ui.available_size();
        if available.x >= 920.0
        {
            let inspect_width = 310.0;
            ui.horizontal(|ui|
            {
                ui.allocate_ui(
                    egui::vec2((available.x - inspect_width - 14.0).max(300.0), available.y),
                    |ui|
                    {
                        draw_canvas(
                            ui,
                            &query.graph,
                            layout,
                            &mut self.selected_node,
                            self.zoom,
                            self.show_search_links,
                            query.symbol,
                        );
                    },
                );
                ui.separator();
                ui.allocate_ui(
                    egui::vec2(inspect_width, available.y),
                    |ui|
                    {
                        details::show(
                            ui, report, query, &mut self.selected_node,
                        );
                    },
                );
            });
        }
        else
        {
            let canvas_height = (available.y * 0.63).max(210.0);
            ui.allocate_ui(
                egui::vec2(available.x, canvas_height),
                |ui|
                {
                    draw_canvas(
                        ui,
                        &query.graph,
                        layout,
                        &mut self.selected_node,
                        self.zoom,
                        self.show_search_links,
                        query.symbol,
                    );
                },
            );
            ui.separator();
            details::show(ui, report, query, &mut self.selected_node);
        }
    }
}

#[cfg(test)]
mod tests
{
    use super::*;

    #[test]
    fn empty_viewer_has_no_selection()
    {
        let viewer = GraphViewer::default();
        assert_eq!(viewer.selected_node, None);
        assert_eq!(viewer.query_index, 0);
    }

    #[test]
    fn label_truncation_is_unicode_safe()
    {
        assert_eq!(short_label("A"), "A");
        assert!(short_label("Réécriture très très longue").ends_with('…'));
    }
}
