/// KelpyShark Interpreter
///
/// Tree-walking interpreter that evaluates AST nodes.
/// Supports script execution and REPL mode.

use std::collections::HashMap;
use std::io::{self, BufRead, Write};

use kelpyshark_compiler::ast::*;
use kelpyshark_compiler::error::KelpyError;
use kelpyshark_compiler::lexer::Lexer;
use kelpyshark_compiler::parser::Parser;

use crate::environment::Environment;
use crate::value::Value;

/// Signal returned from statement execution to handle control flow.
enum Signal {
    None,
    Return(Value),
    Break,
    Continue,
    Throw(Value),
}

pub struct Interpreter {
    pub env: Environment,
    /// Captured output for testing. If None, prints to stdout.
    output_buffer: Option<Vec<String>>,
}

impl Interpreter {
    pub fn new() -> Self {
        let mut interp = Interpreter {
            env: Environment::new(),
            output_buffer: None,
        };
        interp.register_builtins();
        interp
    }

    /// Create an interpreter that captures output (for testing).
    pub fn new_with_buffer() -> Self {
        let mut interp = Interpreter {
            env: Environment::new(),
            output_buffer: Some(Vec::new()),
        };
        interp.register_builtins();
        interp
    }

    /// Get captured output lines (for testing).
    pub fn get_output(&self) -> Vec<String> {
        self.output_buffer.clone().unwrap_or_default()
    }

    fn register_builtins(&mut self) {
        // Built-in: len(collection)
        self.env.set(
            "len",
            Value::NativeFunction {
                name: "len".to_string(),
                arity: 1,
                func: |args| match &args[0] {
                    Value::String(s) => Ok(Value::Number(s.len() as f64)),
                    Value::List(l) => Ok(Value::Number(l.len() as f64)),
                    Value::Dict(d) => Ok(Value::Number(d.len() as f64)),
                    other => Err(format!("len() not supported for {}", other.type_name())),
                },
            },
        );

        // Built-in: type(value)
        self.env.set(
            "type",
            Value::NativeFunction {
                name: "type".to_string(),
                arity: 1,
                func: |args| Ok(Value::String(args[0].type_name().to_string())),
            },
        );

        // Built-in: str(value)
        self.env.set(
            "str",
            Value::NativeFunction {
                name: "str".to_string(),
                arity: 1,
                func: |args| Ok(Value::String(format!("{}", args[0]))),
            },
        );

        // Built-in: num(value)
        self.env.set(
            "num",
            Value::NativeFunction {
                name: "num".to_string(),
                arity: 1,
                func: |args| match &args[0] {
                    Value::Number(n) => Ok(Value::Number(*n)),
                    Value::String(s) => s
                        .parse::<f64>()
                        .map(Value::Number)
                        .map_err(|_| format!("Cannot convert '{}' to number", s)),
                    Value::Boolean(b) => Ok(Value::Number(if *b { 1.0 } else { 0.0 })),
                    other => Err(format!(
                        "Cannot convert {} to number",
                        other.type_name()
                    )),
                },
            },
        );

        // Built-in: push(list, value)
        self.env.set(
            "push",
            Value::NativeFunction {
                name: "push".to_string(),
                arity: 2,
                func: |args| {
                    let mut args = args;
                    match args.remove(0) {
                        Value::List(mut l) => {
                            l.push(args.remove(0));
                            Ok(Value::List(l))
                        }
                        other => Err(format!(
                            "push() expects a list, got {}",
                            other.type_name()
                        )),
                    }
                },
            },
        );

        // Built-in: range(n) or range(start, stop) or range(start, stop, step)
        self.env.set(
            "range",
            Value::NativeFunction {
                name: "range".to_string(),
                arity: usize::MAX, // variadic — handled manually
                func: |_| Ok(Value::Null), // stub; dispatched specially
            },
        );

        // Built-in: int(value) — alias for num() but truncates
        self.env.set(
            "int",
            Value::NativeFunction {
                name: "int".to_string(),
                arity: 1,
                func: |args| match &args[0] {
                    Value::Number(n) => Ok(Value::Number((*n).trunc())),
                    Value::String(s) => s
                        .parse::<f64>()
                        .map(|n| Value::Number(n.trunc()))
                        .map_err(|_| format!("Cannot convert '{}' to int", s)),
                    Value::Boolean(b) => Ok(Value::Number(if *b { 1.0 } else { 0.0 })),
                    other => Err(format!("Cannot convert {} to int", other.type_name())),
                },
            },
        );

        // Built-in: float(value)
        self.env.set(
            "float",
            Value::NativeFunction {
                name: "float".to_string(),
                arity: 1,
                func: |args| match &args[0] {
                    Value::Number(n) => Ok(Value::Number(*n)),
                    Value::String(s) => s
                        .parse::<f64>()
                        .map(Value::Number)
                        .map_err(|_| format!("Cannot convert '{}' to float", s)),
                    Value::Boolean(b) => Ok(Value::Number(if *b { 1.0 } else { 0.0 })),
                    other => Err(format!("Cannot convert {} to float", other.type_name())),
                },
            },
        );

        // Built-in: print() as a callable function (in addition to the print statement)
        self.env.set(
            "input",
            Value::NativeFunction {
                name: "input".to_string(),
                arity: 1,
                func: |args| {
                    print!("{}", args[0]);
                    io::stdout().flush().ok();
                    let mut line = String::new();
                    io::stdin().read_line(&mut line).ok();
                    Ok(Value::String(line.trim_end_matches('\n').trim_end_matches('\r').to_string()))
                },
            },
        );

        // Built-in: abs(n)
        self.env.set(
            "abs",
            Value::NativeFunction {
                name: "abs".to_string(),
                arity: 1,
                func: |args| match &args[0] {
                    Value::Number(n) => Ok(Value::Number(n.abs())),
                    other => Err(format!("abs() expects a number, got {}", other.type_name())),
                },
            },
        );

        // Built-in: sqrt(n)
        self.env.set(
            "sqrt",
            Value::NativeFunction {
                name: "sqrt".to_string(),
                arity: 1,
                func: |args| match &args[0] {
                    Value::Number(n) => Ok(Value::Number(n.sqrt())),
                    other => Err(format!("sqrt() expects a number, got {}", other.type_name())),
                },
            },
        );

        // Built-in: floor(n)
        self.env.set(
            "floor",
            Value::NativeFunction {
                name: "floor".to_string(),
                arity: 1,
                func: |args| match &args[0] {
                    Value::Number(n) => Ok(Value::Number(n.floor())),
                    other => Err(format!("floor() expects a number, got {}", other.type_name())),
                },
            },
        );

        // Built-in: ceil(n)
        self.env.set(
            "ceil",
            Value::NativeFunction {
                name: "ceil".to_string(),
                arity: 1,
                func: |args| match &args[0] {
                    Value::Number(n) => Ok(Value::Number(n.ceil())),
                    other => Err(format!("ceil() expects a number, got {}", other.type_name())),
                },
            },
        );

        // Built-in: round(n)
        self.env.set(
            "round",
            Value::NativeFunction {
                name: "round".to_string(),
                arity: 1,
                func: |args| match &args[0] {
                    Value::Number(n) => Ok(Value::Number(n.round())),
                    other => Err(format!("round() expects a number, got {}", other.type_name())),
                },
            },
        );

        // Built-in: min(a, b)
        self.env.set(
            "min",
            Value::NativeFunction {
                name: "min".to_string(),
                arity: 2,
                func: |args| match (&args[0], &args[1]) {
                    (Value::Number(a), Value::Number(b)) => Ok(Value::Number(a.min(*b))),
                    _ => Err("min() expects two numbers".to_string()),
                },
            },
        );

        // Built-in: max(a, b)
        self.env.set(
            "max",
            Value::NativeFunction {
                name: "max".to_string(),
                arity: 2,
                func: |args| match (&args[0], &args[1]) {
                    (Value::Number(a), Value::Number(b)) => Ok(Value::Number(a.max(*b))),
                    _ => Err("max() expects two numbers".to_string()),
                },
            },
        );

        // Built-in: pow(base, exp)
        self.env.set(
            "pow",
            Value::NativeFunction {
                name: "pow".to_string(),
                arity: 2,
                func: |args| match (&args[0], &args[1]) {
                    (Value::Number(a), Value::Number(b)) => Ok(Value::Number(a.powf(*b))),
                    _ => Err("pow() expects two numbers".to_string()),
                },
            },
        );
    }

