//! The Molang that keyframe values may hold: mochafloats' syntax tree (`org.redlance.mocha`),
//! its parser (`MolangLexerImpl`, `MolangParserImpl`), its byte form (`ExprBytesUtils`, which
//! the animation binary v6 carries for values that aren't constants), `IsConstantExpression`,
//! and an evaluator for what an emote uses: numbers, `math.*`, `query.anim_time`, variables,
//! the operators, conditionals and scopes.
//!
//! Ported from mochafloats (Copyright (c) 2021-2025 Unnamed Team, MIT, see NOTICE).

use crate::buf::{Reader, Result, Writer, err};
use std::collections::HashMap;

/// `UnaryExpression.Op`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnaryOp {
    LogicalNegation,
    ArithmeticalNegation,
    Return,
}

/// `StatementExpression.Op`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatementOp {
    Break,
    Continue,
}

/// `BinaryExpression.Op`, in its ordinal order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinOp {
    And,
    Or,
    Lt,
    Lte,
    Gt,
    Gte,
    Add,
    Sub,
    Mul,
    Div,
    Arrow,
    NullCoalesce,
    Assign,
    Conditional,
    Eq,
    Neq,
}

impl BinOp {
    const ALL: [BinOp; 16] = [
        BinOp::And,
        BinOp::Or,
        BinOp::Lt,
        BinOp::Lte,
        BinOp::Gt,
        BinOp::Gte,
        BinOp::Add,
        BinOp::Sub,
        BinOp::Mul,
        BinOp::Div,
        BinOp::Arrow,
        BinOp::NullCoalesce,
        BinOp::Assign,
        BinOp::Conditional,
        BinOp::Eq,
        BinOp::Neq,
    ];
    fn precedence(self) -> i32 {
        match self {
            BinOp::And => 300,
            BinOp::Or => 200,
            BinOp::Lt | BinOp::Lte | BinOp::Gt | BinOp::Gte => 700,
            BinOp::Add | BinOp::Sub => 900,
            BinOp::Mul | BinOp::Div => 1000,
            BinOp::Arrow => 2000,
            BinOp::NullCoalesce => 2,
            BinOp::Assign | BinOp::Conditional => 1,
            BinOp::Eq | BinOp::Neq => 500,
        }
    }
}

/// A Molang expression (`org.redlance.mocha.parser.ast`).
#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    Unary(UnaryOp, Box<Expr>),
    Ternary(Box<Expr>, Box<Expr>, Box<Expr>),
    Str(String),
    Statement(StatementOp),
    Ident(String),
    Float(f32),
    Scope(Vec<Expr>),
    Call(Box<Expr>, Vec<Expr>),
    Binary(BinOp, Box<Expr>, Box<Expr>),
    Index(Box<Expr>, Box<Expr>),
    Access(Box<Expr>, String),
}

/// A keyframe value: one or more expressions, the last one's value counts.
pub type Exprs = Vec<Expr>;

pub fn constant(v: f32) -> Exprs {
    vec![Expr::Float(v)]
}

// ---- the byte form (`ExprBytesUtils`) -----------------------------------------------------

pub fn write_expr(e: &Expr, w: &mut Writer) {
    match e {
        Expr::Unary(op, x) => {
            w.u8(0);
            w.u8(match op {
                UnaryOp::LogicalNegation => 0,
                UnaryOp::ArithmeticalNegation => 1,
                UnaryOp::Return => 2,
            });
            write_expr(x, w);
        }
        Expr::Ternary(c, t, f) => {
            w.u8(1);
            write_expr(c, w);
            write_expr(t, w);
            write_expr(f, w);
        }
        Expr::Str(s) => {
            w.u8(2);
            w.vstring(Some(s));
        }
        Expr::Statement(op) => {
            w.u8(3);
            w.u8(match op {
                StatementOp::Break => 0,
                StatementOp::Continue => 1,
            });
        }
        Expr::Ident(s) => {
            w.u8(4);
            w.vstring(Some(s));
        }
        Expr::Float(v) => {
            w.u8(5);
            w.f32(*v);
        }
        Expr::Scope(list) => {
            w.u8(6);
            write_exprs(list, w);
        }
        Expr::Call(f, args) => {
            w.u8(7);
            write_expr(f, w);
            write_exprs(args, w);
        }
        Expr::Binary(op, l, r) => {
            w.u8(8);
            w.u8(BinOp::ALL.iter().position(|o| o == op).unwrap_or(0) as u8);
            write_expr(l, w);
            write_expr(r, w);
        }
        Expr::Index(a, i) => {
            w.u8(9);
            write_expr(a, w);
            write_expr(i, w);
        }
        Expr::Access(o, p) => {
            w.u8(10);
            write_expr(o, w);
            w.vstring(Some(p));
        }
    }
}

