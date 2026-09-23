use std::fs;

mod lexer;
mod tokenizer;
mod grammar;
mod system;
mod ast;

use system::System;

use crate::system::parser;

fn main() -> Result<(), std::io::Error>
{
    let file = fs::read_to_string("test.txt")?;
    let split: Vec<&str> = file.lines().filter(|line| !line.trim().is_empty()).collect();
    let mut system = System::new();
    let _ =parser(&mut system, split);
    Ok(())
}