    fn output(&mut self, text: &str) {
        match &mut self.output_buffer {
            Some(buf) => buf.push(text.to_string()),
            None => println!("{}", text),
        }
    }

    // ── Public API ──

    /// Execute a KelpyShark source string.
    pub fn exec(&mut self, source: &str) -> Result<(), KelpyError> {
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize()?;
        let mut parser = Parser::new(tokens);
        let program = parser.parse()?;
        self.exec_program(&program)?;
        Ok(())
    }

    /// Run a REPL (Read-Eval-Print Loop).
    pub fn repl(&mut self) {
        println!("🦈 KelpyShark v0.1.0 REPL");
        println!("Type 'exit' to quit.\n");

        let stdin = io::stdin();
        loop {
            print!("ks> ");
            io::stdout().flush().unwrap();

            let mut line = String::new();
            if stdin.lock().read_line(&mut line).is_err() {
                break;
            }
            let line = line.trim();

            if line.is_empty() {
                continue;
            }
            if line == "exit" || line == "quit" {
                println!("Goodbye! 🦈");
                break;
            }

            match self.exec(line) {
                Ok(()) => {}
                Err(e) => eprintln!("{}", e),
            }
        }
    }

    // ── Execution ──

    fn exec_program(&mut self, program: &Program) -> Result<(), KelpyError> {
        for stmt in &program.statements {
            let signal = self.exec_statement(stmt)?;
            match signal {
                Signal::Return(_) | Signal::Break | Signal::Continue => break,
                Signal::Throw(v) => {
                    return Err(KelpyError::RuntimeError {
                        message: format!("Uncaught exception: {}", v),
                    });
                }
                Signal::None => {}
            }
        }
        Ok(())
    }

