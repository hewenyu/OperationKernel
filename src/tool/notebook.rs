use super::base::{Tool, ToolContext, ToolError, ToolResult};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::PathBuf;

/// NotebookEdit tool - Edit Jupyter notebook (.ipynb) cells
pub struct NotebookEditTool;

#[derive(Debug, Deserialize)]
struct NotebookEditParams {
    notebook_path: PathBuf,
    new_source: String,
    #[serde(default)]
    cell_id: Option<String>,
    #[serde(default)]
    cell_type: Option<String>,
    #[serde(default = "default_edit_mode")]
    edit_mode: String,
}

fn default_edit_mode() -> String {
    "replace".to_string()
}

/// Jupyter notebook structure
#[derive(Debug, Deserialize, Serialize)]
struct Notebook {
    cells: Vec<Cell>,
    metadata: Value,
    nbformat: i32,
    nbformat_minor: i32,
}

/// Jupyter notebook cell
#[derive(Debug, Clone, Deserialize, Serialize)]
struct Cell {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    cell_type: String,
    source: SourceLines,
    #[serde(default)]
    metadata: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    execution_count: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    outputs: Option<Vec<Value>>,
}

/// Jupyter notebook source can be a string or array of strings
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
enum SourceLines {
    Single(String),
    Multiple(Vec<String>),
}

impl SourceLines {
    fn from_string(s: String) -> Self {
        // Split into lines preserving newlines
        let lines: Vec<String> = s.split_inclusive('\n').map(|s| s.to_string()).collect();
        if lines.len() == 1 {
            SourceLines::Single(lines[0].clone())
        } else {
            SourceLines::Multiple(lines)
        }
    }
}

impl Cell {
    fn new_code_cell(source: String, id: Option<String>) -> Self {
        Cell {
            id,
            cell_type: "code".to_string(),
            source: SourceLines::from_string(source),
            metadata: json!({}),
            execution_count: Some(Value::Null),
            outputs: Some(vec![]),
        }
    }

    fn new_markdown_cell(source: String, id: Option<String>) -> Self {
        Cell {
            id,
            cell_type: "markdown".to_string(),
            source: SourceLines::from_string(source),
            metadata: json!({}),
            execution_count: None,
            outputs: None,
        }
    }
}

#[async_trait::async_trait]
impl Tool for NotebookEditTool {
    fn id(&self) -> &str {
        "notebook_edit"
    }

