use std::{env, fs};

mod lexer;
mod tokenizer;
mod grammar;
mod system;
mod ast;
mod trace;
mod report;

use system::System;

use crate::system::parser;

fn main() -> Result<(), std::io::Error>
{
    let parsing = std::time::Instant::now();
    let args: Vec<String> = env::args().collect();
    if args.len() < 2
    {
        eprintln!("You need to give a filename in argument");
        return Ok(());
    }
    let file = fs::read_to_string(args[1].clone())?;
    let split: Vec<&str> = file.lines().filter(|line| !line.trim().is_empty()).collect();
    let mut system = System::new();
    match parser(&mut system, split)
    {
        Ok(()) => 
        {
            println!("Finished parsing/rewrite in {:?}", parsing.elapsed());
        }
        Err(err) =>
        {
            eprintln!("{err}");
            return Ok(());
        }
    }
    let compute_symbol = std::time::Instant::now();
    system.compute_rules_for_symbols();
    println!("Finished symbol compute in {:?}", compute_symbol.elapsed());
    let parallel_solve = std::time::Instant::now();
    system.solve_parallel();
    println!("Finished parallel solve in {:?}", parallel_solve.elapsed());
    let normal_solve = std::time::Instant::now();
    system.solve();
    println!("Finished normal solve in {:?}", normal_solve.elapsed());

    if args.len() >= 4 && args[2] == "--report"
    {
        system.export_execution_json(&args[3])
            .map_err(std::io::Error::other)?;
        println!("Wrote reasoning report to {}", args[3]);
    }
    Ok(())
}
