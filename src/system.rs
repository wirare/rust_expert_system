use crate::grammar;
use crate::tokenizer::{Token, TokenType};
use crate::ast::{ASTNode, rpn, build_ast};
use crate::lexer;

use std::collections::{HashSet, VecDeque};

pub type Rule<'a> = ASTNode<'a>;
type Constraint<'a> = (ASTNode<'a>, HashSet<char>);

#[derive(Debug)]
pub struct System<'a>
{
    facts: Vec<char>,
    query: Vec<char>,
    rules: Vec<Rule<'a>>,
    symbols_state: [ProofState;26],
    constraints: Vec<Constraint<'a>>,
}

#[derive(Debug, Clone, Copy)]
struct ProofState
{
    positive: bool,
    negative: bool,
}

enum FactState
{
    True,
    False,
    Undetermined,
    Contradiction,
}

impl ProofState
{
    fn new(p: bool, n: bool) -> Self
    {
        Self
        {
            positive: p,
            negative: n,
        }
    }

    fn state(&self) -> FactState
    {
        match (self.positive, self.negative)
        {
            (false, false) => FactState::False,
            (false, true) => FactState::False,
            (true, false) => FactState::True,
            (true, true) => FactState::Contradiction,
        }
    }
}

impl<'a> System<'a>
{
    pub fn new() -> Self
    {
        Self
        {
            facts: Vec::new(),
            query: Vec::new(),
            rules: Vec::new(),
            symbols_state: [ProofState::new(false, false); 26],
            constraints: Vec::new(),
        }
    }

    pub fn add_rule(&mut self, rule: Rule<'a>)
    {
        self.rules.push(rule);
    }

    pub fn get_rules(&self) -> &Vec<Rule<'a>>
    {
        &self.rules
    }

    pub fn set_fact(&mut self, facts: Vec<char>)
    {
        self.facts = facts;
    }

    pub fn get_fact(&self) -> &Vec<char>
    {
        &self.facts
    }

    pub fn set_query(&mut self, query: Vec<char>)
    {
        self.query = query;
    }

    pub fn get_query(&self) -> &Vec<char>
    {
        &self.query
    }

    fn can_prove(&self, symbol: char, state: bool) -> bool
    {
        
        false
    }
}

fn rewrite_rules<'a, F>(rules: &mut Vec<Vec<Token<'a>>>, rewrite: F)
where
	F: Fn(&[Token<'a>]) -> Option<Vec<Vec<Token<'a>>>>,
{
    let mut pending: VecDeque<Vec<Token<'a>>> = rules.drain(..).collect();
    let mut result: Vec<Vec<Token<'a>>> = Vec::new();

    while let Some(rule) = pending.pop_front()
    {
        if let Some(remplacements) = rewrite(&rule)
        {
            for remplacement in remplacements.into_iter().rev()
            {
                pending.push_front(remplacement);
            }
        }
        else
        {
            result.push(rule);
        }
    }

    *rules = result;
}

fn rewrite_iff<'a>(rule: &[Token<'a>]) -> Option<Vec<Vec<Token<'a>>>>
{
    let Some(iff_pos) = rule
        .iter()
        .position(|token| token.token_type == TokenType::Iff)
    else
    {
        return None;
    };

    let lhs = &rule[1..iff_pos];
    let rhs = &rule[iff_pos + 1..rule.len() - 1];

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
    
    Some(vec![forward, backward])
}

fn rewrite_and_conclusion<'a>(rule: &[Token<'a>]) -> Option<Vec<Vec<Token<'a>>>>
{
    let Some(then_pos) = rule
        .iter()
        .position(|token| token.token_type == TokenType::Then)
    else
    {
        return None;
    };

    let mut depth = 0;
    let mut and_pos = None;

    for i in then_pos + 1..rule.len() - 1
    {
        match rule[i].token_type
        {
            TokenType::Lp =>
            {
                depth += 1;
            }

            TokenType::Rp =>
            {
                depth -= 1;
            }

            TokenType::And if depth == 0 =>
            {
                and_pos = Some(i);
                break;
            }

            _ => {}
        }
    }

    let and_pos = and_pos?;

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

    let lhs = &rule[..then_pos];

    let and_lhs = &rule[then_pos + 1..and_pos];
    let and_rhs = &rule[and_pos + 1..rule.len() - 1];

    let mut first = Vec::new();
    let mut second = Vec::new();

    first.extend_from_slice(lhs);
    first.push(then);
    first.extend_from_slice(and_lhs);
    first.push(end);

    second.extend_from_slice(lhs);
    second.push(then);
    second.extend_from_slice(and_rhs);
    second.push(end);

    Some(vec![first, second])
}

pub fn parser<'a>(system: &mut System<'a>, lines: Vec<&'a str>) -> Result<(), String>
{
    let mut token_list: Vec<Vec<Token<'a>>> = match lines
        .into_iter()
        .map(lexer::lex)
        .collect()
    {
        Ok(tokens) => tokens,

        Err(err) =>
        {
            eprintln!("Lexer error: {err}");
            return Ok(());
        }
    };
    
    rewrite_rules(&mut token_list, rewrite_iff);
    rewrite_rules(&mut token_list, rewrite_and_conclusion);

    let mut fact_found = false;
    let mut query_found = false;

    for tokens in &mut token_list
    {
        grammar::check_grammar(&tokens)?;	
        
        tokens.remove(0);
	    tokens.pop();
        
        grammar::verify_par(&tokens)?;
        grammar::verify_conditional_token(&tokens)?;
        if grammar::check_fact(&tokens)?
        {
            if fact_found
            {
                return Err("Two or more Fact token in file".to_string());
            }
            fact_found = true;
            system.set_fact(
                tokens[1..]
                .iter()
                .map(|token| token.value.chars().next().unwrap())
                .collect()
            );
            continue;
        }
        if grammar::check_query(&tokens)?
        {
            if query_found
            {
                return Err("Two or more Query token in file".to_string());
            }
            query_found = true;
            system.set_query(
                tokens[1..]
                .iter()
                .map(|token| token.value.chars().next().unwrap())
                .collect()
            );
            continue;
        }

        system.add_rule(build_ast(rpn(tokens.clone()))?);
    }
    Ok(())
}
