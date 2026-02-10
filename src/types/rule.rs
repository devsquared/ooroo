use super::expr::Expr;

#[derive(Debug, Clone)]
pub struct Rule {
    pub name: String,
    pub condition: Expr,
}

#[derive(Debug, Clone)]
pub struct CompiledRule {
    pub name: String,
    pub condition: Expr,
    pub index: usize,
}

#[derive(Debug, Clone)]
pub struct Terminal {
    pub rule_name: String,
    pub priority: u32,
}
