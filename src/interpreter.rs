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
            if let Signal::Return(_) = signal {
                break; // Top-level return stops execution
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
                        if let Signal::Return(_) = &signal {
                            return Ok(signal);
                        }
                    }
                } else if let Some(else_stmts) = else_body {
                    for s in else_stmts {
                        let signal = self.exec_statement(s)?;
                        if let Signal::Return(_) = &signal {
                            return Ok(signal);
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
                        if let Signal::Return(_) = &signal {
                            return Ok(signal);
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
                        for item in items {
                            self.env.update(variable, item);
                            for s in body {
                                let signal = self.exec_statement(s)?;
                                if let Signal::Return(_) = &signal {
                                    return Ok(signal);
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
