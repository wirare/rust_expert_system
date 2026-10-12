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

    /// Save to the current path, or ask for a path if this document is new.
    /// Returns false only when the user cancelled the Save As dialog.
    pub fn save(&mut self) -> Result<bool, String>
    {
        match self.path.clone()
        {
            Some(path) =>
            {
                self.save_to(path)?;
                Ok(true)
            }
            None => self.save_as_dialog(),
        }
    }

    /// Choose a destination without altering the current path on cancellation
    /// or on an I/O failure.
    pub fn save_as_dialog(&mut self) -> Result<bool, String>
    {
        let mut picker = rfd::FileDialog::new()
            .set_title("Save expert-system rules")
            .add_filter("Text rule files", &["txt"])
            .set_file_name(if self.path.is_some() { self.name() } else { "rules.txt" });

        if let Some(directory) = self.path.as_ref().and_then(|path| path.parent())
        {
            picker = picker.set_directory(directory);
        }

        if let Some(path) = picker.save_file()
        {
            self.save_to(path)?;
            Ok(true)
        }
        else
        {
            Ok(false)
        }
    }

    /// Write only the editor's original text. Runtime fact overrides are
    /// deliberately owned elsewhere and never serialized here.
    pub fn save_to(&mut self, path: PathBuf) -> Result<(), String>
    {
        std::fs::write(&path, self.text.as_bytes())
            .map_err(|error| format!("Unable to save '{}': {error}", path.display()))?;

        self.path = Some(path);
        self.modified = false;
        Ok(())
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

#[cfg(test)]
mod tests
{
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn save_and_save_existing_document_round_trip()
    {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!(
            "expert_system_gui_save_{}_{}.txt", std::process::id(), nonce
        ));

        let mut document = SourceDocument::default();
        document.text = "A=>B\n=A\n?B\n".to_owned();
        document.mark_modified();
        document.save_to(path.clone()).unwrap();

        assert!(!document.modified);
        assert_eq!(document.path.as_ref(), Some(&path));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), document.text);

        document.text.push_str("# new comment\n");
        document.mark_modified();
        assert!(document.save().unwrap());
        assert_eq!(SourceDocument::load(path.clone()).unwrap().text, document.text);
        assert!(!document.modified);

        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn failed_save_keeps_document_dirty_and_path()
    {
        let mut document = SourceDocument::default();
        document.text = "=A\n?A".to_owned();
        document.mark_modified();

        let error = document.save_to(std::env::temp_dir()).unwrap_err();
        assert!(error.contains("Unable to save"));
        assert!(document.modified);
        assert!(document.path.is_none());
    }
}