pub fn write_exprs(list: &[Expr], w: &mut Writer) {
    w.varint(list.len() as i32);
    for e in list {
        write_expr(e, w);
    }
}

/// How tall an expression tree may be, read from bytes or parsed from text: the evaluator
/// and the byte form recurse on it, so a taller one (a crafted keyframe) is rejected before
/// it can exhaust the stack.
const MAX_DEPTH: u32 = 256;

/// How deep the text parser may recurse (brackets, signs, right-hand operands in each
/// other); each level costs it far more stack than a level of the tree costs the evaluator.
const MAX_NEST: u32 = 64;

fn read_depth(r: &mut Reader<'_>, depth: u32) -> Result<Expr> {
    if depth > MAX_DEPTH {
        return err("Molang expression nests too deep");
    }
    let id = r.u8()?;
    let d = depth + 1;
    Ok(match id {
        0 => {
            let op = match r.u8()? {
                0 => UnaryOp::LogicalNegation,
                1 => UnaryOp::ArithmeticalNegation,
                2 => UnaryOp::Return,
                n => return err(format!("Unknown UnaryExpression.Op ordinal: {n}")),
            };
            Expr::Unary(op, Box::new(read_depth(r, d)?))
        }
        1 => Expr::Ternary(Box::new(read_depth(r, d)?), Box::new(read_depth(r, d)?), Box::new(read_depth(r, d)?)),
        2 => Expr::Str(r.vstring()?.unwrap_or_default()),
        3 => Expr::Statement(match r.u8()? {
            0 => StatementOp::Break,
            1 => StatementOp::Continue,
            n => return err(format!("Unknown StatementExpression.Op ordinal: {n}")),
        }),
        4 => Expr::Ident(r.vstring()?.unwrap_or_default()),
        5 => Expr::Float(r.f32()?),
        6 => Expr::Scope(read_list(r, d)?),
        7 => {
            let f = read_depth(r, d)?;
            Expr::Call(Box::new(f), read_list(r, d)?)
        }
        8 => {
            let n = r.u8()? as usize;
            let op = *BinOp::ALL.get(n).ok_or_else(|| crate::buf::Error(format!("Unknown BinaryExpression.Op ordinal: {n}")))?;
            Expr::Binary(op, Box::new(read_depth(r, d)?), Box::new(read_depth(r, d)?))
        }
        9 => Expr::Index(Box::new(read_depth(r, d)?), Box::new(read_depth(r, d)?)),
        10 => {
            let o = read_depth(r, d)?;
            Expr::Access(Box::new(o), r.vstring()?.unwrap_or_default())
        }
        _ => return err("Unknown expression id in buffer"),
    })
}

fn read_list(r: &mut Reader<'_>, depth: u32) -> Result<Exprs> {
    let n = r.varint()?;
    if n < 0 || n as usize > r.remaining() {
        return err("bad expression count");
    }
    (0..n).map(|_| read_depth(r, depth)).collect()
}

pub fn read_expr(r: &mut Reader<'_>) -> Result<Expr> {
    read_depth(r, 0)
}

pub fn read_exprs(r: &mut Reader<'_>) -> Result<Exprs> {
    read_list(r, 0)
}

// ---- constants (`IsConstantExpression` without a scope) -----------------------------------

pub fn is_constant(e: &Expr) -> bool {
    match e {
        Expr::Float(_) | Expr::Str(_) | Expr::Statement(_) => true,
        Expr::Ident(_) | Expr::Access(..) | Expr::Call(..) => false,
        Expr::Index(a, i) => is_constant(a) && is_constant(i),
        Expr::Ternary(c, t, f) => is_constant(c) && is_constant(t) && is_constant(f),
        Expr::Unary(_, x) => is_constant(x),
        Expr::Scope(list) => list.iter().all(is_constant),
        Expr::Binary(_, l, r) => is_constant(l) && is_constant(r),
    }
}

/// `MolangLoader.isConstant`: any of them is.
pub fn any_constant(list: &[Expr]) -> bool {
    list.iter().any(is_constant)
}

/// The single constant a value is, if it is one (`size == 1 && IsConstantExpression.test`).
pub fn single_constant(list: &[Expr]) -> bool {
    list.len() == 1 && is_constant(&list[0])
}

