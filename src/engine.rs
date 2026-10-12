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

/// Read the source's initial-facts declaration for frontend controls.
///
/// The engine parser still validates the input on Run. This lightweight
/// projection intentionally ignores malformed characters here so users can
/// correct bad source text in the editor without losing the fact controls.
pub fn source_initial_facts(source: &str) -> [bool; 26]
{
    let mut enabled = [false; 26];

    for raw in source.lines()
    {
        let line = raw.split('#').next().unwrap_or("").trim_start();
        if let Some(facts) = line.strip_prefix('=')
        {
            for symbol in facts.bytes()
            {
                if symbol.is_ascii_uppercase()
                {
                    enabled[(symbol - b'A') as usize] = true;
                }
            }
            break;
        }
    }

    enabled
}

/// Parse, normalize, and evaluate the *current source snapshot*.
///
/// An input/initialization error fails the entire run. An inference error
/// belongs to one query and is retained beside that query's symbol.
pub fn run_source(source: &str, mode: SolveMode) -> Result<RunReport, String>
{
    run_source_inner(source, mode, None)
}

/// Evaluate using an independently selected initial-fact set. Neither the
/// input string nor its '=...' declaration is changed.
pub fn run_source_with_facts(
    source: &str,
    mode: SolveMode,
    active_facts: [bool; 26],
) -> Result<RunReport, String>
{
    run_source_inner(source, mode, Some(active_facts))
}

fn run_source_inner(
    source: &str,
    mode: SolveMode,
    active_facts: Option<[bool; 26]>,
) -> Result<RunReport, String>
{
    let started = Instant::now();
    let mut system = System::new();

    parser(&mut system, source.lines().collect())?;
    if let Some(facts) = active_facts
    {
        system.replace_initial_facts(facts)?;
    }
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
        // B has no support: this engine's default-false behavior applies.
        assert_eq!(output.results[0].value, Ok(LogicValue::False));
    }

    #[test]
    fn source_fact_detection_skips_comments_and_whitespace()
    {
        let input = "# =AB ignored\nA=>B\n  = A C # D is commented\n?B";
        let facts = source_initial_facts(input);
        assert!(facts[0]);
        assert!(!facts[1]);
        assert!(facts[2]);
        assert!(!facts[3]);
    }

    #[test]
    fn runtime_fact_overrides_are_independent_of_source_text()
    {
        let source = "A=>B\nC=>D\n=A\n?BCD".to_owned();
        let saved = source.clone();

        let original = run_source(&source, SolveMode::Sequential).unwrap();
        assert_eq!(original.results[0].value, Ok(LogicValue::True));
        assert_eq!(original.results[2].value, Ok(LogicValue::False));

        let mut facts = [false; 26];
        facts[2] = true; // C enabled, A disabled (despite '=A' in source).

        let sequential = run_source_with_facts(&source, SolveMode::Sequential, facts).unwrap();
        let parallel = run_source_with_facts(&source, SolveMode::Parallel, facts).unwrap();

        assert_eq!(sequential.results, parallel.results);
        assert_eq!(sequential.results[0].value, Ok(LogicValue::False));
        assert_eq!(sequential.results[1].value, Ok(LogicValue::True));
        assert_eq!(source, saved);
    }

    #[test]
    fn override_does_not_hide_syntax_errors()
    {
        let error = run_source_with_facts("A=>\n=A\n?B", SolveMode::Parallel, [true; 26])
            .unwrap_err();
        assert!(error.contains("Line 1"));
    }

    #[test]
    fn contradictory_evidence_is_not_a_runtime_error()
    {
        let source = "A=>B\nC=>!B\n=AC\n?B";
        let report = run_source(source, SolveMode::Sequential).unwrap();
        assert_eq!(report.results[0].value, Ok(LogicValue::Contradiction));
    }
}
