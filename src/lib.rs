mod input;
mod sequence;

use input::{Input, InputRef};
use sequence::{Container, OrderedSequence};
use std::marker::PhantomData;

#[derive(Debug, PartialEq)]
enum ParseError {
    SyntaxError,
}

type ParseResult<O> = Result<O, ParseError>;

trait Parser<'input, I, O>
where
    I: Input<'input>,
{
    fn parse(&self, input: I) -> ParseResult<O> {
        let mut input_ref = InputRef::new(&input);
        self.go(&mut input_ref)
    }

    // Helper function
    // All the logic for parsing resides in this method.
    fn go(&self, input_ref: &mut InputRef<'input, '_, I>) -> ParseResult<O>;

    // `map` operator, works the same way as the map function on iterators (Functors
    // generally).
    fn map<U, F>(self, mapper: F) -> Map<I, Self, O, F, U>
    where
        F: Fn(O) -> U,
        Self: Sized,
    {
        Map {
            mapper,
            parser: self,
            phantom: PhantomData,
        }
    }

    // `right_bind` operator, you can think of it as the right bind operator in haskell (>>). It helps
    // binding multiple parsers together while only keeping the results of the second parser.
    fn right_bind<P2, OP2>(self, second_parser: P2) -> RightBind<I, Self, O, P2, OP2>
    where
        P2: Parser<'input, I, OP2>,
        Self: Sized,
    {
        RightBind(Bind {
            first_parser: self,
            second_parser,
            phantom: PhantomData,
        })
    }

    // `left_bind` operator, you can think of it as the left bind operator in haskell (<<) It helps
    // binding multiple parsers together while only keeping the results of the first parser.
    fn left_bind<P2, OP2>(self, second_parser: P2) -> LeftBind<I, Self, O, P2, OP2>
    where
        P2: Parser<'input, I, OP2>,
        Self: Sized,
    {
        LeftBind(Bind {
            first_parser: self,
            second_parser,
            phantom: PhantomData,
        })
    }

    // `and` operator allows you to run two parsers and return the output of both in a
    // container.
    fn and<P2, OP2>(self, second_parser: P2) -> And<I, Self, O, P2, OP2>
    where
        Self: Sized,
        P2: Parser<'input, I, OP2>,
    {
        And(Bind {
            first_parser: self,
            second_parser,
            phantom: PhantomData,
        })
    }

    // `repeated` operator allows you to parse the same pattern multiple times.
    // You can either specify an exact number of times the pattern must be parsed or give a range
    // (i.e a lower bound and/or an upper bound)
    fn repeated(self) -> Repeated<I, Self, O>
    where
        Self: Sized,
    {
        Repeated {
            parser: self,
            range: RepeatedRange::AtLeast(1),
            phantom: PhantomData,
        }
    }

    fn filter<F>(self, filter_func: F) -> Filter<I, Self, O, F>
    where
        Self: Sized,
    {
        Filter {
            parser: self,
            filter_func,
            phantom: PhantomData,
        }
    }

    fn padded<P2, OP2>(self, padded_by: P2) -> Padded<I, Self, O, P2, OP2>
    where
        Self: Sized,
    {
        Padded {
            parser: self,
            padded_by,
            phantom: PhantomData,
        }
    }

    fn or<P2>(self, second_parser: P2) -> Or<Self, P2>
    where
        Self: Sized,
    {
        Or {
            first_parser: self,
            second_parser,
        }
    }
}

#[derive(Clone, Copy)]
struct Or<P1, P2> {
    first_parser: P1,
    second_parser: P2,
}

impl<'input, I, P1, P2, OP> Parser<'input, I, OP> for Or<P1, P2>
where
    I: Input<'input>,
    P1: Parser<'input, I, OP>,
    P2: Parser<'input, I, OP>,
{
    fn go(&self, input_ref: &mut InputRef<'input, '_, I>) -> ParseResult<OP> {
        let prev_state = input_ref.offset();
        if let Ok(out) = self.first_parser.go(input_ref) {
            Ok(out)
        } else {
            input_ref.rewind(prev_state);
            self.second_parser.go(input_ref)
        }
    }
}

