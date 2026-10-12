//! Solver-facing API for frontends.
//!
//! No UI toolkit types cross this boundary. The source string is borrowed only
//! for the duration of a single run, which keeps AST token lifetimes local.

use std::time::{Duration, Instant};

use crate::system::{System, parser};
use crate::trace::LogicValue;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SolveMode
{
    Sequential,
    Parallel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryResult
{
    pub symbol: char,
    pub value: Result<LogicValue, String>,
}

#[derive(Debug)]
pub struct RunReport
{
    pub mode: SolveMode,
    pub results: Vec<QueryResult>,
    pub normalized_rules: usize,
    pub parse_duration: Duration,
    pub solve_duration: Duration,
    pub total_duration: Duration,
}

/// Parse, normalize, and evaluate the *current source snapshot*.
///
/// An input/initialization error fails the entire run. An inference error
/// belongs to one query and is retained beside that query's symbol.
pub fn run_source(source: &str, mode: SolveMode) -> Result<RunReport, String>
{
    let started = Instant::now();
    let mut system = System::new();

    parser(&mut system, source.lines().collect())?;
    system.compute_rules_for_symbols();

    let parse_duration = started.elapsed();
    let solve_started = Instant::now();
    let outputs = match mode
    {
        SolveMode::Sequential => system.solve_results()?,
        SolveMode::Parallel => system.solve_parallel_results()?,
    };
    let solve_duration = solve_started.elapsed();

    Ok(RunReport
    {
        mode,
        normalized_rules: system.rewrite_history().normalized_rules.len(),
        results: outputs.into_iter().map(|(symbol, value)|
        {
            QueryResult { symbol, value }
        }).collect(),
        parse_duration,
        solve_duration,
        total_duration: started.elapsed(),
    })
}

#[cfg(test)]
mod tests
{
    use super::*;

    #[test]
    fn sequential_and_parallel_results_match()
    {
        let source = include_str!("../test.txt");
        let sequential = run_source(source, SolveMode::Sequential).unwrap();
        let parallel = run_source(source, SolveMode::Parallel).unwrap();

        assert_eq!(sequential.results, parallel.results);
        assert_eq!(sequential.results.len(), 26);
        assert!(sequential.results.iter().any(|result|
        {
            result.symbol == 'C' && result.value == Ok(LogicValue::Contradiction)
        }));
    }

    #[test]
    fn returns_reliable_parse_errors()
    {
        let bad_lexer = run_source("A=>B\n=ABC\n?B\n@", SolveMode::Sequential)
            .unwrap_err();
        assert!(bad_lexer.contains("Line 4"));
        assert!(bad_lexer.contains("Unknown token"));

        let bad_expression = run_source("A=>\n=A\n?B", SolveMode::Sequential)
            .unwrap_err();
        assert!(bad_expression.contains("Line 1"));

        let missing_query = run_source("A=>B\n=A", SolveMode::Sequential)
            .unwrap_err();
        assert!(missing_query.contains("Missing queries"));
    }

    #[test]
    fn accepts_comments_and_empty_facts()
    {
        let source = "# Heading\n\nA => B # rule\n= # no facts\n? B";
        let output = run_source(source, SolveMode::Sequential).unwrap();
        assert_eq!(output.results.len(), 1);
        assert_eq!(output.results[0].value, Ok(LogicValue::Undetermined));
    }

    #[test]
    fn contradictory_evidence_is_not_a_runtime_error()
    {
        let source = "A=>B\nC=>!B\n=AC\n?B";
        let report = run_source(source, SolveMode::Sequential).unwrap();
        assert_eq!(report.results[0].value, Ok(LogicValue::Contradiction));
    }
}
