//! Walks a script's statements and expressions, recording what to erase.
//!
//! No tree is built and operator precedence is ignored: the walk only has
//! to know, at each token, whether it is looking at a type. It is strict
//! about TypeScript and loose about JavaScript, so a script QuickJS would
//! reject can pass through here, never the other way round.
//!
//! Where TypeScript is ambiguous (`(a) => b` or `(a)`, `f<T>(x)` or
//! `f < T > (x)`), the walk tries the TypeScript reading and rewinds when
//! it doesn't hold, as `tsc` does.

use std::collections::HashSet;
use std::mem;
use std::ptr;

use super::lexer::{Kind, Lexer, Token, is_line_break};
use super::{Edits, Refusal, Stop};

pub(super) type Parsed<T = ()> = Result<T, Stop>;

/// How much stack the walk may use, in bytes. Source nested deeper than
/// this allows is left for QuickJS, whose own limit then reports it, so a
/// script can't overflow the worker's stack here.
const STACK_BUDGET: usize = 256 * 1024;

/// The readings [`Parser::attempt`] tries. A failure is remembered by kind
/// and position, so nested ambiguities are each tried once and the walk
/// stays near linear.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum Reading {
    Arrow,
    AsyncArrow,
    GenericArrow,
    TypeArguments,
    GenericType,
    IndexSignature,
    FunctionType,
}

pub(super) struct Parser<'a> {
    pub(super) source: &'a str,
    lexer: Lexer<'a>,
    /// The token being looked at.
    pub(super) token: Token,
    /// Where the token before it ended.
    pub(super) previous_end: usize,
    edits: Edits,
    /// The address of a local in [`Parser::program`], which stack use is
    /// measured from.
    stack_base: usize,
    /// Set once the stack budget is spent, so no other reading is tried.
    gave_up: bool,
    failed: HashSet<(usize, Reading)>,
    /// Whether the next operand opens the middle of `a ? b : c`.
    in_consequent: bool,
    /// Whether the operand being read is a class's `extends` clause.
    in_heritage: bool,
}

struct Snapshot {
    position: usize,
    token: Token,
    previous_end: usize,
    erased: usize,
    placed: usize,
    in_consequent: bool,
    in_heritage: bool,
}

impl<'a> Parser<'a> {
    pub fn new(source: &'a str) -> Self {
        Self {
            source,
            lexer: Lexer::new(source),
            token: Token {
                kind: Kind::End,
                start: 0,
                end: 0,
                newline_before: true,
            },
            previous_end: 0,
            edits: Edits::default(),
            stack_base: 0,
            gave_up: false,
            failed: HashSet::new(),
            in_consequent: false,
            in_heritage: false,
        }
    }

    pub fn into_edits(self) -> Edits {
        self.edits
    }

    pub fn program(&mut self) -> Parsed {
        let base = 0u8;
        self.stack_base = ptr::from_ref(&base).addr();
        self.token = self.lexer.next()?;
        while self.token.kind != Kind::End {
            self.statement()?;
        }
        Ok(())
    }

    // Tokens

