//! Validated, project-local canvas edits. These never touch database or API contents.
use crate::{
    domain::{CanvasGroup, CanvasLayout, Position, Source},
    error::{AppError, Result},
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
#[derive(Serialize, Deserialize, schemars::JsonSchema)]
pub struct NodeMove {
    pub node_id: String,
    pub x: f64,
    pub y: f64,
}
#[derive(Serialize, Deserialize, schemars::JsonSchema)]
pub struct GroupArgs {
    pub source_id: String,
    pub name: String,
    pub node_ids: Vec<String>,
    pub group_id: Option<String>,
    /// Optional #RRGGBB color. Omit to preserve an existing group color.
    pub color: Option<String>,
}
fn snapshot(source: &Source) -> CanvasLayout {
    CanvasLayout {
        positions: source.positions.clone(),
        groups: source.groups.clone(),
    }
}
pub fn move_nodes(source: &mut Source, nodes: Vec<NodeMove>) -> Result<()> {
    if nodes.is_empty() || nodes.len() > 5000 {
        return Err(AppError::Validation(
            "Move between 1 and 5,000 nodes.".into(),
        ));
    }
    let ids: HashSet<_> = source
        .graph
        .entities
        .iter()
        .map(|e| e.id.as_str())
        .collect();
    let mut positions = BTreeMap::new();
    for node in nodes {
        if !ids.contains(node.node_id.as_str())
            || !node.x.is_finite()
            || !node.y.is_finite()
            || node.x.abs() > 1e7
            || node.y.abs() > 1e7
            || positions.contains_key(&node.node_id)
        {
            return Err(AppError::Validation(
                "Every move needs a unique existing node ID and finite canvas coordinates.".into(),
            ));
        }
        positions.insert(
            node.node_id,
            Position {
                x: node.x,
                y: node.y,
            },
        );
    }
    source.layout_backup = Some(snapshot(source));
    source.positions.extend(positions);
    Ok(())
}
pub fn set_group(source: &mut Source, args: GroupArgs) -> Result<String> {
    let name = crate::domain::valid_name(&args.name)?;
    if args.node_ids.is_empty() || args.node_ids.len() > 5000 {
        return Err(AppError::Validation(
            "A group needs at least one member.".into(),
        ));
    }
    let ids: HashSet<_> = source
        .graph
        .entities
        .iter()
        .map(|e| e.id.as_str())
        .collect();
    let unique: HashSet<_> = args.node_ids.iter().collect();
    if unique.len() != args.node_ids.len()
        || args.node_ids.iter().any(|id| !ids.contains(id.as_str()))
    {
        return Err(AppError::Validation(
            "Group members must be unique nodes from this source.".into(),
        ));
    }
    if args.group_id.is_none() && source.groups.len() >= 100 {
        return Err(AppError::Validation(
            "Use at most 100 groups per source.".into(),
        ));
    }
    let index = match &args.group_id {
        Some(id) => Some(
            source
                .groups
                .iter()
                .position(|g| &g.id == id)
                .ok_or(AppError::NotFound)?,
        ),
        None => None,
    };
    let color = match args.color {
        Some(color)
            if color.len() == 7
                && color.starts_with('#')
                && color.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit) =>
        {
            color.to_ascii_lowercase()
        }
        Some(_) => {
            return Err(AppError::Validation(
                "Use a group color in #RRGGBB format.".into(),
            ))
        }
        None => index
            .map(|i| source.groups[i].color.clone())
            .unwrap_or_else(crate::domain::default_group_color),
    };
    source.layout_backup = Some(snapshot(source));
    let id = args
        .group_id
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let group = CanvasGroup {
        id: id.clone(),
        name,
        node_ids: args.node_ids,
        color,
    };
    if let Some(index) = index {
        source.groups[index] = group;
    } else {
        source.groups.push(group);
    }
    Ok(id)
}
pub fn remove_group(source: &mut Source, id: &str) -> Result<()> {
    if !source.groups.iter().any(|g| g.id == id) {
        return Err(AppError::NotFound);
    }
    source.layout_backup = Some(snapshot(source));
    source.groups.retain(|g| g.id != id);
    Ok(())
}
pub fn undo(source: &mut Source) -> Result<()> {
    let backup = source
        .layout_backup
        .take()
        .ok_or_else(|| AppError::Validation("No canvas edit to undo.".into()))?;
    source.positions = backup.positions;
    source.groups = backup.groups;
    Ok(())
}
