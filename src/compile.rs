use std::collections::{HashMap, HashSet, VecDeque};

use crate::{CompileError, CompiledRule, Expr, Rule, RuleSet, Terminal};

pub(crate) fn compile(
    rules: &[Rule],
    mut terminals: Vec<Terminal>,
) -> Result<RuleSet, CompileError> {
    check_duplicates(rules)?;
    check_terminals(&terminals, rules)?;

    let rule_map: HashMap<&str, &Rule> = rules.iter().map(|r| (r.name.as_str(), r)).collect();

    check_references(rules, &rule_map)?;

    let sorted_names = topological_sort(rules, &rule_map)?;

    let rule_indices: HashMap<String, usize> = sorted_names
        .iter()
        .enumerate()
        .map(|(i, name): (usize, &String)| (name.clone(), i))
        .collect();

    let compiled_rules: Vec<CompiledRule> = sorted_names
        .iter()
        .enumerate()
        .map(|(i, name): (usize, &String)| {
            let rule = rule_map[name.as_str()];
            CompiledRule {
                name: rule.name.clone(),
                condition: rule.condition.clone(),
                index: i,
            }
        })
        .collect();

    terminals.sort_by_key(|t| t.priority);

    Ok(RuleSet {
        rules: compiled_rules,
        terminals,
        rule_indices,
    })
}

fn check_duplicates(rules: &[Rule]) -> Result<(), CompileError> {
    let mut seen = HashSet::new();
    for rule in rules {
        if !seen.insert(&rule.name) {
            return Err(CompileError::DuplicateRule {
                name: rule.name.clone(),
            });
        }
    }
    Ok(())
}

fn check_terminals(terminals: &[Terminal], rules: &[Rule]) -> Result<(), CompileError> {
    if terminals.is_empty() {
        return Err(CompileError::NoTerminals);
    }
    let rule_names: HashSet<&str> = rules.iter().map(|r| r.name.as_str()).collect();
    for terminal in terminals {
        if !rule_names.contains(terminal.rule_name.as_str()) {
            return Err(CompileError::UndefinedTerminal {
                terminal: terminal.rule_name.clone(),
            });
        }
    }
    Ok(())
}

fn check_references(rules: &[Rule], rule_map: &HashMap<&str, &Rule>) -> Result<(), CompileError> {
    for rule in rules {
        collect_and_check_refs(&rule.condition, &rule.name, rule_map)?;
    }
    Ok(())
}

fn collect_and_check_refs(
    expr: &Expr,
    rule_name: &str,
    rule_map: &HashMap<&str, &Rule>,
) -> Result<(), CompileError> {
    match expr {
        Expr::RuleRef(name) => {
            if !rule_map.contains_key(name.as_str()) {
                return Err(CompileError::UndefinedRuleRef {
                    rule: rule_name.to_owned(),
                    reference: name.clone(),
                });
            }
            Ok(())
        }
        Expr::And(a, b) | Expr::Or(a, b) => {
            collect_and_check_refs(a, rule_name, rule_map)?;
            collect_and_check_refs(b, rule_name, rule_map)?;
            Ok(())
        }
        Expr::Not(inner) => collect_and_check_refs(inner, rule_name, rule_map),
        Expr::Compare { .. } => Ok(()),
    }
}

/// Kahn's algorithm for topological sort with cycle detection.
fn topological_sort(
    rules: &[Rule],
    rule_map: &HashMap<&str, &Rule>,
) -> Result<Vec<String>, CompileError> {
    let rule_names: HashSet<&str> = rules.iter().map(|r| r.name.as_str()).collect();

    // dependents[X] = list of rules that depend on X (X must be evaluated before them)
    let mut dependents: HashMap<String, Vec<String>> = HashMap::new();
    let mut in_degree: HashMap<String, usize> = HashMap::new();

    for rule in rules {
        in_degree.entry(rule.name.clone()).or_insert(0);
        dependents.entry(rule.name.clone()).or_default();
    }

    for rule in rules {
        let deps = collect_rule_refs(&rule.condition);
        for dep in deps {
            if rule_names.contains(dep.as_str()) {
                dependents
                    .entry(dep.clone())
                    .or_default()
                    .push(rule.name.clone());
                *in_degree.entry(rule.name.clone()).or_insert(0) += 1;
            }
        }
    }

    let mut queue: VecDeque<String> = in_degree
        .iter()
        .filter(|(_, deg)| **deg == 0)
        .map(|(name, _)| name.clone())
        .collect();

    let mut sorted = Vec::new();

    while let Some(name) = queue.pop_front() {
        if let Some(deps) = dependents.get(&name) {
            for dependent in deps {
                if let Some(deg) = in_degree.get_mut(dependent) {
                    *deg -= 1;
                    if *deg == 0 {
                        queue.push_back(dependent.clone());
                    }
                }
            }
        }
        sorted.push(name);
    }

    if sorted.len() != rules.len() {
        let cycle = find_cycle(rules, rule_map);
        return Err(CompileError::CyclicDependency { path: cycle });
    }

    Ok(sorted)
}

