use crate::tokenizer::{Token, TokenType, TokenMask};
use std::collections::HashMap;
use std::sync::LazyLock;

pub fn verify_par(tokens: &Vec<Token>) -> Result<(), &'static str>
{
    let mut par_count = 0;
    for token in tokens
    {
        let tt = token.token_type;
        if matches!(tt, TokenType::Then | TokenType::Iff)
        {
            if par_count != 0
            {
                return Err("Conditional token in between parenthesis");
            }
            continue;
        }
        if tt == TokenType::Lp
        {
            par_count += 1;
            continue;
        }
        if tt == TokenType::Rp
        {
            par_count -= 1;
            if par_count < 0
            {
                return Err("Unmatched opening parenthesis");
            }
        }
    }
    if par_count != 0
    {
        return Err("Unmatche closing parenthesis");
    }
    Ok(())
}

pub fn verify_conditional_token(tokens: &Vec<Token>) -> Result<(), &'static str>
{
    let mut cond_token_found = false;
    let mut fact_or_query_token_found = false;

    for token in tokens
    {
        let tt = token.token_type;
        if matches!(tt, TokenType::Fact | TokenType::Query)
        {
            if cond_token_found
            {
                if tt == TokenType::Fact
                {
                    return Err("Fact can't be conditional");
                }
                else
                {
                    return Err("Query can't be conditional");
                }
            }
            if fact_or_query_token_found
            {
                return Err("Two or more Fact/Query tokens found");
            }
            fact_or_query_token_found = true;
            continue;
        }
        if matches!(tt, TokenType::Then | TokenType::Iff)
        {
            if cond_token_found
            {
                return Err("Two or more Conditional tokens found");
            }
            cond_token_found = true;
            continue;
        }
    }
    if !cond_token_found && !fact_or_query_token_found
    {
        return Err("Missing Conditional or Fact or Query token");
    }
    Ok(())
}

pub fn check_fact(tokens: &Vec<Token>) -> Result<bool, &'static str>
{
    let mut fact_found = false;
    let mut symbols: Vec<char> = vec![];

    for token in tokens
    {
        let tt = token.token_type;
        if tt == TokenType::Fact
        {
            if fact_found
            {
                return Err("More than one Fact token found");
            }
            fact_found = true;
            continue;
        }
        if fact_found && tt == TokenType::Symbol
        {
            let val = token.value.chars().next().unwrap();
            if symbols.contains(&val)
            {
                return Err("Duplicate Symbol token in Fact");
            }
            symbols.push(val);
            continue;
        }
        if fact_found && tt != TokenType::Symbol
        {
            return Err("Token other than Symbol found in Fact");
        }
    }
    Ok(fact_found)
}

pub fn check_query(tokens: &Vec<Token>) -> Result<bool, &'static str>
{
    let mut query_found = false;
    let mut symbols: Vec<char> = vec![];

    for token in tokens
    {
        let tt = token.token_type;
        if tt == TokenType::Query
        {
            if query_found
            {
                return Err("More than one Query token found");
            }
            query_found = true;
            continue;
        }
        if query_found && tt == TokenType::Symbol
        {
            let val = token.value.chars().next().unwrap();
            if symbols.contains(&val)
            {
                return Err("Duplicate Symbol token in Query");
            }
            symbols.push(val);
            continue;
        }
        if query_found && tt != TokenType::Symbol
        {
            return Err("Token other than Symbol found in Query");
        }
    }
    Ok(query_found)
}

static GRAMMAR_MAP: LazyLock<HashMap<TokenType, TokenMask>> = LazyLock::new(||
    {
        HashMap::from(
            [
                (
                    TokenType::Start,
                    TokenType::Symbol | TokenType::Not | TokenType::Lp | TokenType::Query | TokenType::Fact,
                ),
                (
                    TokenType::Symbol,
                    TokenType::Xor | TokenType::Or | TokenType::And | TokenType::Cond | TokenType::Rp | TokenType::Symbol | TokenType::End,
                ),
                (
                    TokenType::Not,
                    TokenType::Symbol | TokenType::Lp | TokenType::Not,
                ),
                (
                    TokenType::Xor,
                    TokenType::Not | TokenType::Symbol | TokenType::Lp,
                ),                
                (
                    TokenType::Or,
                    TokenType::Not | TokenType::Symbol | TokenType::Lp,
                ),
                (
                    TokenType::And,
                    TokenType::Not | TokenType::Symbol | TokenType::Lp,
                ),
                (
                    TokenType::Query,
                    TokenType::Symbol.into(),
                ),
                (
                    TokenType::Then,
                    TokenType::Lp | TokenType::Symbol | TokenType::Not,
                ),
                (
                    TokenType::Iff,
                    TokenType::Lp | TokenType::Symbol | TokenType::Not,
                ),
                (
                    TokenType::Fact,
                    TokenType::Symbol | TokenType::End,
                ),
                (
                    TokenType::Lp,
                    TokenType::Not | TokenType::Symbol,
                ),
                (
                    TokenType::Rp,
                    TokenType::Cond | TokenType::Or | TokenType::Xor | TokenType::And | TokenType::Rp | TokenType::End,
                ),
            ]
        )
    });

pub fn check_grammar(tokens: &Vec<Token>) -> Result<(), String>
{
    let mut current: &Token = tokens.iter().nth(1).unwrap();
    let mut prev: &Token = tokens.iter().nth(0).unwrap();
    let mut i = 2;

    while current.token_type != TokenType::End
    {
        if !GRAMMAR_MAP[&prev.token_type].contains(current.token_type)
        {
            return Err(format!("Unexpected token: {}", current.value));
        }
        prev = current;
        current = tokens.iter().nth(i).unwrap();
        i += 1;
    }
    Ok(())
}