#[derive(Clone, Copy)]
struct Padded<I, P1, OP1, P2, OP2> {
    parser: P1,
    padded_by: P2,
    phantom: PhantomData<(I, OP1, OP2)>,
}

impl<'input, I, P1, OP1, P2, OP2> Parser<'input, I, OP1> for Padded<I, P1, OP1, P2, OP2>
where
    I: Input<'input>,
    P1: Parser<'input, I, OP1>,
    P2: Parser<'input, I, OP2>,
{
    fn go(&self, input_ref: &mut InputRef<'input, '_, I>) -> ParseResult<OP1> {
        _ = self.padded_by.go(input_ref);

        let out = self.parser.go(input_ref)?;

        _ = self.padded_by.go(input_ref);

        Ok(out)
    }
}

#[derive(Clone, Copy)]
struct Filter<I, P, O, F> {
    parser: P,
    filter_func: F,
    phantom: PhantomData<(I, O)>,
}

impl<'input, I, P, O, F> Parser<'input, I, O> for Filter<I, P, O, F>
where
    I: Input<'input>,
    P: Parser<'input, I, O>,
    F: Fn(&O) -> bool,
{
    fn go(&self, input_ref: &mut InputRef<'input, '_, I>) -> ParseResult<O> {
        let prev_state = input_ref.offset();
        self.parser.go(input_ref).and_then(|out| {
            if (self.filter_func)(&out) {
                Ok(out)
            } else {
                input_ref.rewind(prev_state);
                Err(ParseError::SyntaxError)
            }
        })
    }
}

// This is a bit too awkward. Maybe put all the entities related to a specific parser into a
// module.
#[derive(Debug, Clone, Copy)]
enum RepeatedRange {
    AtLeast(usize),
    Between(usize, usize),
    Exactly(usize),
}

impl RepeatedRange {
    #[inline]
    fn start(&self) -> usize {
        match self {
            &RepeatedRange::AtLeast(start) => start,
            &RepeatedRange::Between(start, _end) => start,
            &RepeatedRange::Exactly(count) => count,
        }
    }

    #[inline]
    fn end(&self) -> Option<usize> {
        match self {
            &RepeatedRange::AtLeast(_start) => None,
            &RepeatedRange::Between(_start, end) => Some(end),
            &RepeatedRange::Exactly(count) => Some(count),
        }
    }
}

#[derive(Clone, Copy)]
struct Repeated<I, P, OP> {
    parser: P,
    range: RepeatedRange,
    phantom: PhantomData<(I, OP)>,
}

#[derive(Clone, Copy)]
struct AtLeast<I, P, OP>(Repeated<I, P, OP>);

#[derive(Clone, Copy)]
struct Exactly<I, P, OP>(Repeated<I, P, OP>);

#[derive(Clone, Copy)]
struct AtMost<I, P, OP>(Repeated<I, P, OP>);

impl<I, P, OP> Repeated<I, P, OP> {
    fn at_least(self, at_least: usize) -> AtLeast<I, P, OP> {
        AtLeast(Repeated {
            range: RepeatedRange::AtLeast(at_least),
            parser: self.parser,
            phantom: PhantomData,
        })
    }

    fn exactly(self, count: usize) -> Exactly<I, P, OP> {
        Exactly(Repeated {
            range: RepeatedRange::Exactly(count),
            parser: self.parser,
            phantom: PhantomData,
        })
    }
}

impl<I, P, OP> AtLeast<I, P, OP> {
    fn at_most(self, at_most: usize) -> AtMost<I, P, OP> {
        let at_least = self.0.range.start();

        AtMost(Repeated {
            range: RepeatedRange::Between(at_least, at_most),
            parser: self.0.parser,
            phantom: PhantomData,
        })
    }

