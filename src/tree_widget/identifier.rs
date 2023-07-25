#![allow(clippy::module_name_repetitions)]

/// Reference to a [`TreeItem`](crate::TreeItem) in a [`Tree`](crate::Tree)
pub type TreeIdentifier<'a> = &'a [usize];
/// Reference to a [`TreeItem`](crate::TreeItem) in a [`Tree`](crate::Tree)
pub type TreeIdentifierVec = Vec<usize>;
