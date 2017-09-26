use super::{ParserResult, ParserError};
use super::super::{SymTab, TypeTab};

use std::rc::Rc;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    Block(Vec<Statement>),
    Number(f64),
    Bool(bool),
    Str(Rc<String>),
    Identifier(Rc<String>),
    Assignment(Rc<Expression>, Rc<Expression>),
    Operation {
        left:  Rc<Expression>,
        op:    Operand,
        right: Rc<Expression>,
    },
    Arm(Vec<Rc<Expression>>, Rc<Expression>),
    Call(Rc<Expression>, Rc<Vec<Expression>>),
    EOF,
}

#[allow(dead_code)]
impl Expression {
    pub fn get_type(&self, sym: &Rc<SymTab>, env: &Rc<TypeTab>) -> ParserResult<Type> {
        match *self {
            Expression::Number(_) => Ok(Type::Num),
            Expression::Str(_)    => Ok(Type::Str),
            Expression::Bool(_)   => Ok(Type::Bool),
            Expression::Identifier(ref n) => match sym.get_name(&*n) {
                Some((i, env_index)) => {
                    Ok(env.get_type(i, env_index).unwrap())
                },
                None => Err(ParserError::new(&format!("unexpected use of: {}", n))),
            },
            Expression::Assignment(ref id, _) => id.get_type(&sym, &env),
            Expression::Call(ref id, _) => match id.get_type(sym, env)? {
                Type::Any | Type::Undefined => Ok(Type::Any),
                Type::Block(ref t) => {
                    let ref t = **t;
                    Ok(t.clone())
                },
                t => Err(ParserError::new(&format!("{}: can't call {:?}", id, t))),
            },
            Expression::Operation { ref left, ref op, ref right, } => Ok(op.operate((left.get_type(sym, env)?, right.get_type(sym, env)?))?),
            Expression::Block(ref statements) => {
                let local_sym = Rc::new(SymTab::new(sym.clone(), &[]));
                let local_env = Rc::new(TypeTab::new(env.clone(), &Vec::new()));

                let mut acc = 1;
                for s in statements {
                    if acc == statements.len() {
                        match *s {
                            Statement::Expression(ref e) => return Ok(Type::Block(Rc::new(e.get_type(&local_sym, &local_env)?))),
                            _                            => return Err(ParserError::new(&format!("missing return value"))),
                        }
                    }
                    acc += 1
                }
                
                Err(ParserError::new(&format!("missing return value")))
            }
            Expression::Arm(ref params, ref body) => {
                let mut param_names = Vec::new();
                let mut param_types = Vec::new();

                for p in params {
                    match **p {
                        Expression::Identifier(ref n) => {
                            param_types.push(Type::Any);
                            param_names.push(n.clone())
                        },
                        _ => (),
                    }
                }

                let local_sym = Rc::new(SymTab::new(sym.clone(), param_names.as_slice()));
                let local_env = Rc::new(TypeTab::new(env.clone(), &param_types));

                body.get_type(&local_sym, &local_env)
            }
            _ => Ok(Type::Undefined),
        }
    }

    pub fn visit(&self, sym: &Rc<SymTab>, env: &Rc<TypeTab>) -> ParserResult<()> {
        match *self {
            Expression::Block(ref statements) => {
                for s in statements {
                    s.visit(&sym, &env)?
                }
                Ok(())
            },
            Expression::Identifier(ref id) => match sym.get_name(&*id) {
                Some(_) => Ok(()),
                None    => Err(ParserError::new(&format!("{}: undeclared", id))),
            },
            Expression::Arm(ref params, ref body) => {
                let mut param_names = Vec::new();
                let mut param_types = Vec::new();

                for p in params {
                    match **p {
                        Expression::Identifier(ref n) => {
                            param_types.push(Type::Any);
                            param_names.push(n.clone())
                        },
                        _ => (),
                    }
                }
              
                let local_sym = Rc::new(SymTab::new(sym.clone(), param_names.as_slice()));
                let local_env = Rc::new(TypeTab::new(env.clone(), &param_types));

                body.visit(&local_sym, &local_env)
            },
            Expression::Assignment(_, ref r) => { r.visit(&sym, &env) },
            Expression::Operation {ref left, ref op, ref right} => {
                left.visit(&sym, &env)?;
                right.visit(&sym, &env)
            },
            _ => Ok(())
        }
    }
    