    fn collect<C: Container>(self) -> Collect<I, P, OP, C> {
        Collect {
            parser: self.0.parser,
            range: self.0.range,
            phantom: PhantomData,
        }
    }
}

impl<I, P, OP> AtMost<I, P, OP> {
    fn collect<C: Container>(self) -> Collect<I, P, OP, C> {
        Collect {
            parser: self.0.parser,
            range: self.0.range,
            phantom: PhantomData,
        }
    }
}

impl<I, P, OP> Exactly<I, P, OP> {
    fn collect<C: Container>(self) -> Collect<I, P, OP, C> {
        Collect {
            parser: self.0.parser,
            range: self.0.range,
            phantom: PhantomData,
        }
    }
}

#[derive(Clone, Copy)]
struct Collect<I, P, OP, C> {
    parser: P,
    range: RepeatedRange,
    phantom: PhantomData<(I, OP, C)>,
}

impl<'input, I, P, OP, C> Parser<'input, I, C> for Collect<I, P, OP, C>
where
    I: Input<'input>,
    P: Parser<'input, I, OP>,
    C: Container<Item = OP>,
{
    fn go(&self, input_ref: &mut InputRef<'input, '_, I>) -> ParseResult<C> {
        let at_least = self.range.start();
        let at_most = self.range.end();

        let mut ret = C::default();
        for _ in 0..at_least {
            ret.push(self.parser.go(input_ref)?);
        }

        for out in (at_least..at_most.unwrap_or(usize::MAX))
            .map(|_| self.parser.go(input_ref))
            .take_while(|x| x.is_ok())
        {
            ret.push(out.unwrap());
        }
        Ok(ret)
    }
}

#[derive(Clone, Copy)]
struct Bind<I, P1, OP1, P2, OP2> {
    // First parser to run. The result of this parser will be discarded.
    first_parser: P1,
    // Second parser to run. The result of this parser will be returned.
    second_parser: P2,
    phantom: PhantomData<(I, OP1, OP2)>,
}

// `and` operator allows you to run two parsers and return the output of both in a
// container.
#[derive(Clone, Copy)]
struct And<I, P1, OP1, P2, OP2>(Bind<I, P1, OP1, P2, OP2>);

// `left_bind` operator, similar to (<<) in haskell
#[derive(Clone, Copy)]
struct LeftBind<I, P1, OP1, P2, OP2>(Bind<I, P1, OP1, P2, OP2>);

// `right_bind` operator, similar to (>>) in haskell
#[derive(Clone, Copy)]
struct RightBind<I, P1, OP1, P2, OP2>(Bind<I, P1, OP1, P2, OP2>);

impl<'input, I, P1, OP1, P2, OP2> Parser<'input, I, (OP1, OP2)> for And<I, P1, OP1, P2, OP2>
where
    I: Input<'input>,
    P1: Parser<'input, I, OP1>,
    P2: Parser<'input, I, OP2>,
{
    fn go(&self, input_ref: &mut InputRef<'input, '_, I>) -> ParseResult<(OP1, OP2)> {
        Ok((
            self.0.first_parser.go(input_ref)?,
            self.0.second_parser.go(input_ref)?,
        ))
    }
}

impl<'input, I, P1, OP1, P2, OP2> Parser<'input, I, OP1> for LeftBind<I, P1, OP1, P2, OP2>
where
    I: Input<'input>,
    P1: Parser<'input, I, OP1>,
    P2: Parser<'input, I, OP2>,
{
    fn go(&self, input_ref: &mut InputRef<'input, '_, I>) -> ParseResult<OP1> {
        let ret = self.0.first_parser.go(input_ref)?;
        self.0.second_parser.go(input_ref)?;
        Ok(ret)
    }
}

impl<'input, I, P1, OP1, P2, OP2> Parser<'input, I, OP2> for RightBind<I, P1, OP1, P2, OP2>
where
    I: Input<'input>,
    P1: Parser<'input, I, OP1>,
    P2: Parser<'input, I, OP2>,
{
    fn go(&self, input_ref: &mut InputRef<'input, '_, I>) -> ParseResult<OP2> {
        self.0.first_parser.go(input_ref)?;
        self.0.second_parser.go(input_ref)
    }
}

