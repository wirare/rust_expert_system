//! Obstacle-aware orthogonal routing for static proof graphs.
//!
//! A rule should point directly to the clause it activated. For vertically
//! separated nodes, follow the horizontal lane beside the source and enter
//! the destination from its top/bottom (the intended hand-drawn style).
//! Use a detour outside the cards only when that direct route is obstructed.

use std::collections::HashMap;

use eframe::egui;

use super::model::DisplayNodeId;

const CLEARANCE: f32 = 8.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryPort
{
    Left,
    Top,
    Bottom,
}

#[derive(Debug, Clone)]
pub struct RoutedEdge
{
    pub points: Vec<egui::Pos2>,
    pub entry: EntryPort,
}

fn segment_crosses(rect: egui::Rect, start: egui::Pos2, end: egui::Pos2) -> bool
{
    const EPS: f32 = 0.01;
    if (start.y - end.y).abs() < EPS
    {
        let xmin = start.x.min(end.x);
        let xmax = start.x.max(end.x);
        start.y >= rect.top() && start.y <= rect.bottom()
            && xmax >= rect.left() && xmin <= rect.right()
    }
    else if (start.x - end.x).abs() < EPS
    {
        let ymin = start.y.min(end.y);
        let ymax = start.y.max(end.y);
        start.x >= rect.left() && start.x <= rect.right()
            && ymax >= rect.top() && ymin <= rect.bottom()
    }
    else
    {
        // Every candidate is Manhattan. A nonorthogonal segment is invalid.
        true
    }
}

fn obstructed(
    points: &[egui::Pos2],
    rectangles: &HashMap<DisplayNodeId, egui::Rect>,
    source_id: DisplayNodeId,
    target_id: DisplayNodeId,
    clearance: f32,
) -> bool
{
    rectangles.iter()
        .filter(|(id, _)| **id != source_id && **id != target_id)
        .any(|(_, rect)|
        {
            let padded = rect.expand(clearance);
            points.windows(2).any(|segment|
                segment_crosses(padded, segment[0], segment[1])
            )
        })
}

fn length(points: &[egui::Pos2]) -> f32
{
    points.windows(2).map(|pair|
    {
        (pair[1].x - pair[0].x).abs()
            + (pair[1].y - pair[0].y).abs()
    }).sum()
}

