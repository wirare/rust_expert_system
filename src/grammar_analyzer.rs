use crate::tokenizer::{Token, TokenType};

fn grammar_verify_par(tokens: Vec<Token>) -> Result<(), &'static str>
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

fn grammar_verify_conditional_token(tokens: Vec<Token>) -> Result<(), &'static str>
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
