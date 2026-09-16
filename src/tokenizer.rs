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

#[derive(Debug)]
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
		#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
	Dflt   => ("default",   1 << 0),
	Fact   => ("fact",      1 << 1,  "="),
	Symbol => ("symbol",    1 << 2,  parse_symbol),
	Then   => ("then",      1 << 3,  "=>"),
	Iff    => ("iff",       1 << 4,  "<=>"),
	Cond   => ("condition", 1 << 5),
	Xor    => ("xor",       1 << 6,  "^"),
	And    => ("and",       1 << 7,  "+"),
	Or     => ("or",        1 << 8,  "|"),
	Not    => ("not",       1 << 9,  "!"),
	Query  => ("query",     1 << 10, "?"),
	Lp     => ("(",         1 << 12, "("),
	Rp     => (")",         1 << 13, ")"),
	Par    => ("par",       1 << 14),
	Start  => ("start",     1 << 15),
	End    => ("end",       1 << 16),
}
