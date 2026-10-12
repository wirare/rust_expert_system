//! Source document and native file selection, independent of the solver.

use std::path::{Path, PathBuf};

#[derive(Default)]
pub struct SourceDocument
{
    pub path: Option<PathBuf>,
    pub text: String,
    pub modified: bool,
}

impl SourceDocument
{
    pub fn open_dialog() -> Result<Option<Self>, String>
    {
        let selected = rfd::FileDialog::new()
            .set_title("Open expert-system rules")
            .add_filter("Text rule files", &["txt"])
            .pick_file();

        selected.map(Self::load).transpose()
    }

    pub fn load(path: PathBuf) -> Result<Self, String>
    {
        let text = std::fs::read_to_string(&path)
            .map_err(|error| format!("Unable to open '{}': {error}", path.display()))?;

        Ok(Self
        {
            path: Some(path),
            text,
            modified: false,
        })
    }

    pub fn name(&self) -> &str
    {
        self.path
            .as_ref()
            .and_then(|path| path.file_name())
            .and_then(|name| name.to_str())
            .unwrap_or("Untitled")
    }

    pub fn location(&self) -> Option<&Path>
    {
        self.path.as_deref()
    }

    pub fn has_input(&self) -> bool
    {
        self.path.is_some() || !self.text.trim().is_empty()
    }

    pub fn mark_modified(&mut self)
    {
        self.modified = true;
    }
}
