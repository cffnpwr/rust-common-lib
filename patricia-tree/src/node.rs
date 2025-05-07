use std::collections::BTreeMap;
use std::fmt::Debug;
use std::mem;
use std::str::Bytes;

use thiserror::Error;

pub trait IntoLabel<T>
where
    T: Ord,
{
    fn into_label(&self) -> Box<[T]>;
}
impl IntoLabel<u8> for &str {
    fn into_label(&self) -> Box<[u8]> {
        self.bytes().collect::<Vec<u8>>().into_boxed_slice()
    }
}
impl IntoLabel<u8> for String {
    fn into_label(&self) -> Box<[u8]> {
        self.bytes().collect::<Vec<u8>>().into_boxed_slice()
    }
}
impl IntoLabel<u8> for Bytes<'_> {
    fn into_label(&self) -> Box<[u8]> {
        self.clone()
            .into_iter()
            .collect::<Vec<u8>>()
            .into_boxed_slice()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Operation {
    Insert,
    Update,
    Upsert,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PatriciaError {
    #[error("The same label value already exists.")]
    AlreadyExists,
    #[error("The node was not found.")]
    NodeNotFound,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatriciaNode<T: Ord, V> {
    label: Box<[T]>,
    children: BTreeMap<T, PatriciaNode<T, V>>,
    value: Option<V>,
}
impl<T, V> PatriciaNode<T, V>
where
    T: Ord + Clone + Debug,
    V: Clone + Debug,
{
    pub fn new() -> Self {
        PatriciaNode {
            label: Box::new([]),
            children: BTreeMap::new(),
            value: None,
        }
    }

    pub fn new_with_label(label: &[T]) -> Self {
        PatriciaNode {
            label: label.into(),
            children: BTreeMap::new(),
            value: None,
        }
    }

    pub fn new_with_label_and_value(label: &[T], value: &V) -> Self {
        PatriciaNode {
            label: label.into(),
            children: BTreeMap::new(),
            value: Some(value.clone()),
        }
    }

    pub fn label(&self) -> &[T] {
        &self.label
    }

    pub fn children(&self) -> &BTreeMap<T, PatriciaNode<T, V>> {
        &self.children
    }

    pub fn value(&self) -> Option<&V> {
        self.value.as_ref()
    }

    fn split_common_prefix<'a>(&self, label: &'a [T]) -> (&'a [T], &'a [T], usize) {
        let common_prefix_len = self
            .label
            .iter()
            .zip(label.iter())
            .take_while(|(a, b)| a == b)
            .count();
        let (common_prefix, suffix) = label.split_at(common_prefix_len);

        (common_prefix, suffix, common_prefix_len)
    }

    fn internal_insert(
        &mut self,
        key: &[T],
        value: &V,
        operation: Operation,
    ) -> Result<(), PatriciaError> {
        let (common_prefix, insert_suffix, common_prefix_len) = self.split_common_prefix(key);
        let label_suffix: Box<[T]> = self.label[common_prefix_len..].into();

        if label_suffix.is_empty() {
            if insert_suffix.is_empty() {
                match operation {
                    Operation::Insert => return Err(PatriciaError::AlreadyExists),
                    _ => {
                        self.value = Some(value.clone());
                        return Ok(());
                    }
                }
            }

            if let Some(child) = self.children.get_mut(&insert_suffix[0]) {
                return child.internal_insert(insert_suffix, value, operation);
            }
            if operation != Operation::Update {
                let new_node = Self::new_with_label_and_value(insert_suffix, value);
                if self.children.is_empty() {
                    if self.value.is_none() {
                        let _ = mem::replace(self, new_node);
                    } else {
                        self.children = BTreeMap::new();
                        self.children.insert(insert_suffix[0].clone(), new_node);
                    }
                } else {
                    self.children.insert(insert_suffix[0].clone(), new_node);
                }
            }
        } else if operation != Operation::Update {
            let label_suffix_head = label_suffix[0].clone();
            let new_node = Self {
                label: label_suffix,
                children: mem::replace(&mut self.children, BTreeMap::new()),
                value: mem::replace(&mut self.value, Some(value.clone())),
            };
            self.label = common_prefix.into();
            self.children.insert(label_suffix_head, new_node);

            if insert_suffix.len() > 0 {
                let insert_node = Self::new_with_label_and_value(insert_suffix, value);
                self.children.insert(insert_suffix[0].clone(), insert_node);
                self.value = None;
            }
        } else {
            return Err(PatriciaError::NodeNotFound);
        }
        Ok(())
    }

    pub fn insert(&mut self, key: &[T], value: &V) -> Result<(), PatriciaError> {
        self.internal_insert(key, value, Operation::Insert)
    }

    pub fn update(&mut self, key: &[T], value: &V) -> Result<(), PatriciaError> {
        self.internal_insert(key, value, Operation::Update)
    }

    pub fn upsert(&mut self, key: &[T], value: &V) -> Result<(), PatriciaError> {
        self.internal_insert(key, value, Operation::Upsert)
    }

    pub fn search(&self, key: &[T]) -> Option<V> {
        let (_, suffix, common_prefix_len) = self.split_common_prefix(key);
        if common_prefix_len == 0 {
            return None;
        }
        if common_prefix_len == key.len() {
            return self.value.clone();
        }
        if let Some(child) = self.children.get(&key[common_prefix_len]) {
            return child.search(suffix);
        }
        None
    }

    pub fn remove(&mut self, key: &[T]) -> Result<(), PatriciaError> {
        let (_, suffix, common_prefix_len) = self.split_common_prefix(key);
        if common_prefix_len == 0 {
            return Err(PatriciaError::NodeNotFound);
        }

        if common_prefix_len == key.len() {
            if self.value.is_none() || !self.label[common_prefix_len..].is_empty() {
                return Err(PatriciaError::NodeNotFound);
            }
            self.value = None;
        } else if let Some(child) = self.children.get_mut(&key[common_prefix_len]) {
            child.remove(suffix)?;
            if child.value.is_none() && child.children.is_empty() {
                self.children.remove(&key[common_prefix_len]);
            }
        }

        if self.value.is_none() && self.children.len() == 1 {
            let (_, mut child) = self.children.pop_first().unwrap();
            let new_label = [self.label.clone(), child.label.clone()].concat();
            self.label = new_label.into_boxed_slice();
            self.value = child.value.take();
            self.children = mem::take(&mut child.children);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;

    use super::*;

    #[test]
    fn test_insert() -> Result<()> {
        let mut node = PatriciaNode::new();

        // 空のノード
        let expected = PatriciaNode {
            label: Box::new([]),
            children: BTreeMap::new(),
            value: None,
        };
        assert_eq!(node, expected);

        // `tea`を挿入する
        let tea = String::from("tea");
        let expected = PatriciaNode {
            label: tea.as_bytes().into(),
            children: BTreeMap::new(),
            value: Some(tea.clone()),
        };
        let result = node.insert(tea.as_bytes(), &tea);
        assert!(result.is_ok());
        assert_eq!(node, expected);

        // `team`を挿入する
        let team = String::from("team");
        let expected = PatriciaNode {
            label: tea.as_bytes().into(),
            children: {
                let mut children = BTreeMap::new();
                children.insert(
                    b'm',
                    PatriciaNode {
                        label: Box::new([b'm']),
                        children: BTreeMap::new(),
                        value: Some(team.clone()),
                    },
                );
                children
            },
            value: Some(tea.clone()),
        };
        let result = node.insert(team.as_bytes(), &team);
        assert!(result.is_ok());
        assert_eq!(node, expected);

        // `t`を挿入する
        let t = String::from("t");
        let expected = PatriciaNode {
            label: Box::new([b't']),
            children: {
                let mut children = BTreeMap::new();
                children.insert(
                    b'e',
                    PatriciaNode {
                        label: Box::new([b'e', b'a']),
                        children: {
                            let mut children = BTreeMap::new();
                            children.insert(
                                b'm',
                                PatriciaNode {
                                    label: Box::new([b'm']),
                                    children: BTreeMap::new(),
                                    value: Some(team.clone()),
                                },
                            );
                            children
                        },
                        value: Some(tea.clone()),
                    },
                );
                children
            },
            value: Some(t.clone()),
        };
        let result = node.insert(t.as_bytes(), &t);
        assert!(result.is_ok());
        assert_eq!(node, expected);

        // `teapot`を挿入する
        let teapot = String::from("teapot");
        let expected = PatriciaNode {
            label: Box::new([b't']),
            children: {
                let mut children = BTreeMap::new();
                children.insert(
                    b'e',
                    PatriciaNode {
                        label: Box::new([b'e', b'a']),
                        children: {
                            let mut children = BTreeMap::new();
                            children.insert(
                                b'm',
                                PatriciaNode {
                                    label: Box::new([b'm']),
                                    children: BTreeMap::new(),
                                    value: Some(team.clone()),
                                },
                            );
                            children.insert(
                                b'p',
                                PatriciaNode {
                                    label: Box::new([b'p', b'o', b't']),
                                    children: BTreeMap::new(),
                                    value: Some(teapot.clone()),
                                },
                            );
                            children
                        },
                        value: Some(tea.clone()),
                    },
                );
                children
            },
            value: Some(t.clone()),
        };
        let result = node.insert(teapot.as_bytes(), &teapot);
        assert!(result.is_ok());
        assert_eq!(node, expected);

        // `teapod`を挿入する
        let teapod = String::from("teapod");
        let expected = PatriciaNode {
            label: Box::new([b't']),
            children: {
                let mut children = BTreeMap::new();
                children.insert(
                    b'e',
                    PatriciaNode {
                        label: Box::new([b'e', b'a']),
                        children: {
                            let mut children = BTreeMap::new();
                            children.insert(
                                b'm',
                                PatriciaNode {
                                    label: Box::new([b'm']),
                                    children: BTreeMap::new(),
                                    value: Some(team.clone()),
                                },
                            );
                            children.insert(
                                b'p',
                                PatriciaNode {
                                    label: Box::new([b'p', b'o']),
                                    children: {
                                        let mut children = BTreeMap::new();
                                        children.insert(
                                            b't',
                                            PatriciaNode {
                                                label: Box::new([b't']),
                                                children: BTreeMap::new(),
                                                value: Some(teapot.clone()),
                                            },
                                        );
                                        children.insert(
                                            b'd',
                                            PatriciaNode {
                                                label: Box::new([b'd']),
                                                children: BTreeMap::new(),
                                                value: Some(teapod.clone()),
                                            },
                                        );
                                        children
                                    },
                                    value: None,
                                },
                            );
                            children
                        },
                        value: Some(tea.clone()),
                    },
                );
                children
            },
            value: Some(t.clone()),
        };
        let result = node.insert(teapod.as_bytes(), &teapod);
        assert!(result.is_ok());
        assert_eq!(node, expected);

        // ラベル重複時エラー
        let result = node.insert(teapot.as_bytes(), &teapot);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), PatriciaError::AlreadyExists);

        Ok(())
    }

    #[test]
    fn test_update() -> Result<()> {
        let mut node = PatriciaNode::new();

        // `tea`を挿入する
        let tea = String::from("tea");
        node.insert(tea.as_bytes(), &tea).unwrap();
        // `team`を挿入する
        let team = String::from("team");
        node.insert(team.as_bytes(), &team).unwrap();
        // `t`を挿入する
        let t = String::from("t");
        node.insert(t.as_bytes(), &t).unwrap();
        // `teapot`を挿入する
        let teapot = String::from("teapot");
        node.insert(teapot.as_bytes(), &teapot).unwrap();
        // `teapod`を挿入する
        let teapod = String::from("teapod");
        node.insert(teapod.as_bytes(), &teapod).unwrap();

        // 更新テスト
        let new_tea = String::from("new_tea");
        let result = node.update(tea.as_bytes(), &new_tea);
        assert!(result.is_ok());

        let new_unknown = String::from("new_unknown");
        let result = node.update(b"unknown", &new_unknown);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), PatriciaError::NodeNotFound);

        Ok(())
    }

    #[test]
    fn test_upsert() -> Result<()> {
        let mut node = PatriciaNode::new();

        // `tea`を挿入する
        let tea = String::from("tea");
        node.insert(tea.as_bytes(), &tea).unwrap();
        // `team`を挿入する
        let team = String::from("team");
        node.insert(team.as_bytes(), &team).unwrap();
        // `t`を挿入する
        let t = String::from("t");
        node.insert(t.as_bytes(), &t).unwrap();
        // `teapot`を挿入する
        let teapot = String::from("teapot");
        node.insert(teapot.as_bytes(), &teapot).unwrap();
        // `teapod`を挿入する
        let teapod = String::from("teapod");
        node.insert(teapod.as_bytes(), &teapod).unwrap();

        // 更新テスト
        let new_tea = String::from("new_tea");
        let result = node.upsert(tea.as_bytes(), &new_tea);
        assert!(result.is_ok());

        let new_unknown = String::from("new_unknown");
        let result = node.upsert(b"unknown", &new_unknown);
        assert!(result.is_ok());

        Ok(())
    }
    #[test]
    fn test_search() -> Result<()> {
        let mut node = PatriciaNode::new();

        // `tea`を挿入する
        let tea = String::from("tea");
        node.insert(tea.as_bytes(), &tea).unwrap();
        // `team`を挿入する
        let team = String::from("team");
        node.insert(team.as_bytes(), &team).unwrap();
        // `t`を挿入する
        let t = String::from("t");
        node.insert(t.as_bytes(), &t).unwrap();
        // `teapot`を挿入する
        let teapot = String::from("teapot");
        node.insert(teapot.as_bytes(), &teapot).unwrap();
        // `teapod`を挿入する
        let teapod = String::from("teapod");
        node.insert(teapod.as_bytes(), &teapod).unwrap();

        // 検索テスト
        assert_eq!(node.search(b"tea"), Some(tea.clone()));
        assert_eq!(node.search(b"team"), Some(team.clone()));
        assert_eq!(node.search(b"t"), Some(t.clone()));
        assert_eq!(node.search(b"teapot"), Some(teapot.clone()));
        assert_eq!(node.search(b"teapod"), Some(teapod.clone()));
        assert_eq!(node.search(b"teaa"), None);
        assert_eq!(node.search(b"unknown"), None);

        Ok(())
    }

    #[test]
    fn test_remove() -> Result<()> {
        let mut node = PatriciaNode::new();

        // `tea`を挿入する
        let tea = String::from("tea");
        node.insert(tea.as_bytes(), &tea).unwrap();
        // `team`を挿入する
        let team = String::from("team");
        node.insert(team.as_bytes(), &team).unwrap();
        // `t`を挿入する
        let t = String::from("t");
        node.insert(t.as_bytes(), &t).unwrap();
        // `teapot`を挿入する
        let teapot = String::from("teapot");
        node.insert(teapot.as_bytes(), &teapot).unwrap();
        // `teapod`を挿入する
        let teapod = String::from("teapod");
        node.insert(teapod.as_bytes(), &teapod).unwrap();

        let expected = PatriciaNode {
            label: Box::new([b't']),
            children: {
                let mut children = BTreeMap::new();
                children.insert(
                    b'e',
                    PatriciaNode {
                        label: Box::new([b'e', b'a']),
                        children: {
                            let mut children = BTreeMap::new();
                            children.insert(
                                b'm',
                                PatriciaNode {
                                    label: Box::new([b'm']),
                                    children: BTreeMap::new(),
                                    value: Some(team.clone()),
                                },
                            );
                            children.insert(
                                b'p',
                                PatriciaNode {
                                    label: Box::new([b'p', b'o']),
                                    children: {
                                        let mut children = BTreeMap::new();
                                        children.insert(
                                            b't',
                                            PatriciaNode {
                                                label: Box::new([b't']),
                                                children: BTreeMap::new(),
                                                value: Some(teapot.clone()),
                                            },
                                        );
                                        children.insert(
                                            b'd',
                                            PatriciaNode {
                                                label: Box::new([b'd']),
                                                children: BTreeMap::new(),
                                                value: Some(teapod.clone()),
                                            },
                                        );
                                        children
                                    },
                                    value: None,
                                },
                            );
                            children
                        },
                        value: Some(tea.clone()),
                    },
                );
                children
            },
            value: Some(t.clone()),
        };
        assert_eq!(node, expected);

        // `teapot`を削除する
        let result = node.remove(b"teapot");
        let expected = PatriciaNode {
            label: Box::new([b't']),
            children: {
                let mut children = BTreeMap::new();
                children.insert(
                    b'e',
                    PatriciaNode {
                        label: Box::new([b'e', b'a']),
                        children: {
                            let mut children = BTreeMap::new();
                            children.insert(
                                b'm',
                                PatriciaNode {
                                    label: Box::new([b'm']),
                                    children: BTreeMap::new(),
                                    value: Some(team.clone()),
                                },
                            );
                            children.insert(
                                b'p',
                                PatriciaNode {
                                    label: Box::new([b'p', b'o', b'd']),
                                    children: BTreeMap::new(),
                                    value: Some(teapod.clone()),
                                },
                            );
                            children
                        },
                        value: Some(tea.clone()),
                    },
                );
                children
            },
            value: Some(t.clone()),
        };
        assert!(result.is_ok());
        assert_eq!(node, expected);

        // `t`を削除する
        let result = node.remove(b"t");
        let expected = PatriciaNode {
            label: Box::new([b't', b'e', b'a']),
            children: {
                let mut children = BTreeMap::new();
                children.insert(
                    b'm',
                    PatriciaNode {
                        label: Box::new([b'm']),
                        children: BTreeMap::new(),
                        value: Some(team.clone()),
                    },
                );
                children.insert(
                    b'p',
                    PatriciaNode {
                        label: Box::new([b'p', b'o', b'd']),
                        children: BTreeMap::new(),
                        value: Some(teapod.clone()),
                    },
                );
                children
            },
            value: Some(tea.clone()),
        };
        assert!(result.is_ok());
        assert_eq!(node, expected);

        // `t`を削除する
        let result = node.remove(b"t");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), PatriciaError::NodeNotFound);
        assert_eq!(node, expected);

        // `tea`を削除する
        let result = node.remove(b"tea");
        let expected = PatriciaNode {
            label: Box::new([b't', b'e', b'a']),
            children: {
                let mut children = BTreeMap::new();
                children.insert(
                    b'm',
                    PatriciaNode {
                        label: Box::new([b'm']),
                        children: BTreeMap::new(),
                        value: Some(team.clone()),
                    },
                );
                children.insert(
                    b'p',
                    PatriciaNode {
                        label: Box::new([b'p', b'o', b'd']),
                        children: BTreeMap::new(),
                        value: Some(teapod.clone()),
                    },
                );
                children
            },
            value: None,
        };
        assert!(result.is_ok());
        assert_eq!(node, expected);

        // `unknown`を削除する
        let result = node.remove(b"unknown");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), PatriciaError::NodeNotFound);
        assert_eq!(node, expected);

        Ok(())
    }
}
