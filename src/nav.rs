use std::{fmt::Display, ops::{Index, IndexMut}};

use ratatui::{
    style::{Style, Color},
    Frame,
};

use crate::tree_widget::{TreeState, TreeItem, Tree, TreeIdentifier, TreeView};

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum BookmarkLink {
    PageNumber(u32),
    PageLink(String),
}

impl BookmarkLink {
    pub fn from_string(input: &str) -> Self {
        if let Ok(num) = input.trim().parse() {
            Self::PageNumber(num)
        }
        else {
            Self::PageLink(String::from(input))
        }
    }
}

impl Display for BookmarkLink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            Self::PageNumber(x) => write!(f, "{}", x),
            Self::PageLink(s) => write!(f, "{}", s),
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct NavNode {
    pub string: String,
    pub link: BookmarkLink,
    pub children: Vec<NavNode>,
}

fn escape_characters(input: String) -> String {
    let mut res = String::new();
    for c in input.chars() {
        if c == '"' {
            res.push('\\');
        }
        res.push(c);
    }
    res
}

impl NavNode {
    fn to_djvu(&self, depth: usize) -> String {
        let depth_space = " ".repeat(depth);
        let first_line = format!("{}(\"{}\"", depth_space, escape_characters(self.string.clone()));
        let second_line_beg = format!("{} \"#{}\"", depth_space, self.link);
        if self.children.is_empty() {
            format!("{}\n{} )", first_line, second_line_beg)
        }
        else {
            let mut s = format!("{}\n{}", first_line, second_line_beg);
            for child in &self.children {
                let child_s = child.to_djvu(depth + 1);
                s.push_str(&format!("\n{}", child_s));
            }
            s.push_str(" )");
            s
        }
    }

    fn get_node_from_id(&self, id: TreeIdentifier) -> &NavNode {
        if id.is_empty() {
            &self
        }
        else if id[0] >= self.children.len() {
            panic!("Node ID does not exist");
        }
        else {
            self.children[id[0]].get_node_from_id(&id[1..])
        }
    }

    fn get_node_from_id_mut(&mut self, id: TreeIdentifier) -> &mut NavNode {
        if id.is_empty() {
            self
        }
        else if id[0] >= self.children.len() {
            panic!("Node ID does not exist");
        }
        else {
            self.children[id[0]].get_node_from_id_mut(&id[1..])
        }
    }

    fn new_child(&mut self, node_id: TreeIdentifier) {
        if node_id.is_empty() {
            self.children.insert(0, NavNode::default());
        } else {
            self.children[node_id[0]].new_child(&node_id[1..]);
        }
    }

    fn new_sibling_above(&mut self, node_id: TreeIdentifier) {
        if node_id.is_empty() {
            panic!("Node ID cannot be empty");
        } else if node_id.len() == 1 {
            self.children.insert(node_id[0], NavNode::default());
        } else {
            self.children[node_id[0]].new_sibling_above(&node_id[1..]);
        }
    }

    fn new_sibling_below(&mut self, node_id: TreeIdentifier) {
        if node_id.is_empty() {
            panic!("Node ID cannot be empty");
        } else if node_id.len() == 1 {
            self.children.insert(node_id[0] + 1, NavNode::default());
        } else {
            self.children[node_id[0]].new_sibling_above(&node_id[1..]);
        }
    }

    fn delete_entry(&mut self, node_id: TreeIdentifier) {
        if node_id.is_empty() {
            panic!("Node ID cannot be empty");
        } else if node_id.len() == 1 {
            self.children.remove(node_id[0]);
        } else {
            self.children[node_id[0]].delete_entry(&node_id[1..]);
        }
    }
}

impl Default for NavNode {
    fn default() -> Self {
        Self { 
            string: String::new(), 
            link: BookmarkLink::PageNumber(0), 
            children: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Nav {
    pub nodes: Vec<NavNode>,
}

impl Nav {
    /// Return a `String` describing `self` in a way understandable by `djvused`.
    pub fn to_djvu(&self) -> String {
        let mut s = String::from("(bookmarks");
        for node in &self.nodes {
            s.push_str(&format!("\n{}", node.to_djvu(1)));
        }
        s.push_str(" )\n");
        s
    }

    /// Render `self` to the `Frame` `f`, as a tree. Use `state` for persistence of open and
    /// selected nodes.
    pub fn ui(&self, f: &mut Frame, state: &mut TreeState) {
        let tree = Tree::new(self)
            .highlight_style(
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::LightGreen)
            )
            .highlight_symbol("> ");
        f.render_stateful_widget(tree, f.size(), state);
    }

    pub fn new_first_child(&mut self, index: TreeIdentifier) {
        if index.is_empty() {
            self.nodes.insert(0,NavNode::default());
        } else {
            self[index].children.insert(0, NavNode::default());
        }
    }

    pub fn new_sibling_below(&mut self, index: TreeIdentifier) {
        let father = &index[..index.len() - 1];
        if father.is_empty() {
            self.nodes.insert(index[0] + 1, NavNode::default());
        } else {
            self[father].children.insert(index.last().unwrap() + 1, NavNode::default());
        }
    }

    pub fn delete_entry(&mut self, index: TreeIdentifier) {
        if index.is_empty() {
            return;
        }

        let father = &index[..index.len() - 1];
        let last = index[index.len() - 1];

        if father.is_empty() {
            self.nodes.remove(last);
        }
        else {
            self[father].children.remove(last);
        }
    }
}

impl<'a> Into<TreeItem<'a>> for &'a NavNode {
    fn into(self) -> TreeItem<'a> {
        let children: Vec<_> = self.children
            .iter()
            .map(|child| Into::<TreeItem>::into(child))
            .collect();
        TreeItem::new(
            self.string.as_str(), 
            children,
        )
    }
}

impl TreeView for Nav {
    fn num_children(&self, index: TreeIdentifier) -> usize {
        if index.is_empty() {
            self.nodes.len()
        } else {
            self[index].children.len()
        }
    }
}

impl<'a> Into<Vec<TreeItem<'a>>> for &'a Nav {
    fn into(self) -> Vec<TreeItem<'a>> {
        self.nodes.iter()
            .map(|child| child.into()).collect()
    }
}

impl<'a> Index<TreeIdentifier<'a>> for Nav {
    type Output = NavNode;

    fn index(&self, index: TreeIdentifier) -> &Self::Output {
        if index.is_empty() {
            panic!("Trying to get node with empty index");
        }
        if index[0] >= self.nodes.len() {
            panic!("Node with ID does not exist");
        }
        self.nodes[index[0]].get_node_from_id(&index[1..])
    }
}

impl<'a> IndexMut<TreeIdentifier<'a>> for Nav {
    fn index_mut(&mut self, index: TreeIdentifier) -> &mut Self::Output {
        if index.is_empty() {
            panic!("Trying to get node with empty index");
        }
        if index[0] >= self.nodes.len() {
            panic!("Node with ID does not exist");
        }
        self.nodes[index[0]].get_node_from_id_mut(&index[1..])
    }
}
