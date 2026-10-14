//! Runtime query selection, independent of the editor's '?...' declaration.
//!
//! The source determines checkbox defaults, but user selections do not modify
//! the file and persist when the source is edited, until explicitly reset.

use crate::engine::source_queries;

#[derive(Debug, Clone, Default)]
pub struct QueryControls
{
    from_source: [bool; 26],
    overrides: [Option<bool>; 26],
}

impl QueryControls
{
    pub fn from_document(text: &str) -> Self
    {
        Self
        {
            from_source: source_queries(text),
            overrides: [None; 26],
        }
    }

    /// Refresh the defaults after editing; keep deliberate manual overrides.
    pub fn sync_source(&mut self, text: &str)
    {
        self.from_source = source_queries(text);
    }

    pub fn enabled(&self, index: usize) -> bool
    {
        self.overrides[index].unwrap_or(self.from_source[index])
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

    pub fn enabled_count(&self) -> usize
    {
        (0..26).filter(|&index| self.enabled(index)).count()
    }

    pub fn override_count(&self) -> usize
    {
        self.overrides.iter().filter(|override_value|
            override_value.is_some()
        ).count()
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
    fn queries_follow_source_until_overridden()
    {
        let mut queries = QueryControls::from_document("A=>B\n=A\n?BAC");
        assert!(queries.enabled(0));
        assert!(queries.enabled(1));
        assert!(queries.enabled(2));
        assert!(!queries.enabled(3));

        queries.set_enabled(1, false);
        queries.set_enabled(3, true);
        assert_eq!(queries.override_count(), 2);
        assert_eq!(queries.enabled_count(), 3);

        // Edits refresh the source defaults but preserve chosen overrides.
        queries.sync_source("A=>B\n=A\n?ABCD");
        assert!(queries.enabled(0));
        assert!(!queries.enabled(1));
        assert!(queries.enabled(2));
        assert!(queries.enabled(3));

        queries.restore_source();
        assert_eq!(queries.override_count(), 0);
        assert_eq!(queries.enabled_count(), 4);
    }

    #[test]
    fn opening_another_file_drops_runtime_query_selections()
    {
        let mut first = QueryControls::from_document("=A\n?A");
        first.set_enabled(0, false);
        first.set_enabled(2, true);

        let next = QueryControls::from_document("=A\n?B");
        assert!(!next.enabled(0));
        assert!(next.enabled(1));
        assert!(!next.enabled(2));
        assert_eq!(next.override_count(), 0);
    }

    #[test]
    fn all_queries_can_be_disabled_until_another_is_selected()
    {
        let mut queries = QueryControls::from_document("=A\n?C");
        queries.set_enabled(2, false);
        assert_eq!(queries.enabled_count(), 0);
        queries.set_enabled(25, true);
        assert_eq!(queries.enabled_count(), 1);
        assert!(queries.enabled_mask()[25]);
    }
}
