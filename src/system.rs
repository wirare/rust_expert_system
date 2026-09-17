use crate::grammar_analyzer;
use crate::tokenizer::{Token, TokenType};

enum FactState
{
    True,
    False,
    Undetermined,
}

fn split_iff<'a>(rules: &mut Vec<Vec<Token<'a>>>)
{
    let mut new_rules: Vec<Vec<Token<'a>>> = Vec::new();
    let mut iff_rules_nb: Vec<usize> = Vec::new();

    for (n, rule) in rules.iter().enumerate()
    {
        let Some(iff_pos) = rule
            .iter()
            .position(|token| token.token_type == TokenType::Iff)
        else
        {
            continue;
        };

        iff_rules_nb.push(n);
       
        let lhs = &rule[1..iff_pos];
        let rhs = &rule[iff_pos + 1..rule.len()-1];
        
        let start = Token
        {
            token_type: TokenType::Start,
            value: "",
        };

        let then = Token
        {
            token_type: TokenType::Then,
            value: "=>",
        };

        let end = Token
        {
            token_type: TokenType::End,
            value: "",
        };

        let mut forward = Vec::new();

        forward.push(start);
        forward.extend_from_slice(lhs);
        forward.push(then);
        forward.extend_from_slice(rhs);
        forward.push(end);

        let mut backward = Vec::new();

        backward.push(start);
        backward.extend_from_slice(rhs);
        backward.push(then);
        backward.extend_from_slice(lhs);
        backward.push(end);

        new_rules.push(forward);
        new_rules.push(backward);
    }

    for &idx in iff_rules_nb.iter().rev()
    {
        rules.remove(idx);
    }

    rules.extend(new_rules);
}