// ---- the lexer and parser ------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Float(String),
    Str(String),
    Ident(String),
    True,
    False,
    Break,
    Continue,
    Return,
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Dot,
    Comma,
    Semicolon,
    Ques,
    QuesQues,
    Colon,
    Eq,
    EqEq,
    Bang,
    BangEq,
    Lt,
    Lte,
    Gt,
    Gte,
    AmpAmp,
    BarBar,
    Plus,
    Sub,
    Star,
    Slash,
    Arrow,
    Eof,
    Error(String),
}

fn lex(s: &str) -> Vec<Tok> {
    let c: Vec<char> = s.chars().collect();
    let mut i = 0;
    let mut out = Vec::new();
    loop {
        while i < c.len() && matches!(c[i], ' ' | '\t' | '\n' | '\r') {
            i += 1;
        }
        if i >= c.len() {
            out.push(Tok::Eof);
            return out;
        }
        let ch = c[i];
        if ch.is_ascii_digit() {
            let start = i;
            while i < c.len() && c[i].is_ascii_digit() {
                i += 1;
            }
            if i < c.len() && c[i] == '.' {
                i += 1;
                while i < c.len() && c[i].is_ascii_digit() {
                    i += 1;
                }
            }
            if i < c.len() && (c[i] == 'e' || c[i] == 'E') {
                i += 1;
                if i < c.len() && (c[i] == '+' || c[i] == '-') {
                    i += 1;
                }
                if i >= c.len() || !c[i].is_ascii_digit() {
                    out.push(Tok::Error("Malformed floating point literal".into()));
                    out.push(Tok::Eof);
                    return out;
                }
                while i < c.len() && c[i].is_ascii_digit() {
                    i += 1;
                }
            }
            out.push(Tok::Float(c[start..i].iter().collect()));
            continue;
        }
        if ch.is_ascii_alphabetic() || ch == '_' {
            let start = i;
            while i < c.len() && (c[i].is_ascii_alphanumeric() || c[i] == '_') {
                i += 1;
            }
            let word: String = c[start..i].iter().collect();
            out.push(match word.to_ascii_lowercase().as_str() {
                "break" => Tok::Break,
                "continue" => Tok::Continue,
                "return" => Tok::Return,
                "true" => Tok::True,
                "false" => Tok::False,
                _ => Tok::Ident(word),
            });
            continue;
        }
        if ch == '\'' {
            let start = i + 1;
            i += 1;
            while i < c.len() && c[i] != '\'' {
                i += 1;
            }
            if i >= c.len() {
                out.push(Tok::Error("Found end-of-file before closing quote".into()));
                out.push(Tok::Eof);
                return out;
            }
            out.push(Tok::Str(c[start..i].iter().collect()));
            i += 1;
            continue;
        }
        let next = c.get(i + 1).copied();
        let (t, n) = match (ch, next) {
            ('!', Some('=')) => (Tok::BangEq, 2),
            ('!', _) => (Tok::Bang, 1),
            ('&', Some('&')) => (Tok::AmpAmp, 2),
            ('|', Some('|')) => (Tok::BarBar, 2),
            ('<', Some('=')) => (Tok::Lte, 2),
            ('<', _) => (Tok::Lt, 1),
            ('>', Some('=')) => (Tok::Gte, 2),
            ('>', _) => (Tok::Gt, 1),
            ('=', Some('=')) => (Tok::EqEq, 2),
            ('=', _) => (Tok::Eq, 1),
            ('-', Some('>')) => (Tok::Arrow, 2),
            ('-', _) => (Tok::Sub, 1),
            ('?', Some('?')) => (Tok::QuesQues, 2),
            ('?', _) => (Tok::Ques, 1),
            ('+', _) => (Tok::Plus, 1),
            ('*', _) => (Tok::Star, 1),
            ('/', _) => (Tok::Slash, 1),
            ('(', _) => (Tok::LParen, 1),
            (')', _) => (Tok::RParen, 1),
            ('{', _) => (Tok::LBrace, 1),
            ('}', _) => (Tok::RBrace, 1),
            ('[', _) => (Tok::LBracket, 1),
            (']', _) => (Tok::RBracket, 1),
            ('.', _) => (Tok::Dot, 1),
            (',', _) => (Tok::Comma, 1),
            (';', _) => (Tok::Semicolon, 1),
            (':', _) => (Tok::Colon, 1),
            _ => (Tok::Error(format!("Unexpected character '{ch}'")), 1),
        };
        i += n;
        let stop = matches!(t, Tok::Error(_));
        out.push(t);
        if stop {
            out.push(Tok::Eof);
            return out;
        }
    }
}

struct Parser {
    toks: Vec<Tok>,
    at: usize,
    /// How deep the parser is in itself: every recursion passes `compound_expr` or
    /// `unary_operand`.
    nest: u32,
}

type PResult<T> = std::result::Result<T, String>;

