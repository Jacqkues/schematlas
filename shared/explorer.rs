//! Project explorer organization. Only folder names and item references are stored here;
//! document contents stay in the repository. Shared by the service and browser preview.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Folder {
    pub id: String,
    pub name: String,
    pub parent: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Explorer {
    pub folders: Vec<Folder>,
    /// A null destination places an item at the project root.
    pub locations: BTreeMap<String, Option<String>>,
}

impl Default for Explorer {
    fn default() -> Self {
        Self {
            folders: [
                ("diagrams", "Diagrams"),
                ("docs", "Documents"),
                ("databases", "Database schemas"),
                ("apis", "APIs"),
            ]
            .into_iter()
            .map(|(id, name)| Folder {
                id: id.into(),
                name: name.into(),
                parent: None,
            })
            .collect(),
            locations: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "camelCase")]
pub enum Edit {
    Create {
        id: String,
        name: String,
        parent: Option<String>,
    },
    Update {
        id: String,
        name: String,
        parent: Option<String>,
    },
    Remove {
        id: String,
    },
    Move {
        item: String,
        folder: Option<String>,
    },
}

impl Explorer {
    pub fn location(&self, item: &str, default: &str) -> Option<String> {
        self.locations.get(item).cloned().unwrap_or_else(|| {
            self.folders
                .iter()
                .any(|f| f.id == default)
                .then(|| default.into())
        })
    }

    /// Includes the folder itself. The bound also makes malformed stored input safe to inspect.
    pub fn within(&self, id: &str, ancestor: &str) -> bool {
        let mut current = Some(id);
        for _ in 0..=self.folders.len() {
            let Some(id) = current else { return false };
            if id == ancestor {
                return true;
            }
            current = self
                .folders
                .iter()
                .find(|f| f.id == id)
                .and_then(|f| f.parent.as_deref());
        }
        false
    }

    pub fn path(&self, id: &str) -> String {
        let mut names = vec![];
        let mut current = Some(id);
        for _ in 0..self.folders.len() {
            let Some(folder) = current.and_then(|id| self.folders.iter().find(|f| f.id == id))
            else {
                break;
            };
            names.push(folder.name.as_str());
            current = folder.parent.as_deref();
        }
        names.reverse();
        names.join(" / ")
    }

    /// Apply atomically. The service supplies all existing references and their default folders,
    /// so moving a missing item or deleting a nonempty folder cannot silently lose anything.
    pub fn apply(&mut self, edit: Edit, items: &[(String, String)]) -> Result<(), String> {
        let mut next = self.clone();
        next.locations
            .retain(|item, _| items.iter().any(|(id, _)| id == item));
        match edit {
            Edit::Create { id, name, parent } => {
                if id.is_empty() || id.len() > 80 || next.folders.iter().any(|f| f.id == id) {
                    return Err("This folder already exists or has an invalid identifier.".into());
                }
                next.folders.push(Folder {
                    id,
                    name: name.trim().into(),
                    parent,
                });
            }
            Edit::Update { id, name, parent } => {
                let folder = next
                    .folders
                    .iter_mut()
                    .find(|f| f.id == id)
                    .ok_or("Folder no longer exists.")?;
                folder.name = name.trim().into();
                folder.parent = parent;
            }
            Edit::Remove { id } => {
                if !next.folders.iter().any(|f| f.id == id) {
                    return Err("Folder no longer exists.".into());
                }
                if next
                    .folders
                    .iter()
                    .any(|f| f.parent.as_deref() == Some(&id))
                    || items
                        .iter()
                        .any(|(item, default)| next.location(item, default).as_deref() == Some(&id))
                {
                    return Err("Move this folder’s contents elsewhere before deleting it.".into());
                }
                next.folders.retain(|f| f.id != id);
            }
            Edit::Move { item, folder } => {
                if !items.iter().any(|(id, _)| id == &item) {
                    return Err("Item no longer exists in this project.".into());
                }
                next.locations.insert(item, folder);
            }
        }
        next.validate()?;
        *self = next;
        Ok(())
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.folders.len() > 256 {
            return Err("A project can have up to 256 folders.".into());
        }
        let mut ids = HashSet::new();
        let mut names = HashSet::new();
        for folder in &self.folders {
            if !ids.insert(&folder.id) {
                return Err("Duplicate folder identifier.".into());
            }
            if folder.name.trim().is_empty()
                || folder.name.chars().count() > 80
                || folder.name.contains(['/', '\\'])
                || folder.name.chars().any(char::is_control)
                || [".", ".."].contains(&folder.name.as_str())
            {
                return Err("Use a folder name of 1–80 characters without slashes.".into());
            }
            if !names.insert((&folder.parent, folder.name.to_lowercase())) {
                return Err("A folder with this name already exists here.".into());
            }
            let mut parent = folder.parent.as_deref();
            let mut visited = HashSet::from([folder.id.as_str()]);
            while let Some(id) = parent {
                if !visited.insert(id) {
                    return Err("A folder cannot be placed inside itself.".into());
                }
                if visited.len() > 8 {
                    return Err("Folders can be nested up to eight levels deep.".into());
                }
                parent = self
                    .folders
                    .iter()
                    .find(|f| f.id == id)
                    .ok_or("Parent folder no longer exists.")?
                    .parent
                    .as_deref();
            }
        }
        if self
            .locations
            .values()
            .flatten()
            .any(|id| !ids.contains(id))
        {
            return Err("Destination folder no longer exists.".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn edits_are_atomic_and_preserve_a_valid_tree() {
        let mut explorer = Explorer::default();
        let items = vec![("diagram:map".into(), "diagrams".into())];
        explorer
            .apply(
                Edit::Create {
                    id: "feature".into(),
                    name: " Checkout ".into(),
                    parent: Some("diagrams".into()),
                },
                &items,
            )
            .unwrap();
        let before = explorer.clone();
        for edit in [
            Edit::Update {
                id: "diagrams".into(),
                name: "Diagrams".into(),
                parent: Some("feature".into()),
            },
            Edit::Create {
                id: "duplicate".into(),
                name: "checkout".into(),
                parent: Some("diagrams".into()),
            },
            Edit::Create {
                id: "bad".into(),
                name: "../bad".into(),
                parent: None,
            },
            Edit::Move {
                item: "diagram:missing".into(),
                folder: None,
            },
            Edit::Move {
                item: "diagram:map".into(),
                folder: Some("missing".into()),
            },
            Edit::Remove {
                id: "diagrams".into(),
            },
        ] {
            assert!(explorer.apply(edit, &items).is_err());
            assert_eq!(explorer, before);
        }
        explorer
            .apply(
                Edit::Move {
                    item: "diagram:map".into(),
                    folder: Some("feature".into()),
                },
                &items,
            )
            .unwrap();
        assert_eq!(explorer.path("feature"), "Diagrams / Checkout");
        assert!(explorer
            .apply(
                Edit::Remove {
                    id: "feature".into()
                },
                &items
            )
            .is_err());
        explorer
            .apply(
                Edit::Move {
                    item: "diagram:map".into(),
                    folder: None,
                },
                &items,
            )
            .unwrap();
        explorer
            .apply(
                Edit::Remove {
                    id: "feature".into(),
                },
                &items,
            )
            .unwrap();
        assert_eq!(explorer.location("diagram:map", "diagrams"), None);
    }
}