// `map` operator, works the same way as the map function on iterators (Functors
// generally).
#[derive(Clone, Copy)]
struct Map<I, P, OP, F, U> {
    // function mapping the output of the parser to the output desired.
    mapper: F,
    // parser we are mapping
    parser: P,
    phantom: PhantomData<(I, U, OP)>,
}

impl<'input, I, P, OP, F, U> Parser<'input, I, U> for Map<I, P, OP, F, U>
where
    I: Input<'input>,
    P: Parser<'input, I, OP>,
    F: Fn(OP) -> U,
{
    fn go(&self, input_ref: &mut InputRef<'input, '_, I>) -> ParseResult<U> {
        let out = self.parser.go(input_ref)?;
        Ok((self.mapper)(out))
    }
}

// `Exact` combinator matches an exact sequence of tokens.
// Returns an error if there is a mismatch.
#[derive(Clone, Copy)]
struct Exact<I, T> {
    seq: T,
    phantom: PhantomData<I>,
}

fn exact<'input, I, T>(seq: T) -> Exact<I, T>
where
    I: Input<'input>,
    T: OrderedSequence<Token = I::Token>,
{
    Exact {
        seq,
        phantom: PhantomData,
    }
}

impl<'input, I, T> Parser<'input, I, I::Slice> for Exact<I, T>
where
    I: Input<'input>,
    T: OrderedSequence<Token = I::Token>,
{
    fn go(&self, input_ref: &mut InputRef<'input, '_, I>) -> ParseResult<I::Slice> {
        let start = input_ref.offset();

        if let Some(_token) = self.seq.iterator().find_map(|seq_token| {
            if Some(seq_token) == input_ref.peek_token() {
                input_ref.next_token();
                None
            } else {
                Some(())
            }
        }) {
            Err(ParseError::SyntaxError)
        } else {
            Ok(input_ref.slice(start, input_ref.offset()))
        }
    }
}

// `End` combinator matches the EOI (end of input).
// Returns an error if the input is not yet fully consumed.
#[derive(Clone, Copy)]
struct End<I> {
    phantom: PhantomData<I>,
}

fn end<'input, I>() -> End<I>
where
    I: Input<'input>,
{
    End {
        phantom: PhantomData,
    }
}

impl<'input, I> Parser<'input, I, ()> for End<I>
where
    I: Input<'input>,
{
    fn go(&self, input_ref: &mut InputRef<'input, '_, I>) -> ParseResult<()> {
        if input_ref.peek_token().is_some() {
            Err(ParseError::SyntaxError)
        } else {
            Ok(())
        }
    }
}

// `Any` combinator matches any token except the EOI (end of input).
// Returns an error if the input was totally consumed (i.e empty).
#[derive(Clone, Copy)]
struct Any<I> {
    phantom: PhantomData<I>,
}

fn any<'input, I>() -> Any<I>
where
    I: Input<'input>,
{
    Any {
        phantom: PhantomData,
    }
}

impl<'input, I> Parser<'input, I, I::Token> for Any<I>
where
    I: Input<'input>,
{
    fn go(&self, input_ref: &mut InputRef<'input, '_, I>) -> ParseResult<I::Token> {
        if input_ref.peek_token().is_some() {
            Ok(input_ref.next_token().unwrap())
        } else {
            Err(ParseError::SyntaxError)
        }
    }
}

// `OneOf` primitive, matches one of the sequence passed in as a parameter
#[derive(Clone)]
struct OneOf<I, S> {
    container: Vec<S>,
    phantom: PhantomData<I>,
}

fn one_of<'input, I, S>(container: Vec<S>) -> OneOf<I, S>
where
    I: Input<'input>,
    S: OrderedSequence<Token = I::Token>,
{
    OneOf {
        container,
        phantom: PhantomData,
    }
}

