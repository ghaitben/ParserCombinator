// The input trait abstracts over &str and &[u8] input streams.
// The tokens yielded by each of those input streams are cheap to copy, in fact, copying the tokens
// aforementioned is faster than copying their references.
pub trait Input<'input>: 'input {
    type Token: Copy + Eq;

    type Offset: Copy + Eq + Into<usize>;

    type Slice: Copy;

    fn next(&self, offset: Self::Offset) -> (Self::Offset, Option<Self::Token>);

    fn slice(&self, start: Self::Offset, end: Self::Offset) -> Self::Slice;

    fn start(&self) -> Self::Offset;
}

impl<'input> Input<'input> for &'input str {
    type Token = char;

    type Offset = usize;

    type Slice = &'input str;

    fn next(&self, offset: Self::Offset) -> (Self::Offset, Option<Self::Token>) {
        if let Some(c) = self[offset..].chars().next() {
            (offset + c.len_utf8(), Some(c))
        } else {
            (offset, None)
        }
    }

    #[inline(always)]
    fn slice(&self, start: Self::Offset, end: Self::Offset) -> Self::Slice {
        &self[start..end]
    }

    #[inline(always)]
    fn start(&self) -> Self::Offset {
        0
    }
}

impl<'input> Input<'input> for &'input [u8] {
    type Token = u8;

    type Offset = usize;

    type Slice = &'input [u8];

    fn next(&self, offset: Self::Offset) -> (Self::Offset, Option<Self::Token>) {
        if let Some(byte) = self[offset..].iter().next().copied() {
            (offset + 1, Some(byte))
        } else {
            (offset, None)
        }
    }

    #[inline(always)]
    fn slice(&self, start: Self::Offset, end: Self::Offset) -> Self::Slice {
        &self[start..end]
    }

    #[inline(always)]
    fn start(&self) -> Self::Offset {
        0
    }
}

// Why are we even take the input by reference?
// the input is cheaply copiable so maybe store it by value instead?
pub struct InputRef<'input, 'parse, I>
where
    I: Input<'input>,
{
    input: &'parse I,
    offset: I::Offset,
}

impl<'input, 'parse, I> InputRef<'input, 'parse, I>
where
    I: Input<'input>,
{
    pub fn new(input: &'parse I) -> Self {
        Self {
            input,
            offset: input.start(),
        }
    }

    pub fn next(&mut self) -> (I::Offset, Option<I::Token>) {
        let (next_offset, next) = self.input.next(self.offset);
        self.offset = next_offset;
        (self.offset, next)
    }

    #[inline(always)]
    pub fn next_offset(&mut self) -> I::Offset {
        self.next().0
    }

    #[inline(always)]
    pub fn next_token(&mut self) -> Option<I::Token> {
        self.next().1
    }

    pub fn peek(&self) -> (I::Offset, Option<I::Token>) {
        self.input.next(self.offset)
    }

    #[inline(always)]
    pub fn peek_token(&self) -> Option<I::Token> {
        self.peek().1
    }

    #[inline]
    pub fn rewind(&mut self, offset: I::Offset) {
        self.offset = offset;
    }

    #[inline]
    pub fn start(&self) -> I::Offset {
        self.input.start()
    }

    #[inline(always)]
    pub fn offset(&self) -> I::Offset {
        self.offset
    }

    #[inline(always)]
    pub fn slice(&self, start: I::Offset, end: I::Offset) -> I::Slice {
        self.input.slice(start, end)
    }
}
