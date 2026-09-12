//! Viewport math shared by the canvas, the minimap and the edges.
use crate::layout::{Bounds, MAX_FIELDS, NODE_WIDTH};
use crate::types::{Entity, Position};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
    pub x: f64,
    pub y: f64,
    pub zoom: f64,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            zoom: 1.0,
        }
    }
}

pub const MIN_ZOOM: f64 = 0.01;
pub const MAX_ZOOM: f64 = 1.8;
/// Below this zoom, column text is dropped to reduce rendering work.
pub const OVERVIEW_ZOOM: f64 = 0.25;

impl Viewport {
    pub fn transform(&self) -> String {
        format!("translate({}px, {}px) scale({})", self.x, self.y, self.zoom)
    }
    /// Screen point (relative to the canvas) to graph coordinates.
    pub fn to_graph(self, x: f64, y: f64) -> Position {
        Position {
            x: (x - self.x) / self.zoom,
            y: (y - self.y) / self.zoom,
        }
    }
    /// Graph-space rectangle currently on screen.
    pub fn visible_rect(&self, width: f64, height: f64) -> Bounds {
        let origin = self.to_graph(0.0, 0.0);
        Bounds {
            x: origin.x,
            y: origin.y,
            width: width / self.zoom,
            height: height / self.zoom,
        }
    }
    pub fn zoomed_at(&self, factor: f64, x: f64, y: f64) -> Viewport {
        let zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        let ratio = zoom / self.zoom;
        Viewport {
            x: x - (x - self.x) * ratio,
            y: y - (y - self.y) * ratio,
            zoom,
        }
    }
    /// Viewport that frames `bounds` inside a canvas of the given size, like Svelte Flow's fitBounds.
    pub fn fitting(bounds: Bounds, width: f64, height: f64, padding: f64) -> Viewport {
        if bounds.width <= 0.0 || bounds.height <= 0.0 || width <= 0.0 || height <= 0.0 {
            return Viewport::default();
        }
        let x_zoom = width / (bounds.width * (1.0 + padding));
        let y_zoom = height / (bounds.height * (1.0 + padding));
        let zoom = x_zoom.min(y_zoom).clamp(MIN_ZOOM, MAX_ZOOM);
        let x = width / 2.0 - (bounds.x + bounds.width / 2.0) * zoom;
        let y = height / 2.0 - (bounds.y + bounds.height / 2.0) * zoom;
        Viewport { x, y, zoom }
    }
}

pub fn intersects(a: &Bounds, x: f64, y: f64, width: f64, height: f64) -> bool {
    x < a.x + a.width && x + width > a.x && y < a.y + a.height && y + height > a.y
}

