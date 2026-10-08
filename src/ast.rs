use std::{collections::{HashMap, HashSet, VecDeque}, mem::swap, sync::LazyLock};
use crate::{system::Rule, tokenizer::{Token, TokenMask, TokenType}};

#[derive(Hash, PartialEq)]
pub enum Assoc
{
    Left,
    Right,
}

#[derive(Hash)]
pub struct OperatorInfo
{
    pub precedence: u8,
    pub associativity: Assoc,
    pub arity: u8,
}

#[derive(Debug, Clone)]
pub struct ASTNode<'a>
{
	pub token: Token<'a>,
	pub left: Option<Box<ASTNode<'a>>>,
	pub right: Option<Box<ASTNode<'a>>>,
}

impl<'a> ASTNode<'a>
{
    fn new(token: Token<'a>) -> Self
    {
        Self
        {
            token,
            left: None,
            right: None,
        }
    }

    fn with_child(token: Token<'a>, child: ASTNode<'a>) -> Self
    {
        Self
        {
            token,
            left: Some(Box::new(child)),
            right: None,
        }
    }

    fn with_children(token: Token<'a>, left: ASTNode<'a>, right: ASTNode<'a>) -> Self
    {
        Self
        {
            token,
            left: Some(Box::new(left)),
            right: Some(Box::new(right)),
        }
    }

    pub fn contain_token_mask(&self, tmask: TokenMask, depth: usize) -> (bool, usize)
    {
        if tmask.contains(self.token.token_type)
        {
            return (true, depth);
        }
        else
        {
            if let Some(left) = &self.left
            {
                let res = left.contain_token_mask(tmask, depth + 1);
                if res.0
                {
                    return (true, res.1);
                }
                else
                {
                    if let Some(right) = &self.right
                    {
                        return right.contain_token_mask(tmask, depth + 1);
                    }
                }
            }
            return (false, depth - 1);
        }
    }

    fn symbols_contained_intern(&self, set: &mut HashSet<char>)
    {
        if self.token.token_type == TokenType::Symbol
        {
            set.insert(self.token.value.chars().next().unwrap());
        }
        else
        {
            if self.left.is_some()
            {
                self.left.as_ref().unwrap().symbols_contained_intern(set);
            }
            if self.right.is_some()
            {
                self.right.as_ref().unwrap().symbols_contained_intern(set);
            }
        }
    }

    pub fn symbols_contained(&self) -> HashSet<char>
    {
        let mut symbols: HashSet<char> = HashSet::new();
        
        self.symbols_contained_intern(&mut symbols);

        symbols
    }
}

static OP_INFO_MAP: LazyLock<HashMap<TokenType, OperatorInfo>> = LazyLock::new(||
    {
        HashMap::from(
            [
                (
                    TokenType::Not, 
                    OperatorInfo
                    {
                        precedence: 5,
                        associativity: Assoc::Right,
                        arity: 1,
                    }
                ),
                (
                    TokenType::And, 
                    OperatorInfo
                    {
                        precedence: 4,
                        associativity: Assoc::Left,
                        arity: 2,
                    }
                ),
                (
                    TokenType::Xor, 
                    OperatorInfo
                    {
                        precedence: 3,
                        associativity: Assoc::Left,
                        arity: 2,
                    }
                ),
                (
                    TokenType::Or, 
                    OperatorInfo
                    {
                        precedence: 2,
                        associativity: Assoc::Left,
                        arity: 2,
                    }
                ),
                (
                    TokenType::Then, 
                    OperatorInfo
                    {
                        precedence: 1,
                        associativity: Assoc::Left,
                        arity: 2,
                    }
                ),
                (
                    TokenType::Iff, 
                    OperatorInfo
                    {
                        precedence: 1,
                        associativity: Assoc::Left,
                        arity: 2,
                    }
                ),
            ]
        )
    });

pub fn rpn<'a>(tokens: Vec<Token<'a>>) -> Vec<Token<'a>>
{
	let mut output: Vec<Token<'a>> = Vec::new();
	let mut op_stack: Vec<Token<'a>> = Vec::new();

	for token in tokens
	{
		let tt = token.token_type;

		if tt == TokenType::Symbol
		{
			output.push(token);
			continue;
		}

		if tt & TokenType::Operator
		{
			loop
			{
				let should_pop = match op_stack.last()
				{
					Some(top) if top.token_type & TokenType::Operator =>
					{
						let top_info = &OP_INFO_MAP[&top.token_type];
						let token_info = &OP_INFO_MAP[&tt];

						(token_info.associativity == Assoc::Left
							&& token_info.precedence <= top_info.precedence)
						||
						(token_info.associativity == Assoc::Right
							&& token_info.precedence < top_info.precedence)
					}

					_ => false,
				};

				if !should_pop
				{
					break;
				}

				output.push(op_stack.pop().unwrap());
			}

			op_stack.push(token);
			continue;
		}

		if tt == TokenType::Lp
		{
			op_stack.push(token);
			continue;
		}

		if tt == TokenType::Rp
		{
			while let Some(top) = op_stack.pop()
			{
				if top.token_type == TokenType::Lp
				{
					break;
				}

				output.push(top);
			}

			continue;
		}
	}

	while let Some(token) = op_stack.pop()
	{
		output.push(token);
	}

	output
}

pub fn build_ast<'a>(rpn: Vec<Token<'a>>) -> Result<ASTNode<'a>, String>
{
    let mut stack: Vec<ASTNode<'a>> = Vec::new();

    for token in rpn
    {
        let tt = token.token_type;

        if tt == TokenType::Symbol
        {
            stack.push(ASTNode::new(token));
            continue;
        }

        if tt & TokenType::Operator
        {
            let info = &OP_INFO_MAP[&tt];

            if info.arity == 1
            {
                let child = stack.pop().ok_or("Missing operand for unary operator")?;

                stack.push(ASTNode::with_child(token, child));

                continue;
            }
            
            if info.arity == 2
            {
                let right = stack.pop().ok_or("Missing right operand")?;
                let left = stack.pop().ok_or("Missing left operand")?;

                stack.push(ASTNode::with_children(token, left, right));

                continue;
            }
        }
    }

    if stack.len() != 1
    {
        return Err("Invalid RPN expression".to_string());
    }

    let head = stack.pop().unwrap();

    Ok(head)
}