/// A parsed expression with its height (0 for a leaf), so that chains the parser builds
/// in a loop (`a + b + c`, `a.b.c`) are bounded as well as nesting.
type Node = (Expr, u32);

/// The height of a new node over children as high as `child`.
fn above(child: u32) -> PResult<u32> {
    if child >= MAX_DEPTH {
        return Err("Expression nests too deep".into());
    }
    Ok(child + 1)
}

impl Parser {
    fn cur(&self) -> &Tok {
        self.toks.get(self.at).unwrap_or(&Tok::Eof)
    }
    fn next(&mut self) -> &Tok {
        if self.at < self.toks.len() {
            self.at += 1;
        }
        self.cur()
    }

    /// `f` a level deeper, unless the text nests too deep already.
    fn nested(&mut self, f: impl FnOnce(&mut Self) -> PResult<Node>) -> PResult<Node> {
        if self.nest >= MAX_NEST {
            return Err("Expression nests too deep".into());
        }
        self.nest += 1;
        let r = f(self);
        self.nest -= 1;
        r
    }

    fn single(&mut self) -> PResult<Node> {
        let t = self.cur().clone();
        match t {
            Tok::Float(v) => {
                self.next();
                Ok((Expr::Float(v.parse::<f32>().unwrap_or(0.0)), 0))
            }
            Tok::Str(s) => {
                self.next();
                Ok((Expr::Str(s), 0))
            }
            Tok::True => {
                self.next();
                Ok((Expr::Float(1.0), 0))
            }
            Tok::False => {
                self.next();
                Ok((Expr::Float(0.0), 0))
            }
            Tok::LParen => {
                self.next();
                let e = self.compound_expr(0)?;
                if *self.cur() != Tok::RParen {
                    return Err("Non closed expression".into());
                }
                self.next();
                Ok(e)
            }
            Tok::LBrace => {
                self.next();
                let mut list = Vec::new();
                let mut h = 0;
                loop {
                    while *self.cur() == Tok::Semicolon {
                        self.next();
                    }
                    if *self.cur() == Tok::RBrace {
                        self.next();
                        break;
                    }
                    let (e, eh) = self.compound_expr(0)?;
                    list.push(e);
                    h = h.max(eh);
                    match self.cur().clone() {
                        Tok::RBrace => {
                            self.next();
                            break;
                        }
                        Tok::Eof => return Err("Found the end before the execution scope closing token".into()),
                        Tok::Error(e) => return Err(format!("Found an invalid token (error): {e}")),
                        Tok::Semicolon => {
                            self.next();
                        }
                        _ => return Err("Missing semicolon".into()),
                    }
                }
                Ok((Expr::Scope(list), above(h)?))
            }
            Tok::Break => {
                self.next();
                Ok((Expr::Statement(StatementOp::Break), 0))
            }
            Tok::Continue => {
                self.next();
                Ok((Expr::Statement(StatementOp::Continue), 0))
            }
            Tok::Ident(name) => {
                let (mut e, mut h) = (Expr::Ident(name), 0);
                let mut t = self.next().clone();
                while t == Tok::Dot {
                    let Tok::Ident(field) = self.next().clone() else {
                        return Err("Unexpected token, expected a valid field token".into());
                    };
                    h = above(h)?;
                    e = Expr::Access(Box::new(e), field);
                    t = self.next().clone();
                }
                Ok((e, h))
            }
            Tok::Sub => {
                self.next();
                let (x, h) = self.unary_operand()?;
                Ok(match x {
                    Expr::Float(v) => (Expr::Float(-v), 0),
                    x => (Expr::Unary(UnaryOp::ArithmeticalNegation, Box::new(x)), above(h)?),
                })
            }
            Tok::Bang => {
                self.next();
                let (x, h) = self.unary_operand()?;
                Ok((Expr::Unary(UnaryOp::LogicalNegation, Box::new(x)), above(h)?))
            }
            Tok::Return => {
                self.next();
                let (x, h) = self.compound_expr(0)?;
                Ok((Expr::Unary(UnaryOp::Return, Box::new(x)), above(h)?))
            }
            Tok::Error(e) => Err(format!("Found error token: {e}")),
            t => Err(format!("Expected an expression, but found {t:?}")),
        }
    }

    fn unary_operand(&mut self) -> PResult<Node> {
        self.nested(|p| {
            let mut e = p.single()?;
            while matches!(p.cur(), Tok::LParen | Tok::LBracket) {
                e = p.compound(e, 0)?;
            }
            Ok(e)
        })
    }

