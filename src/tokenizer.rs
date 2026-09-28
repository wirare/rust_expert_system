use std::ops::{BitOr, BitAnd};

pub type ParseFn = fn(&str) -> Option<usize>;

#[derive(Clone, Copy, Debug)]
enum TokenMatcher
{
	None,
	Fixed(&'static str),
	Custom(ParseFn),
}

#[derive(Clone, Copy, Debug)]
pub struct TokenInfo
{
	pub name: &'static str,
	pub mask: u32,
	matcher: TokenMatcher,
}

#[derive(Debug, Clone, Copy)]
pub struct Token<'a>
{
    pub token_type: TokenType,
    pub value: &'a str,
}

pub fn parse_symbol(input: &str) -> Option<usize>
{
	input
		.as_bytes()
		.first()
		.filter(|c| c.is_ascii_uppercase())
		.map(|_| 1)
}

macro_rules! define_tokens
{
	(
		$(
			$variant:ident =>
			(
				$name:literal,
				$mask:expr
				$(, $matcher:tt)?
			)
		),* $(,)?
	) =>
	{
		#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum TokenType
        {
			$(
				$variant,
			)*
		}

		impl TokenType
		{
            pub const ALL: &'static [Self] = 
            &[
                $(
                    Self::$variant,
                )*
            ];

			pub fn info(self) -> TokenInfo
			{
				match self
				{
					$(
						Self::$variant => TokenInfo
						{
							name: $name,
							mask: $mask,
							matcher: define_tokens!(
								@matcher $( $matcher )?
							),
						},
					)*
				}
			}

			pub fn match_len(self, input: &str) -> Option<usize>
			{
				match self.info().matcher
				{
					TokenMatcher::None =>
					{
						None
					}

					TokenMatcher::Fixed(pattern) =>
					{
						input
							.starts_with(pattern)
							.then_some(pattern.len())
					}

					TokenMatcher::Custom(parser) =>
					{
						parser(input)
					}
				}
			}

            pub fn mask(self) -> u32
            {
                match self
                {
                    $(
                        Self::$variant => $mask,
                    )*
                }
            }
		}
	};

	(@matcher $pattern:literal) =>
	{
		TokenMatcher::Fixed($pattern)
	};

	(@matcher $parser:ident) =>
	{
		TokenMatcher::Custom($parser)
	};

	(@matcher) =>
	{
		TokenMatcher::None
	};
}

define_tokens!
{
	Dflt   		=> ("default",      1 << 0),
	Fact   		=> ("fact",         1 << 1,  "="),
	Symbol 		=> ("symbol",       1 << 2,  parse_symbol),
	Then   		=> ("then",         1 << 3,  "=>"),
	Iff    		=> ("iff",          1 << 4,  "<=>"),
	Cond   		=> ("condition",    1 << 3 | 1 << 4),
	Xor    		=> ("xor",          1 << 5,  "^"),
	And    		=> ("and",          1 << 6,  "+"),
	Or     		=> ("or",           1 << 7,  "|"),
	Not    		=> ("not",          1 << 8,  "!"),
    Operator    => ("operator",     1 << 3 | 1 << 5 | 1 << 6 | 1 << 7 | 1 << 8),
	Query  		=> ("query",        1 << 9, "?"),
	Lp     		=> ("(",            1 << 10, "("),
	Rp     		=> (")",            1 << 11, ")"),
	Par    		=> ("par",          1 << 10 | 1 << 11),
	Start  		=> ("start",        1 << 12),
	End    		=> ("end",          1 << 13),
}

#[derive(Clone, Copy)]
pub struct TokenMask(u32);

impl BitOr for TokenType
{
    type Output = TokenMask;

    fn bitor(self, rhs: Self) -> Self::Output
    {
        TokenMask(self.mask() | rhs.mask())
    }
}

impl BitAnd for TokenType
{
    type Output = bool;

    fn bitand(self, rhs: Self) -> Self::Output
    {
        self.mask() & rhs.mask() != 0
    }
}

impl BitOr<TokenType> for TokenMask
{
    type Output = TokenMask;

    fn bitor(self, rhs: TokenType) -> Self::Output
    {
        TokenMask(self.0 | rhs.mask())
    }
}

impl TokenMask
{
    pub fn contains(self, token: TokenType) -> bool
    {
        self.0 & token.mask() != 0
    }
}

impl From<TokenType> for TokenMask
{
    fn from(token: TokenType) -> Self
    {
        TokenMask(token.mask())
    }
}

impl<'a> Token<'a>
{
    pub fn is_operator(&self) -> bool
    {
        self.token_type & TokenType::Operator
    }
}