fn split_iff_ast(node: ASTNode) -> [ASTNode; 2]
{
    let then_token = Token::new(TokenType::Then, "=>");

    let first_new_node = ASTNode::with_children(then_token, *node.left.clone().unwrap(), *node.right.clone().unwrap());
    let second_new_node = ASTNode::with_children(then_token, *node.right.clone().unwrap(), *node.left.clone().unwrap());

    [first_new_node, second_new_node]
}

fn split_and_conc_ast(node: ASTNode) -> [ASTNode; 2]
{
    return [*node.left.clone().unwrap(), *node.right.clone().unwrap()];
}

fn distribute_or_inside_and_exp_in_conc_ast(node: ASTNode) -> ASTNode
{
    let and_token = Token::new(TokenType::And, "+");
    let or_token = Token::new(TokenType::Or, "|");
    let mut copy = node.clone();

    if copy.left.as_ref().unwrap().token.token_type == TokenType::And
    {
        swap(&mut copy.right, &mut copy.left);
    }

    let node_to_distrib = copy
        .left
        .as_deref()
        .unwrap()
        .clone();

    let left_node = ASTNode::with_children(
        or_token,
        node_to_distrib.clone(),
        *copy
        .right
        .as_ref()
        .unwrap()
        .left
        .as_ref()
        .unwrap()
        .clone()
    );

    let right_node = ASTNode::with_children(
        or_token, 
        node_to_distrib, 
        *copy
        .right
        .as_ref()
        .unwrap()
        .right
        .as_ref()
        .unwrap()
        .clone()
    );

    ASTNode::with_children(and_token, left_node, right_node)
}

fn rewrite_not_or_in_conc_ast(node: ASTNode) -> ASTNode
{
    let not_token = Token::new(TokenType::Not, "!");
    let and_token = Token::new(TokenType::And, "+");

    let left_node = ASTNode::with_child(
        not_token, 
        *node
        .left
        .as_ref()
        .unwrap()
        .left
        .as_ref()
        .unwrap()
        .clone()
    );

    let right_node = ASTNode::with_child(
        not_token, 
        *node
        .left
        .as_ref()
        .unwrap()
        .right
        .as_ref()
        .unwrap()
        .clone()
    );

    ASTNode::with_children(and_token, left_node, right_node)
}

fn rewrite_not_and_in_conc_ast(node: ASTNode) -> ASTNode
{
    let not_token = Token::new(TokenType::Not, "!");
    let or_token = Token::new(TokenType::Or, "|");

    let left_node = ASTNode::with_child(
        not_token, 
        *node
        .left
        .as_ref()
        .unwrap()
        .left
        .as_ref()
        .unwrap()
        .clone()
    );

    let right_node = ASTNode::with_child(
        not_token, 
        *node
        .left
        .as_ref()
        .unwrap()
        .right
        .as_ref()
        .unwrap()
        .clone()
    );

    ASTNode::with_children(or_token, left_node, right_node)
}

