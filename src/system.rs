use crate::grammar;
use crate::tokenizer::{Token, TokenType};
use crate::ast::{ASTNode, rpn, build_ast};
use crate::lexer;

use std::collections::{HashSet, VecDeque};

pub type Rule<'a> = (ASTNode<'a>, HashSet<char>);
type Constraint = (u32, (HashSet<char>, ProofState));

#[derive(Debug)]
pub struct System<'a>
{
    facts: Vec<char>,
    facts_filled: bool,
    query: Vec<char>,
    query_filled: bool,
    rules: Vec<Rule<'a>>,
    rules_for_symbol: [(Vec<usize>, bool);26],
    base_state: [ProofState;26],
    symbols_state: [ProofState;26],
    constraints: Vec<Constraint>,
    visited: [bool; 26],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
            (false, false) => FactState::Undetermined,
            (false, true) => FactState::False,
            (true, false) => FactState::True,
            (true, true) => FactState::Contradiction,
        }
    }
}

fn symbol_to_nb(symbol: char) -> usize
{
    symbol as usize - 'A' as usize
}

impl<'a> System<'a>
{
    pub fn new() -> Self
    {
        Self
        {
            facts: Vec::new(),
            facts_filled: false,
            query: Vec::new(),
            query_filled: false,
            rules: Vec::new(),
            rules_for_symbol: std::array::from_fn(|_| (Vec::new(), false)),
            base_state: [ProofState::new(false, false); 26],
            symbols_state: [ProofState::new(false, false); 26],
            constraints: Vec::new(),
            visited: [false; 26],
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

    pub fn set_fact(&mut self, facts: Vec<char>) -> Result<(), String>
    {
        if self.facts_filled
        {
            return Err("Facts are already set".to_string());
        }
        self.facts = facts;
        self.facts_filled = true;
        for fact in &self.facts
        {
            self.base_state[symbol_to_nb(*fact)] = ProofState::new(true, false);
        }
        Ok(())
    }

    pub fn get_fact(&self) -> &Vec<char>
    {
        &self.facts
    }

    pub fn set_query(&mut self, query: Vec<char>) -> Result<(), String>
    {
        if self.query_filled
        {
            return Err("Query are already set".to_string());
        }
        self.query = query;
        self.query_filled = true;
        Ok(())
    }

    pub fn get_query(&self) -> &Vec<char>
    {
        &self.query
    }

    fn set_symbol_state(&mut self, symbol: char, state: ProofState)
    {
        self.symbols_state[symbol_to_nb(symbol)] = state;
    }

    fn get_symbol_state(&self, symbol: char) -> ProofState
    {
        self.symbols_state[symbol_to_nb(symbol)]
    }

    fn add_constraint(&mut self, constraint: Constraint)
    {
        self.constraints.push(constraint);
    }

    fn get_constraint_for_symbol(&self, symbol: char) -> Vec<&Constraint>
    {
        self.constraints
            .iter()
            .filter(|constraint| constraint.1.0.contains(&symbol))
            .collect()
    }
    
    fn get_rules_for_symbol(&mut self, symbol: char) -> &[usize]
    {
        let index = symbol_to_nb(symbol);

        if !self.rules_for_symbol[index].1
        {
            self.rules_for_symbol[index].0 = self
                .rules
                .iter()
                .enumerate()
                .filter_map(|(i, rule)|
                    {
                        rule.1
                            .contains(&symbol)
                            .then_some(i)
                    })
                .collect();

            self.rules_for_symbol[index].1 = true;
        }

        &self.rules_for_symbol[index].0
    }


/*
    fn can_prove(&mut self, symbol: char, state: bool) -> Result<bool, String>
    {
        let index = symbol_to_nb(symbol);

        if self.visited[index]
        {
            return Err(format!("Cycle detected while trying to prove {} state for {}", state, symbol).to_string());
        }

        self.visited[index] = true;

        let rules_index_to_explore = self.get_rules_for_symbol(symbol).to_vec();

        for rule_idx in rules_index_to_explore
        {
            let rule = &self.rules[rule_idx];

            if rule
                .0
                .right
                .as_ref()
                .unwrap()
                .contain_token_mask(TokenType::Or | TokenType::Xor)
            {
                
            }
        }

        Ok(false)
    }
*/
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
            )?;
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
            )?;
            continue;
        }

        system.add_rule(build_ast(rpn(tokens.clone()))?);
    }
    Ok(())
}