    pub(super) fn text(&self) -> &'a str {
        &self.source[self.token.start..self.token.end]
    }

    fn text_of(&self, token: Token) -> &'a str {
        &self.source[token.start..token.end]
    }

    /// Whether the token is this punctuator or word.
    pub(super) fn at(&self, text: &str) -> bool {
        matches!(self.token.kind, Kind::Punctuator | Kind::Word) && self.text() == text
    }

    pub(super) fn at_word(&self) -> bool {
        self.token.kind == Kind::Word
    }

    /// Whether the token is a word, string or number: a property name.
    fn at_name(&self) -> bool {
        is_name(self.token)
    }

    pub(super) fn advance(&mut self) -> Parsed {
        self.previous_end = self.token.end;
        self.token = self.lexer.next()?;
        Ok(())
    }

    pub(super) fn eat(&mut self, text: &str) -> Parsed<bool> {
        let found = self.at(text);
        if found {
            self.advance()?;
        }
        Ok(found)
    }

    pub(super) fn expect(&mut self, text: &str) -> Parsed {
        if self.eat(text)? {
            Ok(())
        } else {
            Err(Stop::Unparsed)
        }
    }

    pub(super) fn expect_word(&mut self) -> Parsed {
        if !self.at_word() {
            return Err(Stop::Unparsed);
        }
        self.advance()
    }

    /// The token after this one.
    pub(super) fn peek(&mut self) -> Parsed<Token> {
        let position = self.lexer.position;
        let token = self.lexer.next();
        self.lexer.position = position;
        Ok(token?)
    }

    pub(super) fn peek_is(&mut self, text: &str) -> Parsed<bool> {
        let token = self.peek()?;
        Ok(matches!(token.kind, Kind::Punctuator | Kind::Word) && self.text_of(token) == text)
    }

    /// Whether the next token is a word on this line that could be a
    /// declaration's name, which is what makes `type`, `interface` and
    /// their like keywords rather than identifiers.
    fn peek_is_name_on_line(&mut self) -> Parsed<bool> {
        let token = self.peek()?;
        Ok(token.kind == Kind::Word
            && !token.newline_before
            && !is_binary_word(self.text_of(token)))
    }

    /// Consumes a `>`, taking only the first character of `>>` or `>=`.
    pub(super) fn expect_closing_angle(&mut self) -> Parsed {
        if self.token.kind != Kind::Punctuator || !self.text().starts_with('>') {
            return Err(Stop::Unparsed);
        }
        if self.token.end - self.token.start > 1 {
            self.token = self.lexer.first_character(self.token);
        }
        self.advance()
    }

    pub(super) fn erase(&mut self, span: std::ops::Range<usize>) {
        self.edits.erase(span);
    }

    /// Erases from `start` to the end of the last token consumed.
    pub(super) fn erase_from(&mut self, start: usize) {
        self.edits.erase(start..self.previous_end);
    }

    /// Erases the token and moves past it.
    fn erase_token(&mut self) -> Parsed {
        let start = self.token.start;
        self.advance()?;
        self.erase_from(start);
        Ok(())
    }

    fn erase_statement_from(&mut self, start: usize) {
        self.edits.erase_statement(start..self.previous_end);
    }

    // Control

    /// Runs `parse`, a step that can recurse, unless the stack budget is
    /// spent.
    pub(super) fn nested<T>(&mut self, parse: impl FnOnce(&mut Self) -> Parsed<T>) -> Parsed<T> {
        let here = 0u8;
        if ptr::from_ref(&here).addr().abs_diff(self.stack_base) > STACK_BUDGET {
            self.gave_up = true;
            return Err(Stop::Unparsed);
        }
        parse(self)
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            position: self.lexer.position,
            token: self.token,
            previous_end: self.previous_end,
            erased: self.edits.erased.len(),
            placed: self.edits.placed.len(),
            in_consequent: self.in_consequent,
            in_heritage: self.in_heritage,
        }
    }

    fn restore(&mut self, snapshot: Snapshot) {
        self.lexer.position = snapshot.position;
        self.token = snapshot.token;
        self.previous_end = snapshot.previous_end;
        self.edits.erased.truncate(snapshot.erased);
        self.edits.placed.truncate(snapshot.placed);
        self.in_consequent = snapshot.in_consequent;
        self.in_heritage = snapshot.in_heritage;
    }

    /// Tries `parse` from the current token. When it doesn't hold, rewinds
    /// as though nothing was read and returns `false`.
    pub(super) fn attempt(
        &mut self,
        reading: Reading,
        parse: impl FnOnce(&mut Self) -> Parsed,
    ) -> Parsed<bool> {
        let key = (self.token.start, reading);
        if self.failed.contains(&key) {
            return Ok(false);
        }
        let snapshot = self.snapshot();
        if parse(self).is_ok() {
            return Ok(true);
        }
        self.restore(snapshot);
        if self.gave_up {
            return Err(Stop::Unparsed);
        }
        self.failed.insert(key);
        Ok(false)
    }

    // Statements

    /// Reads one statement, returning whether all of it was erased.
    fn statement(&mut self) -> Parsed<bool> {
        self.nested(Self::statement_unnested)
    }

    fn statement_unnested(&mut self) -> Parsed<bool> {
        let start = self.token.start;
        // Scripts aren't modules, but a declaration behind `export` is
        // still stripped, so QuickJS's complaint is about the `export`.
        if self.at("export") && self.peek()?.kind == Kind::Word {
            self.advance()?;
            self.eat("default")?;
        }
        if self.at_word() {
            match self.text() {
                "var" | "const" => {
                    if self.peek_is("enum")? {
                        return self.refuse(Refusal::Enum, start);
                    }
                    self.variables()?;
                    return Ok(false);
                }
                "let"
                    if matches!(self.peek()?.kind, Kind::Word)
                        || self.peek_is("[")?
                        || self.peek_is("{")? =>
                {
                    self.variables()?;
                    return Ok(false);
                }
                "function" => return self.function_declaration(start),
                "async" if self.peek_is("function")? && !self.peek()?.newline_before => {
                    self.advance()?;
                    return self.function_declaration(start);
                }
                "class" => {
                    self.class()?;
                    return Ok(false);
                }
                "abstract" if self.peek_is("class")? && !self.peek()?.newline_before => {
                    self.erase_token()?;
                    self.class()?;
                    return Ok(false);
                }
                "if" => {
                    self.advance()?;
                    self.condition()?;
                    self.statement()?;
                    if self.eat("else")? {
                        self.statement()?;
                    }
                    return Ok(false);
                }
                "for" => {
                    self.for_statement()?;
                    return Ok(false);
                }
                "while" | "with" => {
                    self.advance()?;
                    self.condition()?;
                    self.statement()?;
                    return Ok(false);
                }
                "do" => {
                    self.advance()?;
                    self.statement()?;
                    self.expect("while")?;
                    self.condition()?;
                    self.eat(";")?;
                    return Ok(false);
                }
                "switch" => {
                    self.switch()?;
                    return Ok(false);
                }
                "try" => {
                    self.advance()?;
                    self.block()?;
                    if self.eat("catch")? {
                        if self.eat("(")? {
                            self.binding()?;
                            if self.at(":") {
                                self.type_annotation()?;
                            }
                            self.expect(")")?;
                        }
                        self.block()?;
                    }
                    if self.eat("finally")? {
                        self.block()?;
                    }
                    return Ok(false);
                }
                "return" | "throw" => {
                    self.advance()?;
                    let ends = self.token.newline_before
                        || self.at(";")
                        || self.at("}")
                        || self.token.kind == Kind::End;
                    if !ends {
                        self.expression()?;
                    }
                    self.eat(";")?;
                    return Ok(false);
                }
                "break" | "continue" => {
                    self.advance()?;
                    if self.at_word() && !self.token.newline_before {
                        self.advance()?;
                    }
                    self.eat(";")?;
                    return Ok(false);
                }
                "type" if self.peek_is_name_on_line()? => {
                    self.advance()?;
                    self.advance()?;
                    if self.at("<") {
                        self.type_parameters()?;
                    }
                    self.expect("=")?;
                    self.ty()?;
                    self.eat(";")?;
                    self.erase_statement_from(start);
                    return Ok(true);
                }
                "interface" if self.peek_is_name_on_line()? => {
                    self.advance()?;
                    self.advance()?;
                    if self.at("<") {
                        self.type_parameters()?;
                    }
                    if self.eat("extends")? {
                        self.type_list()?;
                    }
                    self.object_type()?;
                    self.erase_statement_from(start);
                    return Ok(true);
                }
                "enum" if self.peek_is_name_on_line()? => {
                    return self.refuse(Refusal::Enum, start);
                }
                "namespace" | "module" if self.peek_is_namespace_name()? => {
                    self.namespace(false)?;
                    self.erase_statement_from(start);
                    return Ok(true);
                }
                "declare" if self.peek_is_name_on_line()? => {
                    self.advance()?;
                    self.ambient_declaration()?;
                    self.eat(";")?;
                    self.erase_statement_from(start);
                    return Ok(true);
                }
                // A label.
                _ if self.peek_is(":")? => {
                    self.advance()?;
                    self.advance()?;
                    return self.statement();
                }
                _ => {}
            }
        }
        if self.at("{") {
            self.block()?;
        } else if !self.eat(";")? {
            self.expression()?;
            self.eat(";")?;
        }
        Ok(false)
    }

    fn refuse<T>(&self, refusal: Refusal, at: usize) -> Parsed<T> {
        Err(Stop::Refused { refusal, at })
    }

    fn peek_is_namespace_name(&mut self) -> Parsed<bool> {
        let token = self.peek()?;
        Ok(self.peek_is_name_on_line()? || (token.kind == Kind::String && !token.newline_before))
    }

    /// Reads what follows `declare`, all of which the caller erases.
    fn ambient_declaration(&mut self) -> Parsed {
        if self.at("const") && self.peek_is("enum")? {
            self.advance()?;
        }
        match self.text() {
            "enum" => {
                self.advance()?;
                self.expect_word()?;
                self.skip_braces()
            }
            "namespace" | "module" | "global" => self.namespace(true),
            _ => self.statement().map(drop),
        }
    }

    /// Reads a namespace from its keyword. Unless it is `ambient`, one
    /// holding anything but types is refused.
    fn namespace(&mut self, ambient: bool) -> Parsed {
        self.advance()?;
        if self.token.kind == Kind::String {
            self.advance()?;
        } else if !self.at("{") {
            self.expect_word()?;
            while self.eat(".")? {
                self.expect_word()?;
            }
        }
        if !self.eat("{")? {
            self.eat(";")?;
            return Ok(());
        }
        while !self.at("}") {
            let start = self.token.start;
            let erased = self.statement()?;
            if !erased && !ambient {
                return self.refuse(Refusal::Namespace, start);
            }
        }
        self.advance()
    }

    fn skip_braces(&mut self) -> Parsed {
        self.expect("{")?;
        let mut open = 1;
        while open > 0 {
            match self.token.kind {
                Kind::End => return Err(Stop::Unparsed),
                Kind::Punctuator if self.text() == "{" => open += 1,
                Kind::Punctuator if self.text() == "}" => open -= 1,
                _ => {}
            }
            self.advance()?;
        }
        Ok(())
    }

    fn block(&mut self) -> Parsed {
        self.expect("{")?;
        while !self.at("}") {
            self.statement()?;
        }
        self.advance()
    }

    /// Reads the parenthesized head of `if`, `while` and the like.
    fn condition(&mut self) -> Parsed {
        self.expect("(")?;
        self.expression()?;
        self.expect(")")
    }

    /// Reads a `var`, `let` or `const` declaration from its keyword.
    fn variables(&mut self) -> Parsed {
        self.advance()?;
        self.declarators()?;
        self.eat(";")?;
        Ok(())
    }

    fn declarators(&mut self) -> Parsed {
        loop {
            self.binding()?;
            // `let x!: T`
            if self.at("!") {
                self.erase_token()?;
            }
            if self.at(":") {
                self.type_annotation()?;
            }
            if self.eat("=")? {
                self.assignment()?;
            }
            if !self.eat(",")? {
                return Ok(());
            }
        }
    }

    fn for_statement(&mut self) -> Parsed {
        self.advance()?;
        self.eat("await")?;
        self.expect("(")?;
        if self.at("var") || self.at("let") || self.at("const") {
            self.advance()?;
            self.declarators()?;
        } else if !self.at(";") {
            // `x in y` reads as one expression here.
            self.expression()?;
        }
        if self.at("of") || self.at("in") {
            self.advance()?;
            self.expression()?;
        }
        while self.eat(";")? {
            if !self.at(";") && !self.at(")") {
                self.expression()?;
            }
        }
        self.expect(")")?;
        self.statement().map(drop)
    }

    fn switch(&mut self) -> Parsed {
        self.advance()?;
        self.condition()?;
        self.expect("{")?;
        while !self.at("}") {
            if self.eat("case")? {
                self.expression()?;
                self.expect(":")?;
            } else if self.at("default") && self.peek_is(":")? {
                self.advance()?;
                self.advance()?;
            } else {
                self.statement()?;
            }
        }
        self.advance()
    }

    /// Erases a `: Type` annotation, from its colon.
    pub(super) fn type_annotation(&mut self) -> Parsed {
        let start = self.token.start;
        self.expect(":")?;
        self.ty()?;
        self.erase_from(start);
        Ok(())
    }

    // Functions and classes

    /// Reads a function declaration from `function`. An overload, which
    /// has no body, is erased.
    fn function_declaration(&mut self, start: usize) -> Parsed<bool> {
        if self.function()? {
            return Ok(false);
        }
        self.eat(";")?;
        self.erase_statement_from(start);
        Ok(true)
    }

    /// Reads a function from `function`, returning whether it has a body.
    fn function(&mut self) -> Parsed<bool> {
        self.advance()?;
        self.eat("*")?;
        if self.at_word() {
            self.advance()?;
        }
        self.callable()
    }

    /// Reads type parameters, parameters, a return type and a body,
    /// returning whether there was a body.
    fn callable(&mut self) -> Parsed<bool> {
        if self.at("<") {
            self.type_parameters()?;
        }
        self.parameters()?;
        if self.at(":") {
            self.type_annotation()?;
        }
        if !self.at("{") {
            return Ok(false);
        }
        self.block()?;
        Ok(true)
    }

    /// Erases `<T extends U = V, …>`.
    pub(super) fn type_parameters(&mut self) -> Parsed {
        let start = self.token.start;
        self.expect("<")?;
        while !self.at(">") {
            // `const`, `in` and `out` modifiers, then the name.
            self.expect_word()?;
            while self.at_word() && !self.at("extends") {
                self.advance()?;
            }
            if self.eat("extends")? {
                self.ty()?;
            }
            if self.eat("=")? {
                self.ty()?;
            }
            if !self.eat(",")? {
                break;
            }
        }
        self.expect_closing_angle()?;
        self.erase_from(start);
        Ok(())
    }

    /// Reads a parenthesized parameter list, erasing its types.
    pub(super) fn parameters(&mut self) -> Parsed {
        self.expect("(")?;
        while !self.at(")") {
            let start = self.token.start;
            // `this: T` names no parameter, so its comma goes too.
            if self.at("this") && self.peek_is(":")? {
                self.advance()?;
                self.advance()?;
                self.ty()?;
                self.eat(",")?;
                self.erase_from(start);
                continue;
            }
            if matches!(
                self.text(),
                "public" | "private" | "protected" | "readonly" | "override"
            ) {
                let next = self.peek()?;
                let names_parameter = next.kind == Kind::Word
                    || matches!(self.text_of(next), "{" | "[") && next.kind == Kind::Punctuator;
                if names_parameter {
                    return self.refuse(Refusal::ParameterProperty, start);
                }
            }
            self.eat("...")?;
            self.binding()?;
            let optional = self.token.start;
            self.eat("?")?;
            if self.eat(":")? {
                self.ty()?;
            }
            self.erase_from(optional);
            if self.eat("=")? {
                self.assignment()?;
            }
            if !self.eat(",")? {
                break;
            }
        }
        self.expect(")")
    }

    /// Reads a name or destructuring pattern being bound.
    fn binding(&mut self) -> Parsed {
        self.nested(|parser| {
            if parser.eat("{")? {
                while !parser.at("}") {
                    if parser.eat("...")? {
                        parser.binding()?;
                    } else {
                        parser.property_name()?;
                        if parser.eat(":")? {
                            parser.binding()?;
                        }
                        if parser.eat("=")? {
                            parser.assignment()?;
                        }
                    }
                    if !parser.eat(",")? {
                        break;
                    }
                }
                parser.expect("}")
            } else if parser.eat("[")? {
                while !parser.at("]") {
                    if parser.eat(",")? {
                        continue;
                    }
                    parser.eat("...")?;
                    parser.binding()?;
                    if parser.eat("=")? {
                        parser.assignment()?;
                    }
                    if !parser.eat(",")? {
                        break;
                    }
                }
                parser.expect("]")
            } else {
                parser.expect_word()
            }
        })
    }

    /// Reads a property's name: a word, string, number or `[computed]`.
    pub(super) fn property_name(&mut self) -> Parsed {
        if self.eat("[")? {
            self.assignment()?;
            self.expect("]")
        } else if self.at_name() {
            self.advance()
        } else {
            Err(Stop::Unparsed)
        }
    }

    /// Reads a class from `class`.
    fn class(&mut self) -> Parsed {
        self.advance()?;
        if self.at_word() && !self.at("extends") && !self.at("implements") {
            self.advance()?;
        }
        if self.at("<") {
            self.type_parameters()?;
        }
        if self.eat("extends")? {
            let outer = mem::replace(&mut self.in_heritage, true);
            let parsed = self.unary();
            self.in_heritage = outer;
            parsed?;
        }
        if self.at("implements") {
            let start = self.token.start;
            self.advance()?;
            self.type_list()?;
            self.erase_from(start);
        }
        self.expect("{")?;
        while !self.at("}") {
            self.member()?;
        }
        self.advance()
    }

    fn member(&mut self) -> Parsed {
        if self.eat(";")? {
            return Ok(());
        }
        let start = self.token.start;
        // `declare` and `abstract` members exist only for the checker.
        let mut type_only = false;
        while self.at_word() {
            let word = self.text();
            if !matches!(
                word,
                "public"
                    | "private"
                    | "protected"
                    | "readonly"
                    | "override"
                    | "abstract"
                    | "declare"
                    | "static"
                    | "async"
                    | "get"
                    | "set"
                    | "accessor"
            ) {
                break;
            }
            // Followed by one of these, the word is the member's name.
            let next = self.peek()?;
            let names_member = next.kind == Kind::End
                || next.kind == Kind::Punctuator
                    && matches!(
                        self.text_of(next),
                        "(" | "=" | ";" | ":" | "?" | "!" | "<" | "}" | ","
                    );
            if names_member {
                break;
            }
            if word == "static" && self.text_of(next) == "{" && next.kind == Kind::Punctuator {
                self.advance()?;
                return self.block();
            }
            match word {
                "static" | "async" | "get" | "set" | "accessor" => self.advance()?,
                "declare" | "abstract" => {
                    type_only = true;
                    self.erase_token()?;
                }
                _ => self.erase_token()?,
            }
        }
        self.eat("*")?;
        if self.at("[") && self.attempt(Reading::IndexSignature, Self::index_signature)? {
            self.eat(";")?;
            self.erase_statement_from(start);
            return Ok(());
        }
        self.property_name()?;
        // `name?: T` and `name!: T`
        if self.at("?") || self.at("!") {
            self.erase_token()?;
        }
        if self.at("(") || self.at("<") {
            // An overload or abstract method has no body.
            if !self.callable()? {
                self.eat(";")?;
                self.erase_statement_from(start);
            }
            return Ok(());
        }
        if self.at(":") {
            self.type_annotation()?;
        }
        if self.eat("=")? {
            self.assignment()?;
        }
        self.eat(";")?;
        if type_only {
            self.erase_statement_from(start);
        }
        Ok(())
    }

    /// Reads `[key: K]: V`.
    pub(super) fn index_signature(&mut self) -> Parsed {
        self.expect("[")?;
        self.expect_word()?;
        self.expect(":")?;
        self.ty()?;
        self.expect("]")?;
        self.expect(":")?;
        self.ty()
    }

    // Expressions

    pub(super) fn expression(&mut self) -> Parsed {
        loop {
            self.assignment()?;
            if !self.eat(",")? {
                return Ok(());
            }
        }
    }

    /// Reads an expression up to a comma: operands joined by any binary
    /// operators, whatever their precedence.
    pub(super) fn assignment(&mut self) -> Parsed {
        self.nested(|parser| {
            parser.unary()?;
            loop {
                if parser.eat("?")? {
                    let outer = mem::replace(&mut parser.in_consequent, true);
                    let parsed = parser.assignment();
                    parser.in_consequent = outer;
                    parsed?;
                    parser.expect(":")?;
                    parser.assignment()?;
                } else if parser.at_binary_operator() {
                    parser.advance()?;
                    parser.unary()?;
                } else if (parser.at("as") || parser.at("satisfies"))
                    && !parser.token.newline_before
                {
                    let start = parser.token.start;
                    parser.advance()?;
                    parser.ty()?;
                    parser.erase_from(start);
                } else {
                    return Ok(());
                }
            }
        })
    }

    fn at_binary_operator(&self) -> bool {
        match self.token.kind {
            Kind::Word => matches!(self.text(), "in" | "instanceof"),
            Kind::Punctuator => is_binary_punctuator(self.text()),
            _ => false,
        }
    }

    /// Whether the token can begin an operand.
    fn at_operand(&self) -> bool {
        match self.token.kind {
            Kind::Word => !is_binary_word(self.text()),
            Kind::Number | Kind::String | Kind::Template { .. } => true,
            Kind::Punctuator => matches!(
                self.text(),
                "(" | "[" | "{" | "!" | "~" | "+" | "-" | "++" | "--" | "/" | "/=" | "<" | "..."
            ),
            Kind::End => false,
        }
    }

    /// Reads prefix operators, an operand and what follows it.
    fn unary(&mut self) -> Parsed {
        self.nested(|parser| {
            let consequent = mem::take(&mut parser.in_consequent);
            loop {
                match (parser.token.kind, parser.text()) {
                    (Kind::Punctuator, "!" | "~" | "+" | "-" | "++" | "--" | "...")
                    | (Kind::Word, "typeof" | "void" | "delete") => parser.advance()?,
                    (Kind::Word, "await" | "yield") => {
                        parser.advance()?;
                        parser.eat("*")?;
                        // A bare `yield`.
                        if !parser.at_operand() {
                            return Ok(());
                        }
                    }
                    (Kind::Word, "new") if !parser.peek_is(".")? => parser.advance()?,
                    (Kind::Punctuator, "<") => {
                        // `<T>(x: T) => x`, or else the assertion `<T>x`.
                        let arrow = parser.attempt(Reading::GenericArrow, |parser| {
                            parser.type_parameters()?;
                            parser.arrow(consequent)
                        })?;
                        if arrow {
                            return Ok(());
                        }
                        let start = parser.token.start;
                        parser.advance()?;
                        parser.ty()?;
                        parser.expect_closing_angle()?;
                        parser.erase_from(start);
                    }
                    _ => break,
                }
            }
            parser.operand(consequent)?;
            parser.suffixes()
        })
    }

    /// Reads one operand. `consequent` says it opens the middle of a
    /// conditional, where an arrow function's return type is in doubt.
    fn operand(&mut self, consequent: bool) -> Parsed {
        let outer_heritage = mem::take(&mut self.in_heritage);
        let parsed = self.operand_unnested(consequent);
        self.in_heritage = outer_heritage;
        parsed
    }

    fn operand_unnested(&mut self, consequent: bool) -> Parsed {
        match self.token.kind {
            Kind::Number | Kind::String => self.advance(),
            Kind::Template { .. } => self.template(Self::expression),
            Kind::End => Err(Stop::Unparsed),
            Kind::Punctuator => match self.text() {
                "(" => {
                    if self.attempt(Reading::Arrow, |parser| parser.arrow(consequent))? {
                        return Ok(());
                    }
                    self.advance()?;
                    self.expression()?;
                    self.expect(")")
                }
                "[" => {
                    self.advance()?;
                    while !self.at("]") {
                        if self.eat(",")? {
                            continue;
                        }
                        self.assignment()?;
                        if !self.eat(",")? {
                            break;
                        }
                    }
                    self.expect("]")
                }
                "{" => self.object_literal(),
                "/" | "/=" => {
                    self.token = self.lexer.regular_expression(self.token)?;
                    self.advance()
                }
                _ => Err(Stop::Unparsed),
            },
            Kind::Word => match self.text() {
                "function" => self.function_expression(),
                "class" => self.class(),
                "async" => {
                    let next = self.peek()?;
                    let next_text = self.text_of(next);
                    if next.newline_before {
                        return self.advance();
                    }
                    if next_text == "function" {
                        self.advance()?;
                        return self.function_expression();
                    }
                    let is_arrow = self.attempt(Reading::AsyncArrow, |parser| {
                        parser.advance()?;
                        if parser.at_word() {
                            parser.advance()?;
                            return parser.arrow_body();
                        }
                        if parser.at("<") {
                            parser.type_parameters()?;
                        }
                        parser.arrow(consequent)
                    })?;
                    if is_arrow {
                        return Ok(());
                    }
                    // A call to something named `async`.
                    self.advance()
                }
                _ => {
                    self.advance()?;
                    if self.at("=>") {
                        self.arrow_body()?;
                    }
                    Ok(())
                }
            },
        }
    }

    fn function_expression(&mut self) -> Parsed {
        if self.function()? {
            Ok(())
        } else {
            Err(Stop::Unparsed)
        }
    }

    /// Reads an arrow function from its `(`. Fails when this is some other
    /// parenthesized thing.
    fn arrow(&mut self, consequent: bool) -> Parsed {
        self.parameters()?;
        let close = self.previous_end - 1;
        let mut return_type = None;
        if self.at(":") {
            let start = self.token.start;
            self.advance()?;
            self.ty()?;
            return_type = Some(start..self.previous_end);
        }
        if !self.at("=>") {
            return Err(Stop::Unparsed);
        }
        if let Some(return_type) = &return_type {
            if self.source[close..self.token.start].contains(is_line_break) {
                // `=>` must share a line with the `)` before it, so when
                // the return type spans lines the `)` moves to its end.
                let last = self.source[..return_type.end]
                    .char_indices()
                    .next_back()
                    .map_or(close, |(at, _)| at);
                self.erase(close..return_type.end);
                self.edits.placed.push((last, b')'));
            } else {
                self.erase(return_type.clone());
            }
        }
        self.arrow_body()?;
        // In `a ? (b) : c => d`, `(b)` is the middle operand and `c => d`
        // the last. `(b): c => d` is an arrow function only when a `:`
        // follows it.
        if return_type.is_some() && consequent && !self.at(":") {
            return Err(Stop::Unparsed);
        }
        Ok(())
    }

    fn arrow_body(&mut self) -> Parsed {
        self.expect("=>")?;
        if self.at("{") {
            self.block()
        } else {
            self.assignment()
        }
    }

    /// Reads a template literal from its first piece, reading each
    /// substitution with `substitution`.
    pub(super) fn template(&mut self, substitution: fn(&mut Self) -> Parsed) -> Parsed {
        loop {
            let Kind::Template { tail } = self.token.kind else {
                return Err(Stop::Unparsed);
            };
            if tail {
                return self.advance();
            }
            self.advance()?;
            substitution(self)?;
            if !self.at("}") {
                return Err(Stop::Unparsed);
            }
            self.token = self.lexer.resume_template(self.token)?;
        }
    }

    fn object_literal(&mut self) -> Parsed {
        self.expect("{")?;
        while !self.at("}") {
            if self.eat("...")? {
                self.assignment()?;
            } else {
                // `get`, `set` and `async` before a name are modifiers.
                while self.at("get") || self.at("set") || self.at("async") {
                    let next = self.peek()?;
                    let before_name = is_name(next)
                        || next.kind == Kind::Punctuator && matches!(self.text_of(next), "[" | "*");
                    if !before_name {
                        break;
                    }
                    self.advance()?;
                }
                self.eat("*")?;
                self.property_name()?;
                if self.at("(") || self.at("<") {
                    if !self.callable()? {
                        return Err(Stop::Unparsed);
                    }
                } else if self.eat(":")? || self.eat("=")? {
                    self.assignment()?;
                }
            }
            if !self.eat(",")? {
                break;
            }
        }
        self.expect("}")
    }

    /// Reads member accesses, calls and the other things that follow an
    /// operand.
    fn suffixes(&mut self) -> Parsed {
        loop {
            match self.token.kind {
                Kind::Template { .. } => self.template(Self::expression)?,
                Kind::Punctuator => match self.text() {
                    "." => {
                        self.advance()?;
                        self.expect_word()?;
                    }
                    "?." => {
                        self.advance()?;
                        if self.at_word() {
                            self.advance()?;
                        }
                    }
                    "(" => {
                        self.advance()?;
                        while !self.at(")") {
                            self.assignment()?;
                            if !self.eat(",")? {
                                break;
                            }
                        }
                        self.expect(")")?;
                    }
                    "[" => {
                        self.advance()?;
                        self.expression()?;
                        self.expect("]")?;
                    }
                    "++" | "--" if !self.token.newline_before => self.advance()?,
                    // The non-null assertion `x!`.
                    "!" if !self.token.newline_before => self.erase_token()?,
                    "<" => {
                        if !self.attempt(Reading::TypeArguments, Self::call_type_arguments)? {
                            return Ok(());
                        }
                    }
                    _ => return Ok(()),
                },
                _ => return Ok(()),
            }
        }
    }

    /// Erases the type arguments of `f<T>(x)`, `new C<T>` and `tag<T>```.
    /// Fails when the `<` is a comparison, by `tsc`'s rule: the closing
    /// `>` must be followed by a call, a template or something that can't
    /// start an operand.
    fn call_type_arguments(&mut self) -> Parsed {
        let start = self.token.start;
        self.advance()?;
        self.type_list()?;
        // `a < b >= c` and `a < b >> c` are comparisons.
        self.expect(">")?;
        let called = self.at("(") || matches!(self.token.kind, Kind::Template { .. });
        let extended = self.in_heritage && (self.at("{") || self.at("implements"));
        let compared = self.at("<") || self.at(">") || self.at("+") || self.at("-");
        let ends_operand =
            self.token.newline_before || self.at_binary_operator() || !self.at_operand();
        if called || extended || (!compared && ends_operand) {
            self.erase_from(start);
            Ok(())
        } else {
            Err(Stop::Unparsed)
        }
    }
}

/// Whether `token` can name a property: a word, string or number.
pub(super) fn is_name(token: Token) -> bool {
    matches!(token.kind, Kind::Word | Kind::String | Kind::Number)
}

/// Words that join two operands, so can't name a declaration or begin an
/// operand.
fn is_binary_word(word: &str) -> bool {
    matches!(word, "in" | "instanceof" | "as" | "satisfies" | "of")
}

fn is_binary_punctuator(text: &str) -> bool {
    matches!(
        text,
        "=" | "+="
            | "-="
            | "*="
            | "/="
            | "%="
            | "**="
            | "<<="
            | ">>="
            | ">>>="
            | "&="
            | "|="
            | "^="
            | "&&="
            | "||="
            | "??="
            | "=="
            | "!="
            | "==="
            | "!=="
            | "<"
            | ">"
            | "<="
            | ">="
            | "<<"
            | ">>"
            | ">>>"
            | "+"
            | "-"
            | "*"
            | "/"
            | "%"
            | "**"
            | "&"
            | "|"
            | "^"
            | "&&"
            | "||"
            | "??"
    )
}