    fn exec_statement(&mut self, stmt: &Statement) -> Result<Signal, KelpyError> {
        match stmt {
            Statement::Assignment {
                name, value, ..
            } => {
                let val = self.eval_expr(value)?;
                self.env.update(name, val);
                Ok(Signal::None)
            }
            Statement::FunctionDef {
                name,
                params,
                body,
                ..
            } => {
                let func = Value::Function {
                    name: name.clone(),
                    params: params.clone(),
                    body: body.clone(),
                };
                self.env.set(name, func);
                Ok(Signal::None)
            }
            Statement::If {
                condition,
                then_body,
                else_body,
                ..
            } => {
                let cond = self.eval_expr(condition)?;
                if cond.is_truthy() {
                    for s in then_body {
                        let signal = self.exec_statement(s)?;
                        match signal {
                            Signal::None => {}
                            other => return Ok(other),
                        }
                    }
                } else if let Some(else_stmts) = else_body {
                    for s in else_stmts {
                        let signal = self.exec_statement(s)?;
                        match signal {
                            Signal::None => {}
                            other => return Ok(other),
                        }
                    }
                }
                Ok(Signal::None)
            }
            Statement::While {
                condition, body, ..
            } => {
                loop {
                    let cond = self.eval_expr(condition)?;
                    if !cond.is_truthy() {
                        break;
                    }
                    for s in body {
                        let signal = self.exec_statement(s)?;
                        match signal {
                            Signal::None => {}
                            Signal::Break => return Ok(Signal::None),
                            Signal::Continue => break,
                            other => return Ok(other),
                        }
                    }
                }
                Ok(Signal::None)
            }
            Statement::For {
                variable,
                iterable,
                body,
                ..
            } => {
                let iter_val = self.eval_expr(iterable)?;
                match iter_val {
                    Value::List(items) => {
                        'outer: for item in items {
                            self.env.update(variable, item);
                            for s in body {
                                let signal = self.exec_statement(s)?;
                                match signal {
                                    Signal::None => {}
                                    Signal::Break => break 'outer,
                                    Signal::Continue => continue 'outer,
                                    other => return Ok(other),
                                }
                            }
                        }
                    }
                    other => {
                        return Err(KelpyError::RuntimeError {
                            message: format!(
                                "Cannot iterate over {}",
                                other.type_name()
                            ),
                        });
                    }
                }
                Ok(Signal::None)
            }
            Statement::Return { value, .. } => {
                let val = match value {
                    Some(expr) => self.eval_expr(expr)?,
                    None => Value::Null,
                };
                Ok(Signal::Return(val))
            }
            Statement::Import { module, .. } => {
                // Placeholder: just acknowledge the import for now
                // Real module system will come with stdlib
                self.output(&format!("[import: {}]", module));
                Ok(Signal::None)
            }
            Statement::Print { value, .. } => {
                let val = self.eval_expr(value)?;
                self.output(&format!("{}", val));
                Ok(Signal::None)
            }
            Statement::ExprStatement { expr, .. } => {
                self.eval_expr(expr)?;
                Ok(Signal::None)
            }

            Statement::Break { .. } => Ok(Signal::Break),

            Statement::Continue { .. } => Ok(Signal::Continue),

            Statement::Throw { value, .. } => {
                let val = self.eval_expr(value)?;
                Ok(Signal::Throw(val))
            }

            Statement::TryCatch {
                try_body,
                catch_var,
                catch_body,
                ..
            } => {
                // Execute try body, collecting any Throw signal
                let mut thrown: Option<Value> = None;
                let try_body = try_body.clone();
                for stmt in &try_body {
                    let signal = self.exec_statement(stmt)?;
                    match signal {
                        Signal::Throw(val) => {
                            thrown = Some(val);
                            break;
                        }
                        Signal::None => {}
                        other => return Ok(other),
                    }
                }

                if let Some(err_val) = thrown {
                    // Bind the caught value and run catch body
                    self.env.push_scope();
                    if !catch_var.is_empty() {
                        self.env.set(catch_var, err_val);
                    }
                    let catch_body = catch_body.clone();
                    for stmt in &catch_body {
                        let signal = self.exec_statement(stmt)?;
                        match signal {
                            Signal::None => {}
                            other => {
                                self.env.pop_scope();
                                return Ok(other);
                            }
                        }
                    }
                    self.env.pop_scope();
                }
                Ok(Signal::None)
            }

            Statement::CompoundAssignment { name, op, value, .. } => {
                let current = self.env.get(name).cloned().ok_or_else(|| {
                    KelpyError::RuntimeError {
                        message: format!("Undefined variable: '{}'", name),
                    }
                })?;
                let rhs = self.eval_expr(value)?;
                let result = match (current, op) {
                    (Value::Number(a), CompoundOp::Add) => match rhs {
                        Value::Number(b) => Ok(Value::Number(a + b)),
                        Value::String(b) => Ok(Value::String(format!("{}{}", a as i64, b))),
                        other => Err(format!("Cannot += {} to number", other.type_name())),
                    },
                    (Value::Number(a), CompoundOp::Subtract) => match rhs {
                        Value::Number(b) => Ok(Value::Number(a - b)),
                        other => Err(format!("Cannot -= {} from number", other.type_name())),
                    },
                    (Value::Number(a), CompoundOp::Multiply) => match rhs {
                        Value::Number(b) => Ok(Value::Number(a * b)),
                        other => Err(format!("Cannot *= {} with number", other.type_name())),
                    },
                    (Value::Number(a), CompoundOp::Divide) => match rhs {
                        Value::Number(b) => {
                            if b == 0.0 {
                                Err("Division by zero".to_string())
                            } else {
                                Ok(Value::Number(a / b))
                            }
                        }
                        other => Err(format!("Cannot /= {} with number", other.type_name())),
                    },
                    (Value::String(a), CompoundOp::Add) => {
                        Ok(Value::String(format!("{}{}", a, rhs)))
                    }
                    (current, op) => Err(format!(
                        "Cannot apply {:?} to {}",
                        op,
                        current.type_name()
                    )),
                }
                .map_err(|msg| KelpyError::RuntimeError { message: msg })?;
                self.env.update(name, result);
                Ok(Signal::None)
            }

            Statement::ClassDef { name, methods, .. } => {
                let mut method_map = HashMap::new();
                for method in methods {
                    if let Statement::FunctionDef { name: mname, params, body, .. } = method {
                        method_map.insert(
                            mname.clone(),
                            Value::Function {
                                name: mname.clone(),
                                params: params.clone(),
                                body: body.clone(),
                            },
                        );
                    }
                }
                let class_val = Value::Class {
                    name: name.clone(),
                    methods: method_map,
                };
                self.env.set(name, class_val);
                Ok(Signal::None)
            }
        }
    }

    // ── Expression evaluation ──

    fn eval_expr(&mut self, expr: &Expr) -> Result<Value, KelpyError> {
        match expr {
            Expr::NumberLiteral { value, .. } => Ok(Value::Number(*value)),
            Expr::StringLiteral { value, .. } => Ok(Value::String(value.clone())),
            Expr::BooleanLiteral { value, .. } => Ok(Value::Boolean(*value)),

            Expr::Identifier { name, .. } => {
                self.env.get(name).cloned().ok_or_else(|| {
                    KelpyError::RuntimeError {
                        message: format!("Undefined variable: '{}'", name),
                    }
                })
            }

            Expr::BinaryOp {
                left, op, right, ..
            } => {
                let lhs = self.eval_expr(left)?;
                let rhs = self.eval_expr(right)?;
                self.eval_binary_op(&lhs, op, &rhs)
            }

            Expr::UnaryOp { op, operand, .. } => {
                let val = self.eval_expr(operand)?;
                match op {
                    UnaryOperator::Negate => match val {
                        Value::Number(n) => Ok(Value::Number(-n)),
                        _ => Err(KelpyError::RuntimeError {
                            message: format!(
                                "Cannot negate {}",
                                val.type_name()
                            ),
                        }),
                    },
                    UnaryOperator::Not => Ok(Value::Boolean(!val.is_truthy())),
                }
            }

            Expr::FunctionCall { callee, args, .. } => {
                // Special-case range() which is variadic
                if let Expr::Identifier { name, .. } = callee.as_ref() {
                    if name == "range" {
                        let mut arg_vals = Vec::new();
                        for arg in args {
                            arg_vals.push(self.eval_expr(arg)?);
                        }
                        return self.call_range(arg_vals);
                    }
                }
                let func = self.eval_expr(callee)?;
                let mut arg_vals = Vec::new();
                for arg in args {
                    arg_vals.push(self.eval_expr(arg)?);
                }
                self.call_function(&func, arg_vals)
            }

            Expr::Index {
                object, index, ..
            } => {
                let obj = self.eval_expr(object)?;
                let idx = self.eval_expr(index)?;
                match (&obj, &idx) {
                    (Value::List(items), Value::Number(n)) => {
                        let i = *n as usize;
                        items.get(i).cloned().ok_or_else(|| {
                            KelpyError::RuntimeError {
                                message: format!(
                                    "Index {} out of bounds (list length {})",
                                    i,
                                    items.len()
                                ),
                            }
                        })
                    }
                    (Value::Dict(map), Value::String(key)) => {
                        map.get(key).cloned().ok_or_else(|| {
                            KelpyError::RuntimeError {
                                message: format!("Key '{}' not found in dict", key),
                            }
                        })
                    }
                    _ => Err(KelpyError::RuntimeError {
                        message: format!(
                            "Cannot index {} with {}",
                            obj.type_name(),
                            idx.type_name()
                        ),
                    }),
                }
            }

            Expr::MemberAccess {
                object, member, ..
            } => {
                let obj = self.eval_expr(object)?;
                match &obj {
                    Value::Dict(map) => {
                        map.get(member).cloned().ok_or_else(|| {
                            KelpyError::RuntimeError {
                                message: format!("Key '{}' not found in dict", member),
                            }
                        })
                    }
                    Value::Instance { fields, .. } => {
                        fields.get(member).cloned().ok_or_else(|| {
                            KelpyError::RuntimeError {
                                message: format!("Field '{}' not found on instance", member),
                            }
                        })
                    }
                    _ => Err(KelpyError::RuntimeError {
                        message: format!(
                            "Cannot access member '{}' on {}",
                            member,
                            obj.type_name()
                        ),
                    }),
                }
            }

            Expr::ListLiteral { elements, .. } => {
                let mut items = Vec::new();
                for elem in elements {
                    items.push(self.eval_expr(elem)?);
                }
                Ok(Value::List(items))
            }

            Expr::DictLiteral { entries, .. } => {
                let mut map = HashMap::new();
                for (key_expr, val_expr) in entries {
                    let key = self.eval_expr(key_expr)?;
                    let key_str = match key {
                        Value::String(s) => s,
                        other => format!("{}", other),
                    };
                    let val = self.eval_expr(val_expr)?;
                    map.insert(key_str, val);
                }
                Ok(Value::Dict(map))
            }

            Expr::StringInterpolation { parts, .. } => {
                let mut result = String::new();
                for part in parts {
                    match part {
                        StringPart::Literal(s) => result.push_str(s),
                        StringPart::Expression(expr) => {
                            let val = self.eval_expr(expr)?;
                            result.push_str(&format!("{}", val));
                        }
                    }
                }
                Ok(Value::String(result))
            }

            Expr::NullLiteral { .. } => Ok(Value::Null),

            Expr::MethodCall { object, method, args, .. } => {
                let obj = self.eval_expr(object)?;
                let mut arg_vals = Vec::new();
                for arg in args {
                    arg_vals.push(self.eval_expr(arg)?);
                }
                self.call_method(obj, method, arg_vals)
            }

            Expr::New { class_name, args, .. } => {
                let class_val = self.env.get(class_name).cloned().ok_or_else(|| {
                    KelpyError::RuntimeError {
                        message: format!("Undefined class: '{}'", class_name),
                    }
                })?;
                match class_val {
                    Value::Class { name, methods } => {
                        let instance = Value::Instance {
                            class_name: name.clone(),
                            fields: HashMap::new(),
                        };
                        // Call __init__ if it exists
                        if let Some(init_fn) = methods.get("__init__").cloned() {
                            let mut arg_vals = vec![instance.clone()];
                            for arg in args {
                                arg_vals.push(self.eval_expr(arg)?);
                            }
                            self.call_function_with_self(&init_fn, &name, methods, arg_vals)
                        } else {
                            Ok(instance)
                        }
                    }
                    other => Err(KelpyError::RuntimeError {
                        message: format!("'{}' is not a class", other.type_name()),
                    }),
                }
            }
        }
    }

    fn eval_binary_op(
        &self,
        lhs: &Value,
        op: &BinaryOperator,
        rhs: &Value,
    ) -> Result<Value, KelpyError> {
        match (lhs, op, rhs) {
            // Number arithmetic
            (Value::Number(a), BinaryOperator::Add, Value::Number(b)) => {
                Ok(Value::Number(a + b))
            }
            (Value::Number(a), BinaryOperator::Subtract, Value::Number(b)) => {
                Ok(Value::Number(a - b))
            }
            (Value::Number(a), BinaryOperator::Multiply, Value::Number(b)) => {
                Ok(Value::Number(a * b))
            }
            (Value::Number(a), BinaryOperator::Divide, Value::Number(b)) => {
                if *b == 0.0 {
                    Err(KelpyError::RuntimeError {
                        message: "Division by zero".to_string(),
                    })
                } else {
                    Ok(Value::Number(a / b))
                }
            }
            (Value::Number(a), BinaryOperator::Modulo, Value::Number(b)) => {
                if *b == 0.0 {
                    Err(KelpyError::RuntimeError {
                        message: "Modulo by zero".to_string(),
                    })
                } else {
                    Ok(Value::Number(a % b))
                }
            }

            // String concatenation
            (Value::String(a), BinaryOperator::Add, Value::String(b)) => {
                Ok(Value::String(format!("{}{}", a, b)))
            }
            // String + any (coerce to string)
            (Value::String(a), BinaryOperator::Add, other) => {
                Ok(Value::String(format!("{}{}", a, other)))
            }
            (other, BinaryOperator::Add, Value::String(b)) => {
                Ok(Value::String(format!("{}{}", other, b)))
            }

            // Number comparisons
            (Value::Number(a), BinaryOperator::LessThan, Value::Number(b)) => {
                Ok(Value::Boolean(a < b))
            }
            (Value::Number(a), BinaryOperator::LessEqual, Value::Number(b)) => {
                Ok(Value::Boolean(a <= b))
            }
            (Value::Number(a), BinaryOperator::GreaterThan, Value::Number(b)) => {
                Ok(Value::Boolean(a > b))
            }
            (Value::Number(a), BinaryOperator::GreaterEqual, Value::Number(b)) => {
                Ok(Value::Boolean(a >= b))
            }

            // Equality (any types)
            (a, BinaryOperator::Equal, b) => Ok(Value::Boolean(a == b)),
            (a, BinaryOperator::NotEqual, b) => Ok(Value::Boolean(a != b)),

            // Logical operators
            (a, BinaryOperator::And, b) => {
                Ok(Value::Boolean(a.is_truthy() && b.is_truthy()))
            }
            (a, BinaryOperator::Or, b) => {
                Ok(Value::Boolean(a.is_truthy() || b.is_truthy()))
            }

            // Type mismatch
            _ => Err(KelpyError::RuntimeError {
                message: format!(
                    "Cannot apply '{}' to {} and {}",
                    op,
                    lhs.type_name(),
                    rhs.type_name()
                ),
            }),
        }
    }

    fn call_function(
        &mut self,
        func: &Value,
        args: Vec<Value>,
    ) -> Result<Value, KelpyError> {
        match func {
            Value::Function {
                name,
                params,
                body,
            } => {
                if args.len() != params.len() {
                    return Err(KelpyError::RuntimeError {
                        message: format!(
                            "Function '{}' expects {} arguments, got {}",
                            name,
                            params.len(),
                            args.len()
                        ),
                    });
                }

                self.env.push_scope();
                for (param, arg) in params.iter().zip(args.into_iter()) {
                    self.env.set(param, arg);
                }

                let mut result = Value::Null;
                let body = body.clone(); // Clone to avoid borrow issues
                for stmt in &body {
                    let signal = self.exec_statement(stmt)?;
                    if let Signal::Return(val) = signal {
                        result = val;
                        break;
                    }
                }

                self.env.pop_scope();
                Ok(result)
            }
            Value::NativeFunction {
                name,
                arity,
                func: native_fn,
            } => {
                if args.len() != *arity {
                    return Err(KelpyError::RuntimeError {
                        message: format!(
                            "Native function '{}' expects {} arguments, got {}",
                            name, arity, args.len()
                        ),
                    });
                }
                native_fn(args).map_err(|msg| KelpyError::RuntimeError {
                    message: msg,
                })
            }
            other => Err(KelpyError::RuntimeError {
                message: format!("'{}' is not a function", other.type_name()),
            }),
        }
    }

    /// Handle `range(stop)`, `range(start, stop)`, `range(start, stop, step)`.
    fn call_range(&self, args: Vec<Value>) -> Result<Value, KelpyError> {
        let (start, stop, step) = match args.len() {
            1 => match &args[0] {
                Value::Number(n) => (0.0, *n, 1.0),
                _ => return Err(KelpyError::RuntimeError { message: "range() expects numbers".to_string() }),
            },
            2 => match (&args[0], &args[1]) {
                (Value::Number(a), Value::Number(b)) => (*a, *b, 1.0),
                _ => return Err(KelpyError::RuntimeError { message: "range() expects numbers".to_string() }),
            },
            3 => match (&args[0], &args[1], &args[2]) {
                (Value::Number(a), Value::Number(b), Value::Number(c)) => (*a, *b, *c),
                _ => return Err(KelpyError::RuntimeError { message: "range() expects numbers".to_string() }),
            },
            _ => return Err(KelpyError::RuntimeError {
                message: "range() expects 1–3 arguments".to_string(),
            }),
        };
        if step == 0.0 {
            return Err(KelpyError::RuntimeError { message: "range() step cannot be zero".to_string() });
        }
        let mut items = Vec::new();
        let mut i = start;
        while (step > 0.0 && i < stop) || (step < 0.0 && i > stop) {
            items.push(Value::Number(i));
            i += step;
        }
        Ok(Value::List(items))
    }

    /// Dispatch a method call on a value.
    fn call_method(
        &mut self,
        obj: Value,
        method: &str,
        args: Vec<Value>,
    ) -> Result<Value, KelpyError> {
        match obj.clone() {
            Value::List(mut items) => match method {
                "append" | "push" => {
                    if args.len() != 1 {
                        return Err(KelpyError::RuntimeError { message: format!("append() takes 1 argument") });
                    }
                    items.push(args.into_iter().next().unwrap());
                    Ok(Value::List(items))
                }
                "pop" => {
                    if items.is_empty() {
                        Err(KelpyError::RuntimeError { message: "pop() on empty list".to_string() })
                    } else {
                        items.pop();
                        Ok(Value::List(items))
                    }
                }
                "len" | "length" => Ok(Value::Number(items.len() as f64)),
                "contains" => {
                    if args.len() != 1 { return Err(KelpyError::RuntimeError { message: "contains() takes 1 argument".to_string() }); }
                    Ok(Value::Boolean(items.contains(&args[0])))
                }
                "reverse" => {
                    items.reverse();
                    Ok(Value::List(items))
                }
                "sort" => {
                    items.sort_by(|a, b| match (a, b) {
                        (Value::Number(x), Value::Number(y)) => x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal),
                        (Value::String(x), Value::String(y)) => x.cmp(y),
                        _ => std::cmp::Ordering::Equal,
                    });
                    Ok(Value::List(items))
                }
                "join" => {
                    if args.len() != 1 { return Err(KelpyError::RuntimeError { message: "join() takes 1 separator argument".to_string() }); }
                    let sep = match &args[0] {
                        Value::String(s) => s.clone(),
                        other => format!("{}", other),
                    };
                    let parts: Vec<String> = items.iter().map(|v| format!("{}", v)).collect();
                    Ok(Value::String(parts.join(&sep)))
                }
                "first" => items.first().cloned().ok_or_else(|| KelpyError::RuntimeError { message: "first() on empty list".to_string() }),
                "last"  => items.last().cloned().ok_or_else(|| KelpyError::RuntimeError { message: "last() on empty list".to_string() }),
                other => Err(KelpyError::RuntimeError {
                    message: format!("List has no method '{}'", other),
                }),
            },

            Value::String(s) => match method {
                "upper"      => Ok(Value::String(s.to_uppercase())),
                "lower"      => Ok(Value::String(s.to_lowercase())),
                "trim"       => Ok(Value::String(s.trim().to_string())),
                "len" | "length" => Ok(Value::Number(s.len() as f64)),
                "reverse"    => Ok(Value::String(s.chars().rev().collect())),
                "split" => {
                    let sep = if args.is_empty() {
                        " ".to_string()
                    } else {
                        match &args[0] {
                            Value::String(sep) => sep.clone(),
                            other => format!("{}", other),
                        }
                    };
                    let parts: Vec<Value> = s.split(&sep as &str).map(|p| Value::String(p.to_string())).collect();
                    Ok(Value::List(parts))
                }
                "contains" => {
                    if args.len() != 1 { return Err(KelpyError::RuntimeError { message: "contains() takes 1 argument".to_string() }); }
                    let needle = match &args[0] { Value::String(n) => n.clone(), other => format!("{}", other) };
                    Ok(Value::Boolean(s.contains(&needle as &str)))
                }
                "starts_with" => {
                    if args.len() != 1 { return Err(KelpyError::RuntimeError { message: "starts_with() takes 1 argument".to_string() }); }
                    let prefix = match &args[0] { Value::String(n) => n.clone(), other => format!("{}", other) };
                    Ok(Value::Boolean(s.starts_with(&prefix as &str)))
                }
                "ends_with" => {
                    if args.len() != 1 { return Err(KelpyError::RuntimeError { message: "ends_with() takes 1 argument".to_string() }); }
                    let suffix = match &args[0] { Value::String(n) => n.clone(), other => format!("{}", other) };
                    Ok(Value::Boolean(s.ends_with(&suffix as &str)))
                }
                "replace" => {
                    if args.len() != 2 { return Err(KelpyError::RuntimeError { message: "replace() takes 2 arguments".to_string() }); }
                    let from = match &args[0] { Value::String(n) => n.clone(), other => format!("{}", other) };
                    let to   = match &args[1] { Value::String(n) => n.clone(), other => format!("{}", other) };
                    Ok(Value::String(s.replace(&from as &str, &to as &str)))
                }
                "char_at" => {
                    if args.len() != 1 { return Err(KelpyError::RuntimeError { message: "char_at() takes 1 argument".to_string() }); }
                    let idx = match &args[0] { Value::Number(n) => *n as usize, _ => return Err(KelpyError::RuntimeError { message: "char_at() index must be a number".to_string() }) };
                    s.chars().nth(idx).map(|c| Value::String(c.to_string())).ok_or_else(|| KelpyError::RuntimeError { message: format!("char_at({}) out of bounds", idx) })
                }
                "substring" => {
                    if args.len() != 2 { return Err(KelpyError::RuntimeError { message: "substring() takes 2 arguments (start, end)".to_string() }); }
                    let start = match &args[0] { Value::Number(n) => *n as usize, _ => return Err(KelpyError::RuntimeError { message: "substring() args must be numbers".to_string() }) };
                    let end   = match &args[1] { Value::Number(n) => *n as usize, _ => return Err(KelpyError::RuntimeError { message: "substring() args must be numbers".to_string() }) };
                    let chars: Vec<char> = s.chars().collect();
                    let slice: String = chars.get(start..end).unwrap_or(&[]).iter().collect();
                    Ok(Value::String(slice))
                }
                "to_num" | "to_number" => s.parse::<f64>().map(Value::Number).map_err(|_| KelpyError::RuntimeError { message: format!("Cannot parse '{}' as number", s) }),
                other => Err(KelpyError::RuntimeError {
                    message: format!("String has no method '{}'", other),
                }),
            },

            Value::Dict(map) => match method {
                "keys"   => Ok(Value::List(map.keys().map(|k| Value::String(k.clone())).collect())),
                "values" => Ok(Value::List(map.values().cloned().collect())),
                "items"  => Ok(Value::List(map.iter().map(|(k, v)| Value::List(vec![Value::String(k.clone()), v.clone()])).collect())),
                "contains" | "has_key" => {
                    if args.len() != 1 { return Err(KelpyError::RuntimeError { message: "has_key() takes 1 argument".to_string() }); }
                    let key = match &args[0] { Value::String(k) => k.clone(), other => format!("{}", other) };
                    Ok(Value::Boolean(map.contains_key(&key)))
                }
                "len" | "length" => Ok(Value::Number(map.len() as f64)),
                other => Err(KelpyError::RuntimeError {
                    message: format!("Dict has no method '{}'", other),
                }),
            },

            Value::Instance { class_name, fields } => {
                // Look up the class and dispatch the method
                let class_val = self.env.get(&class_name).cloned().ok_or_else(|| {
                    KelpyError::RuntimeError { message: format!("Class '{}' not found", class_name) }
                })?;
                match class_val {
                    Value::Class { name: cname, methods } => {
                        let method_fn = methods.get(method).cloned().ok_or_else(|| {
                            KelpyError::RuntimeError { message: format!("'{}' has no method '{}'", class_name, method) }
                        })?;
                        let self_val = Value::Instance { class_name: cname.clone(), fields };
                        let mut full_args = vec![self_val];
                        full_args.extend(args);
                        self.call_function_with_self(&method_fn, &cname, methods, full_args)
                    }
                    _ => Err(KelpyError::RuntimeError { message: format!("'{}' is not a class", class_name) }),
                }
            }

            other => Err(KelpyError::RuntimeError {
                message: format!("{} does not support method calls", other.type_name()),
            }),
        }
    }

    /// Call a function that belongs to a class (has access to self and sibling methods).
    fn call_function_with_self(
        &mut self,
        func: &Value,
        _class_name: &str,
        methods: HashMap<String, Value>,
        args: Vec<Value>,
    ) -> Result<Value, KelpyError> {
        match func {
            Value::Function { name, params, body } => {
                if args.len() != params.len() {
                    return Err(KelpyError::RuntimeError {
                        message: format!(
                            "Method '{}' expects {} arguments, got {}",
                            name, params.len(), args.len()
                        ),
                    });
                }
                self.env.push_scope();
                for (param, arg) in params.iter().zip(args.into_iter()) {
                    self.env.set(param, arg);
                }
                // Inject all sibling methods as locals so self.method() works
                for (mname, mfunc) in &methods {
                    // Don't override existing bindings (like self)
                    if self.env.get(mname).is_none() {
                        self.env.set(mname, mfunc.clone());
                    }
                }
                let mut result = Value::Null;
                let body = body.clone();
                for stmt in &body {
                    let signal = self.exec_statement(stmt)?;
                    if let Signal::Return(val) = signal {
                        result = val;
                        break;
                    }
                }
                self.env.pop_scope();
                Ok(result)
            }
            other => Err(KelpyError::RuntimeError {
                message: format!("'{}' is not callable", other.type_name()),
            }),
        }
    }
}