/// Produce only axis-aligned segments and keep the arrow tip on the exact
/// target rectangle (no fixed global gutter shared by every long edge).
pub fn route(
    source_id: DisplayNodeId,
    target_id: DisplayNodeId,
    rectangles: &HashMap<DisplayNodeId, egui::Rect>,
    edge_index: usize,
    zoom: f32,
) -> Option<RoutedEdge>
{
    let source = *rectangles.get(&source_id)?;
    let target = *rectangles.get(&target_id)?;
    let start = source.right_center();
    let clearance = CLEARANCE * zoom;

    // The exact example in the screenshot:
    // top rule -> far-away OR clause: horizontal at the source's y, then
    // vertical down into the clause's top edge.
    if source.bottom() + clearance < target.top()
    {
        let end = target.center_top();
        let candidate = vec![start, egui::pos2(end.x, start.y), end];
        if !obstructed(&candidate, rectangles, source_id, target_id, clearance)
        {
            return Some(RoutedEdge {
                points: candidate,
                entry: EntryPort::Top,
            });
        }
    }

    // bottom rule -> earlier OR clause: horizontal at source level, then
    // vertical up into the clause's bottom edge.
    if source.top() > target.bottom() + clearance
    {
        let end = target.center_bottom();
        let candidate = vec![start, egui::pos2(end.x, start.y), end];
        if !obstructed(&candidate, rectangles, source_id, target_id, clearance)
        {
            return Some(RoutedEdge {
                points: candidate,
                entry: EntryPort::Bottom,
            });
        }
    }

    // Consecutive nodes on a row keep a compact left-to-right connection.
    if start.x + clearance < target.left()
    {
        let end = target.left_center();
        let middle = (start.x + end.x) * 0.5;
        let candidate = vec![
            start,
            egui::pos2(middle, start.y),
            egui::pos2(middle, end.y),
            end,
        ];
        if !obstructed(&candidate, rectangles, source_id, target_id, clearance)
        {
            return Some(RoutedEdge {
                points: candidate,
                entry: EntryPort::Left,
            });
        }
    }

    // Another node lies along the horizontal path. Build a detour through
    // an empty upper/lower corridor, entering the target through its left
    // edge. The arrival vertical is placed in the *column gap*, not over
    // any intermediate node's text.
    let top = rectangles.values().map(|rect| rect.top())
        .fold(f32::INFINITY, f32::min);
    let bottom = rectangles.values().map(|rect| rect.bottom())
        .fold(f32::NEG_INFINITY, f32::max);

    let mut candidates: Vec<RoutedEdge> = Vec::new();

    for side in 0..2
    {
        for channel in 0..6
        {
            let lane = if side == 0
            {
                top - (22.0 + (edge_index % 4) as f32 * 12.0
                    + channel as f32 * 10.0) * zoom
            }
            else
            {
                bottom + (22.0 + (edge_index % 4) as f32 * 12.0
                    + channel as f32 * 10.0) * zoom
            };

            for exit_distance in [18.0, 32.0, 52.0]
            {
                let departure_x = start.x + exit_distance * zoom;

                // Keep the arrival outside the target, where there is
                // typically a full free column-gap lane.
                let arrival_x = target.left() - 32.0 * zoom;
                if departure_x >= arrival_x - 3.0 * zoom
                {
                    continue;
                }

                let end = target.left_center();
                let candidate = vec![
                    start,
                    egui::pos2(departure_x, start.y),
                    egui::pos2(departure_x, lane),
                    egui::pos2(arrival_x, lane),
                    egui::pos2(arrival_x, end.y),
                    end,
                ];

                if !obstructed(
                    &candidate, rectangles, source_id, target_id, clearance
                )
                {
                    candidates.push(RoutedEdge {
                        points: candidate,
                        entry: EntryPort::Left,
                    });
                }
            }
        }
    }

    candidates.into_iter().min_by(|a, b|
    {
        length(&a.points).total_cmp(&length(&b.points))
    })
    // An awkward or backwards edge is still drawn, rather than silently
    // disappearing. This should not occur for ordinary layered proof DAGs.
    .or_else(||
    {
        let end = target.left_center();
        let mid = (start.x + end.x) * 0.5;
        Some(RoutedEdge {
            points: vec![
                start,
                egui::pos2(mid, start.y),
                egui::pos2(mid, end.y),
                end,
            ],
            entry: EntryPort::Left,
        })
    })
}

#[cfg(test)]
mod tests
{
    use super::*;

    fn rect(x: f32, y: f32) -> egui::Rect
    {
        egui::Rect::from_min_size(
            egui::pos2(x, y), egui::vec2(170.0, 70.0),
        )
    }

    fn assert_manhattan(points: &[egui::Pos2])
    {
        assert!(points.windows(2).all(|segment|
        {
            (segment[0].x - segment[1].x).abs() < 0.01
                || (segment[0].y - segment[1].y).abs() < 0.01
        }));
    }

    #[test]
    fn screenshot_upper_rule_enters_its_clause_from_above()
    {
        let map = HashMap::from([
            (1, rect(320.0, 138.0)),  // A => (E | K)
            (2, rect(1350.0, 235.0)), // E | K
            (3, rect(835.0, 235.0)),  // !K | L
            (4, rect(1090.0, 235.0)), // !K established
        ]);
        let result = route(1, 2, &map, 0, 1.0).unwrap();
        assert_eq!(result.entry, EntryPort::Top);
        assert_eq!(result.points[0], map[&1].right_center());
        assert_eq!(*result.points.last().unwrap(), map[&2].center_top());
        assert_eq!(result.points.len(), 3);
        assert!(result.points[1].y > 100.0);
        assert_manhattan(&result.points);
        assert!(!obstructed(&result.points, &map, 1, 2, 8.0));
    }

    #[test]
    fn screenshot_lower_rule_enters_its_clause_from_below()
    {
        let map = HashMap::from([
            (1, rect(320.0, 333.0)),  // D => (!K | L)
            (2, rect(835.0, 235.0)),  // !K | L
            (3, rect(578.0, 235.0)),  // !L established
            (4, rect(1090.0, 235.0)), // !K established
        ]);
        let result = route(1, 2, &map, 1, 1.0).unwrap();
        assert_eq!(result.entry, EntryPort::Bottom);
        assert_eq!(*result.points.last().unwrap(), map[&2].center_bottom());
        assert_eq!(result.points.len(), 3);
        assert_manhattan(&result.points);
        assert!(!obstructed(&result.points, &map, 1, 2, 8.0));
    }