    fn description(&self) -> &str {
        "Edit Jupyter notebook (.ipynb) cells. Supports replace, insert, and delete operations. \
         Can target cells by ID or modify the first cell of a given type."
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "notebook_path": {
                    "type": "string",
                    "description": "Absolute or relative path to the .ipynb file"
                },
                "new_source": {
                    "type": "string",
                    "description": "New source code/text for the cell"
                },
                "cell_id": {
                    "type": "string",
                    "description": "Cell ID to edit (optional, defaults to first cell)"
                },
                "cell_type": {
                    "type": "string",
                    "description": "Cell type for insert mode: 'code' or 'markdown'",
                    "enum": ["code", "markdown"]
                },
                "edit_mode": {
                    "type": "string",
                    "description": "Edit operation: 'replace', 'insert', or 'delete'",
                    "enum": ["replace", "insert", "delete"],
                    "default": "replace"
                }
            },
            "required": ["notebook_path", "new_source"]
        })
    }

    async fn execute(
        &self,
        params: Value,
        ctx: &ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let params: NotebookEditParams = serde_json::from_value(params)
            .map_err(|e| ToolError::InvalidParams(e.to_string()))?;

        tracing::debug!(
            notebook_path = %params.notebook_path.display(),
            edit_mode = %params.edit_mode,
            cell_id = ?params.cell_id,
            "notebook_edit start"
        );

        // 1. Resolve path
        let path = if params.notebook_path.is_absolute() {
            params.notebook_path
        } else {
            ctx.working_dir.join(&params.notebook_path)
        };

        // 2. Check file exists for replace/delete modes
        if params.edit_mode != "insert" && !path.exists() {
            return Err(ToolError::FileNotFound(path));
        }

        // 3. Read and parse notebook (or create new for insert without existing file)
        let mut notebook: Notebook = if path.exists() {
            let content = tokio::fs::read_to_string(&path)
                .await
                .map_err(|e| ToolError::Other(e.into()))?;
            serde_json::from_str(&content)
                .map_err(|e| ToolError::InvalidParams(format!("Invalid notebook JSON: {}", e)))?
        } else {
            // Create new notebook for insert mode
            Notebook {
                cells: vec![],
                metadata: json!({
                    "kernelspec": {
                        "display_name": "Python 3",
                        "language": "python",
                        "name": "python3"
                    },
                    "language_info": {
                        "name": "python",
                        "version": "3.8.0"
                    }
                }),
                nbformat: 4,
                nbformat_minor: 5,
            }
        };

        // 4. Perform edit operation
        let operation_summary = match params.edit_mode.as_str() {
            "replace" => {
                let cell_idx = if let Some(ref id) = params.cell_id {
                    notebook
                        .cells
                        .iter()
                        .position(|c| c.id.as_ref() == Some(id))
                        .ok_or_else(|| {
                            ToolError::InvalidParams(format!("Cell with ID '{}' not found", id))
                        })?
                } else {
                    if notebook.cells.is_empty() {
                        return Err(ToolError::InvalidParams(
                            "Notebook has no cells to replace".into(),
                        ));
                    }
                    0 // Default to first cell
                };

                let cell = &mut notebook.cells[cell_idx];
                cell.source = SourceLines::from_string(params.new_source.clone());

                // Update cell type if specified
                if let Some(cell_type) = params.cell_type {
                    cell.cell_type = cell_type;
                }

                format!("Replaced cell {} (index {})",
                    cell.id.as_deref().unwrap_or("unknown"), cell_idx)
            }

            "insert" => {
                let cell_type = params.cell_type.as_deref().unwrap_or("code");
                let new_cell = match cell_type {
                    "code" => Cell::new_code_cell(params.new_source.clone(), None),
                    "markdown" => Cell::new_markdown_cell(params.new_source.clone(), None),
                    _ => {
                        return Err(ToolError::InvalidParams(format!(
                            "Invalid cell_type: {}",
                            cell_type
                        )))
                    }
                };

                // Find insertion position
                let insert_idx = if let Some(ref id) = params.cell_id {
                    // Insert after the specified cell
                    let idx = notebook
                        .cells
                        .iter()
                        .position(|c| c.id.as_ref() == Some(id))
                        .ok_or_else(|| {
                            ToolError::InvalidParams(format!("Cell with ID '{}' not found", id))
                        })?;
                    idx + 1
                } else {
                    // Insert at end
                    notebook.cells.len()
                };

                notebook.cells.insert(insert_idx, new_cell);
                format!("Inserted {} cell at index {}", cell_type, insert_idx)
            }

            "delete" => {
                let cell_idx = if let Some(ref id) = params.cell_id {
                    notebook
                        .cells
                        .iter()
                        .position(|c| c.id.as_ref() == Some(id))
                        .ok_or_else(|| {
                            ToolError::InvalidParams(format!("Cell with ID '{}' not found", id))
                        })?
                } else {
                    if notebook.cells.is_empty() {
                        return Err(ToolError::InvalidParams(
                            "Notebook has no cells to delete".into(),
                        ));
                    }
                    0 // Default to first cell
                };

                let deleted_cell = notebook.cells.remove(cell_idx);
                format!(
                    "Deleted cell {} (index {})",
                    deleted_cell.id.as_deref().unwrap_or("unknown"),
                    cell_idx
                )
            }

            _ => {
                return Err(ToolError::InvalidParams(format!(
                    "Invalid edit_mode: {}",
                    params.edit_mode
                )))
            }
        };

        // 5. Write back notebook
        let new_content = serde_json::to_string_pretty(&notebook)
            .map_err(|e| ToolError::Other(e.into()))?;
        tokio::fs::write(&path, new_content)
            .await
            .map_err(|e| ToolError::Other(e.into()))?;

        tracing::debug!(
            path = %path.display(),
            total_cells = notebook.cells.len(),
            "notebook_edit complete"
        );

        // 6. Return result
        Ok(ToolResult::new(
            format!("Edited {}", path.display()),
            format!(
                "{}\nNotebook now has {} cells",
                operation_summary,
                notebook.cells.len()
            ),
        )
        .with_metadata("total_cells", json!(notebook.cells.len()))
        .with_metadata("operation", json!(params.edit_mode)))
    }
}
