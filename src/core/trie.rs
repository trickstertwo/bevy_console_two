//! Trie data structure for fast prefix lookup.
//!
//! Optimized for ASCII command names with O(k) lookup where k = key length.

use std::collections::HashMap;

/// A trie (prefix tree) for fast string lookup and prefix iteration.
///
/// # Complexity
///
/// - `get`: O(k) where k = key length
/// - `insert`: O(k)
/// - `prefix_iter`: O(k + m) where m = number of matches
///
/// # Examples
///
/// ```
/// use bevy_console::core::Trie;
///
/// let mut trie = Trie::new();
/// trie.insert("sv_gravity", 800);
/// trie.insert("sv_cheats", 0);
/// trie.insert("cl_fov", 90);
///
/// assert_eq!(trie.get("sv_gravity"), Some(&800));
///
/// // Prefix search
/// let sv_vars: Vec<_> = trie.prefix_iter("sv_").collect();
/// assert_eq!(sv_vars.len(), 2);
/// ```
#[derive(Debug, Clone)]
pub struct Trie<V> {
    root: TrieNode<V>,
    len: usize,
}

#[derive(Debug, Clone)]
struct TrieNode<V> {
    children: HashMap<u8, TrieNode<V>>,
    value: Option<V>,
    // Store full key at leaf for iteration
    key: Option<Box<str>>,
}

impl<V> Default for TrieNode<V> {
    fn default() -> Self {
        Self {
            children: HashMap::new(),
            value: None,
            key: None,
        }
    }
}

impl<V> Default for Trie<V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<V> Trie<V> {
    /// Create a new empty trie.
    pub fn new() -> Self {
        Self {
            root: TrieNode::default(),
            len: 0,
        }
    }

    /// Get the number of entries.
    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Check if the trie is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Insert a key-value pair.
    ///
    /// Returns the previous value if the key already existed.
    pub fn insert(&mut self, key: &str, value: V) -> Option<V> {
        let mut node = &mut self.root;

        for &byte in key.as_bytes() {
            node = node.children.entry(byte).or_default();
        }

        let old = node.value.take();
        node.value = Some(value);
        node.key = Some(key.into());

        if old.is_none() {
            self.len += 1;
        }

        old
    }

    /// Get a reference to the value for the given key.
    pub fn get(&self, key: &str) -> Option<&V> {
        let mut node = &self.root;

        for &byte in key.as_bytes() {
            node = node.children.get(&byte)?;
        }

        node.value.as_ref()
    }

    /// Get a mutable reference to the value for the given key.
    pub fn get_mut(&mut self, key: &str) -> Option<&mut V> {
        let mut node = &mut self.root;

        for &byte in key.as_bytes() {
            node = node.children.get_mut(&byte)?;
        }

        node.value.as_mut()
    }

    /// Check if the trie contains the given key.
    pub fn contains(&self, key: &str) -> bool {
        self.get(key).is_some()
    }

    /// Remove a key from the trie.
    ///
    /// Returns the removed value if it existed.
    pub fn remove(&mut self, key: &str) -> Option<V> {
        let mut node = &mut self.root;

        for &byte in key.as_bytes() {
            node = node.children.get_mut(&byte)?;
        }

        if node.value.is_some() {
            self.len -= 1;
            node.key = None;
        }

        node.value.take()
    }

    /// Iterate over all key-value pairs with the given prefix.
    ///
    /// The prefix itself is not required to be a key in the trie.
    pub fn prefix_iter(&self, prefix: &str) -> PrefixIter<'_, V> {
        let mut node = &self.root;

        for &byte in prefix.as_bytes() {
            match node.children.get(&byte) {
                Some(child) => node = child,
                None => {
                    return PrefixIter {
                        stack: Vec::new(),
                    };
                }
            }
        }

        let mut stack = Vec::new();
        stack.push(node);

