use std::cmp::max;

use crate::tokenizer::{Token, TokenType};

pub fn lex<'a>(input: &'a str) -> Result <Vec<Token<'a>>, String>
{
    let mut pos = 0;

    let mut tokens: Vec<Token<'a>> = std::iter::from_fn(||
        {
            while pos < input.len()
                && input.as_bytes()[pos].is_ascii_whitespace()
            {
                pos += 1;
            }
            
            if pos >= input.len()
            {
                return None;
            }

            let remaining = &input[pos..];

            let remaining = remaining
                .split_once('#')
                .map_or(remaining, |(before, _)| before)
                .trim_end();

            let matched = TokenType::ALL
                .iter()
                .filter_map(|&token_type| 
                    {
                        token_type
                            .match_len(remaining)
                            .map(|len| (token_type, len))
                    })
                .max_by_key(|(_, len)| *len);

            match matched
            {
                Some((token_type, len)) =>
                {
                    let value = &remaining[..len];

                    pos += len;

                    Some(Ok(Token{token_type, value}))
                }

                None =>
                {
                    let error_pos = pos;
                    let c = remaining.chars().next().unwrap();

                    pos = input.len();

                    Some(Err(format!("Unknown token '{}' at byte {}", c, error_pos)))
                }
            }
        }).collect::<Result<Vec<_>, _>>()?;

    tokens.insert(0, Token
        {
            token_type: TokenType::Start,
            value: "",
        });

    tokens.push(Token
        {
            token_type: TokenType::End,
            value: "",
        });

    Ok(tokens)
}
