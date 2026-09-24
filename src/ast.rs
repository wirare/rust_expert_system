use std::{collections::{HashMap, HashSet}, sync::LazyLock};
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

#[derive(Debug)]
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

    pub fn contain_symbol(&self, symbol: char) -> bool
    {
        if self.token.value.chars().next().unwrap() == symbol
        {
            return true;
        }
        else
        {
            if let Some(left) = &self.left
            {
                let res = left.contain_symbol(symbol);
                if res
                {
                    return true;
                }
                else
                {
                    if let Some(right) = &self.right
                    {
                        return right.contain_symbol(symbol);
                    }
                }
            }
            return false;
        }
    }

    pub fn contain_token_mask(&self, tmask: TokenMask) -> bool
    {
        if tmask.contains(self.token.token_type)
        {
            return true;
        }
        else
        {
            if let Some(left) = &self.left
            {
                let res = left.contain_token_mask(tmask);
                if res
                {
                    return true;
                }
                else
                {
                    if let Some(right) = &self.right
                    {
                        return right.contain_token_mask(tmask);
                    }
                }
            }
            return false;
        }
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

pub fn build_ast<'a>(rpn: Vec<Token<'a>>) -> Result<Rule<'a>, String>
{
    let mut stack: Vec<ASTNode<'a>> = Vec::new();
    let mut symbols: HashSet<char> = HashSet::new();

    for token in rpn
    {
        let tt = token.token_type;

        if tt == TokenType::Symbol
        {
            stack.push(ASTNode::new(token));
            symbols.insert(token.value.chars().next().unwrap());
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
    let right = head.right.as_ref().unwrap();
    let conc_symbol: HashSet<char> = symbols
        .iter()
        .copied()
        .filter(|s| right.contain_symbol(*s))
        .collect();

    Ok((head, conc_symbol))
}
