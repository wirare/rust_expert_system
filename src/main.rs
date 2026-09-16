use std::fs;

mod lexer;
mod tokenizer;
mod grammar_analyzer;

use tokenizer::Token;

fn main() -> Result<(), std::io::Error>
{
    let file = fs::read_to_string("test.txt")?;
    let split: Vec<&str> = file.lines().filter(|line| !line.trim().is_empty()).collect();
    let tokens: Vec<Vec<Token>> = match split
        .iter()
        .map(|str| lexer::lex(str))
        .collect()
    {
        Ok(tokens) => tokens,

        Err(err) =>
        {
            eprintln!("Lexer error: {err}");
            return Ok(());
        }
    };

    print!("{:#?}", tokens);
    Ok(())
}