/// Vertical offset of a port on a card: a visible column row, or the heading anchor.
pub fn port_y(entity: &Entity, field: Option<&str>) -> f64 {
    field
        .and_then(|name| {
            entity
                .fields
                .iter()
                .take(MAX_FIELDS)
                .position(|f| f.name == name)
        })
        .map(|index| 64.0 + 6.0 + index as f64 * 29.0 + 14.5)
        .unwrap_or(31.0)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PortSide {
    Left,
    Right,
}

impl PortSide {
    fn direction(self) -> f64 {
        match self {
            Self::Left => -1.0,
            Self::Right => 1.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EdgeAnchor {
    pub x: f64,
    pub y: f64,
    pub side: PortSide,
}

impl EdgeAnchor {
    /// Place cardinality outside the card, whichever side the edge uses.
    pub fn label_position(self) -> (f64, f64) {
        (self.x + self.side.direction() * 34.0, self.y - 12.0)
    }
}

pub struct EdgeRoute {
    pub source: EdgeAnchor,
    pub target: EdgeAnchor,
    pub path: String,
}

/// Choose ports from geometry, independently of foreign-key direction.
/// Column rows remain the anchors. Horizontally overlapping/stacked cards use
/// a shared outside lane, including self-references. This is local routing;
/// unrelated cards still occlude edges, as they do elsewhere on the canvas.
pub fn route_edge(source: Position, source_y: f64, target: Position, target_y: f64) -> EdgeRoute {
    let (source_side, target_side) = if source.x + NODE_WIDTH <= target.x {
        (PortSide::Right, PortSide::Left)
    } else if target.x + NODE_WIDTH <= source.x {
        (PortSide::Left, PortSide::Right)
    } else {
        (PortSide::Right, PortSide::Right)
    };
    let anchor = |position: Position, y, side| EdgeAnchor {
        x: position.x
            + if side == PortSide::Right {
                NODE_WIDTH
            } else {
                0.0
            },
        y,
        side,
    };
    let source = anchor(source, source_y, source_side);
    let target = anchor(target, target_y, target_side);
    let (source_control, target_control) = if source_side == target_side {
        let clearance = (64.0 + (target_y - source_y).abs() * 0.15).min(160.0);
        let lane = source.x.max(target.x) + clearance;
        // Distinct control points make a same-column self-reference visible.
        let loop_height = if (source_y - target_y).abs() < 1.0 {
            48.0
        } else {
            0.0
        };
        (
            (lane, source_y - loop_height),
            (lane, target_y + loop_height),
        )
    } else {
        let offset = (target.x - source.x).abs() / 2.0;
        (
            (source.x + source_side.direction() * offset, source_y),
            (target.x + target_side.direction() * offset, target_y),
        )
    };
    EdgeRoute {
        source,
        target,
        path: format!(
            "M{},{} C{},{} {},{} {},{}",
            source.x,
            source.y,
            source_control.0,
            source_control.1,
            target_control.0,
            target_control.1,
            target.x,
            target.y,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn facing_sides_follow_positions_without_reversing_relationship_direction() {
        let left = Position { x: -400.0, y: 30.0 };
        let right = Position { x: 500.0, y: 170.0 };
        let forward = route_edge(left, 114.5, right, 283.5);
        assert_eq!(forward.source.side, PortSide::Right);
        assert_eq!(forward.target.side, PortSide::Left);
        assert_eq!(forward.source.x, left.x + NODE_WIDTH);
        assert_eq!(forward.target.x, right.x);
        assert_eq!(forward.source.y, 114.5);
        assert_eq!(forward.target.y, 283.5);
        let reversed = route_edge(right, 283.5, left, 114.5);
        assert_eq!(reversed.source.side, PortSide::Left);
        assert_eq!(reversed.target.side, PortSide::Right);
        assert_eq!(reversed.source, forward.target);
        assert_eq!(reversed.target, forward.source);
        assert!(reversed.source.label_position().0 < reversed.source.x);
        assert!(reversed.target.label_position().0 > reversed.target.x);
    }

    #[test]
    fn stacked_cards_and_self_references_curve_outside_the_cards() {
        let top = Position { x: 20.0, y: 0.0 };
        let bottom = Position { x: 60.0, y: 400.0 };
        for (source, target, sy, ty) in [(top, bottom, 84.5, 484.5), (bottom, top, 484.5, 84.5)] {
            let route = route_edge(source, sy, target, ty);
            assert_eq!(route.source.side, PortSide::Right);
            assert_eq!(route.target.side, PortSide::Right);
            let lane = 60.0 + NODE_WIDTH + 124.0;
            assert!(route.path.contains(&format!("C{lane},{sy} {lane},{ty}")));
        }
        let same_column = route_edge(top, 84.5, top, 84.5);
        assert!(same_column.path.contains(",36.5 "));
        assert!(same_column.path.contains(",132.5 "));
    }

    #[test]
    fn touching_cards_have_a_finite_route_on_the_facing_sides() {
        let route = route_edge(
            Position { x: 0.0, y: 0.0 },
            84.5,
            Position {
                x: NODE_WIDTH,
                y: 400.0,
            },
            484.5,
        );
        assert_eq!(route.source.side, PortSide::Right);
        assert_eq!(route.target.side, PortSide::Left);
        assert_eq!(route.source.x, route.target.x);
        assert!(!route.path.contains("NaN"));
        assert!(!route.path.contains("inf"));
    }

    #[test]
    fn fitting_centers_bounds_and_respects_zoom_limits() {
        let view = Viewport::fitting(
            Bounds {
                x: 0.0,
                y: 0.0,
                width: 1000.0,
                height: 500.0,
            },
            1150.0,
            575.0,
            0.15,
        );
        assert!((view.zoom - 1.0).abs() < 1e-9);
        assert!((view.x - 75.0).abs() < 1e-9);
        let tiny = Viewport::fitting(
            Bounds {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
            },
            4000.0,
            4000.0,
            0.0,
        );
        assert_eq!(tiny.zoom, MAX_ZOOM);
    }
    #[test]
    fn zooming_keeps_the_anchor_point_fixed() {
        let view = Viewport {
            x: 100.0,
            y: 50.0,
            zoom: 1.0,
        };
        let anchor = view.to_graph(300.0, 200.0);
        let zoomed = view.zoomed_at(1.5, 300.0, 200.0);
        let after = zoomed.to_graph(300.0, 200.0);
        assert!((anchor.x - after.x).abs() < 1e-9 && (anchor.y - after.y).abs() < 1e-9);
    }
}