fn split_xor_in_conc_ast(node: ASTNode) -> [ASTNode; 2]
{
    let not_token = Token::new(TokenType::Not, "!");
    let or_token = Token::new(TokenType::Or, "|");

    let left_exp_node = node.left.as_ref().unwrap();
    let right_exp_node = node.right.as_ref().unwrap();

    [
        ASTNode::with_children(
            or_token,
            *left_exp_node.clone(),
            *right_exp_node.clone(),
        ),
        ASTNode::with_children(
            or_token,
            ASTNode::with_child(
                not_token,
                *left_exp_node.clone()
            ),
            ASTNode::with_child(
                not_token,
                *right_exp_node.clone()
            ),
        ),
    ]
}

fn split_not_xor_in_conc_ast(node: ASTNode) -> [ASTNode; 2]
{
    let not_token = Token::new(TokenType::Not, "!");
    let or_token = Token::new(TokenType::Or, "|");

    let xor_node = node.left.as_ref().unwrap();
    let left_exp_node = xor_node.left.as_ref().unwrap();
    let right_exp_node = xor_node.right.as_ref().unwrap();

    [
        ASTNode::with_children(
            or_token,
            ASTNode::with_child(
                not_token,
                *left_exp_node.clone()
            ),
            *right_exp_node.clone(),
        ),
        ASTNode::with_children(
            or_token,
            *left_exp_node.clone(),
            ASTNode::with_child(
                not_token,
                *right_exp_node.clone()
            ),
        ),
    ]
}

pub fn simplify_and_rewrite_rules(asts: Vec<ASTNode>) -> Vec<Rule>
{
    let mut asts_to_explores: VecDeque<ASTNode> = asts.into();
    let mut final_asts = Vec::new();
    let then_token = Token::new(TokenType::Then, "=>");

    while let Some(ast) = asts_to_explores.pop_front()
    {
        if ast.token.token_type == TokenType::Iff
        {
            asts_to_explores.extend(split_iff_ast(ast));
            continue;
        }
        match ast.right.as_ref().unwrap().token.token_type
        {
            TokenType::And =>
            {
                let left = ast.left.as_deref().unwrap().clone();
                let [rule_0, rule_1] = split_and_conc_ast(*ast.right.unwrap());
                asts_to_explores.push_back(
                    ASTNode::with_children(then_token, left.clone(), rule_0)
                );
                asts_to_explores.push_back(
                    ASTNode::with_children(then_token, left, rule_1)
                );
            }

            TokenType::Or =>
            {
                let res = ast.right.as_deref().unwrap().contain_token_mask(TokenType::And.into(), 0);
                if !res.0 || res.1 != 1
                {
                    final_asts.push(ast);
                    continue;
                }
                let left = ast.left.as_deref().unwrap().clone();
                let right = distribute_or_inside_and_exp_in_conc_ast(*ast.right.unwrap());
                asts_to_explores.push_back(
                    ASTNode::with_children(then_token, left, right)
                );
            }

            TokenType::Xor =>
            {
                let left = ast.left.as_deref().unwrap().clone();
                let [rule_0, rule_1] = split_xor_in_conc_ast(*ast.right.unwrap());
                asts_to_explores.push_back(
                    ASTNode::with_children(then_token, left.clone(), rule_0)
                );
                asts_to_explores.push_back(
                    ASTNode::with_children(then_token, left, rule_1)
                );
            }

            TokenType::Not =>
            {
                match ast.right.clone().unwrap().left.unwrap().token.token_type
                {
                    TokenType::Xor =>
                    {
                        let left = ast.left.as_deref().unwrap().clone();
                        let [rule_0, rule_1] = split_not_xor_in_conc_ast(*ast.right.unwrap());
                        asts_to_explores.push_back(
                            ASTNode::with_children(then_token, left.clone(), rule_0)
                        );
                        asts_to_explores.push_back(
                            ASTNode::with_children(then_token, left, rule_1)
                        );
                    }

                    TokenType::Or =>
                    {
                        let left = ast.left.as_deref().unwrap().clone();
                        let right = rewrite_not_or_in_conc_ast(*ast.right.unwrap());
                        asts_to_explores.push_back(
                            ASTNode::with_children(then_token, left, right)
                        );
                    }

                    TokenType::And =>
                    {
                        let left = ast.left.as_deref().unwrap().clone();
                        let right = rewrite_not_and_in_conc_ast(*ast.right.unwrap());
                        asts_to_explores.push_back(
                            ASTNode::with_children(then_token, left, right)
                        );
                    }

                    _ => {final_asts.push(ast);}
                }
            } 

            _ => {final_asts.push(ast);}
        }
    }

    final_asts
        .into_iter()
        .map(|node|
            {
                let symbols = node.right.as_ref().unwrap().symbols_contained();
                (node, symbols)
            })
        .collect()
}