    fn compound_expr(&mut self, last: i32) -> PResult<Node> {
        self.nested(|p| {
            let mut e = p.single()?;
            loop {
                let before = p.at;
                let c = p.compound(e, last)?;
                if matches!(p.cur(), Tok::Eof | Tok::Semicolon) {
                    return Ok(c);
                }
                if p.at == before {
                    return Ok(c);
                }
                e = c;
            }
        })
    }

    fn compound(&mut self, (left, lh): Node, last: i32) -> PResult<Node> {
        match self.cur().clone() {
            Tok::RParen | Tok::Eof => return Ok((left, lh)),
            Tok::LBracket => {
                let t = self.next().clone();
                if t == Tok::RBracket {
                    return Err("Expected a expression, got RBRACKET".into());
                } else if t == Tok::Eof {
                    return Err("Found EOF before closing RBRACKET".into());
                }
                let (index, ih) = self.compound_expr(0)?;
                if *self.cur() != Tok::RBracket {
                    return Err("Expected a closing RBRACKET".into());
                }
                self.next();
                return Ok((Expr::Index(Box::new(left), Box::new(index)), above(lh.max(ih))?));
            }
            Tok::LParen => {
                let t = self.next().clone();
                let mut args = Vec::new();
                let mut h = lh;
                if t == Tok::Eof {
                    return Err("Found EOF before closing RPAREN".into());
                } else if t == Tok::RParen {
                    self.next();
                } else {
                    loop {
                        let (a, ah) = self.compound_expr(0)?;
                        args.push(a);
                        h = h.max(ah);
                        match self.cur().clone() {
                            Tok::Eof => return Err("Found EOF before closing RPAREN".into()),
                            Tok::RParen => {
                                self.next();
                                break;
                            }
                            Tok::Comma => {
                                self.next();
                            }
                            Tok::Error(e) => return Err(format!("Found error token: {e}")),
                            t => return Err(format!("Expected a comma, got {t:?}")),
                        }
                    }
                }
                return Ok((Expr::Call(Box::new(left), args), above(h)?));
            }
            Tok::Ques => {
                let p = BinOp::Conditional.precedence();
                if last >= p {
                    return Ok((left, lh));
                }
                self.next();
                let (t, th) = self.compound_expr(0)?;
                if *self.cur() == Tok::Colon {
                    self.next();
                    let (f, fh) = self.compound_expr(p - 1)?;
                    let h = above(lh.max(th).max(fh))?;
                    return Ok((Expr::Ternary(Box::new(left), Box::new(t), Box::new(f)), h));
                }
                return Ok((Expr::Binary(BinOp::Conditional, Box::new(left), Box::new(t)), above(lh.max(th))?));
            }
            _ => {}
        }
        let op = match self.cur() {
            Tok::AmpAmp => BinOp::And,
            Tok::BarBar => BinOp::Or,
            Tok::Lt => BinOp::Lt,
            Tok::Lte => BinOp::Lte,
            Tok::Gt => BinOp::Gt,
            Tok::Gte => BinOp::Gte,
            Tok::Plus => BinOp::Add,
            Tok::Sub => BinOp::Sub,
            Tok::Star => BinOp::Mul,
            Tok::Slash => BinOp::Div,
            Tok::QuesQues => BinOp::NullCoalesce,
            Tok::Eq => BinOp::Assign,
            Tok::EqEq => BinOp::Eq,
            Tok::BangEq => BinOp::Neq,
            Tok::Arrow => BinOp::Arrow,
            _ => return Ok((left, lh)),
        };
        let p = op.precedence();
        if last >= p {
            return Ok((left, lh));
        }
        self.next();
        let rp = if op == BinOp::Assign { p - 1 } else { p };
        let (right, rh) = self.compound_expr(rp)?;
        Ok((Expr::Binary(op, Box::new(left), Box::new(right)), above(lh.max(rh))?))
    }
}

/// `MolangParser.parseAll`: every expression of the text, `;` apart.
pub fn parse(text: &str) -> std::result::Result<Exprs, String> {
    let mut p = Parser { toks: lex(text), at: 0, nest: 0 };
    let mut out = Vec::new();
    loop {
        while *p.cur() == Tok::Semicolon {
            p.next();
        }
        match p.cur() {
            Tok::Eof => return Ok(out),
            Tok::Error(e) => return Err(format!("Found an invalid token (error): {e}")),
            _ => {}
        }
        out.push(p.compound_expr(0)?.0);
        if !matches!(p.cur(), Tok::Eof | Tok::Semicolon) {
            return Err(format!("Expected a semicolon, but was {:?}", p.cur()));
        }
    }
}

// ---- evaluation ----------------------------------------------------------------------------

