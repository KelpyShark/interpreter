/// KelpyShark Integration Tests
///
/// End-to-end tests that exercise the full pipeline:
///   source → lexer → parser → interpreter
///
/// These tests run actual KelpyShark programs and verify their output.

use kelpyshark_interpreter::interpreter::Interpreter;
use kelpyshark_compiler::lexer::Lexer;
use kelpyshark_compiler::parser::Parser;
use kelpyshark_compiler::semantic::SemanticAnalyzer;

/// Helper: run a KelpyShark program and return captured output lines.
fn run_program(source: &str) -> Vec<String> {
    let mut interp = Interpreter::new_with_buffer();
    interp.exec(source).expect("Program should execute without errors");
    interp.get_output()
}

/// Helper: run and expect an error.
#[allow(dead_code)]
fn run_expect_error(source: &str) -> String {
    let mut interp = Interpreter::new_with_buffer();
    match interp.exec(source) {
        Err(e) => e.to_string(),
        Ok(_) => panic!("Expected an error but program succeeded"),
    }
}

// ══════════════════════════════════════════════════════════════
// Basic output
// ══════════════════════════════════════════════════════════════

#[test]
fn test_hello_world() {
    let output = run_program(r#"print "Hello, World!""#);
    assert_eq!(output, vec!["Hello, World!"]);
}

#[test]
fn test_print_number() {
    let output = run_program("print 42");
    assert_eq!(output, vec!["42"]);
}

#[test]
fn test_print_float() {
    let output = run_program("print 3.14");
    assert_eq!(output, vec!["3.14"]);
}

#[test]
fn test_print_boolean() {
    let output = run_program("print true\nprint false");
    assert_eq!(output, vec!["true", "false"]);
}

// ══════════════════════════════════════════════════════════════
// Variables & arithmetic
// ══════════════════════════════════════════════════════════════

#[test]
fn test_variable_assignment() {
    let output = run_program("x = 10\nprint x");
    assert_eq!(output, vec!["10"]);
}

#[test]
fn test_arithmetic_operations() {
    let output = run_program(r#"
        print 2 + 3
        print 10 - 4
        print 3 * 7
        print 20 / 4
        print 17 % 5
    "#);
    assert_eq!(output, vec!["5", "6", "21", "5", "2"]);
}

#[test]
fn test_string_concatenation() {
    let output = run_program(r#"
        a = "Hello"
        b = " World"
        print a + b
    "#);
    assert_eq!(output, vec!["Hello World"]);
}

#[test]
fn test_string_interpolation() {
    let output = run_program(r#"
        name = "Kelpy"
        print "Hi {$name}!"
    "#);
    assert_eq!(output, vec!["Hi Kelpy!"]);
}

// ══════════════════════════════════════════════════════════════
// Control flow
// ══════════════════════════════════════════════════════════════

#[test]
fn test_if_true_branch() {
    let output = run_program(r#"
        if true {
            print "yes"
        } else {
            print "no"
        }
    "#);
    assert_eq!(output, vec!["yes"]);
}

#[test]
fn test_if_false_branch() {
    let output = run_program(r#"
        if false {
            print "yes"
        } else {
            print "no"
        }
    "#);
    assert_eq!(output, vec!["no"]);
}

#[test]
fn test_while_loop() {
    let output = run_program(r#"
        x = 0
        while x < 5 {
            print x
            x = x + 1
        }
    "#);
    assert_eq!(output, vec!["0", "1", "2", "3", "4"]);
}

#[test]
fn test_for_loop() {
    let output = run_program(r#"
        items = ["a", "b", "c"]
        for item in items {
            print item
        }
    "#);
    assert_eq!(output, vec!["a", "b", "c"]);
}

// ══════════════════════════════════════════════════════════════
// Functions
// ══════════════════════════════════════════════════════════════

#[test]
fn test_function_def_and_call() {
    let output = run_program(r#"
        def greet(name) {
            print "Hello, {$name}!"
        }
        greet("World")
    "#);
    assert_eq!(output, vec!["Hello, World!"]);
}

#[test]
fn test_function_return_value() {
    let output = run_program(r#"
        def add(a, b) {
            return a + b
        }
        result = add(3, 4)
        print result
    "#);
    assert_eq!(output, vec!["7"]);
}

#[test]
fn test_recursive_function() {
    let output = run_program(r#"
        def factorial(n) {
            if n <= 1 {
                return 1
            }
            return n * factorial(n - 1)
        }
        print factorial(5)
    "#);
    assert_eq!(output, vec!["120"]);
}

// ══════════════════════════════════════════════════════════════
// Data structures
// ══════════════════════════════════════════════════════════════

#[test]
fn test_list_creation_and_access() {
    let output = run_program(r#"
        items = [10, 20, 30]
        print items[0]
        print items[2]
    "#);
    assert_eq!(output, vec!["10", "30"]);
}

#[test]
fn test_dict_creation_and_access() {
    let output = run_program(r#"
        person = {"name": "Bob", "age": 25}
        print person["name"]
        print person["age"]
    "#);
    assert_eq!(output, vec!["Bob", "25"]);
}

#[test]
fn test_list_length() {
    let output = run_program(r#"
        items = [1, 2, 3, 4, 5]
        print len(items)
    "#);
    assert_eq!(output, vec!["5"]);
}

// ══════════════════════════════════════════════════════════════
// Built-in functions
// ══════════════════════════════════════════════════════════════

#[test]
fn test_len_string() {
    let output = run_program(r#"print len("hello")"#);
    assert_eq!(output, vec!["5"]);
}

#[test]
fn test_type_function() {
    let output = run_program(r#"
        print type(42)
        print type("hello")
        print type(true)
        print type([1, 2])
    "#);
    assert_eq!(output, vec!["number", "string", "boolean", "list"]);
}

#[test]
fn test_str_function() {
    let output = run_program(r#"
        x = str(42)
        print type(x)
        print x
    "#);
    assert_eq!(output, vec!["string", "42"]);
}

#[test]
fn test_push_function() {
    let output = run_program(r#"
        items = [1, 2]
        items = push(items, 3)
        print len(items)
        print items[2]
    "#);
    assert_eq!(output, vec!["3", "3"]);
}

// ══════════════════════════════════════════════════════════════
// Logical operators
// ══════════════════════════════════════════════════════════════

#[test]
fn test_and_operator() {
    let output = run_program(r#"
        print true and true
        print true and false
    "#);
    assert_eq!(output, vec!["true", "false"]);
}

#[test]
fn test_or_operator() {
    let output = run_program(r#"
        print false or true
        print false or false
    "#);
    assert_eq!(output, vec!["true", "false"]);
}

#[test]
fn test_not_operator() {
    let output = run_program(r#"
        print not true
        print not false
    "#);
    assert_eq!(output, vec!["false", "true"]);
}

// ══════════════════════════════════════════════════════════════
// Comparison operators
// ══════════════════════════════════════════════════════════════

#[test]
fn test_comparison_operators() {
    let output = run_program(r#"
        print 5 > 3
        print 5 < 3
        print 5 >= 5
        print 5 <= 4
        print 5 == 5
        print 5 != 3
    "#);
    assert_eq!(output, vec!["true", "false", "true", "false", "true", "true"]);
}

// ══════════════════════════════════════════════════════════════
// Semantic analysis
// ══════════════════════════════════════════════════════════════

#[test]
fn test_semantic_catches_undefined_var() {
    let source = "print unknown_var";
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    let program = parser.parse().unwrap();
    let result = SemanticAnalyzer::check(&program);
    assert!(result.is_err());
}

#[test]
fn test_semantic_accepts_valid_program() {
    let source = "x = 42\nprint x";
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    let program = parser.parse().unwrap();
    let result = SemanticAnalyzer::check(&program);
    assert!(result.is_ok());
}

// ══════════════════════════════════════════════════════════════
// Code generation
// ══════════════════════════════════════════════════════════════

#[test]
fn test_c_codegen_produces_valid_c() {
    let source = r#"
        x = 42
        print x
        def add(a, b) {
            return a + b
        }
        result = add(1, 2)
        print result
    "#;
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    let program = parser.parse().unwrap();
    let c_code = kelpyshark_compiler::codegen::c::generate(&program);

    assert!(c_code.contains("#include <stdio.h>"));
    assert!(c_code.contains("int main(void)"));
    assert!(c_code.contains("ks_print"));
    assert!(c_code.contains("KsValue ks_add("));
}

#[test]
fn test_js_codegen_produces_valid_js() {
    let source = r#"
        x = 42
        print x
        def greet(name) {
            print "Hello {$name}"
        }
        greet("World")
    "#;
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    let program = parser.parse().unwrap();
    let js_code = kelpyshark_compiler::codegen::javascript::generate(&program);

    assert!(js_code.contains("\"use strict\""));
    assert!(js_code.contains("console.log"));
    assert!(js_code.contains("function greet(name)"));
}

// ══════════════════════════════════════════════════════════════
// Edge cases & complex programs
// ══════════════════════════════════════════════════════════════

#[test]
fn test_nested_if() {
    let output = run_program(r#"
        x = 15
        if x > 10 {
            if x > 20 {
                print "very big"
            } else {
                print "medium"
            }
        } else {
            print "small"
        }
    "#);
    assert_eq!(output, vec!["medium"]);
}

#[test]
fn test_fizzbuzz() {
    let output = run_program(r#"
        i = 1
        while i <= 15 {
            if i % 15 == 0 {
                print "FizzBuzz"
            } else {
                if i % 3 == 0 {
                    print "Fizz"
                } else {
                    if i % 5 == 0 {
                        print "Buzz"
                    } else {
                        print i
                    }
                }
            }
            i = i + 1
        }
    "#);
    assert_eq!(output, vec![
        "1", "2", "Fizz", "4", "Buzz",
        "Fizz", "7", "8", "Fizz", "Buzz",
        "11", "Fizz", "13", "14", "FizzBuzz"
    ]);
}

#[test]
fn test_fibonacci_sequence() {
    let output = run_program(r#"
        def fib(n) {
            if n <= 0 { return 0 }
            if n == 1 { return 1 }
            return fib(n - 1) + fib(n - 2)
        }
        i = 0
        while i < 8 {
            print fib(i)
            i = i + 1
        }
    "#);
    assert_eq!(output, vec!["0", "1", "1", "2", "3", "5", "8", "13"]);
}

#[test]
fn test_closure_like_scope() {
    let output = run_program(r#"
        x = "outer"
        def show() {
            print x
        }
        show()
    "#);
    assert_eq!(output, vec!["outer"]);
}

#[test]
fn test_comments_ignored() {
    let output = run_program(r#"
        # This is a comment
        print "visible"
        ### Multi-line
        comment ###
        print "also visible"
    "#);
    assert_eq!(output, vec!["visible", "also visible"]);
}
