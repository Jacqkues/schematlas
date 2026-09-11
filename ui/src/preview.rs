//! Browser-only design preview backed by localStorage. Database and import operations need the desktop runtime.
use crate::api::{local_storage_get, local_storage_set, now_iso};
use crate::types::{CanvasGroup, LayoutBackup, Position, Project, Source};
use serde_json::Value;
use std::collections::HashMap;

const KEY: &str = "schema-atlas-browser-preview-v1";
const SAMPLE: &str = include_str!("sample.json");

fn load() -> Vec<Project> {
    local_storage_get(KEY)
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

fn save(projects: &[Project]) {
    if let Ok(raw) = serde_json::to_string(projects) {
        local_storage_set(KEY, &raw);
    }
}

fn text(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn backup(source: &Source) -> LayoutBackup {
    LayoutBackup {
        positions: source.positions.clone(),
        groups: source.groups().to_vec(),
    }
}

fn to_value<T: serde::Serialize>(value: &T) -> Result<Value, String> {
    serde_json::to_value(value).map_err(|e| e.to_string())
}

pub fn preview(command: &str, args: Value) -> Result<Value, String> {
    let mut projects = load();
    let project_id = text(&args, "projectId");
    let index = projects.iter().position(|p| p.id == project_id);
    let result = match command {
        "list_projects" => return to_value(&projects),
        "create_demo" => {
            let mut demo: Project = serde_json::from_str(SAMPLE).map_err(|e| e.to_string())?;
            demo.id = uuid::Uuid::new_v4().to_string();
            demo.created_at = now_iso();
            demo.updated_at = demo.created_at.clone();
            for source in &mut demo.sources {
                source.id = uuid::Uuid::new_v4().to_string();
            }
            projects.push(demo.clone());
            to_value(&demo)?
        }
        "create_project" => {
            let project = Project {
                id: uuid::Uuid::new_v4().to_string(),
                name: text(&args, "name").trim().to_string(),
                description: text(&args, "description"),
                created_at: now_iso(),
                updated_at: now_iso(),
                sources: vec![],
                working_directory: None,
            };
            projects.push(project.clone());
            to_value(&project)?
        }
        "rename_project" => {
            let project = index
                .map(|i| &mut projects[i])
                .ok_or("Project not found.")?;
            project.name = text(&args, "name");
            project.description = text(&args, "description");
            to_value(project)?
        }
        "delete_project" => {
            projects.retain(|p| p.id != project_id);
            save(&projects);
            return Ok(Value::Null);
        }
        "remove_source" => {
            let source_id = text(&args, "sourceId");
            let project = index
                .map(|i| &mut projects[i])
                .ok_or("Project not found.")?;
            project.sources.retain(|s| s.id != source_id);
            to_value(project)?
        }
        "save_positions" => {
            let source_id = text(&args, "sourceId");
            let positions: HashMap<String, Position> =
                serde_json::from_value(args.get("positions").cloned().unwrap_or(Value::Null))
                    .map_err(|e| e.to_string())?;
            let project = index.map(|i| &mut projects[i]).ok_or("Source not found.")?;
            let source = project
                .sources
                .iter_mut()
                .find(|s| s.id == source_id)
                .ok_or("Source not found.")?;
            source.layout_backup = Some(backup(source));
            source.positions.extend(positions);
            to_value(project)?
        }
        "undo_canvas" | "remove_canvas_group" => {
            let source_id = text(&args, "sourceId");
            let project = index.map(|i| &mut projects[i]).ok_or("Source not found.")?;
            let source = project
                .sources
                .iter_mut()
                .find(|s| s.id == source_id)
                .ok_or("Source not found.")?;
            if command == "undo_canvas" {
                let restored = source
                    .layout_backup
                    .take()
                    .ok_or("No canvas edit to undo.")?;
                source.positions = restored.positions;
                source.groups = Some(restored.groups);
            } else {
                let group_id = text(&args, "groupId");
                source.layout_backup = Some(backup(source));
                source.groups = Some(
                    source
                        .groups()
                        .iter()
                        .filter(|g| g.id != group_id)
                        .cloned()
                        .collect(),
                );
            }
            to_value(project)?
        }
        "set_canvas_group" => {
            let input = args.get("args").cloned().unwrap_or(Value::Null);
            let source_id = text(&input, "source_id");
            let group_id = input
                .get("group_id")
                .and_then(Value::as_str)
                .map(str::to_string);
            let name = text(&input, "name").trim().to_string();
            let node_ids: Vec<String> = input
                .get("node_ids")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default();
            let project = index.map(|i| &mut projects[i]).ok_or("Source not found.")?;
            let source = project
                .sources
                .iter_mut()
                .find(|s| s.id == source_id)
                .ok_or("Source not found.")?;
            let previous = source
                .groups()
                .iter()
                .find(|g| Some(&g.id) == group_id.as_ref())
                .cloned();
            if group_id.is_some() && previous.is_none() {
                return Err("Group not found.".into());
            }
            let color = input
                .get("color")
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| previous.as_ref().and_then(|g| g.color.clone()))
                .unwrap_or_else(|| crate::layout::DEFAULT_GROUP_COLOR.to_string());
            let mut unique = node_ids.clone();
            unique.sort();
            unique.dedup();
            let valid_color = color.len() == 7
                && color.starts_with('#')
                && color[1..].chars().all(|c| c.is_ascii_hexdigit());
            if name.is_empty()
                || name.len() > 80
                || !valid_color
                || node_ids.is_empty()
                || unique.len() != node_ids.len()
                || node_ids
                    .iter()
                    .any(|id| !source.graph.entities.iter().any(|e| &e.id == id))
            {
                return Err("Choose a name, color, and valid group members.".into());
            }
            if previous.is_none() && source.groups().len() >= 100 {
                return Err("Use at most 100 groups per source.".into());
            }
            source.layout_backup = Some(backup(source));
            let group = CanvasGroup {
                id: previous
                    .as_ref()
                    .map(|g| g.id.clone())
                    .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
                name,
                node_ids,
                color: Some(color.to_lowercase()),
            };
            let mut groups = source.groups().to_vec();
            if previous.is_some() {
                for g in &mut groups {
                    if g.id == group.id {
                        *g = group.clone();
                    }
                }
            } else {
                groups.push(group);
            }
            source.groups = Some(groups);
            to_value(project)?
        }
        _ => {
            return Err(
                "Open the Schematlas desktop app to connect databases or import OpenAPI files."
                    .into(),
            )
        }
    };
    save(&projects);
    Ok(result)
}