/// What an expression gives.
#[derive(Clone, Debug, PartialEq)]
enum Val {
    Num(f64),
    Str(String),
    Null,
}

impl Val {
    fn num(&self) -> f64 {
        match self {
            Val::Num(v) => *v,
            _ => 0.0,
        }
    }
    fn truthy(&self) -> bool {
        match self {
            Val::Num(v) => *v != 0.0,
            Val::Str(s) => !s.is_empty(),
            Val::Null => false,
        }
    }
}

enum Flow {
    Normal(Val),
    Return(Val),
    Break,
    Continue,
}

/// One controller's Molang state (`MolangLoader.createNewEngine(controller)`): its variables,
/// the animation time it reads (`query.anim_time`, seconds) and the step limit
/// (`EVALUATION_STEP_LIMIT`).
#[derive(Default, Clone)]
pub struct Runtime {
    pub anim_time: f64,
    pub controller_speed: f64,
    variables: HashMap<String, Val>,
    temps: HashMap<String, Val>,
    steps: u64,
    seed: u64,
}

const STEP_LIMIT: u64 = 100_000;

impl Runtime {
    pub fn new() -> Runtime {
        Runtime { controller_speed: 1.0, seed: 0x9E37_79B9_7F4A_7C15, ..Runtime::default() }
    }

    /// `MolangInterpreter.eval(list)`: each expression in turn, the last one's value (or a
    /// `return`'s), as a float.
    pub fn eval(&mut self, list: &[Expr]) -> f32 {
        // Constants need no state (the common case: every keyframe value of a JSON emote).
        if let [Expr::Float(v)] = list {
            return *v;
        }
        self.steps = 0;
        self.temps.clear();
        let mut last = Val::Num(0.0);
        for e in list {
            match self.ev(e) {
                Flow::Normal(v) => last = v,
                Flow::Return(v) => return v.num() as f32,
                Flow::Break | Flow::Continue => {}
            }
        }
        last.num() as f32
    }

    fn val(&mut self, e: &Expr) -> Val {
        match self.ev(e) {
            Flow::Normal(v) | Flow::Return(v) => v,
            _ => Val::Num(0.0),
        }
    }

    fn random(&mut self) -> f64 {
        // xorshift: `math.random` only needs some randomness.
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        (self.seed >> 11) as f64 / (1u64 << 53) as f64
    }

    /// `v.x`, `variable.x`, `t.x`, `temp.x`, `q.anim_time`...: the name an access chain stands for.
    fn path(e: &Expr) -> Option<(String, String)> {
        match e {
            Expr::Access(o, p) => match &**o {
                Expr::Ident(root) => Some((root.to_ascii_lowercase(), p.to_ascii_lowercase())),
                _ => None,
            },
            _ => None,
        }
    }

    fn ev(&mut self, e: &Expr) -> Flow {
        self.steps += 1;
        if self.steps > STEP_LIMIT {
            return Flow::Return(Val::Num(0.0));
        }
        let v = match e {
            Expr::Float(v) => Val::Num(f64::from(*v)),
            Expr::Str(s) => Val::Str(s.clone()),
            Expr::Statement(StatementOp::Break) => return Flow::Break,
            Expr::Statement(StatementOp::Continue) => return Flow::Continue,
            Expr::Ident(_) => Val::Num(0.0),
            Expr::Access(..) => match Self::path(e) {
                Some((root, name)) => match root.as_str() {
                    "query" | "q" => Val::Num(match name.as_str() {
                        "anim_time" => self.anim_time,
                        "controller_speed" => self.controller_speed,
                        _ => 0.0,
                    }),
                    "variable" | "v" => self.variables.get(&name).cloned().unwrap_or(Val::Num(0.0)),
                    "temp" | "t" => self.temps.get(&name).cloned().unwrap_or(Val::Num(0.0)),
                    "math" => Val::Num(match name.as_str() {
                        "pi" => std::f64::consts::PI,
                        _ => 0.0,
                    }),
                    _ => Val::Num(0.0),
                },
                None => Val::Num(0.0),
            },
            Expr::Index(..) => Val::Num(0.0),
            Expr::Unary(op, x) => match op {
                UnaryOp::Return => return Flow::Return(self.val(x)),
                UnaryOp::LogicalNegation => Val::Num(if self.val(x).truthy() { 0.0 } else { 1.0 }),
                UnaryOp::ArithmeticalNegation => Val::Num(-self.val(x).num()),
            },
            Expr::Ternary(c, t, f) => {
                if self.val(c).truthy() {
                    return self.ev(t);
                }
                return self.ev(f);
            }
            Expr::Scope(list) => {
                for x in list {
                    match self.ev(x) {
                        Flow::Normal(_) => {}
                        other => return other,
                    }
                }
                Val::Num(0.0)
            }
            Expr::Binary(op, l, r) => self.binary(*op, l, r),
            Expr::Call(f, args) => self.call(f, args),
        };
        Flow::Normal(v)
    }

