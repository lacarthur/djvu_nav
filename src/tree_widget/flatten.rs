use crate::tree_widget::identifier::{TreeIdentifier, TreeIdentifierVec};
use crate::tree_widget::TreeItem;

pub struct Flattened<'a> {
    pub identifier: Vec<usize>,
    pub item: &'a TreeItem<'a>,
}

impl<'a> Flattened<'a> {
    #[must_use]
    pub fn depth(&self) -> usize {
        self.identifier.len() - 1
    }
}

/// Get a flat list of all visible [`TreeItem`s](TreeItem)
#[must_use]
pub fn flatten<'a>(opened: &[TreeIdentifierVec], items: &'a [TreeItem<'a>]) -> Vec<Flattened<'a>> {
    internal(opened, items, &[])
}

#[must_use]
fn internal<'a>(
    opened: &[TreeIdentifierVec],
    items: &'a [TreeItem<'a>],
    current: TreeIdentifier,
) -> Vec<Flattened<'a>> {
    let mut result = Vec::new();

    for (index, item) in items.iter().enumerate() {
        let mut child_identifier = current.to_vec();
        child_identifier.push(index);

        result.push(Flattened {
            item,
            identifier: child_identifier.clone(),
        });

        if opened.contains(&child_identifier) {
            let mut child_result = internal(opened, &item.children, &child_identifier);
            result.append(&mut child_result);
        }
    }
    result
}