    pub fn lua(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Expression::Block(ref statements) => {
                writeln!(f, "function(...) local __args = {{...}}")?;
                let mut acc = 1;
                for s in statements {
                    if acc == statements.len() {
                        match *s {
                            Statement::Expression(ref e) => match **e {
                                Expression::Arm(_, _) => (),
                                _ => write!(f, "return ")?,
                            },
                            _ => (),
                        }
                    }

                    acc += 1;
                    s.lua(f)?;
                    writeln!(f)?;
                }
                writeln!(f, "end")
            }
            Expression::Number(ref n)     => write!(f, "{}", n),
            Expression::Str(ref n)        => write!(f, r#""{}""#, n),
            Expression::Bool(ref n)       => write!(f, "{}", n),
            Expression::Identifier(ref n) => {
                match n.as_str() {
                    "while"  |
                    "if"     |
                    "else"   |
                    "elseif" |
                    "do"     |
                    "local"  |
                    "end"    |
                    "then" => write!(f, "_{}", n),
                    _      => write!(f, "{}", n),
                }
            },
            Expression::Assignment(ref a, ref b) => write!(f, "{} = {}", a, b),
            Expression::Call(ref id, ref args) => {
                write!(f, "{}", id)?;
                write!(f, "(")?;

                let mut acc = 1;
                for e in args.iter() {
                    write!(f, "{}", e)?;
                    if acc != args.len() {
                        write!(f, ",")?;
                    }
                    acc += 1;
                }

                write!(f, ")")
            },
            Expression::Arm(ref params, ref body) => {
                writeln!(f, "if {} == #__args then", params.len())?;
                
                let mut acc  = 0usize;

                for p in params {
                    acc += 1;
    
                    match **p {
                        ref c @ Expression::Identifier(_) => writeln!(f, "local {} = __args[{}]", c, acc)?,
                        _ => (),
                    }
                }
                
                let mut acc  = 0usize;
                let mut flag = true;

                for p in params {
                    acc += 1;
                    match **p {
                        ref c @ Expression::Number(_) |
                        ref c @ Expression::Bool(_) |
                        ref c @ Expression::Operation { .. } |
                        ref c @ Expression::Str(_) => {
                            flag = false;

                            writeln!(f, "if {} == __args[{}] then", c, acc)?;
                            match **body {
                                Expression::Block(_) => (),
                                _ => write!(f, "return ")?
                            }
                            writeln!(f, "{}", body)?;
                            writeln!(f, "end")?;
                            continue
                        }
                        
                        _ => (),
                    }
                }
                
                if flag {
                    writeln!(f, "return {}", body)?;
                }

                writeln!(f, "end")
            },
            Expression::Operation {ref left, ref op, ref right,} => {
                write!(f, "(")?;
                write!(f, "{}", left)?;
                write!(f, " {} ", op)?;
                write!(f, "{}", right)?;
                write!(f, ")")
            },
            _ => Ok(()),
        }
    }
}

impl fmt::Display for Expression {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.lua(f)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    Definition(Option<Type>, Rc<String>, Option<Rc<Expression>>),
    Expression(Rc<Expression>),
}

#[allow(dead_code)]
impl Statement {
    pub fn visit(&self, sym: &Rc<SymTab>, env: &Rc<TypeTab>) -> ParserResult<()> {
        match *self {
            Statement::Expression(ref e) => e.visit(sym, env),
            Statement::Definition(ref t, ref id, ref e) => {
                if let &Some(ref expr) = e {
                    let index = sym.add_name(&id);
                    if index >= env.size() {
                        env.grow();
                    }

                    let tp = match *t {
                        Some(ref tt) => {
                            let right_hand = &expr.get_type(sym, env)?;
                            if !tt.compare(right_hand) {
                                return Err(ParserError::new(&format!("{}: expected '{:?}', got '{:?}'", id, tt, right_hand)))
                            }
                            tt.clone()
                        },
                        None => expr.get_type(sym, env)?,
                    };

                    if let Err(e) = env.set_type(index, 0, tp) {
                        Err(ParserError::new(&format!("error setting type: {}", e)))
                    } else {
                        expr.visit(sym, env)?;
                        Ok(())
                    }
                } else {
                    Ok(())
                }
            },
        }
    }
    
