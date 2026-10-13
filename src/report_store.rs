//! Disk-backed execution report for the native GUI.
//!
//! Writing streams one query at a time. Reading also streams: serde ignores
//! unselected queries without allocating their traces or graphs, and only the
//! chosen query plus small shared rewrite metadata are returned to the UI.
//! A generated report is removed automatically when the run is replaced.

use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::de::{DeserializeSeed, Error as DeError, IgnoredAny, MapAccess, SeqAccess, Visitor};

use crate::report::{ExecutionReport, QueryExecution, ReasoningGraph, RewriteHistory};
use crate::system::System;

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
pub struct ReportStore
{
    path: PathBuf,
}

impl ReportStore
{
    /// Stream the report directly into a unique temporary JSON file.
    /// A failed write deletes its partial output; an existing file cannot
    /// accidentally be overwritten by another concurrently running GUI.
    pub fn generate(system: &System<'_>) -> Result<Self, String>
    {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();

        for _ in 0..20
        {
            let id = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "expert-system-report-{}-{timestamp}-{id}.json",
                std::process::id(),
            ));

            let file = match OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&path)
            {
                Ok(file) => file,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists =>
                    continue,
                Err(error) => return Err(format!(
                    "Unable to create a temporary JSON report: {error}"
                )),
            };

            // Install the RAII guard *before* serializing, so errors and
            // unwinding both remove a partial temporary report.
            let store = Self { path };
            system.write_execution_json(BufWriter::new(file), false)
                .map_err(|error| format!("Unable to generate JSON report: {error}"))?;
            return Ok(store);
        }

        Err("Unable to allocate a unique temporary JSON report".to_owned())
    }

    pub fn path(&self) -> &Path
    {
        &self.path
    }

    /// Load a *single* query by its input order, not the entire JSON file.
    /// Non-selected query payloads use serde::de::IgnoredAny, so even if the
    /// file is large their trace events do not accumulate in memory.
    pub fn load_query(&self, index: usize) -> Result<ExecutionReport, String>
    {
        let file = File::open(&self.path).map_err(|error| format!(
            "Unable to open generated JSON report '{}': {error}",
            self.path.display(),
        ))?;

        let mut decoder = serde_json::Deserializer::from_reader(
            BufReader::new(file)
        );
        let report = ReportSelection { index }
            .deserialize(&mut decoder)
            .map_err(|error| format!("Unable to read query #{index} from JSON: {error}"))?;
        decoder.end().map_err(|error|
            format!("Malformed trailing JSON in report: {error}")
        )?;

        Ok(report)
    }
}

impl Drop for ReportStore
{
    fn drop(&mut self)
    {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Deserialize one element in the "queries" array, consuming all remaining
/// JSON with IgnoredAny. No intermediate Vec<QueryExecution> is constructed.
struct QuerySelection
{
    index: usize,
}

impl<'de> DeserializeSeed<'de> for QuerySelection
{
    type Value = QueryExecution;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_seq(QuerySelectionVisitor { index: self.index })
    }
}

struct QuerySelectionVisitor
{
    index: usize,
}

impl<'de> Visitor<'de> for QuerySelectionVisitor
{
    type Value = QueryExecution;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result
    {
        formatter.write_str("the execution report's query array")
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        for index in 0..self.index
        {
            if sequence.next_element::<IgnoredAny>()?.is_none()
            {
                return Err(A::Error::custom(format!(
                    "Query #{index} does not exist in report"
                )));
            }
        }

        let selected = sequence.next_element::<QueryExecution>()?
            .ok_or_else(|| A::Error::custom(format!(
                "Query #{} does not exist in report",
                self.index,
            )))?;

        while sequence.next_element::<IgnoredAny>()?.is_some()
        {
            // Drain to let serde validate the closing array and document.
        }
        Ok(selected)
    }
}

struct ReportSelection
{
    index: usize,
}

impl<'de> DeserializeSeed<'de> for ReportSelection
{
    type Value = ExecutionReport;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_map(ReportSelectionVisitor { index: self.index })
    }
}

