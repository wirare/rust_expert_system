use crate::grammar;
use crate::tokenizer::{Token, TokenType};
use crate::ast::{ASTNode, build_ast, rpn, rewrite_with_history};
use crate::lexer;
use crate::trace::{EvaluationTrace, TraceEventKind, Literal, ProofOrigin, SearchOutcome, ConstraintOutcome, EventId};
use crate::report::{ExecutionReport, QueryExecution, QueryOutcome, RewriteHistory};

use std::collections::{HashSet, VecDeque};
use std::thread;

pub type Rule<'a> = (ASTNode<'a>, HashSet<char>);
type Constraint = HashSet<(char, bool)>;

#[derive(Debug)]
struct EvaluationCtx
{
    symbols_state: [ProofState;26],
    constraints: Vec<Option<Constraint>>,
    visiting: HashSet<(char, FactState)>,
    trace: Option<EvaluationTrace>,
    attempt_stack: Vec<u64>,
    active_attempts: std::collections::HashMap<(char, FactState), u64>,
    proof_events: [Option<EventId>; 52],
}

#[derive(Debug)]
pub struct System<'a>
{
    facts: Vec<char>,
    facts_filled: bool,
    query: Vec<char>,
    query_filled: bool,
    rules: Vec<Rule<'a>>,
    rules_for_symbol: [Vec<usize>;26],
    rules_for_symbol_calculated: bool,
    base_state: [ProofState;26],
    rewrite_history: RewriteHistory,
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

#[derive(Debug, PartialEq, Eq)]
enum LiteralState
{
    Satisfied,
    Refuted,
    Unresolved,
    Contradiction,
}

#[derive(Debug, PartialEq, Eq)]
enum ConstraintState
{
    Satisfied,
    Unresolved,
    Forced(char, FactState),
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

impl EvaluationCtx
{
    pub fn new() -> Self
    {
        Self
        {
            symbols_state: [ProofState::new(false, false); 26],
            constraints: Vec::new(),
            visiting: HashSet::new(),
            trace: None,
            attempt_stack: Vec::new(),
            active_attempts: std::collections::HashMap::new(),
            proof_events: [None; 52],
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
            rules_for_symbol: std::array::from_fn(|_| Vec::new()),
            rules_for_symbol_calculated: false,
            base_state: [ProofState::new(false, false); 26],
            rewrite_history: RewriteHistory::default(),
        }
    }

