//! The query steps HUME's copy leaves out, which cost a query its speed
//! and give HUME nothing.
//!
//! - `context.scm` loses each step only an `@context.end` capture needs.
//!   HUME reads `@context` and `@context.start` alone, and a step such as
//!   `(impl_item body: (_ (_) @context.end))` makes tree-sitter produce a
//!   match for every child of a wide node.
//! - `folds.scm` loses repetition: `+` goes and `*` becomes `?`. A
//!   repeated capture keeps a match in progress at every sibling it could
//!   start on, and each node it captured folds on its own instead.

use std::fmt;
use std::ops::Range;

const CONTEXT_END: &str = "@context.end";

#[derive(Debug)]
pub(crate) struct RewriteError(String);

impl fmt::Display for RewriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "cannot read the query: {}", self.0)
    }
}

/// `text`, the resolved query file `file`, as HUME's copy ships it.
pub(crate) fn for_hume(file: &str, text: &str) -> Result<String, RewriteError> {
    let edits = match file {
        "context.scm" => without_context_end(text, &tokens(text)?)?,
        "folds.scm" => without_repetition(text, &tokens(text)?),
        _ => return Ok(text.to_string()),
    };
    Ok(apply(text, edits))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Open,
    Close,
    Str,
    Capture,
    Field,
    Negated,
    Quantifier,
    Anchor,
    Predicate,
    Word,
    Comment,
}

struct Token {
    kind: Kind,
    span: Range<usize>,
}

/// `text`'s tokens; whitespace separates them and is not one.
fn tokens(text: &str) -> Result<Vec<Token>, RewriteError> {
    let bytes = text.as_bytes();
    let name_end = |from: usize, extra: &[u8]| {
        bytes[from..]
            .iter()
            .position(|b| !(b.is_ascii_alphanumeric() || b"_-".contains(b) || extra.contains(b)))
            .map_or(bytes.len(), |n| from + n)
    };
    let mut out = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        let start = at;
        let kind = match bytes[at] {
            b if b.is_ascii_whitespace() => {
                at += 1;
                continue;
            }
            b';' => {
                at = bytes[at..]
                    .iter()
                    .position(|&b| b == b'\n')
                    .map_or(bytes.len(), |n| at + n);
                Kind::Comment
            }
            b'"' => {
                at += 1;
                loop {
                    match bytes.get(at) {
                        None => return Err(RewriteError("an unterminated string".into())),
                        Some(b'\\') => at += 2,
                        Some(b'"') => break,
                        Some(_) => at += 1,
                    }
                }
                at += 1;
                Kind::Str
            }
            b'(' | b'[' => {
                at += 1;
                Kind::Open
            }
            b')' | b']' => {
                at += 1;
                Kind::Close
            }
            b'?' | b'*' | b'+' => {
                at += 1;
                Kind::Quantifier
            }
            b'.' => {
                at += 1;
                Kind::Anchor
            }
            b'@' => {
                at = name_end(at + 1, b".");
                Kind::Capture
            }
            b'#' => {
                at = name_end(at + 1, b"?!");
                Kind::Predicate
            }
            b'!' => {
                at = name_end(at + 1, b"");
                Kind::Negated
            }
            b if b.is_ascii_alphanumeric() || b == b'_' => {
                // `/` names a subtype of a supertype, `(expression/case)`.
                at = name_end(at, b"/");
                if bytes.get(at) == Some(&b':') {
                    at += 1;
                    Kind::Field
                } else {
                    Kind::Word
                }
            }
            other => {
                return Err(RewriteError(format!(
                    "unexpected {:?} at byte {start}",
                    char::from(other)
                )));
            }
        };
        out.push(Token {
            kind,
            span: start..at,
        });
    }
    Ok(out)
}

/// One step of a pattern, as indices into the token list: an expression
/// with its field, quantifier and captures, or a predicate.
struct Step {
    /// Its first token, the field when it has one.
    first: usize,
    last: usize,
    /// The anchor right before it.
    anchor: Option<usize>,
    /// Its `@context.end` captures.
    ends: Vec<usize>,
    /// Whether it, or a step inside it, has another capture.
    holds_other_capture: bool,
    /// For a predicate, whether it names `@context.end`.
    predicate: Option<bool>,
}

struct Parser<'t> {
    text: &'t str,
    tokens: &'t [Token],
    /// The indices of every token but comments.
    order: Vec<usize>,
    at: usize,
    steps: Vec<Step>,
}

