//! The type grammar. Every type is erased by whoever asked for it, so
//! this only has to find where a type ends.

use super::Stop;
use super::lexer::Kind;
use super::parser::{Parsed, Parser, Reading, is_name};

impl Parser<'_> {
    /// Reads one type.
    pub(super) fn ty(&mut self) -> Parsed {
        self.nested(Self::ty_unnested)
    }

    fn ty_unnested(&mut self) -> Parsed {
        // Function and constructor types.
        if self.at("abstract") && self.peek_is("new")? {
            self.advance()?;
        }
        if self.at("new") || self.at("<") {
            self.eat("new")?;
            return self.function_type();
        }
        if self.at("(") && self.attempt(Reading::FunctionType, Self::function_type)? {
            return Ok(());
        }
        self.union()?;
        // `A extends B ? C : D`
        if self.at("extends") && !self.token.newline_before {
            self.advance()?;
            self.union()?;
            self.expect("?")?;
            self.ty()?;
            self.expect(":")?;
            self.ty()?;
        }
        Ok(())
    }

    /// Reads `<T>(a: A) => R` from its type parameters or `(`.
    fn function_type(&mut self) -> Parsed {
        if self.at("<") {
            self.type_parameters()?;
        }
        self.parameters()?;
        self.expect("=>")?;
        self.ty()
    }

    /// Reads types separated by commas.
    pub(super) fn type_list(&mut self) -> Parsed {
        loop {
            self.ty()?;
            if !self.eat(",")? {
                return Ok(());
            }
        }
    }

    fn union(&mut self) -> Parsed {
        if self.at("|") || self.at("&") {
            self.advance()?;
        }
        loop {
            self.type_operand()?;
            if !(self.eat("|")? || self.eat("&")?) {
                return Ok(());
            }
        }
    }

    /// Reads a type with its operators: `keyof T`, `readonly T[]`,
    /// `T[K]`.
    fn type_operand(&mut self) -> Parsed {
        while matches!(
            self.text(),
            "keyof" | "unique" | "readonly" | "infer" | "asserts"
        ) && self.at_word()
        {
            // Followed by anything else, the word is a type's name.
            let next = self.peek()?;
            let next_text = &self.source[next.start..next.end];
            let before_type = !next.newline_before
                && (is_name(next)
                    || matches!(next.kind, Kind::Template { .. })
                    || next.kind == Kind::Punctuator && matches!(next_text, "(" | "[" | "{"))
                && next_text != "is"
                && next_text != "extends";
            if !before_type {
                break;
            }
            self.advance()?;
        }
        self.type_primary()?;
        // A `[` on a new line starts something else, such as a class
        // member with a computed name.
        while self.at("[") && !self.token.newline_before {
            self.advance()?;
            if !self.at("]") {
                self.ty()?;
            }
            self.expect("]")?;
        }
        Ok(())
    }

    fn type_primary(&mut self) -> Parsed {
        match self.token.kind {
            Kind::Number | Kind::String => self.advance(),
            Kind::Template { .. } => self.template(Self::ty),
            Kind::End => Err(Stop::Unparsed),
            Kind::Punctuator => match self.text() {
                "-" => {
                    self.advance()?;
                    if self.token.kind != Kind::Number {
                        return Err(Stop::Unparsed);
                    }
                    self.advance()
                }
                "(" => {
                    self.advance()?;
                    self.ty()?;
                    self.expect(")")
                }
                "[" => self.tuple_type(),
                "{" => self.object_type(),
                _ => Err(Stop::Unparsed),
            },
            Kind::Word => {
                let word = self.text();
                self.advance()?;
                if word == "typeof" {
                    self.expect_word()?;
                }
                // `import("module").Name`
                if self.at("(") && self.source[..self.previous_end].ends_with("import") {
                    self.advance()?;
                    self.type_list()?;
                    self.expect(")")?;
                }
                // The type predicate `x is T`.
                if self.at("is") && !self.token.newline_before {
                    self.advance()?;
                    return self.ty();
                }
                while self.eat(".")? {
                    self.expect_word()?;
                }
                if self.at("<") {
                    self.attempt(Reading::GenericType, |parser| {
                        parser.advance()?;
                        parser.type_list()?;
                        parser.expect_closing_angle()
                    })?;
                }
                Ok(())
            }
        }
    }

    /// Reads `[A, B?, ...C[]]`, with or without `label:`s.
    fn tuple_type(&mut self) -> Parsed {
        self.expect("[")?;
        while !self.at("]") {
            self.eat("...")?;
            // A label reads as a type's name.
            self.ty()?;
            self.eat("?")?;
            if self.eat(":")? {
                self.ty()?;
            }
            if !self.eat(",")? {
                break;
            }
        }
        self.expect("]")
    }

    /// Reads an object or mapped type, `{ … }`.
    pub(super) fn object_type(&mut self) -> Parsed {
        self.expect("{")?;
        while !self.at("}") {
            self.type_member()?;
            let separated = self.eat(";")? || self.eat(",")?;
            if !separated && !self.at("}") && !self.token.newline_before {
                return Err(Stop::Unparsed);
            }
        }
        self.advance()
    }

    fn type_member(&mut self) -> Parsed {
        // A mapped type's `+readonly` or `-readonly`.
        if self.at("+") || self.at("-") {
            self.advance()?;
        }
        while self.at("readonly") || self.at("get") || self.at("set") {
            let next = self.peek()?;
            let before_name = is_name(next) || &self.source[next.start..next.end] == "[";
            if !before_name {
                break;
            }
            self.advance()?;
        }
        // A call signature has no name. A construct signature reads as a
        // method named `new`.
        if !(self.at("(") || self.at("<")) {
            if self.at("[") {
                self.bracketed_member_name()?;
            } else if is_name(self.token) {
                self.advance()?;
            } else {
                return Err(Stop::Unparsed);
            }
            // `?`, or a mapped type's `+?` and `-?`.
            if self.at("+") || self.at("-") {
                self.advance()?;
            }
            self.eat("?")?;
        }
        if self.at("(") || self.at("<") {
            if self.at("<") {
                self.type_parameters()?;
            }
            self.parameters()?;
        }
        if self.eat(":")? {
            self.ty()?;
        }
        Ok(())
    }

    /// Reads the brackets of an index signature (`[key: string]`), a
    /// mapped type (`[K in keyof T as U]`) or a computed name.
    fn bracketed_member_name(&mut self) -> Parsed {
        self.expect("[")?;
        if self.at_word() && self.peek_is(":")? {
            self.advance()?;
            self.advance()?;
            self.ty()?;
        } else if self.at_word() && self.peek_is("in")? {
            self.advance()?;
            self.advance()?;
            self.ty()?;
            if self.eat("as")? {
                self.ty()?;
            }
        } else {
            self.assignment()?;
        }
        self.expect("]")
    }
}