    pub fn add_rules(&mut self, rules: Vec<Rule<'a>>)
    {
        self.rules = rules;
    }

    pub fn rewrite_history(&self) -> &RewriteHistory
    {
        &self.rewrite_history
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

    fn build_constraint(node: ASTNode, set: &mut HashSet<(char, bool)>, previous_is_not: bool) -> bool
    {
        match node.token.token_type
        {
            TokenType::Symbol =>
            {
                let symbol = node.token.value.chars().next().unwrap();
                let polarity = !previous_is_not;

                if set.contains(&(symbol, !polarity))
                {
                    return true;
                }

                set.insert((symbol, polarity));

                false
            }

            TokenType::Not =>
            {
                System::build_constraint(*node.left.unwrap(), set, !previous_is_not)
            }

            TokenType::Or =>
            {
                if let Some(left) = node.left
                {
                    if System::build_constraint(*left, set, false)
                    {
                        return true;
                    }
                }

                if let Some(right) = node.right
                {
                    if System::build_constraint(*right, set, false)
                    {
                        return true;
                    }
                }

                false
            }

            _ =>
            {
                unreachable!();
            }
        }
    }

    fn add_constraint(eval_ctx: &mut EvaluationCtx, node: ASTNode, rule_idx: usize) -> bool
    {
        let mut set: HashSet<(char, bool)> = HashSet::new();

        let tautology = System::build_constraint(node, &mut set, false);
        
        if tautology
        {
            eval_ctx.constraints[rule_idx] = None;
            return false;
        }
        
        eval_ctx.constraints[rule_idx] = Some(set);
        true
    }

    fn get_constraint_for_symbol(eval_ctx: &EvaluationCtx, symbol: char) -> Vec<usize>
    {
        eval_ctx.constraints
            .iter()
            .enumerate()
            .filter_map(|(idx, constraint)|
            {
                let constraint = constraint.as_ref()?;

                if constraint.iter().any(|(sy, _)| *sy == symbol)
                {
                    Some(idx)
                }
                else
                {
                    None
                }
            })
            .collect()
    }

    pub fn compute_rules_for_symbols(&mut self)
    {
        if self.rules_for_symbol_calculated
        {
            return;
        }

        for symbol in 'A'..='Z'
        {
            let index = symbol_to_nb(symbol);

            self.rules_for_symbol[index] = self
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
        }

        self.rules_for_symbol_calculated = true;
    }
    
    fn get_rules_for_symbol(&self, symbol: char) -> &[usize]
    {
        let index = symbol_to_nb(symbol);
        &self.rules_for_symbol[index]
    }

    fn mark_proven(eval_ctx: &mut EvaluationCtx, symbol: char, target: FactState, origin: ProofOrigin, causes: Vec<EventId>) -> bool
    {
        let state = &mut eval_ctx.symbols_state[symbol_to_nb(symbol)];

        match target
        {
            FactState::True => 
            {
                if state.positive == true
                {
                    return false;
                }
                state.positive = true;
            }

            FactState::False =>
            {
                if state.negative == true
                {
                    return false;
                }
                state.negative = true;
            }
            _ => unreachable!(),
        }

        let literal = Literal { symbol, positive: target == FactState::True };
        let event = Self::emit(eval_ctx, TraceEventKind::LiteralProven { literal, origin }, causes);
        if let Some(id) = event
        {
            let slot = symbol_to_nb(symbol) * 2 + usize::from(!literal.positive);
            eval_ctx.proof_events[slot] = Some(id);
        }
        true
    }

    fn emit(ctx: &mut EvaluationCtx, kind: TraceEventKind, causes: Vec<EventId>) -> Option<EventId>
    {
        let attempt_id = ctx.attempt_stack.last().copied();
        ctx.trace.as_mut().map(|trace| trace.record(attempt_id, kind, causes))
    }

    fn mark_from_rule(ctx: &mut EvaluationCtx, symbol: char, state: FactState, rule_id: usize) -> bool
    {
        Self::mark_proven(ctx, symbol, state, ProofOrigin::Rule { rule_id }, Vec::new())
    }

    fn mark_from_constraint(ctx: &mut EvaluationCtx, symbol: char, state: FactState, constraint_id: usize) -> bool
    {
        let causes = ctx.constraints[constraint_id].as_ref().map(|clause|
        {
            clause.iter().filter_map(|&(other, polarity)|
            {
                if other == symbol { return None; }
                let slot = symbol_to_nb(other) * 2 + usize::from(polarity);
                ctx.proof_events[slot]
            }).collect::<Vec<_>>()
        }).unwrap_or_default();
        Self::mark_proven(ctx, symbol, state, ProofOrigin::UnitPropagation { constraint_id }, causes)
    }

    fn do_operation(op: Operation, lhs: FactState, rhs: Option<FactState>) -> FactState
    {
        if lhs == FactState::Contradiction
        {
            return FactState::Contradiction;
        }

        if rhs == Some(FactState::Contradiction)
        {
            return FactState::Contradiction;
        }

        match op
        {
            Operation::Not =>
            {
                match lhs
                {
                    FactState::True             => FactState::False,
                    FactState::False            => FactState::True,
                    FactState::Undetermined     => FactState::Undetermined,
                    FactState::Contradiction    => unreachable!(),
                }
            }

            Operation::And =>
            {
                let rhs = rhs.expect("Binary operation requires rhs");
                match (lhs, rhs)
                {
                    (FactState::False, _) | (_, FactState::False)   => FactState::False,
                    (FactState::True, FactState::True)              => FactState::True,
                    _                                               => FactState::Undetermined,
                }
            }

            Operation::Or =>
            {
                let rhs = rhs.expect("Binary operation requires rhs");
                match (lhs, rhs)
                {
                    (FactState::True, _) | (_, FactState::True) => FactState::True,
                    (FactState::False, FactState::False)        => FactState::False,
                    _                                           => FactState::Undetermined,
                }
            }

            Operation::Xor =>
            {
                let rhs = rhs.expect("Binary operation requires rhs");
                match (lhs, rhs)
                {
                    (FactState::Undetermined, _)        | (_, FactState::Undetermined)          => FactState::Undetermined,
                    (FactState::True, FactState::False) | (FactState::False, FactState::True)   => FactState::True,
                    (FactState::True, FactState::True)  | (FactState::False, FactState::False)  => FactState::False,
                    _                                                                           => unreachable!(),
                }
            }
        }
    }

    fn is_symbol_in_unresolved_constraint(&self, eval_ctx: &EvaluationCtx, symbol: char) -> bool
    {
        let constraints = System::get_constraint_for_symbol(eval_ctx, symbol);

        for idx in constraints
        {
            let res = self.analyze_constraint(eval_ctx, idx);

            if !matches!(res.0, ConstraintState::Unresolved)
            {
                continue;
            }

            let constraint = eval_ctx.constraints[idx]
                .as_ref()
                .unwrap();

            for &(sy, polarity) in constraint
            {
                if sy == symbol && self.classify_symbol(eval_ctx, sy, polarity) == LiteralState::Unresolved
                {
                    return true;
                }
            }
        }

        false
    }

    fn evaluate_symbol(&self, eval_ctx: &mut EvaluationCtx, symbol: char) -> Result<FactState, String>
    {
        let index = symbol_to_nb(symbol);

        let mut positive = eval_ctx.symbols_state[index].positive;
        let mut negative = eval_ctx.symbols_state[index].negative;

        if !positive
        {
            self.can_prove(eval_ctx, symbol, FactState::True)?;
        }

        if !negative
        {
            self.can_prove(eval_ctx, symbol, FactState::False)?;
        }

        positive = eval_ctx.symbols_state[index].positive;
        negative = eval_ctx.symbols_state[index].negative;

        Ok(match (positive, negative)
            {
                (true, true) => FactState::Contradiction,
                (true, false) => FactState::True,
                (false, true) => FactState::False,
                (false, false) => 
                {
                    if self.is_symbol_in_unresolved_constraint(eval_ctx, symbol)
                    {
                        FactState::Undetermined
                    }
                    else
                    {
                        FactState::False
                    }
                }
            })
    }

    fn evaluate_expression_from_ast(&self, eval_ctx: &mut EvaluationCtx, exp: &ASTNode) -> Result<FactState, String>
    {
        let lhs: FactState;
        let mut rhs: Option<FactState> = None;

        if exp.token.is_operator()
        {
            let left = exp
                .left
                .as_deref()
                .unwrap();

            lhs = self.evaluate_expression_from_ast(eval_ctx, left)?;
            
            if exp.token.token_type != TokenType::Not
            {
                let right = exp
                    .right
                    .as_deref()
                    .unwrap();

                rhs = Some(self.evaluate_expression_from_ast(eval_ctx, right)?);
            }

            return Ok(System::do_operation(exp.token.token_type.into(), lhs, rhs));
        }
        let symbol = exp
            .token
            .value
            .chars()
            .next()
            .unwrap();

        Ok(self.evaluate_symbol(eval_ctx, symbol)?)
    }

    fn evaluate_expression_from_idx(&self, eval_ctx: &mut EvaluationCtx, rule_idx: usize) -> Result<FactState, String>
    {
        let expression = self.rules[rule_idx]
            .0
            .left
            .as_deref()
            .unwrap()
            .clone();

        self.evaluate_expression_from_ast(eval_ctx, &expression)
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

    fn propagate_constraints_from(&self, eval_ctx: &mut EvaluationCtx, symbol: char) -> Result<(), String>
    {
        let mut changed_symbols: VecDeque<char> = VecDeque::new();
        changed_symbols.push_back(symbol);

        while !changed_symbols.is_empty()
        {
            let current = changed_symbols.pop_front().unwrap();
            let constraint_list = System::get_constraint_for_symbol(eval_ctx, current);

            for idx in constraint_list
            {
                let res = self.analyze_constraint(eval_ctx, idx);
                Self::trace_constraint(eval_ctx, idx, &res);
                match res.0
                {
                    ConstraintState::Satisfied => {}

                    ConstraintState::Unresolved =>
                    {
                        let changed = self.resolve_unresolved_from_constraint(eval_ctx, res.1)?;
                        changed_symbols.extend(changed.iter());
                    }

                    ConstraintState::Contradiction =>                                 
                    {
                        return Err(format!("Contradiction in constraint for constraint index {}", idx).to_string());
                    }

                    ConstraintState::Forced(sy, st) =>
                    {
                        if System::mark_from_constraint(eval_ctx, sy, st, idx) == true
                        {
                            changed_symbols.push_back(sy);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn trace_constraint(ctx: &mut EvaluationCtx, idx: usize, res: &(ConstraintState, Vec<(char, bool)>))
    {
        let result = match res.0
        {
            ConstraintState::Satisfied => ConstraintOutcome::Satisfied,
            ConstraintState::Unresolved => ConstraintOutcome::Unresolved {
                literals: res.1.iter().map(|&(symbol, positive)| Literal { symbol, positive }).collect(),
            },
            ConstraintState::Forced(symbol, state) => ConstraintOutcome::Forced {
                literal: Literal { symbol, positive: state == FactState::True },
            },
            ConstraintState::Contradiction => ConstraintOutcome::Contradiction,
        };
        Self::emit(ctx, TraceEventKind::ConstraintEvaluated { constraint_id: idx, result }, vec![]);
    }

    fn conclusion_contains_goal(conclusion: &ASTNode<'a>, symbol: char, target: FactState) -> bool
    {
        match conclusion.token.token_type
        {
            TokenType::Symbol | TokenType::Not =>
            {
                Self::conclusion_matches_goal(conclusion, symbol, target)
            }

            TokenType::Or =>
            {
                let left = conclusion
                    .left
                    .as_deref()
                    .expect("OR conclusion must have a left operand");

                let right = conclusion
                    .right
                    .as_deref()
                    .expect("OR conclusion must have a right operand");

                Self::conclusion_contains_goal(left, symbol, target) || Self::conclusion_contains_goal(right, symbol, target)
            }

            _ =>
            {
                unreachable!("Normalized conclusion must contain only literals and OR");
            }
        }
    }
    
    fn can_prove(&self, eval_ctx: &mut EvaluationCtx, symbol: char, target_state: FactState) -> Result<bool, String>
    {
        if matches!(target_state, FactState::Undetermined | FactState::Contradiction)
        {
            return Err("target_state for can_prove() can only be FactState::{True,False}".to_string());
        }
        
        let index = symbol_to_nb(symbol);

        if eval_ctx.symbols_state[index].proves(target_state)
        {
            return Ok(true);
        }

        let visiting_key = (symbol, target_state);
        let literal = Literal { symbol, positive: target_state == FactState::True };

        if eval_ctx.visiting.contains(&visiting_key)
        {
            if let Some(&previous_attempt) = eval_ctx.active_attempts.get(&visiting_key)
            {
                Self::emit(eval_ctx, TraceEventKind::CycleDetected { goal: literal, previous_attempt }, vec![]);
            }
            return Ok(false);
        }

        let parent_attempt = eval_ctx.attempt_stack.last().copied();
        let attempt_id = eval_ctx.trace.as_mut().map(|trace|
        {
            let id = trace.attempts;
            trace.attempts += 1;
            id
        });
        if let Some(id) = attempt_id
        {
            eval_ctx.attempt_stack.push(id);
            eval_ctx.active_attempts.insert(visiting_key, id);
            Self::emit(eval_ctx, TraceEventKind::SearchStarted { attempt_id: id, parent_attempt, goal: literal }, vec![]);
        }

        eval_ctx.visiting.insert(visiting_key);

        let result = (|| -> Result<bool, String>
            {
                let rules_to_visit = self
                    .get_rules_for_symbol(symbol)
                    .to_vec();

                for rule_idx in rules_to_visit
                {
                    let relevant = 
                    {
                        let conclusion = self.rules[rule_idx]
                            .0
                            .right
                            .as_deref()
                            .unwrap();

                        Self::conclusion_contains_goal(conclusion, symbol, target_state)
                    };

                    if !relevant
                    {
                        continue;
                    }

                    Self::emit(eval_ctx, TraceEventKind::RuleConsidered { rule_id: rule_idx }, vec![]);
                    let premise = self.evaluate_expression_from_idx(eval_ctx, rule_idx)?;
                    Self::emit(eval_ctx, TraceEventKind::PremiseEvaluated { rule_id: rule_idx, result: premise.into() }, vec![]);

                    match premise
                    {
                        FactState::True => {}

                        FactState::False | FactState::Undetermined =>
                        {
                            continue;
                        }

                        FactState::Contradiction =>
                        {
                            // In a contradictory knowledge base a premise has positive support.
                            // Keep collecting proofs; the query can then report both polarities.
                        }
                    }
                    
                    let conclusion = self.rules[rule_idx]
                        .0
                        .right
                        .as_deref()
                        .unwrap();

                    Self::emit(eval_ctx, TraceEventKind::RuleApplied { rule_id: rule_idx }, vec![]);
                    match conclusion.token.token_type
                    {
                        TokenType::Symbol | TokenType::Not =>
                        {
                            let matches_goal = System::conclusion_matches_goal(conclusion, symbol, target_state);

                            if matches_goal
                            {
                                if System::mark_from_rule(eval_ctx, symbol, target_state, rule_idx) == true
                                {
                                    self.propagate_constraints_from(eval_ctx, symbol)?;
                                }
                                return Ok(true);
                            }

                        }

                        TokenType::Or => 
                        {
                            if !System::add_constraint(eval_ctx, conclusion.clone(), rule_idx)
                            {
                                continue;
                            }
                            Self::emit(eval_ctx, TraceEventKind::ConstraintActivated { constraint_id: rule_idx, rule_id: rule_idx }, vec![]);

                            let res = self.analyze_constraint(eval_ctx, rule_idx);
                            Self::trace_constraint(eval_ctx, rule_idx, &res);
                            match res.0
                            {
                                ConstraintState::Forced(sy, st) =>
                                {
                                    if System::mark_from_constraint(eval_ctx, sy, st, rule_idx) == true
                                    {
                                        self.propagate_constraints_from(eval_ctx, sy)?;
                                    }
                                }

                                ConstraintState::Contradiction =>
                                {
                                    println!("Constraints: {:#?}", eval_ctx.constraints);
                                    return Err(format!("Contradiction in constraint for constraint index {}", rule_idx).to_string());
                                }

                                ConstraintState::Satisfied => {}

                                ConstraintState::Unresolved => 
                                {
                                    let changed_symbols = self.resolve_unresolved_from_constraint(eval_ctx, res.1)?;

                                    for sy in changed_symbols
                                    {
                                        self.propagate_constraints_from(eval_ctx, sy)?;
                                    }
                                }
                            }

                            if eval_ctx.symbols_state[index].proves(target_state)
                            {
                                return Ok(true);
                            }
                        }

                        _ => unreachable!()
                    }
                }

                Ok(false)
            })();

        eval_ctx.visiting.remove(&visiting_key);
        if let Some(attempt_id) = attempt_id
        {
            let outcome = match &result
            {
                Ok(true) => SearchOutcome::Proven,
                Ok(false) | Err(_) => SearchOutcome::NotProven,
            };
            Self::emit(eval_ctx, TraceEventKind::SearchFinished { attempt_id, outcome }, vec![]);
            eval_ctx.active_attempts.remove(&visiting_key);
            eval_ctx.attempt_stack.pop();
        }

        result
    }

    //polarity = false -> negative, true -> positive
    fn classify_symbol(&self, eval_ctx: &EvaluationCtx, symbol: char, polarity: bool) -> LiteralState
    {
        let state = eval_ctx.symbols_state[symbol_to_nb(symbol)];
        match polarity
        {
            true =>
            {
                match (state.positive, state.negative)
                {
                    (true, false) => LiteralState::Satisfied,
                    (false, true) => LiteralState::Refuted,
                    (false, false) => LiteralState::Unresolved,
                    (true, true) => LiteralState::Contradiction,
                }
            }

            false =>
            {
                match (state.positive, state.negative)
                {
                    (true, false) => LiteralState::Refuted,
                    (false, true) => LiteralState::Satisfied,
                    (false, false) => LiteralState::Unresolved,
                    (true, true) => LiteralState::Contradiction,
                }
            }
        }
    }

    fn resolve_unresolved_from_constraint(&self, eval_ctx: &mut EvaluationCtx, unresolved: Vec<(char, bool)>) -> Result<Vec<char>, String>
    {
        let mut changed_symbols = Vec::new();

        for (symbol, _polarity) in unresolved
        {
            let index = symbol_to_nb(symbol);

            let positive_before = eval_ctx.symbols_state[index].positive;
            let negative_before = eval_ctx.symbols_state[index].negative;

            if !positive_before
            {
                self.can_prove(eval_ctx, symbol, FactState::True)?;
            }

            if !negative_before
            {
                self.can_prove(eval_ctx, symbol, FactState::False)?;
            }

            let positive_after = eval_ctx.symbols_state[index].positive;
            let negative_after = eval_ctx.symbols_state[index].negative;

            if positive_after != positive_before || negative_after != negative_before
            {
                changed_symbols.push(symbol);
            }

            // Preserve both proofs. A contradictory fact is a reportable logical
            // outcome, not an evaluation failure.
        }

        Ok(changed_symbols)
    }

    fn analyze_constraint(&self, eval_ctx: &EvaluationCtx, constraint_idx: usize) -> (ConstraintState, Vec<(char, bool)>)
    {
        let constraint = eval_ctx.constraints[constraint_idx].clone().unwrap();
        let mut unresolved = Vec::new();
        let mut satisfied = false;

        for &(symbol, polarity) in constraint.iter()
        {
            match self.classify_symbol(eval_ctx, symbol, polarity)
            {
                LiteralState::Contradiction =>
                {
                    return (ConstraintState::Contradiction, unresolved);
                }

                LiteralState::Satisfied => 
                {
                    satisfied = true;
                }

                LiteralState::Unresolved =>
                {
                    unresolved.push((symbol, polarity));
                }

                LiteralState::Refuted => {}
            }
        };

        if satisfied
        {
            return (ConstraintState::Satisfied, unresolved);
        }

        if unresolved.len() == 1
        {
            let (symbol, polarity) = unresolved[0];

            let state = if polarity
            {
                FactState::True
            }
            else
            {
                FactState::False
            };

            return (ConstraintState::Forced(symbol, state), unresolved);
        }

        if unresolved.is_empty()
        {
            return (ConstraintState::Contradiction, unresolved);
        }

        (ConstraintState::Unresolved, unresolved)
    }

    fn verify_system_is_init(&self) -> bool
    {
        if !self.query_filled || !self.facts_filled || !self.rules_for_symbol_calculated
        {
            println!("{:#?}", self);
            return false;
        }
        true
    }

    pub fn evaluate_traced(&self, symbol: char) -> QueryExecution
    {
        let mut ctx = EvaluationCtx::new();
        ctx.symbols_state = self.base_state;
        ctx.constraints = vec![None; self.rules.len()];
        ctx.trace = Some(EvaluationTrace::new(symbol));

        for fact in &self.facts
        {
            let literal = Literal { symbol: *fact, positive: true };
            let event = Self::emit(&mut ctx, TraceEventKind::LiteralProven {
                literal, origin: ProofOrigin::InitialFact,
            }, vec![]);
            if let Some(event) = event
            {
                ctx.proof_events[symbol_to_nb(*fact) * 2] = Some(event);
            }
        }

        let result = self.evaluate_symbol(&mut ctx, symbol);
        let outcome = match result
        {
            Ok(value) => QueryOutcome::Value(value.into()),
            Err(message) => QueryOutcome::Error { message },
        };
        QueryExecution {
            symbol, outcome,
            trace: ctx.trace.take().unwrap(),
        }
    }

    pub fn execution_report(&self) -> ExecutionReport
    {
        ExecutionReport {
            schema_version: 1,
            initial_facts: self.facts.clone(),
            rewrite: self.rewrite_history.clone(),
            queries: self.query.iter().map(|&symbol| self.evaluate_traced(symbol)).collect(),
        }
    }

    pub fn export_execution_json(&self, path: impl AsRef<std::path::Path>) -> Result<(), Box<dyn std::error::Error>>
    {
        if !self.verify_system_is_init()
        {
            return Err("System is not initialized".into());
        }
        let file = std::fs::File::create(path)?;
        serde_json::to_writer_pretty(std::io::BufWriter::new(file), &self.execution_report())?;
        Ok(())
    }

    pub fn solve(&self)
    {
        if !self.verify_system_is_init()
        {
            eprintln!("System is not correctly init");
            return;
        }
        for symbol in self.query.clone()
        {
            let mut eval_ctx = EvaluationCtx::new();
            eval_ctx.symbols_state = self.base_state;
            eval_ctx.constraints = vec![None; self.rules.len()];
            let result = self.evaluate_symbol(&mut eval_ctx, symbol);
            println!("{symbol}: {result:?}");
        }
    }

    pub fn solve_parallel(&self)
    {
        if !self.verify_system_is_init()
        {
            eprintln!("System is not correctly init");
            return;
        }

        if self.query.is_empty()
        {
            return;
        }

        let worker_limit = thread::available_parallelism()
            .map_or(1, |count| count.get())
            .min(self.query.len());

        let batch_size = self.query.len().div_ceil(worker_limit);

        thread::scope(|scope|
        {
            let mut handles = Vec::with_capacity(worker_limit);

            for queries in self.query.chunks(batch_size)
            {
                let handle = scope.spawn(move ||
                {
                    let mut eval_ctx = EvaluationCtx::new();

                    let mut results = Vec::with_capacity(queries.len());

                    for &symbol in queries
                    {
                        eval_ctx.symbols_state = self.base_state;
                        eval_ctx.constraints = vec![None; self.rules.len()];
                        eval_ctx.visiting.clear();

                        let result = self.evaluate_symbol(&mut eval_ctx, symbol);

                        results.push((symbol, result));
                    }

                    results
                });

                handles.push((queries, handle));
            }

            for (queries, handle) in handles
            {
                match handle.join()
                {
                    Ok(results) =>
                    {
                        for (symbol, result) in results
                        {
                            println!("{symbol}: {result:?}");
                        }
                    }

                    Err(_) =>
                    {
                        eprintln!("A worker panicked whil evaluating queries {queries:?}");
                    }
                }
            }
        });
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

    for (line_index, tokens) in token_list.iter_mut().enumerate()
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
        let original = tokens.iter().map(|t| t.value).collect::<String>();
        asts.push((build_ast(rpn(tokens.clone()))?, line_index + 1, original));
    }

    let (rules, history) = rewrite_with_history(asts);
    system.add_rules(rules);
    system.rewrite_history = history;

    Ok(())
}

impl From<FactState> for crate::trace::LogicValue
{
    fn from(value: FactState) -> Self
    {
        match value
        {
            FactState::True => Self::True,
            FactState::False => Self::False,
            FactState::Undetermined => Self::Undetermined,
            FactState::Contradiction => Self::Contradiction,
        }
    }
}