    pub fn lua(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Statement::Expression(ref e) => write!(f, "{}", e),
            Statement::Definition(_, ref id, ref e) => {
                let id = match id.as_str() {
                    "while"  |
                    "if"     |
                    "else"   |
                    "elseif" |
                    "do"     |
                    "local"  |
                    "end"    |
                    "then" => format!("_{}", id),
                    _      => format!("{}", id),
                };

                match *e {
                    Some(ref e) => writeln!(f, "{} = {}", id, e),
                    None        => writeln!(f, "local {}", id),
                }
            },
        }
    }
}

impl fmt::Display for Statement {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.lua(f)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Str, Num, Bool, Any, Undefined, Block(Rc<Type>),
}

#[allow(unused)]
impl Type {
    pub fn compare(&self, other: &Type) -> bool {
        if self == &Type::Any || other == &Type::Any {
            true
        } else {
            self == other
        }
    }
}

pub fn get_type(v: &str) -> Option<Type> {
    match v {
        "str"  => Some(Type::Str),
        "num"  => Some(Type::Num),
        "bool" => Some(Type::Bool),
        "idc"  => Some(Type::Any),
        _      => None,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Operand {
    Pow,
    Mul, Div, Mod,
    Add, Sub,
    Equal, NEqual,
    Lt, Gt, LtEqual, GtEqual,
    And, Or, Not,
    Append,
}

impl Operand {
    pub fn operate(&self, lr: (Type, Type)) -> ParserResult<Type> {
        match *self {
            Operand::Pow => match lr {
                (Type::Num, Type::Num) => Ok(Type::Num),
                (Type::Any, Type::Num) => Ok(Type::Any),
                (Type::Num, Type::Any) => Ok(Type::Any),
                (Type::Str, Type::Num) => Ok(Type::Str),
                (Type::Str, Type::Any) => Ok(Type::Any),
                (Type::Any, Type::Any) => Ok(Type::Any),
                (a, b)                 => Err(ParserError::new(&format!("failed to pow: {:?} and {:?}", a, b))),
            },

            Operand::Mul => match lr {
                (Type::Num, Type::Num)  => Ok(Type::Num),
                (Type::Any, Type::Num)  => Ok(Type::Any),
                (Type::Num, Type::Any)  => Ok(Type::Any),
                (Type::Str, Type::Num)  => Ok(Type::Str),
                (Type::Str, Type::Str)  => Ok(Type::Str),
                (Type::Any, Type::Any)  => Ok(Type::Any),
                (a, b)                  => Err(ParserError::new(&format!("failed to multiply: {:?} and {:?}", a, b))),
            },

            Operand::Div => match lr {
                (Type::Num, Type::Num)  => Ok(Type::Num),
                (Type::Any, Type::Num)  => Ok(Type::Any),
                (Type::Num, Type::Any)  => Ok(Type::Any),
                (Type::Any, Type::Any)  => Ok(Type::Any),
                (a, b)                  => Err(ParserError::new(&format!("failed to divide: {:?} and {:?}", a, b))),
            },

            Operand::Mod => match lr {
                (Type::Num, Type::Num)  => Ok(Type::Num),
                (Type::Any, Type::Num)  => Ok(Type::Any),
                (Type::Num, Type::Any)  => Ok(Type::Any),
                (Type::Any, Type::Any)  => Ok(Type::Any),
                (a, b)                  => Err(ParserError::new(&format!("failed to mod: {:?} and {:?}", a, b))),
            },

            Operand::Add => match lr {
                (Type::Num, Type::Num)  => Ok(Type::Num),
                (Type::Any, Type::Num)  => Ok(Type::Any),
                (Type::Num, Type::Any)  => Ok(Type::Any),
                (Type::Str, Type::Num)  => Ok(Type::Str),
                (Type::Str, Type::Str)  => Ok(Type::Str),
                (Type::Str, Type::Bool) => Ok(Type::Str),
                (Type::Any, Type::Any)  => Ok(Type::Any),
                (a, b)                  => Err(ParserError::new(&format!("failed to add: {:?} and {:?}", a, b))),
            },

            Operand::Sub => match lr {
                (Type::Num, Type::Num)  => Ok(Type::Num),
                (Type::Any, Type::Num)  => Ok(Type::Any),
                (Type::Num, Type::Any)  => Ok(Type::Any),
                (Type::Str, Type::Num)  => Ok(Type::Str),
                (Type::Str, Type::Str)  => Ok(Type::Str),
                (Type::Any, Type::Any)  => Ok(Type::Any),
                (a, b)                  => Err(ParserError::new(&format!("failed to subtract: {:?} and {:?}", a, b))),
            },
            
            Operand::Append => match lr {
                (Type::Num, Type::Num)  => Ok(Type::Str),
                (Type::Any, Type::Num)  => Ok(Type::Str),
                (Type::Num, Type::Any)  => Ok(Type::Str),
                (Type::Str, _)          => Ok(Type::Str),
                (Type::Any, Type::Str)  => Ok(Type::Str),
                (Type::Any, Type::Any)  => Ok(Type::Any),
                (a, b)                  => Err(ParserError::new(&format!("failed to append: {:?} and {:?}", a, b))),
            },

            Operand::Equal | Operand::NEqual => Ok(Type::Bool),

            Operand::Lt | Operand::Gt | Operand::LtEqual | Operand::GtEqual => match lr {
                (a @ Type::Bool, b @ _) => Err(ParserError::new(&format!("failed to '{:?} < {:?}'", a, b))),
                (a @ _, b @ Type::Bool) => Err(ParserError::new(&format!("failed to '{:?} < {:?}'", a, b))),
                (a @ Type::Str, b @ _)  => Err(ParserError::new(&format!("failed to '{:?} < {:?}'", a, b))),
                (a @ _, b @ Type::Str)  => Err(ParserError::new(&format!("failed to '{:?} < {:?}'", a, b))),
                _                       => Ok(Type::Bool),
            },

            Operand::And | Operand::Or | Operand::Not => Ok(Type::Bool),
        }
    }

    pub fn lua(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Operand::Pow     => write!(f, "^"),
            Operand::Mul     => write!(f, "*"),
            Operand::Div     => write!(f, "/"),
            Operand::Mod     => write!(f, "%"),
            Operand::Add     => write!(f, "+"),
            Operand::Sub     => write!(f, "-"),
            Operand::Equal   => write!(f, "=="),
            Operand::NEqual  => write!(f, "~="),
            Operand::Lt      => write!(f, "<"),
            Operand::Gt      => write!(f, ">"),
            Operand::LtEqual => write!(f, "<="),
            Operand::GtEqual => write!(f, ">="),
            Operand::And     => write!(f, "and"),
            Operand::Or      => write!(f, "or"),
            Operand::Not     => write!(f, "not"),
            Operand::Append  => write!(f, ".."),
        }
    }
}

impl fmt::Display for Operand {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.lua(f)
    }
}

pub fn get_operand(v: &str) -> Option<(Operand, u8)> {
    match v {
        "^"   => Some((Operand::Pow, 0)),
        "*"   => Some((Operand::Mul, 1)),
        "/"   => Some((Operand::Div, 1)),
        "%"   => Some((Operand::Mod, 1)),
        "+"   => Some((Operand::Add, 2)),
        "-"   => Some((Operand::Sub, 2)),
        "=="  => Some((Operand::Equal, 3)),
        "!="  => Some((Operand::NEqual, 3)),
        "<"   => Some((Operand::Lt, 4)),
        ">"   => Some((Operand::Gt, 4)),
        "<="  => Some((Operand::LtEqual, 4)),
        ">="  => Some((Operand::GtEqual, 4)),
        "!"   => Some((Operand::Not, 4)),
        "and" => Some((Operand::And, 4)),
        "or"  => Some((Operand::Or, 4)),
        "++"  => Some((Operand::Append, 4)),
        _ => None,
    }
}