impl Parser<'_> {
    fn kind(&self, at: usize) -> Option<Kind> {
        self.order.get(at).map(|&i| self.tokens[i].kind)
    }

    fn index(&self) -> Result<usize, RewriteError> {
        self.order
            .get(self.at)
            .copied()
            .ok_or_else(|| RewriteError("an unclosed parenthesis".into()))
    }

    fn is_context_end(&self, i: usize) -> bool {
        &self.text[self.tokens[i].span.clone()] == CONTEXT_END
    }

    /// The steps up to the closing bracket, or to the end at the top level,
    /// and whether one holds a capture other than `@context.end`.
    fn sequence(&mut self, top: bool) -> Result<bool, RewriteError> {
        let mut holds_other = false;
        let mut anchor = None;
        loop {
            match self.kind(self.at) {
                None if top => return Ok(holds_other),
                None => return Err(RewriteError("an unclosed parenthesis".into())),
                Some(Kind::Close) if top => {
                    return Err(RewriteError("an unopened parenthesis".into()));
                }
                Some(Kind::Close) => return Ok(holds_other),
                Some(Kind::Anchor) => {
                    anchor = Some(self.index()?);
                    self.at += 1;
                }
                Some(_) => holds_other |= self.step(anchor.take())?,
            }
        }
    }

    /// Record one step, and whether it holds a capture other than
    /// `@context.end`.
    fn step(&mut self, anchor: Option<usize>) -> Result<bool, RewriteError> {
        let first = self.index()?;
        if self.kind(self.at) == Some(Kind::Field) {
            self.at += 1;
        }
        let head = self.index()?;
        let opens_predicate = self.kind(self.at + 1) == Some(Kind::Predicate);
        let mut holds_other = false;
        let mut last = head;
        match self.tokens[head].kind {
            Kind::Open if opens_predicate => {
                let mut names_end = false;
                loop {
                    let i = self.index()?;
                    self.at += 1;
                    match self.tokens[i].kind {
                        Kind::Close => {
                            self.steps.push(Step {
                                first,
                                last: i,
                                anchor,
                                ends: Vec::new(),
                                holds_other_capture: false,
                                predicate: Some(names_end),
                            });
                            return Ok(false);
                        }
                        Kind::Capture => names_end |= self.is_context_end(i),
                        _ => {}
                    }
                }
            }
            Kind::Open => {
                self.at += 1;
                holds_other = self.sequence(false)?;
                last = self.index()?;
                self.at += 1;
            }
            Kind::Close => return Err(RewriteError("a field with no step".into())),
            _ => self.at += 1,
        }
        let mut ends = Vec::new();
        while let Some(kind @ (Kind::Quantifier | Kind::Capture)) = self.kind(self.at) {
            let i = self.index()?;
            if kind == Kind::Capture {
                if self.is_context_end(i) {
                    ends.push(i);
                } else {
                    holds_other = true;
                }
            }
            last = i;
            self.at += 1;
        }
        self.steps.push(Step {
            first,
            last,
            anchor,
            ends,
            holds_other_capture: holds_other,
            predicate: None,
        });
        Ok(holds_other)
    }
}

/// The edits that drop each step only `@context.end` needs, with the
/// anchor before it, each predicate naming it, and the capture alone from
/// a step that holds another capture.
fn without_context_end(text: &str, tokens: &[Token]) -> Result<Vec<Edit>, RewriteError> {
    let mut parser = Parser {
        text,
        tokens,
        order: (0..tokens.len())
            .filter(|&i| tokens[i].kind != Kind::Comment)
            .collect(),
        at: 0,
        steps: Vec::new(),
    };
    parser.sequence(true)?;
    let cut = |first: usize, last: usize| Edit {
        span: cut_start(tokens, first)..tokens[last].span.end,
        with: "",
    };
    let mut edits = Vec::new();
    for step in &parser.steps {
        match step.predicate {
            Some(true) => edits.push(cut(step.first, step.last)),
            Some(false) => {}
            None if step.ends.is_empty() => {}
            None if step.holds_other_capture => {
                edits.extend(step.ends.iter().map(|&end| cut(end, end)));
            }
            None => edits.push(cut(step.anchor.unwrap_or(step.first), step.last)),
        }
    }
    Ok(edits)
}

/// Where a cut from token `first` starts: at the end of the token before
/// it, so the whitespace in between goes too, unless that token is a
/// comment, which would then run on over what follows the cut.
fn cut_start(tokens: &[Token], first: usize) -> usize {
    match first.checked_sub(1).map(|i| &tokens[i]) {
        Some(before) if before.kind != Kind::Comment => before.span.end,
        _ => tokens[first].span.start,
    }
}

/// The edits that drop `+` and turn `*` into `?`.
fn without_repetition(text: &str, tokens: &[Token]) -> Vec<Edit> {
    tokens
        .iter()
        .filter(|token| token.kind == Kind::Quantifier)
        .filter_map(|token| {
            let with = match &text[token.span.clone()] {
                "+" => "",
                "*" => "?",
                _ => return None,
            };
            Some(Edit {
                span: token.span.clone(),
                with,
            })
        })
        .collect()
}

struct Edit {
    span: Range<usize>,
    with: &'static str,
}

/// `text` with `edits` made, an edit inside an earlier one dropped.
fn apply(text: &str, mut edits: Vec<Edit>) -> String {
    edits.sort_by_key(|edit| (edit.span.start, std::cmp::Reverse(edit.span.end)));
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    for edit in edits {
        if edit.span.start < at {
            continue;
        }
        out.push_str(&text[at..edit.span.start]);
        out.push_str(edit.with);
        at = edit.span.end;
    }
    out.push_str(&text[at..]);
    out
}

#[cfg(test)]
mod tests;
