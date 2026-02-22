use std::collections::HashMap;

/// Maps field paths (e.g. `"user.profile.age"`) to flat integer indices.
///
/// Built during compilation by collecting all field paths referenced in rule expressions.
/// Used by `ContextBuilder` to place values at the correct index in a `Vec<Option<Value>>`.
#[derive(Debug, Clone)]
pub struct FieldRegistry {
    paths: HashMap<String, usize>,
    len: usize,
}

impl FieldRegistry {
    #[cfg(feature = "binary-cache")]
    pub(crate) fn from_pairs(pairs: Vec<(String, usize)>) -> Self {
        let len = pairs.iter().map(|(_, idx)| idx + 1).max().unwrap_or(0);
        let paths = pairs.into_iter().collect();
        Self { paths, len }
    }

    pub(crate) fn new() -> Self {
        Self {
            paths: HashMap::new(),
            len: 0,
        }
    }

    /// Register a field path, returning its index. If the path is already registered,
    /// returns the existing index.
    pub(crate) fn register(&mut self, path: &str) -> usize {
        if let Some(&idx) = self.paths.get(path) {
            return idx;
        }
        let idx = self.len;
        self.paths.insert(path.to_owned(), idx);
        self.len += 1;
        idx
    }

    /// Look up the index for a field path.
    #[must_use]
    pub fn get(&self, path: &str) -> Option<usize> {
        self.paths.get(path).copied()
    }

    /// The number of registered fields.
    #[must_use]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the registry is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Iterate over all registered (path, index) pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &usize)> {
        self.paths.iter().map(|(k, v)| (k.as_str(), v))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_and_get() {
        let mut reg = FieldRegistry::new();
        let idx = reg.register("user.age");
        assert_eq!(idx, 0);
        assert_eq!(reg.get("user.age"), Some(0));
    }

    #[test]
    fn duplicate_register_returns_same_index() {
        let mut reg = FieldRegistry::new();
        let idx1 = reg.register("user.age");
        let idx2 = reg.register("user.age");
        assert_eq!(idx1, idx2);
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn multiple_fields() {
        let mut reg = FieldRegistry::new();
        let a = reg.register("user.age");
        let b = reg.register("user.status");
        let c = reg.register("request.region");
        assert_eq!(a, 0);
        assert_eq!(b, 1);
        assert_eq!(c, 2);
        assert_eq!(reg.len(), 3);
    }

    #[test]
    fn get_missing_returns_none() {
        let reg = FieldRegistry::new();
        assert_eq!(reg.get("nonexistent"), None);
    }

    #[test]
    fn empty_registry() {
        let reg = FieldRegistry::new();
        assert!(reg.is_empty());
        assert_eq!(reg.len(), 0);
    }
}
