use crate::grammar;
use crate::tokenizer::{Token, TokenType};
use crate::ast::{ASTNode, build_ast, rpn, simplify_and_rewrite_rules};
use crate::lexer;

use std::collections::{HashMap, HashSet, VecDeque};
use std::ops::Deref;

pub type Rule<'a> = (ASTNode<'a>, HashSet<char>);
type Constraint = (u32, HashSet<(char, bool)>);

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
    visiting: HashSet<(char, FactState)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ProofState
{
    positive: bool,
    negative: bool,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
enum FactState
{
    True,
    False,
    Undetermined,
    Contradiction,
}

#[derive(PartialEq, Eq)]
enum Operation
{
    And,
    Or,
    Not,
    Xor,
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

    fn proves(&self, target: FactState) -> bool
    {
        match target
        {
            FactState::True => self.positive,
            FactState::False => self.negative,
            _ => false,
        }
    }
}

fn symbol_to_nb(symbol: char) -> usize
{
    symbol as usize - 'A' as usize
}

impl From<TokenType> for Operation
{
	fn from(tt: TokenType) -> Self
	{
		match tt
		{
			TokenType::Not => Operation::Not,
			TokenType::And => Operation::And,
			TokenType::Or => Operation::Or,
			TokenType::Xor => Operation::Xor,
			_ => unreachable!(),
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
            facts_filled: false,
            query: Vec::new(),
            query_filled: false,
            rules: Vec::new(),
            rules_for_symbol: std::array::from_fn(|_| (Vec::new(), false)),
            base_state: [ProofState::new(false, false); 26],
            symbols_state: [ProofState::new(false, false); 26],
            constraints: Vec::new(),
            visiting: HashSet::new(),
        }
    }

    pub fn add_rule(&mut self, rule: Rule<'a>)
    {
        self.rules.push(rule);
    }

    pub fn add_rules(&mut self, rules: Vec<Rule<'a>>)
    {
        self.rules = rules;
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

    fn get_constraint_for_symbol(&self, symbol: char, polarity: bool) -> Vec<&Constraint>
    {
        self.constraints
            .iter()
            .filter(|constraint| constraint.1.contains(&(symbol, polarity)))
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

    fn mark_proven(&mut self, symbol: char, target: FactState)
    {
        let state = &mut self.symbols_state[symbol_to_nb(symbol)];

        match target
        {
            FactState::True => state.positive = true,
            FactState::False => state.negative = true,
            _ => unreachable!(),
        }
    }

    fn do_operation(op: Operation, lhs: FactState, rhs: Option<FactState>) -> FactState
    {
        if matches!(lhs, FactState::Undetermined | FactState::Contradiction)
        {
            return lhs;
        }
        if op != Operation::Not && matches!(rhs, Some(FactState::Undetermined) | Some(FactState::Contradiction))
        {
            return rhs.unwrap();
        }
        match op
        {
            Operation::Not =>
            {
                match lhs
                {
                    FactState::True => FactState::False,
                    FactState::False => FactState::True,
                    _ => unreachable!(),
                }
            }

            Operation::And =>
            {
                if lhs == FactState::True && rhs.unwrap() == FactState::True
                {
                    FactState::True
                }
                else
                {
                    FactState::False
                }
            }

            Operation::Or =>
            {
                if lhs == FactState::True || rhs.unwrap() == FactState::True
                {
                    FactState::True
                }
                else
                {
                    FactState::False
                }
            }

            Operation::Xor => 
            {
                if (lhs == FactState::True) ^ (rhs.unwrap() == FactState::True)
                {
                    FactState::True
                }
                else
                {
                    FactState::False
                }
            }
        }
    }

    fn evaluate_symbol(&mut self, symbol: char) -> Result<FactState, String>
    {
        let index = symbol_to_nb(symbol);

        let mut positive = self.symbols_state[index].positive;
        let mut negative = self.symbols_state[index].negative;

        if !positive
        {
            positive = self.can_prove(symbol, FactState::True)?;
        }

        if !negative
        {
            negative = self.can_prove(symbol, FactState::False)?;
        }

        Ok(match (positive, negative)
            {
                (true, true) => FactState::Contradiction,
                (true, false) => FactState::True,
                (false, true) => FactState::False,
                (false, false) => FactState::False,
            })
    }

    fn evaluate_expression_from_ast(&mut self, exp: &ASTNode) -> Result<FactState, String>
    {
        let lhs: FactState;
        let mut rhs: Option<FactState> = None;

        if exp.token.is_operator()
        {
            let left = exp
                .left
                .as_deref()
                .unwrap();

            lhs = self.evaluate_expression_from_ast(left)?;
            
            if exp.token.token_type != TokenType::Not
            {
                let right = exp
                    .right
                    .as_deref()
                    .unwrap();

                rhs = Some(self.evaluate_expression_from_ast(right)?);
            }

            return Ok(System::do_operation(exp.token.token_type.into(), lhs, rhs));
        }
        let symbol = exp
            .token
            .value
            .chars()
            .next()
            .unwrap();

        Ok(self.evaluate_symbol(symbol)?)
    }

    fn evaluate_expression_from_idx(&mut self, rule_idx: usize) -> Result<FactState, String>
    {
        let expression = self.rules[rule_idx]
            .0
            .left
            .as_deref()
            .unwrap()
            .clone();

        self.evaluate_expression_from_ast(&expression)
    }

    fn conclusion_matches_goal(conclusion: &ASTNode<'a>, symbol: char, target: FactState) -> bool
    {
        let token = conclusion.token;

        match target
        {
            FactState::True =>
            {
                token.token_type == TokenType::Symbol && token.value.chars().next() == Some(symbol)
            }

            FactState::False =>
            {
                if token.token_type != TokenType::Not
                {
                    return false;
                }

                let child_token = conclusion.left.as_deref().unwrap().token;

                child_token.token_type == TokenType::Symbol && child_token.value.chars().next() == Some(symbol)
            }

            _ => false,
        }
    }
    
    fn can_prove(&mut self, symbol: char, target_state: FactState) -> Result<bool, String>
    {
        if matches!(target_state, FactState::Undetermined | FactState::Contradiction)
        {
            return Err("target_state for can_prove() can only be FactState::{True,False}".to_string());
        }
        
        let index = symbol_to_nb(symbol);

        if self.symbols_state[index].proves(target_state)
        {
            return Ok(true);
        }

        let visiting_key = (symbol, target_state);

        if self.visiting.contains(&visiting_key)
        {
            return Ok(false);
        }

        self.visiting.insert(visiting_key);

        let result = (|| -> Result<bool, String>
            {
                let rules_to_visit = self
                    .get_rules_for_symbol(symbol)
                    .to_vec();

                for rule_idx in rules_to_visit
                {
                    let premise = self.evaluate_expression_from_idx(rule_idx)?;

                    match premise
                    {
                        FactState::True => {}

                        FactState::False | FactState::Undetermined =>
                        {
                            continue;
                        }

                        FactState::Contradiction =>
                        {
                            return Err(format!("Contradiction while evaluating rule {}", rule_idx));
                        }
                    }
                    
                    let conclusion = self.rules[rule_idx]
                        .0
                        .right
                        .as_deref()
                        .unwrap();

                    match conclusion.token.token_type
                    {
                        TokenType::Symbol | TokenType::Not =>
                        {
                            let matches_goal = System::conclusion_matches_goal(conclusion, symbol, target_state);

                            if matches_goal
                            {
                                self.mark_proven(symbol, target_state);
                                return Ok(true);
                            }

                        }

                        TokenType::Or =>
                        {

                        }

                        _ => unreachable!()
                    }
                }

                Ok(false)
            })();

        self.visiting.remove(&visiting_key);

        result
    }

    pub fn solve(&mut self)
    {
        for symbol in self.query.clone()
        {
            self.symbols_state = self.base_state;
            self.visiting.clear();
            println!("{}: {:?}", symbol, self.evaluate_symbol(symbol));
        }
    }
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

    let mut fact_found = false;
    let mut query_found = false;
    let mut asts = Vec::new();

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
        asts.push(build_ast(rpn(tokens.clone()))?);
    }

    system.add_rules(simplify_and_rewrite_rules(asts));

    Ok(())
}