impl Default for Interpreter {
    fn default() -> Self {
        Self::new()
    }
}

// ──────────────────────────────────────────────
//  Tests
// ──────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn run(source: &str) -> Vec<String> {
        let mut interp = Interpreter::new_with_buffer();
        interp.exec(source).expect("Should execute without error");
        interp.get_output()
    }

    fn run_err(source: &str) -> KelpyError {
        let mut interp = Interpreter::new_with_buffer();
        interp.exec(source).expect_err("Should produce an error")
    }

    #[test]
    fn test_print_string() {
        let output = run(r#"print "Hello, KelpyShark!""#);
        assert_eq!(output, vec!["Hello, KelpyShark!"]);
    }

    #[test]
    fn test_print_number() {
        let output = run("print 42");
        assert_eq!(output, vec!["42"]);
    }

    #[test]
    fn test_variable_assignment() {
        let output = run("x = 10\nprint x");
        assert_eq!(output, vec!["10"]);
    }

    #[test]
    fn test_arithmetic() {
        let output = run("print 2 + 3 * 4");
        assert_eq!(output, vec!["14"]);
    }

    #[test]
    fn test_string_concatenation() {
        let output = run(r#"print "hello" + " " + "world""#);
        assert_eq!(output, vec!["hello world"]);
    }

    #[test]
    fn test_function_def_and_call() {
        let output = run(
            r#"
def greet(name) {
    print "Hello, " + name + "!"
}
greet("KelpyShark")
"#,
        );
        assert_eq!(output, vec!["Hello, KelpyShark!"]);
    }

    #[test]
    fn test_function_return() {
        let output = run(
            r#"
def add(a, b) {
    return a + b
}
result = add(3, 7)
print result
"#,
        );
        assert_eq!(output, vec!["10"]);
    }

    #[test]
    fn test_if_true() {
        let output = run(
            r#"
x = 10
if x >= 5 {
    print "big"
}
"#,
        );
        assert_eq!(output, vec!["big"]);
    }

    #[test]
    fn test_if_false() {
        let output = run(
            r#"
x = 2
if x >= 5 {
    print "big"
}
"#,
        );
        assert!(output.is_empty());
    }

    #[test]
    fn test_if_else() {
        let output = run(
            r#"
x = 2
if x >= 5 {
    print "big"
} else {
    print "small"
}
"#,
        );
        assert_eq!(output, vec!["small"]);
    }

    #[test]
    fn test_while_loop() {
        let output = run(
            r#"
x = 0
while x < 3 {
    print x
    x = x + 1
}
"#,
        );
        assert_eq!(output, vec!["0", "1", "2"]);
    }

    #[test]
    fn test_for_loop() {
        let output = run(
            r#"
items = ["apple", "banana", "cherry"]
for item in items {
    print item
}
"#,
        );
        assert_eq!(output, vec!["apple", "banana", "cherry"]);
    }

    #[test]
    fn test_list_literal() {
        let output = run(
            r#"
x = [1, 2, 3]
print x
"#,
        );
        assert_eq!(output, vec!["[1, 2, 3]"]);
    }

    #[test]
    fn test_list_index() {
        let output = run(
            r#"
x = ["a", "b", "c"]
print x[1]
"#,
        );
        assert_eq!(output, vec!["b"]);
    }

    #[test]
    fn test_dict_literal() {
        let output = run(
            r#"
x = {"name": "Bob"}
print x["name"]
"#,
        );
        assert_eq!(output, vec!["Bob"]);
    }

    #[test]
    fn test_string_interpolation() {
        let output = run(
            r#"
name = "world"
print "Hello {$name}!"
"#,
        );
        assert_eq!(output, vec!["Hello world!"]);
    }

    #[test]
    fn test_nested_function_calls() {
        let output = run(
            r#"
def double(x) {
    return x * 2
}
def add_one(x) {
    return x + 1
}
print add_one(double(5))
"#,
        );
        assert_eq!(output, vec!["11"]);
    }

    #[test]
    fn test_builtin_len() {
        let output = run(
            r#"
x = [1, 2, 3, 4, 5]
print len(x)
"#,
        );
        assert_eq!(output, vec!["5"]);
    }

    #[test]
    fn test_builtin_type() {
        let output = run(r#"print type(42)"#);
        assert_eq!(output, vec!["number"]);
    }

    #[test]
    fn test_boolean_logic() {
        let output = run("print true and false");
        assert_eq!(output, vec!["false"]);
    }

    #[test]
    fn test_not_operator() {
        let output = run("print not false");
        assert_eq!(output, vec!["true"]);
    }

    #[test]
    fn test_comparison() {
        let output = run("print 5 == 5");
        assert_eq!(output, vec!["true"]);
    }

    #[test]
    fn test_division_by_zero() {
        let err = run_err("print 10 / 0");
        match err {
            KelpyError::RuntimeError { message } => {
                assert!(message.contains("Division by zero"));
            }
            other => panic!("Expected RuntimeError, got: {:?}", other),
        }
    }

    #[test]
    fn test_undefined_variable() {
        let err = run_err("print xyz");
        match err {
            KelpyError::RuntimeError { message } => {
                assert!(message.contains("Undefined variable"));
            }
            other => panic!("Expected RuntimeError, got: {:?}", other),
        }
    }

    #[test]
    fn test_wrong_arity() {
        let err = run_err(
            r#"
def foo(a, b) {
    return a + b
}
foo(1)
"#,
        );
        match err {
            KelpyError::RuntimeError { message } => {
                assert!(message.contains("expects 2 arguments"));
            }
            other => panic!("Expected RuntimeError, got: {:?}", other),
        }
    }

    #[test]
    fn test_full_example_from_spec() {
        let source = r#"
bob = {
    "age": "27 years",
    "name": "Bob Smith"
}

example_list = ["apple", "banana", "orange"]

def example_function(value, thing) {
    print "You have {$value} {$thing}s!"

    if value >= 25 {
        print "You lost. You have " + value + " of " + thing
    }
}

example_function(30, "point")
"#;
        let output = run(source);
        assert_eq!(output.len(), 2);
        assert_eq!(output[0], "You have 30 points!");
        assert_eq!(output[1], "You lost. You have 30 of point");
    }
}
