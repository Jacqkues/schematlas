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

fn control_offset(distance: f64) -> f64 {
    if distance >= 0.0 {
        0.5 * distance
    } else {
        0.25 * 25.0 * (-distance).sqrt()
    }
}

/// Cubic bezier from a right-side port to a left-side port, matching Svelte Flow's default edge.
pub fn bezier_path(sx: f64, sy: f64, tx: f64, ty: f64) -> String {
    let offset_source = control_offset(tx - sx);
    let offset_target = control_offset(tx - sx);
    format!(
        "M{sx},{sy} C{},{sy} {},{ty} {tx},{ty}",
        sx + offset_source,
        tx - offset_target
    )
}

pub fn node_right(position: Position) -> f64 {
    position.x + NODE_WIDTH
}

#[cfg(test)]
mod tests {
    use super::*;

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