    fn binary(&mut self, op: BinOp, l: &Expr, r: &Expr) -> Val {
        let num = |b: bool| Val::Num(if b { 1.0 } else { 0.0 });
        match op {
            BinOp::And => {
                let a = self.val(l).truthy();
                num(a && self.val(r).truthy())
            }
            BinOp::Or => {
                let a = self.val(l).truthy();
                num(a || self.val(r).truthy())
            }
            BinOp::Conditional => {
                if self.val(l).truthy() {
                    self.val(r)
                } else {
                    Val::Null
                }
            }
            BinOp::NullCoalesce => {
                let a = self.val(l);
                if a == Val::Null { self.val(r) } else { a }
            }
            BinOp::Assign => {
                let v = self.val(r);
                if let Some((root, name)) = Self::path(l) {
                    match root.as_str() {
                        "variable" | "v" => {
                            self.variables.insert(name, v.clone());
                        }
                        "temp" | "t" => {
                            self.temps.insert(name, v.clone());
                        }
                        _ => {}
                    }
                }
                v
            }
            BinOp::Arrow => Val::Num(0.0),
            BinOp::Eq | BinOp::Neq => {
                let (a, b) = (self.val(l), self.val(r));
                let eq = match (&a, &b) {
                    (Val::Num(x), Val::Num(y)) => x == y,
                    _ => a == b,
                };
                num(if op == BinOp::Eq { eq } else { !eq })
            }
            _ => {
                let (a, b) = (self.val(l).num(), self.val(r).num());
                match op {
                    BinOp::Lt => num(a < b),
                    BinOp::Lte => num(a <= b),
                    BinOp::Gt => num(a > b),
                    BinOp::Gte => num(a >= b),
                    BinOp::Add => Val::Num(a + b),
                    BinOp::Sub => Val::Num(a - b),
                    BinOp::Mul => Val::Num(a * b),
                    // Molang: dividing by zero gives zero.
                    BinOp::Div => Val::Num(if b == 0.0 { 0.0 } else { a / b }),
                    _ => Val::Num(0.0),
                }
            }
        }
    }

