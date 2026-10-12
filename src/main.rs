use std::{env, fs};

use expert_system::system::{parser, System};

fn main() -> Result<(), Box<dyn std::error::Error>>
{
    let parsing = std::time::Instant::now();
    let args: Vec<String> = env::args().collect();
    if args.len() < 2
    {
        eprintln!("You need to give a filename in argument");
        return Ok(());
    }
    let file = fs::read_to_string(args[1].clone())?;
    let split: Vec<&str> = file.lines().collect();
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

    if args.len() >= 4 && (args[2] == "--report" || args[2] == "--report-pretty")
    {
        if args[2] == "--report-pretty"
        {
            system.export_execution_json_pretty(&args[3])?;
        }
        else
        {
            system.export_execution_json(&args[3])?;
        }
        println!("Wrote reasoning report to {}", args[3]);
    }
    Ok(())
}