impl<'input, I, S> Parser<'input, I, I::Slice> for OneOf<I, S>
where
    I: Input<'input>,
    S: OrderedSequence<Token = I::Token>,
    I::Token: std::fmt::Display + std::fmt::Debug,
    I::Slice: std::fmt::Display,
{
    fn go(&self, input_ref: &mut InputRef<'input, '_, I>) -> ParseResult<I::Slice> {
        let start_offset = input_ref.offset();

        for seq in self.container.iter() {
            if let Some(_) = seq.iterator().find_map(|seq_token| {
                if Some(seq_token) == input_ref.peek_token() {
                    input_ref.next_token();
                    None
                } else {
                    Some(())
                }
            }) {
                input_ref.rewind(start_offset);
            } else {
                return Ok(input_ref.slice(start_offset, input_ref.offset()));
            }
        }
        Err(ParseError::SyntaxError)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! input_ref {
        ($e: expr) => {
            InputRef::new(&$e)
        };
    }

    // Sanity check `exact` combinator
    // Success case
    #[test]
    fn test_exact_simple_string_ok() {
        let mut input_ref = input_ref!("hello world");

        let parser = exact("hello");

        assert_eq!(parser.go(&mut input_ref), Ok("hello"));
        assert_eq!(input_ref.offset(), "hello".len());

        let parser = exact(" world");
        assert_eq!(parser.go(&mut input_ref), Ok(" world"));
        assert_eq!(input_ref.offset(), "hello world".len());
    }

    // Sanity check for `exact` combinator
    // error case
    #[test]
    fn test_exact_simple_string_err() {
        let mut input_ref = input_ref!(b"hello world" as &[u8]);

        let parser = exact(b"hellqasd" as &[u8]);

        assert_eq!(parser.go(&mut input_ref), Err(ParseError::SyntaxError));
        assert_eq!(input_ref.offset(), "hell".len());
    }

    // Sanity check for `end` combinator
    // Success case.
    #[test]
    fn test_end_ok() {
        let mut input_ref = input_ref!("");
        let parser = end();

        assert_eq!(parser.go(&mut input_ref), Ok(()));
        assert_eq!(input_ref.offset(), 0);
    }

    // Sanity check for `end` combinator
    // Error case.
    #[test]
    fn test_end_err() {
        let mut input_ref = input_ref!("characters left in the input");
        let parser = end();

        assert_eq!(parser.go(&mut input_ref), Err(ParseError::SyntaxError));
        assert_eq!(input_ref.offset(), 0);
    }

    // Sanity check for `any` combinator
    // Success case
    #[test]
    fn test_any_ok() {
        let mut input_ref = input_ref!("any");
        let parser = any();

        assert_eq!(parser.go(&mut input_ref), Ok('a'));
        assert_eq!(input_ref.offset(), "a".len());

        assert_eq!(parser.go(&mut input_ref), Ok('n'));
        assert_eq!(input_ref.offset(), "ab".len());

        assert_eq!(parser.go(&mut input_ref), Ok('y'));
        assert_eq!(input_ref.offset(), "any".len());
    }

    // Sanity check for `any` combinator
    // Success case
    #[test]
    fn test_any_err() {
        let mut input_ref = input_ref!("");
        let parser = any();

        assert_eq!(parser.go(&mut input_ref), Err(ParseError::SyntaxError));
        assert_eq!(input_ref.offset(), 0);
    }

    // Sanity check for `map` operator
    #[test]
    fn test_map() {
        let mut input_ref = input_ref!("123");
        let parser = exact("123").map(|string: &str| string.parse::<u32>().ok());

        assert_eq!(parser.go(&mut input_ref), Ok(Some(123)));
        assert_eq!(input_ref.offset(), "123".len());
        assert_eq!(parser.go(&mut input_ref), Err(ParseError::SyntaxError));
    }

    // Sanity check for `bind` operator
    #[test]
    fn test_bind() {
        let mut input_ref = input_ref!("123456");
        let parser = exact("123").right_bind(exact("456")).left_bind(end());

        assert_eq!(parser.go(&mut input_ref), Ok("456"));
        assert_eq!(input_ref.offset(), "123456".len());

        input_ref.rewind(input_ref.start());

        let parser = exact("123")
            .right_bind(exact("456"))
            .map(|string: &str| string.parse::<u32>().ok());

        assert_eq!(parser.go(&mut input_ref), Ok(Some(456)));
        assert_eq!(input_ref.offset(), "123456".len());

        input_ref.rewind(input_ref.start());

        let parser = exact("123")
            .left_bind(exact("456"))
            .map(|string: &str| string.parse::<u32>().ok());

        assert_eq!(parser.go(&mut input_ref), Ok(Some(123)));
        assert_eq!(input_ref.offset(), "123456".len());
    }

    #[test]
    fn test_bind_with_separated_value() {
        let mut input_ref = input_ref!("123-456");
        let parser = exact("123")
            .left_bind(exact("-"))
            .right_bind(exact("456"))
            .map(|string: &str| string.parse::<u32>().ok());

        assert_eq!(parser.go(&mut input_ref), Ok(Some(456)));

        input_ref.rewind(input_ref.start());

        let parser = exact("123")
            .left_bind(exact("-"))
            .left_bind(exact("456"))
            .map(|string: &str| string.parse::<u32>().ok());

        assert_eq!(parser.go(&mut input_ref), Ok(Some(123)));
        assert_eq!(input_ref.offset(), "123-456".len());
    }

    // Sanity check for `And` operator
    #[test]
    fn test_and() {
        let mut input_ref = input_ref!("https://");
        let https = exact("https");
        let slashes = exact("//");

        let parser = https.left_bind(exact(":")).and(slashes);

        assert_eq!(parser.go(&mut input_ref), Ok(("https", "//")));

        input_ref.rewind(input_ref.start());

        let parser = exact("https")
            .left_bind(exact("er"))
            .right_bind(exact("//"));

        assert_eq!(parser.go(&mut input_ref), Err(ParseError::SyntaxError));
        assert_eq!(input_ref.offset(), "https".len());
    }

    // Sanity check for `Repeated` operator
    #[test]
    fn test_repeated() {
        let mut input_ref = input_ref!("hhhhhhoooooo");

        let parser = exact('h')
            .repeated()
            .at_least(3)
            .at_most(4)
            .collect::<Vec<_>>();

        assert_eq!(parser.go(&mut input_ref), Ok(vec!["h"; 4]));
        assert_eq!(input_ref.offset(), 4);

        input_ref.rewind(input_ref.start());

        let parser = exact('h').repeated().at_least(1).collect::<Vec<_>>();
        assert_eq!(parser.go(&mut input_ref), Ok(vec!["h"; 6]));
        assert_eq!(input_ref.offset(), 6);

        input_ref.rewind(input_ref.start());

        let parser = exact('h')
            .repeated()
            .at_least(1)
            .collect::<Vec<_>>()
            .and(exact('o').repeated().at_least(1).collect::<Vec<_>>())
            .left_bind(end());

        assert_eq!(parser.go(&mut input_ref), Ok((vec!["h"; 6], vec!["o"; 6])));
    }

    #[test]
    fn test_repeated_err() {
        let mut input_ref = input_ref!("hhhhhooooo");
        let parser = exact('h')
            .repeated()
            .at_least(100)
            .at_most(400)
            .collect::<Vec<_>>();

        assert_eq!(
            parser.go(&mut input_ref),
            Err::<Vec<_>, _>(ParseError::SyntaxError)
        );
    }

    #[test]
    fn test_filter_ok() {
        let mut input_ref = input_ref!("132letters");

        let digits = any()
            .filter(|c: &char| c.is_ascii_digit())
            .repeated()
            .at_least(1)
            .collect::<Vec<_>>();

        let letters = any()
            .filter(|c: &char| c.is_ascii_alphabetic())
            .repeated()
            .at_least(1)
            .collect::<Vec<_>>();

        assert_eq!(digits.go(&mut input_ref), Ok(vec!['1', '3', '2']));
        assert_eq!(input_ref.offset(), "132".len());
        assert_eq!(
            letters.go(&mut input_ref),
            Ok(vec!['l', 'e', 't', 't', 'e', 'r', 's'])
        );

        input_ref.rewind(input_ref.start());

        let parser = digits.left_bind::<_, Vec<_>>(letters).left_bind(end());
        assert_eq!(parser.go(&mut input_ref), Ok(vec!['1', '3', '2']));
    }

    #[test]
    fn test_floating_point_number() {
        let mut input_ref = input_ref!("-123.234");

        let digit_seq = any()
            .filter(|c: &char| c.is_ascii_digit())
            .repeated()
            .at_least(1)
            .collect::<Vec<_>>();

        let parser = exact('-')
            .right_bind(digit_seq.clone())
            .left_bind(exact('.'))
            .and(digit_seq);

        assert_eq!(
            parser.go(&mut input_ref),
            Ok((vec!['1', '2', '3'], vec!['2', '3', '4']))
        );
        assert_eq!(input_ref.offset(), "-123.234".len());

        input_ref.rewind(input_ref.start());

        let digit_seq = any()
            .filter(|c: &char| c.is_ascii_digit())
            .repeated()
            .exactly(3)
            .collect::<String>();

        let parser = exact('-')
            .right_bind(digit_seq.clone())
            .left_bind(exact('.'))
            .and(digit_seq);

        assert_eq!(
            parser.go(&mut input_ref),
            Ok((String::from("123"), String::from("234")))
        );
        assert_eq!(input_ref.offset(), "-123.234".len());

        input_ref.rewind(input_ref.start());

        let digit_seq = any()
            .filter(|c: &char| c.is_ascii_digit())
            .repeated()
            .at_least(2)
            .at_most(3)
            .collect::<String>();

        let parser = exact('-')
            .right_bind(digit_seq.clone())
            .left_bind(exact('.'))
            .and(digit_seq);

        assert_eq!(
            parser.go(&mut input_ref),
            Ok((String::from("123"), String::from("234")))
        );
        assert_eq!(input_ref.offset(), "-123.234".len());
    }

    #[test]
    fn identifier() {
        use std::collections::HashSet;

        let mut input_ref = input_ref!("ident_ifier");
        let parser = any()
            .filter(|c: &char| c.is_ascii())
            .repeated()
            .at_least(1)
            .collect::<String>();

        assert_eq!(parser.go(&mut input_ref), Ok(String::from("ident_ifier")));
        assert_eq!(input_ref.offset(), "ident_ifier".len());

        input_ref.rewind(input_ref.start());

        let parser = any()
            .filter(|c: &char| c.is_ascii())
            .repeated()
            .at_least(1)
            .collect::<HashSet<_>>();

        assert_eq!(
            parser.go(&mut input_ref),
            Ok(HashSet::from([
                'i', 'd', 'e', 'n', 't', '_', 'i', 'f', 'i', 'e', 'r'
            ]))
        );
    }

    #[test]
    fn test_one_of() {
        let mut input_ref = input_ref!("12345");

        let parser = one_of(vec!['1', '2', '3']);

        assert_eq!(parser.go(&mut input_ref), Ok("1"));
        assert_eq!(input_ref.offset(), 1);

        input_ref.rewind(input_ref.start());

        let parser = one_of(vec!["124", "1235", "122", "12345"]).left_bind(end());

        assert_eq!(parser.go(&mut input_ref), Ok("12345"));
        assert_eq!(input_ref.offset(), "12345".len());

        input_ref.rewind(input_ref.start());

        let parser = one_of(vec!["124", "1235", "122"]);

        assert_eq!(parser.go(&mut input_ref), Err(ParseError::SyntaxError));
        assert_eq!(input_ref.offset(), input_ref.start());

        input_ref.rewind(input_ref.start());

        let parser = one_of(vec!["124", "1235", "122"]);

        assert_eq!(parser.go(&mut input_ref), Err(ParseError::SyntaxError));
        assert_eq!(input_ref.offset(), input_ref.start());
    }

    #[test]
    fn test_padded_by() {
        let mut input_ref = input_ref!(r#" { "key1": "value1", "key2": "value2", } "#);
        assert_eq!(input_ref.offset(), 0);

        let white_space = any()
            .filter(|c: &char| c == &' ')
            .repeated()
            .at_least(0)
            .collect::<String>();

        let left_brace = exact('{').padded(white_space.clone());
        let right_brace = exact('}').padded(white_space.clone());
        let column = exact(':').padded(white_space.clone());
        let comma = exact(',').padded(white_space.clone());

        let string = exact('"')
            .right_bind(
                any()
                    .filter(|c: &char| c.is_ascii_alphanumeric())
                    .repeated()
                    .at_least(1)
                    .collect::<String>(),
            )
            .left_bind(exact('"'));

        let kvp = string.clone().left_bind(column).and(string);

        let json_file = left_brace
            .clone()
            .right_bind(kvp.clone())
            .left_bind(comma.clone())
            .and(kvp.clone())
            .left_bind(comma)
            .left_bind(right_brace);

        let kvp1 = (String::from("key1"), String::from("value1"));
        let kvp2 = (String::from("key2"), String::from("value2"));

        assert_eq!(json_file.go(&mut input_ref), Ok((kvp1, kvp2)));
    }

    #[test]
    fn test_string() {
        let mut input_ref = input_ref!(r#"    "       string"   "#);
        let white_space = any()
            .filter(|c: &char| c == &' ')
            .repeated()
            .at_least(0)
            .collect::<String>();

        let string = exact('"').padded(white_space).right_bind(
            any()
                .filter(|c: &char| c.is_ascii_alphabetic())
                .repeated()
                .at_least(1)
                .collect::<String>(),
        );

        assert_eq!(string.go(&mut input_ref), Ok(String::from("string")));
    }

    #[test]
    fn test_or() {
        let mut input_ref = input_ref!("http://localhost");

        let parser = exact("https").or(exact("http"));
        assert_eq!(parser.go(&mut input_ref), Ok("http"));

        input_ref.rewind(input_ref.start());

        let parser = exact("http::").or(exact("httppp")).or(exact("htttt"));

        assert_eq!(parser.go(&mut input_ref), Err(ParseError::SyntaxError));
    }

    #[test]
    fn test_long_string() {
        const SIZE: usize = 1_000_000;
        let long_string = vec!['c'; SIZE]
            .into_iter()
            .chain(vec!['d'; SIZE].into_iter())
            .into_iter()
            .collect::<String>();

        let long_string_as_str = long_string.as_str();
        let mut input_ref = input_ref!(long_string_as_str);

        let bounded_parser = |at_least, at_most| {
            any()
                .filter(|c: &char| c == &'c')
                .repeated()
                .at_least(at_least)
                .at_most(at_most)
                .collect::<String>()
                .and(
                    any()
                        .filter(|c: &char| c == &'d')
                        .repeated()
                        .at_least(at_least)
                        .at_most(at_most)
                        .collect::<String>(),
                )
        };

        let string = |c, size| vec![c; size].into_iter().collect::<String>();

        let parser = bounded_parser(1, SIZE);
        assert_eq!(
            parser.go(&mut input_ref),
            Ok((string('c', SIZE), string('d', SIZE)))
        );

        assert_eq!(input_ref.offset(), SIZE << 1);

        input_ref.rewind(input_ref.start());

        let parser = bounded_parser(SIZE + 1, SIZE + 2);
        assert_eq!(parser.go(&mut input_ref), Err(ParseError::SyntaxError));
        assert_eq!(input_ref.offset(), SIZE);
    }
}
