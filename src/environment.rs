/// KelpyShark Environment (variable scoping).
///
/// Implements a chain of scopes. Each scope has a parent scope
/// for lexical scoping (functions, blocks).

use std::collections::HashMap;

use crate::value::Value;

#[derive(Debug, Clone)]
pub struct Environment {
    /// Stack of scopes. Last = innermost scope.
    scopes: Vec<HashMap<String, Value>>,
}

impl Environment {
    /// Create a new environment with one global scope.
    pub fn new() -> Self {
        Environment {
            scopes: vec![HashMap::new()],
        }
    }

    /// Push a new scope (entering a function/block).
    pub fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    /// Pop the innermost scope (leaving a function/block).
    pub fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    /// Set a variable in the current (innermost) scope.
    pub fn set(&mut self, name: &str, value: Value) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.to_string(), value);
        }
    }

    /// Update a variable in the nearest enclosing scope that contains it.
    /// If not found in any scope, set it in the current scope.
    pub fn update(&mut self, name: &str, value: Value) {
        // Search from innermost to outermost
        for scope in self.scopes.iter_mut().rev() {
            if scope.contains_key(name) {
                scope.insert(name.to_string(), value);
                return;
            }
        }
        // Not found anywhere — set in current scope
        self.set(name, value);
    }

    /// Get a variable, searching from innermost to outermost scope.
    pub fn get(&self, name: &str) -> Option<&Value> {
        for scope in self.scopes.iter().rev() {
            if let Some(val) = scope.get(name) {
                return Some(val);
            }
        }
        None
    }
}

impl Default for Environment {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_and_get() {
        let mut env = Environment::new();
        env.set("x", Value::Number(42.0));
        assert_eq!(env.get("x"), Some(&Value::Number(42.0)));
    }

    #[test]
    fn test_undefined_variable() {
        let env = Environment::new();
        assert_eq!(env.get("y"), None);
    }

    #[test]
    fn test_scope_shadowing() {
        let mut env = Environment::new();
        env.set("x", Value::Number(1.0));
        env.push_scope();
        env.set("x", Value::Number(2.0));
        assert_eq!(env.get("x"), Some(&Value::Number(2.0)));
        env.pop_scope();
        assert_eq!(env.get("x"), Some(&Value::Number(1.0)));
    }

    #[test]
    fn test_inner_scope_sees_outer() {
        let mut env = Environment::new();
        env.set("global", Value::String("hello".to_string()));
        env.push_scope();
        assert_eq!(
            env.get("global"),
            Some(&Value::String("hello".to_string()))
        );
        env.pop_scope();
    }

    #[test]
    fn test_update_in_outer_scope() {
        let mut env = Environment::new();
        env.set("x", Value::Number(1.0));
        env.push_scope();
        env.update("x", Value::Number(99.0));
        env.pop_scope();
        assert_eq!(env.get("x"), Some(&Value::Number(99.0)));
    }
}
