use std::{fmt::Display, ops::{Index, IndexMut}};

use ratatui::{
    backend::Backend,
    style::{Style, Color},
    Frame,
};

use crate::tree_widget::{TreeState, TreeItem, Tree};

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

    fn get_node_from_id(&self, id: &[usize]) -> &NavNode {
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

    fn get_node_from_id_mut(&mut self, id: &[usize]) -> &mut NavNode {
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

    fn get_tree(&self) -> TreeItem {
        if self.children.is_empty() {
            TreeItem::new_leaf(self.string.as_str())
        }
        else {
            let children: Vec<_> = self.children.iter()
                .map(|n| n.get_tree())
                .collect();

            TreeItem::new(self.string.as_str(), children)
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
    pub fn ui<B: Backend>(&self, f: &mut Frame<B>, state: &mut TreeState) {
        let tree = Tree::new(self.get_tree())
            .highlight_style(
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::LightGreen)
            )
            .highlight_symbol("> ");
        f.render_stateful_widget(tree, f.size(), state);
    }

    /// Return a `Vec` representing the zeroth level nodes of `self`, as `TreeItem` for the tree
    /// widget representation.
    pub fn get_tree(&self) -> Vec<TreeItem> {
        self.nodes.iter()
                .map(|node| node.get_tree())
                .collect()
    }
}

impl Index<&[usize]> for Nav {
    type Output = NavNode;

    fn index(&self, index: &[usize]) -> &Self::Output {
        if index.len() == 0 {
            panic!("Node ID cannot be empty!");
        }
        if index[0] >= self.nodes.len() {
            panic!("Node with ID does not exist");
        }
        self.nodes[index[0]].get_node_from_id(&index[1..])
    }
}

impl IndexMut<&[usize]> for Nav {
    fn index_mut(&mut self, index: &[usize]) -> &mut Self::Output {
        if index.len() == 0 {
            panic!("Node ID cannot be empty!");
        }
        if index[0] >= self.nodes.len() {
            panic!("Node with ID does not exist");
        }
        self.nodes[index[0]].get_node_from_id_mut(&index[1..])
    }
}