struct ReportSelectionVisitor
{
    index: usize,
}

impl<'de> Visitor<'de> for ReportSelectionVisitor
{
    type Value = ExecutionReport;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result
    {
        formatter.write_str("an execution report JSON object")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut version = None;
        let mut facts = None;
        let mut rewrite = None;
        let mut query = None;

        while let Some(key) = map.next_key::<String>()?
        {
            match key.as_str()
            {
                "schema_version" =>
                {
                    version = Some(map.next_value::<u32>()?);
                }
                "initial_facts" =>
                {
                    facts = Some(map.next_value::<Vec<char>>()?);
                }
                "rewrite" =>
                {
                    rewrite = Some(map.next_value::<RewriteHistory>()?);
                }
                "queries" =>
                {
                    query = Some(map.next_value_seed(
                        QuerySelection { index: self.index },
                    )?);
                }
                // The shared rewrite graph is irrelevant to this viewer,
                // and is intentionally NOT loaded into GUI memory.
                _ =>
                {
                    let _: IgnoredAny = map.next_value()?;
                }
            }
        }

        Ok(ExecutionReport {
            schema_version: version.ok_or_else(||
                A::Error::missing_field("schema_version")
            )?,
            initial_facts: facts.ok_or_else(||
                A::Error::missing_field("initial_facts")
            )?,
            rewrite: rewrite.ok_or_else(||
                A::Error::missing_field("rewrite")
            )?,
            rewrite_graph: ReasoningGraph {
                nodes: Vec::new(),
                edges: Vec::new(),
            },
            queries: vec![query.ok_or_else(||
                A::Error::missing_field("queries")
            )?],
        })
    }
}

#[cfg(test)]
mod tests
{
    use super::*;
    use crate::system::parser;
    use crate::trace::LogicValue;

    fn parse<'a>(source: &'a str) -> System<'a>
    {
        let mut system = System::new();
        parser(&mut system, source.lines().collect()).unwrap();
        system.compute_rules_for_symbols();
        system
    }

    #[test]
    fn disk_report_reads_one_selected_query_and_deletes_on_drop()
    {
        let source = "A=>B\nC=>!B\n=AC\n?AB";
        let system = parse(source);
        let store = ReportStore::generate(&system).unwrap();
        let path = store.path().to_path_buf();
        assert!(path.exists());

        let first = store.load_query(0).unwrap();
        assert_eq!(first.queries.len(), 1);
        assert_eq!(first.queries[0].symbol, 'A');
        assert_eq!(first.schema_version, 2);
        assert!(!first.rewrite.normalized_rules.is_empty());

        let second = store.load_query(1).unwrap();
        assert_eq!(second.queries.len(), 1);
        assert_eq!(second.queries[0].symbol, 'B');
        assert!(matches!(
            second.queries[0].outcome,
            crate::report::QueryOutcome::Value(LogicValue::Contradiction),
        ));

        // The on-disk JSON remains a complete, backward-compatible report.
        let file = File::open(&path).unwrap();
        let complete: ExecutionReport = serde_json::from_reader(file).unwrap();
        assert_eq!(complete.queries.len(), 2);
        assert_eq!(complete.queries[1].symbol, 'B');

        assert!(store.load_query(5).is_err());
        drop(store);
        assert!(!path.exists());
    }

    #[test]
    fn streamed_json_is_equivalent_to_existing_report_schema()
    {
        let source = "A<=>B+C\n=A\n?ABC";
        let system = parse(source);

        let expected = serde_json::to_value(system.execution_report()).unwrap();

        let mut streamed = Vec::new();
        system.write_execution_json(&mut streamed, false).unwrap();
        let actual: serde_json::Value = serde_json::from_slice(&streamed).unwrap();

        assert_eq!(actual, expected);
        assert_eq!(actual["queries"].as_array().unwrap().len(), 3);
    }
}