fn collect_rule_refs(expr: &Expr) -> Vec<String> {
    let mut refs = Vec::new();
    collect_rule_refs_inner(expr, &mut refs);
    refs
}

fn collect_rule_refs_inner(expr: &Expr, refs: &mut Vec<String>) {
    match expr {
        Expr::RuleRef(name) => refs.push(name.clone()),
        Expr::And(a, b) | Expr::Or(a, b) => {
            collect_rule_refs_inner(a, refs);
            collect_rule_refs_inner(b, refs);
        }
        Expr::Not(inner) => collect_rule_refs_inner(inner, refs),
        Expr::Compare { .. } => {}
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DfsState {
    Unvisited,
    InStack,
    Done,
}

/// DFS-based cycle finder for error reporting.
fn find_cycle(rules: &[Rule], rule_map: &HashMap<&str, &Rule>) -> Vec<String> {
    let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();
    for rule in rules {
        let deps: Vec<&str> = collect_rule_refs(&rule.condition)
            .into_iter()
            .filter(|r| rule_map.contains_key(r.as_str()))
            .map(|r| *rule_map.keys().find(|&&k| k == r.as_str()).unwrap())
            .collect();
        adj.insert(rule.name.as_str(), deps);
    }

    let mut state: HashMap<&str, DfsState> = rules
        .iter()
        .map(|r| (r.name.as_str(), DfsState::Unvisited))
        .collect();
    let mut stack: Vec<&str> = Vec::new();

    for rule in rules {
        let name = rule.name.as_str();
        if state.get(name) == Some(&DfsState::Unvisited)
            && let Some(cycle) = dfs(name, &adj, &mut state, &mut stack)
        {
            return cycle;
        }
    }

    // Shouldn't reach here if called after topo sort detected a cycle
    vec![]
}

fn dfs<'a>(
    node: &'a str,
    adj: &HashMap<&str, Vec<&'a str>>,
    state: &mut HashMap<&'a str, DfsState>,
    stack: &mut Vec<&'a str>,
) -> Option<Vec<String>> {
    state.insert(node, DfsState::InStack);
    stack.push(node);

    if let Some(neighbors) = adj.get(node) {
        for &neighbor in neighbors {
            match state.get(neighbor) {
                Some(DfsState::InStack) => {
                    let pos = stack.iter().position(|&n| n == neighbor).unwrap();
                    let mut cycle: Vec<String> =
                        stack[pos..].iter().map(|&s| s.to_owned()).collect();
                    cycle.push(neighbor.to_owned());
                    return Some(cycle);
                }
                Some(DfsState::Unvisited) | None => {
                    if let Some(cycle) = dfs(neighbor, adj, state, stack) {
                        return Some(cycle);
                    }
                }
                Some(DfsState::Done) => {}
            }
        }
    }

    stack.pop();
    state.insert(node, DfsState::Done);
    None
}

#[cfg(test)]
mod tests {
    use crate::{CompileError, RuleSetBuilder, field, rule_ref};

    #[test]
    fn compile_simple_ruleset() {
        let result = RuleSetBuilder::new()
            .rule("age_check", |r| r.when(field("age").gte(18_i64)))
            .terminal("age_check", 0)
            .compile();
        assert!(result.is_ok());
        let ruleset = result.unwrap();
        assert_eq!(ruleset.rules.len(), 1);
        assert_eq!(ruleset.rules[0].name, "age_check");
    }

    #[test]
    fn compile_duplicate_rule() {
        let result = RuleSetBuilder::new()
            .rule("r1", |r| r.when(field("x").eq(1_i64)))
            .rule("r1", |r| r.when(field("y").eq(2_i64)))
            .terminal("r1", 0)
            .compile();
        assert!(matches!(result, Err(CompileError::DuplicateRule { .. })));
    }

    #[test]
    fn compile_no_terminals() {
        let result = RuleSetBuilder::new()
            .rule("r1", |r| r.when(field("x").eq(1_i64)))
            .compile();
        assert!(matches!(result, Err(CompileError::NoTerminals)));
    }

    #[test]
    fn compile_undefined_terminal() {
        let result = RuleSetBuilder::new()
            .rule("r1", |r| r.when(field("x").eq(1_i64)))
            .terminal("nonexistent", 0)
            .compile();
        assert!(matches!(
            result,
            Err(CompileError::UndefinedTerminal { .. })
        ));
    }

    #[test]
    fn compile_undefined_rule_ref() {
        let result = RuleSetBuilder::new()
            .rule("r1", |r| r.when(rule_ref("nonexistent")))
            .terminal("r1", 0)
            .compile();
        assert!(matches!(result, Err(CompileError::UndefinedRuleRef { .. })));
    }

    #[test]
    fn compile_cycle_detection() {
        let result = RuleSetBuilder::new()
            .rule("a", |r| r.when(rule_ref("b")))
            .rule("b", |r| r.when(rule_ref("a")))
            .terminal("a", 0)
            .compile();
        assert!(matches!(result, Err(CompileError::CyclicDependency { .. })));
    }

    #[test]
    fn compile_diamond_dependency() {
        // A depends on B and C, both B and C depend on D -- no cycle
        let result = RuleSetBuilder::new()
            .rule("d", |r| r.when(field("x").eq(1_i64)))
            .rule("b", |r| r.when(rule_ref("d")))
            .rule("c", |r| r.when(rule_ref("d")))
            .rule("a", |r| r.when(rule_ref("b").and(rule_ref("c"))))
            .terminal("a", 0)
            .compile();
        assert!(result.is_ok());
    }

    #[test]
    fn topo_sort_dependencies_before_dependents() {
        let ruleset = RuleSetBuilder::new()
            .rule("leaf", |r| r.when(field("x").eq(1_i64)))
            .rule("mid", |r| r.when(rule_ref("leaf")))
            .rule("top", |r| r.when(rule_ref("mid")))
            .terminal("top", 0)
            .compile()
            .unwrap();

        let leaf_idx = ruleset.rule_indices["leaf"];
        let mid_idx = ruleset.rule_indices["mid"];
        let top_idx = ruleset.rule_indices["top"];
        assert!(leaf_idx < mid_idx);
        assert!(mid_idx < top_idx);
    }

    #[test]
    fn terminals_sorted_by_priority() {
        let ruleset = RuleSetBuilder::new()
            .rule("r1", |r| r.when(field("x").eq(1_i64)))
            .rule("r2", |r| r.when(field("y").eq(2_i64)))
            .terminal("r2", 10)
            .terminal("r1", 0)
            .compile()
            .unwrap();

        assert_eq!(ruleset.terminals[0].rule_name, "r1");
        assert_eq!(ruleset.terminals[0].priority, 0);
        assert_eq!(ruleset.terminals[1].rule_name, "r2");
        assert_eq!(ruleset.terminals[1].priority, 10);
    }

    #[test]
    fn compile_three_node_cycle() {
        let result = RuleSetBuilder::new()
            .rule("a", |r| r.when(rule_ref("b")))
            .rule("b", |r| r.when(rule_ref("c")))
            .rule("c", |r| r.when(rule_ref("a")))
            .terminal("a", 0)
            .compile();
        match result {
            Err(CompileError::CyclicDependency { path }) => {
                assert!(path.len() >= 3, "cycle path should have at least 3 nodes");
                // The cycle should repeat the starting node at the end
                assert_eq!(path.first(), path.last());
            }
            other => panic!("expected CyclicDependency, got {other:?}"),
        }
    }
}