    #[test]
    fn same_row_links_remain_short()
    {
        let map = HashMap::from([
            (1, rect(100.0, 200.0)),
            (2, rect(400.0, 200.0)),
        ]);
        let result = route(1, 2, &map, 0, 1.0).unwrap();
        assert_eq!(result.entry, EntryPort::Left);
        assert_eq!(*result.points.last().unwrap(), map[&2].left_center());
        assert_manhattan(&result.points);
    }

    #[test]
    fn real_large_ruleset_routes_each_rule_to_its_own_or_clause()
    {
        use crate::engine::{run_source, SolveMode};
        use super::super::layout::{GraphLayout, NODE_HEIGHT, NODE_WIDTH};
        use super::super::model::{ExplanationGraph, LinkKind, NodeKind};

        let run = run_source(include_str!("../../../big_test.txt"), SolveMode::Sequential)
            .unwrap();
        let report = run.report.load_query(4).unwrap();
        let query = &report.queries[0];
        assert_eq!(query.symbol, 'E');
        if query.trace.truncated
        {
            // A large cyclic ruleset may hit the bounded trace budget.
            // Incomplete proofs cannot safely produce activation links.
            assert!(query.graph.nodes.is_empty());
            assert!(query.trace.events.len()
                <= crate::trace::MAX_TRACE_EVENTS_PER_QUERY);
            return;
        }
        let graph = ExplanationGraph::new(&report, query);
        let layout = GraphLayout::new(&graph);
        let rectangles: HashMap<_, _> = layout.nodes.iter().map(|position|
        {
            (
                position.id,
                egui::Rect::from_min_size(
                    egui::pos2(position.x, position.y),
                    egui::vec2(NODE_WIDTH, NODE_HEIGHT),
                ),
            )
        }).collect();

        let rule_a = graph.nodes.iter().find(|node|
            node.kind == NodeKind::Rule && node.title == "A => (E | K)"
        ).expect("A=>E|K normalized rule");
        let rule_d = graph.nodes.iter().find(|node|
            node.kind == NodeKind::Rule && node.title == "D => (!K | L)"
        ).expect("D=>!K|L normalized rule");
        let clause_ek = graph.nodes.iter().find(|node|
            node.kind == NodeKind::Clause && node.title == "E | K"
        ).expect("E|K clause");
        let clause_kl = graph.nodes.iter().find(|node|
            node.kind == NodeKind::Clause && node.title == "!K | L"
        ).expect("!K|L clause");

        for (rule, clause) in [(rule_a, clause_ek), (rule_d, clause_kl)]
        {
            assert!(graph.links.iter().any(|link|
                link.from == rule.id && link.to == clause.id
                    && link.kind == LinkKind::Activates));

            let connection = route(rule.id, clause.id, &rectangles, 0, 1.0)
                .expect("routable rule activation");
            let start = rectangles[&rule.id];
            let target = rectangles[&clause.id];
            assert_eq!(connection.points.first().copied(), Some(start.right_center()));
            assert_manhattan(&connection.points);

            // The line must never end at some other constraint node.
            let expected_tip = match connection.entry
            {
                EntryPort::Top => target.center_top(),
                EntryPort::Bottom => target.center_bottom(),
                EntryPort::Left => target.left_center(),
            };
            assert_eq!(connection.points.last().copied(), Some(expected_tip));
            assert!(!obstructed(
                &connection.points, &rectangles, rule.id, clause.id, 8.0,
            ));
        }
    }

    #[test]
    fn intermediate_card_forces_detour_without_overlap()
    {
        let map = HashMap::from([
            (1, rect(100.0, 200.0)),
            (2, rect(750.0, 200.0)),
            (3, rect(400.0, 200.0)),
        ]);
        let result = route(1, 2, &map, 1, 1.0).unwrap();
        assert!(result.points.len() > 4);
        assert_manhattan(&result.points);
        assert!(!obstructed(&result.points, &map, 1, 2, 8.0));
    }
}