        PrefixIter { stack }
    }

    /// Iterate over all key-value pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &V)> {
        self.prefix_iter("")
    }

    /// Iterate over all keys.
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.iter().map(|(k, _)| k)
    }

    /// Iterate over all values.
    pub fn values(&self) -> impl Iterator<Item = &V> {
        self.iter().map(|(_, v)| v)
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.root = TrieNode::default();
        self.len = 0;
    }
}

/// Iterator over entries with a common prefix.
pub struct PrefixIter<'a, V> {
    stack: Vec<&'a TrieNode<V>>,
}

impl<'a, V> Iterator for PrefixIter<'a, V> {
    type Item = (&'a str, &'a V);

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(node) = self.stack.pop() {
            // Add children to stack (we'll process them later)
            for child in node.children.values() {
                self.stack.push(child);
            }

            // If this node has a value, return it
            if let (Some(key), Some(value)) = (&node.key, &node.value) {
                return Some((key, value));
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trie_basic() {
        let mut trie = Trie::new();
        assert!(trie.is_empty());

        trie.insert("hello", 1);
        assert_eq!(trie.len(), 1);
        assert!(!trie.is_empty());

        assert_eq!(trie.get("hello"), Some(&1));
        assert_eq!(trie.get("world"), None);
        assert!(trie.contains("hello"));
        assert!(!trie.contains("world"));
    }

    #[test]
    fn test_trie_overwrite() {
        let mut trie = Trie::new();
        assert_eq!(trie.insert("key", 1), None);
        assert_eq!(trie.insert("key", 2), Some(1));
        assert_eq!(trie.get("key"), Some(&2));
        assert_eq!(trie.len(), 1);
    }

    #[test]
    fn test_trie_remove() {
        let mut trie = Trie::new();
        trie.insert("hello", 1);
        trie.insert("world", 2);

        assert_eq!(trie.remove("hello"), Some(1));
        assert_eq!(trie.get("hello"), None);
        assert_eq!(trie.len(), 1);

        assert_eq!(trie.remove("nonexistent"), None);
    }

    #[test]
    fn test_trie_prefix_iter() {
        let mut trie = Trie::new();
        trie.insert("sv_gravity", 800);
        trie.insert("sv_cheats", 0);
        trie.insert("sv_maxrate", 0);
        trie.insert("cl_fov", 90);
        trie.insert("cl_showfps", 0);

        let sv_entries: Vec<_> = trie.prefix_iter("sv_").collect();
        assert_eq!(sv_entries.len(), 3);

        let cl_entries: Vec<_> = trie.prefix_iter("cl_").collect();
        assert_eq!(cl_entries.len(), 2);

        let empty_entries: Vec<_> = trie.prefix_iter("xyz").collect();
        assert!(empty_entries.is_empty());
    }

    #[test]
    fn test_trie_iter() {
        let mut trie = Trie::new();
        trie.insert("a", 1);
        trie.insert("b", 2);
        trie.insert("c", 3);

        let entries: Vec<_> = trie.iter().collect();
        assert_eq!(entries.len(), 3);

        let keys: Vec<_> = trie.keys().collect();
        assert_eq!(keys.len(), 3);
    }

    #[test]
    fn test_trie_get_mut() {
        let mut trie = Trie::new();
        trie.insert("key", 1);

        if let Some(v) = trie.get_mut("key") {
            *v = 42;
        }

        assert_eq!(trie.get("key"), Some(&42));
    }

    #[test]
    fn test_trie_shared_prefix() {
        let mut trie = Trie::new();
        trie.insert("test", 1);
        trie.insert("testing", 2);
        trie.insert("tested", 3);

        assert_eq!(trie.get("test"), Some(&1));
        assert_eq!(trie.get("testing"), Some(&2));
        assert_eq!(trie.get("tested"), Some(&3));
        assert_eq!(trie.get("tes"), None);

        let prefix_entries: Vec<_> = trie.prefix_iter("test").collect();
        assert_eq!(prefix_entries.len(), 3);
    }
}