    fn call(&mut self, f: &Expr, args: &[Expr]) -> Val {
        let Some((root, name)) = Self::path(f) else { return Val::Num(0.0) };
        if root != "math" {
            return Val::Num(0.0);
        }
        let a: Vec<f64> = args.iter().map(|x| self.val(x).num()).collect();
        let arg = |i: usize| a.get(i).copied().unwrap_or(0.0);
        // PAL's `MochaMathExtensions`: math.ease_* (start, end, progress).
        if let Some(easing) = name.strip_prefix("ease_")
            && let Some(t) = crate::easing::Easing::from_name(&format!("ease{}", easing.replace('_', "")))
        {
            return Val::Num(f64::from(t.apply_simple(arg(0) as f32, arg(1) as f32, arg(2) as f32)));
        }
        let deg = |v: f64| v.to_radians();
        Val::Num(match name.as_str() {
            "abs" => arg(0).abs(),
            "acos" => arg(0).acos().to_degrees(),
            "asin" => arg(0).asin().to_degrees(),
            "atan" => arg(0).atan().to_degrees(),
            "atan2" => arg(0).atan2(arg(1)).to_degrees(),
            "ceil" => arg(0).ceil(),
            "clamp" => arg(0).max(arg(1)).min(arg(2)),
            "cos" => deg(arg(0)).cos(),
            "sin" => deg(arg(0)).sin(),
            "exp" => arg(0).exp(),
            "floor" => arg(0).floor(),
            "hermite_blend" => {
                let t = arg(0);
                3.0 * t * t - 2.0 * t * t * t
            }
            "lerp" => arg(0) + (arg(1) - arg(0)) * arg(2),
            "lerprotate" => {
                let wrap = |v: f64| {
                    let mut v = v % 360.0;
                    if v >= 180.0 {
                        v -= 360.0;
                    }
                    if v < -180.0 {
                        v += 360.0;
                    }
                    v
                };
                let (s, e) = (wrap(arg(0)), wrap(arg(1)));
                s + wrap(e - s) * arg(2)
            }
            "ln" => arg(0).ln(),
            "max" => arg(0).max(arg(1)),
            "min" => arg(0).min(arg(1)),
            "min_angle" => {
                let mut v = arg(0) % 360.0;
                if v >= 180.0 {
                    v -= 360.0;
                }
                if v < -180.0 {
                    v += 360.0;
                }
                v
            }
            "mod" => {
                if arg(1) == 0.0 {
                    0.0
                } else {
                    arg(0) % arg(1)
                }
            }
            "pi" => std::f64::consts::PI,
            "pow" => arg(0).powf(arg(1)),
            "random" => {
                let (lo, hi) = (arg(0), arg(1));
                lo + (hi - lo) * self.random()
            }
            "random_integer" => {
                let (lo, hi) = (arg(0).round(), arg(1).round());
                (lo + ((hi - lo + 1.0) * self.random()).floor()).min(hi)
            }
            "die_roll" => {
                let n = arg(0).clamp(0.0, 1000.0) as u32;
                (0..n).map(|_| arg(1) + (arg(2) - arg(1)) * self.random()).sum()
            }
            "die_roll_integer" => {
                let n = arg(0).clamp(0.0, 1000.0) as u32;
                let (lo, hi) = (arg(1).round(), arg(2).round());
                (0..n).map(|_| (lo + ((hi - lo + 1.0) * self.random()).floor()).min(hi)).sum()
            }
            "round" => arg(0).round(),
            "sqrt" => arg(0).sqrt(),
            "trunc" => arg(0).trunc(),
            _ => 0.0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_folds_negative_numbers() {
        assert_eq!(parse("-45").unwrap(), vec![Expr::Float(-45.0)]);
        assert_eq!(parse("22.5").unwrap(), vec![Expr::Float(22.5)]);
        let e = parse("math.sin(q.anim_time * 360) * 10").unwrap();
        assert_eq!(e.len(), 1);
        assert!(!is_constant(&e[0]));
        assert!(is_constant(&parse("1 + 2 * 3").unwrap()[0]));
    }

    #[test]
    fn evaluates() {
        let mut rt = Runtime::new();
        assert_eq!(rt.eval(&parse("1 + 2 * 3").unwrap()), 7.0);
        assert_eq!(rt.eval(&parse("(1 + 2) * 3").unwrap()), 9.0);
        rt.anim_time = 0.5;
        assert!((rt.eval(&parse("math.sin(q.anim_time * 360)").unwrap())).abs() < 1e-6);
        assert_eq!(rt.eval(&parse("v.a = 3; v.a * 2").unwrap()), 6.0);
        assert_eq!(rt.eval(&parse("1 > 2 ? 5 : 6").unwrap()), 6.0);
        assert_eq!(rt.eval(&parse("math.clamp(5, 0, 2)").unwrap()), 2.0);
    }

    #[test]
    fn byte_form_round_trips() {
        let e = parse("v.x = math.cos(q.anim_time * 90) ?? 1; return v.x > 0 ? -v.x : 'a';").unwrap();
        let mut w = Writer::new();
        write_exprs(&e, &mut w);
        let back = read_exprs(&mut Reader::new(&w.data)).unwrap();
        assert_eq!(back, e);
    }

    #[test]
    fn rejects_text_nesting_too_deep() {
        let n = 100_000;
        // the parser's own recursion: brackets, signs, right-hand operands
        assert!(parse(&format!("{}1{}", "(".repeat(n), ")".repeat(n))).is_err());
        assert!(parse(&format!("{}1", "-!".repeat(n))).is_err());
        assert!(parse(&vec!["v.a"; n].join(" = ")).is_err());
        assert!(parse(&format!("{}1", "1 ? 2 : ".repeat(n))).is_err());
        assert!(parse(&format!("{}1", "return ".repeat(n))).is_err());
        assert!(parse(&format!("{}1{}", "f(".repeat(n), ")".repeat(n))).is_err());
        // trees the parser builds in a loop
        assert!(parse(&vec!["1"; n].join(" + ")).is_err());
        assert!(parse(&format!("a{}", ".b".repeat(n))).is_err());
        let nested = |n: usize| format!("{}1{}", "(".repeat(n), ")".repeat(n));
        assert_eq!(parse(&nested(MAX_NEST as usize - 1)).unwrap(), vec![Expr::Float(1.0)]);
        assert!(parse(&nested(MAX_NEST as usize)).is_err());
        // a tree as tall as the byte form reads is accepted, a taller one is not
        let sum = parse(&vec!["1"; MAX_DEPTH as usize + 1].join(" + ")).unwrap();
        let mut w = Writer::new();
        write_exprs(&sum, &mut w);
        assert_eq!(read_exprs(&mut Reader::new(&w.data)).unwrap(), sum);
        assert!(parse(&vec!["1"; MAX_DEPTH as usize + 2].join(" + ")).is_err());
    }
}
