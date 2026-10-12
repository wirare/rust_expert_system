//! Initial-fact overrides owned by the GUI, separate from document text.

use crate::engine::source_initial_facts;

/// Each None follows the source's '=...' declaration; Some overrides it.
/// This keeps deliberate runtime changes when the source text is edited.
#[derive(Debug, Clone, Default)]
pub struct FactControls
{
    from_source: [bool; 26],
    overrides: [Option<bool>; 26],
}

impl FactControls
{
    pub fn from_document(text: &str) -> Self
    {
        Self
        {
            from_source: source_initial_facts(text),
            overrides: [None; 26],
        }
    }

    /// Refresh defaults after editing. Manual overrides remain in effect.
    pub fn sync_source(&mut self, text: &str)
    {
        self.from_source = source_initial_facts(text);
    }

    pub fn enabled(&self, index: usize) -> bool
    {
        self.overrides[index].unwrap_or(self.from_source[index])
    }

    pub fn source_enabled(&self, index: usize) -> bool
    {
        self.from_source[index]
    }

    pub fn set_enabled(&mut self, index: usize, active: bool)
    {
        self.overrides[index] = if active == self.from_source[index]
        {
            None
        }
        else
        {
            Some(active)
        };
    }

    pub fn enabled_mask(&self) -> [bool; 26]
    {
        std::array::from_fn(|index| self.enabled(index))
    }

    pub fn override_count(&self) -> usize
    {
        self.overrides.iter().filter(|value| value.is_some()).count()
    }

    pub fn restore_source(&mut self)
    {
        self.overrides = [None; 26];
    }
}

#[cfg(test)]
mod tests
{
    use super::*;

    #[test]
    fn source_edits_preserve_manual_overrides()
    {
        let mut facts = FactControls::from_document("=AB\n?ABC");
        assert!(facts.enabled(0));
        assert!(facts.enabled(1));
        assert!(!facts.enabled(2));

        facts.set_enabled(0, false);
        facts.set_enabled(2, true);
        assert_eq!(facts.override_count(), 2);

        facts.sync_source("=ABC\n?ABC");
        assert!(!facts.enabled(0));
        assert!(facts.enabled(1));
        assert!(facts.enabled(2));

        facts.restore_source();
        assert_eq!(facts.override_count(), 0);
        assert!(facts.enabled(0));
        assert!(facts.enabled(2));
    }

    #[test]
    fn new_document_does_not_inherit_overrides()
    {
        let mut first = FactControls::from_document("=A\n?A");
        first.set_enabled(0, false);

        let next = FactControls::from_document("=B\n?B");
        assert!(!next.enabled(0));
        assert!(next.enabled(1));
        assert_eq!(next.override_count(), 0);
    }
}
