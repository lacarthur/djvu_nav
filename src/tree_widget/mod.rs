#![forbid(unsafe_code)]

use std::collections::HashSet;

use ratatui::buffer::Buffer;
use ratatui::layout::{Corner, Rect};
use ratatui::style::Style;
use ratatui::text::Text;
use ratatui::widgets::{Block, StatefulWidget, Widget};
use unicode_width::UnicodeWidthStr;

mod flatten;
mod identifier;

pub trait TreeView {
    fn num_children(&self, index: TreeIdentifier) -> usize;
}

#[derive(Debug, Default, Clone)]
pub struct TreeState {
    offset: usize,
    opened: HashSet<TreeIdentifierVec>,
    selected: TreeIdentifierVec,
}

impl TreeState {
    #[must_use]
    pub const fn get_offset(&self) -> usize {
        self.offset
    }

    #[must_use]
    pub fn get_all_opened(&self) -> Vec<TreeIdentifierVec> {
        // Maybe I need to change the signature of this, because sometimes we may not need to
        // clone, so we could return a `Vec<TreeIdentifier<'a>>` where &self outlives 'a. Cloning
        // could be left to the user, if necessary.
        self.opened.iter().cloned().collect()
    }

    #[must_use]
    pub fn is_open(&self, identifier: TreeIdentifier) -> bool {
        if identifier.is_empty() {
            true
        } else {
            self.opened.contains(identifier)
        }
    }

    #[must_use]
    pub fn selected(&self) -> TreeIdentifier {
        &self.selected
    }

    pub fn select<I>(&mut self, identifier: I)
    where
        I: Into<Vec<usize>>,
    {
        self.selected = identifier.into();
    }

    pub fn open(&mut self, identifier: TreeIdentifier) -> bool {
        if identifier.is_empty() {
            false
        } else {
            // potentially unnecessary clone if `identifier` could have been moved.
            self.opened.insert(identifier.into())
        }
    }

    pub fn close(&mut self, identifier: TreeIdentifier) -> bool {
        self.opened.remove(identifier)
    }

    pub fn toggle(&mut self, identifier: TreeIdentifier) {
        if !self.close(identifier) {
            self.open(identifier);
        }
    }

    pub fn toggle_selected(&mut self) {
        let selected = self.selected().to_owned();
        self.toggle(&selected);
    }

    pub fn close_all(&mut self) {
        self.opened.clear();
    }

    pub fn select_first(&mut self) {
        self.select(vec![0]);
    }

    pub fn select_last<T>(&mut self, tree: &T)
    where
        T: TreeView
    {
        let mut index = vec![];
        while self.is_open(&index) && tree.num_children(&index) > 0 {
            let next_value = tree.num_children(&index) - 1;
            index.push(next_value)
        }
        self.selected = index;
    }

    pub fn key_up<T>(&mut self, tree: &T)
    where
        T: TreeView
    {
        if self.selected.is_empty() {
            return;
        }
        if *self.selected.last().unwrap() == 0 {
            self.selected.pop();
        } else {
            let mut index = self.selected.clone();
            *index.last_mut().unwrap() -= 1;
            while self.is_open(&index) && tree.num_children(&index) > 0 {
                let next_value = tree.num_children(&index) - 1;
                index.push(next_value);
            }
            self.selected = index;
        }
    }

    pub fn key_down<T>(&mut self, tree: &T)
    where
        T: TreeView
    {
        if self.selected.is_empty() {
            self.select_first();
            return;
        }

        if self.is_open(&self.selected) && tree.num_children(&self.selected) > 0 {
            self.selected.push(0);
        } else {
            let selected_clone = self.selected.clone();
            let mut father_index = &selected_clone[..self.selected.len() - 1];
            let mut son_index = &selected_clone[..];

            while !father_index.is_empty() {
                if tree.num_children(father_index) - 1 > *son_index.last().unwrap() {
                    self.selected = father_index.to_owned();
                    self.selected.push(*son_index.last().unwrap() + 1);
                    return;
                }
                son_index = father_index;
                father_index = &father_index[..father_index.len() - 1];
            }

            if tree.num_children(&[]) - 1 > son_index[0] {
                self.selected = vec![son_index[0] + 1];
            }
        }
    }

    pub fn key_left(&mut self) {
        let selected = self.selected.clone();
        if !self.close(&selected) {
            self.selected.pop();
        }
    }

    pub fn key_right(&mut self) {
        self.open(&self.selected.clone());
    }
}
pub use flatten::{flatten, Flattened};
pub use identifier::{
    TreeIdentifier, TreeIdentifierVec,
};

/// One item inside a [`Tree`]
///
/// Can have zero or more `children`.
///
/// # Example
///
/// ```
/// # use tui_tree_widget::TreeItem;
/// let a = TreeItem::new_leaf("leaf");
/// let b = TreeItem::new("root", vec![a]);
/// ```
#[derive(Debug, Clone)]
pub struct TreeItem<'a> {
    text: Text<'a>,
    style: Style,
    children: Vec<TreeItem<'a>>,
}

impl<'a> TreeItem<'a> {
    #[must_use]
    pub fn new_leaf<T>(text: T) -> Self
    where
        T: Into<Text<'a>>,
    {
        Self {
            text: text.into(),
            style: Style::default(),
            children: Vec::new(),
        }
    }

    #[must_use]
    pub fn new<T, Children>(text: T, children: Children) -> Self
    where
        T: Into<Text<'a>>,
        Children: Into<Vec<TreeItem<'a>>>,
    {
        Self {
            text: text.into(),
            style: Style::default(),
            children: children.into(),
        }
    }

    #[must_use]
    pub fn children(&self) -> &[TreeItem] {
        &self.children
    }

    #[must_use]
    pub fn child(&self, index: usize) -> Option<&Self> {
        self.children.get(index)
    }

    #[must_use]
    pub fn child_mut(&mut self, index: usize) -> Option<&mut Self> {
        self.children.get_mut(index)
    }

    #[must_use]
    pub fn height(&self) -> usize {
        self.text.height()
    }

    #[must_use]
    pub const fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    pub fn add_child(&mut self, child: TreeItem<'a>) {
        self.children.push(child);
    }
}

/// A `Tree` which can be rendered
///
/// # Example
///
/// ```
/// # use tui_tree_widget::{Tree, TreeItem, TreeState};
/// # use tui::backend::TestBackend;
/// # use tui::Terminal;
/// # use tui::widgets::{Block, Borders};
/// # fn main() -> std::io::Result<()> {
/// #     let mut terminal = Terminal::new(TestBackend::new(32, 32)).unwrap();
/// let mut state = TreeState::default();
///
/// let item = TreeItem::new_leaf("leaf");
/// let items = vec![item];
///
/// terminal.draw(|f| {
///     let area = f.size();
///
///     let tree_widget = Tree::new(items.clone())
///         .block(Block::default().borders(Borders::ALL).title("Tree Widget"));
///
///     f.render_stateful_widget(tree_widget, area, &mut state);
/// })?;
/// #     Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct Tree<'a> {
    items: Vec<TreeItem<'a>>,

    block: Option<Block<'a>>,
    start_corner: Corner,
    /// Style used as a base style for the widget
    style: Style,

    /// Style used to render selected item
    highlight_style: Style,
    /// Symbol in front of the selected item (Shift all items to the right)
    highlight_symbol: &'a str,

    /// Symbol displayed in front of a closed node (As in the children are currently not visible)
    node_closed_symbol: &'a str,
    /// Symbol displayed in front of an open node. (As in the children are currently visible)
    node_open_symbol: &'a str,
    /// Symbol displayed in front of a node without children.
    node_no_children_symbol: &'a str,
}

impl<'a> Tree<'a> {
    #[must_use]
    pub fn new<T>(items: T) -> Self
    where
        T: Into<Vec<TreeItem<'a>>>,
    {
        Self {
            items: items.into(),
            block: None,
            start_corner: Corner::TopLeft,
            style: Style::default(),
            highlight_style: Style::default(),
            highlight_symbol: "",
            node_closed_symbol: "\u{25b6} ", // Arrow to right
            node_open_symbol: "\u{25bc} ",   // Arrow down
            node_no_children_symbol: "  ",
        }
    }

    #[allow(clippy::missing_const_for_fn)]
    #[must_use]
    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }

    #[must_use]
    pub const fn start_corner(mut self, corner: Corner) -> Self {
        self.start_corner = corner;
        self
    }

    #[must_use]
    pub const fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    #[must_use]
    pub const fn highlight_style(mut self, style: Style) -> Self {
        self.highlight_style = style;
        self
    }

    #[must_use]
    pub const fn highlight_symbol(mut self, highlight_symbol: &'a str) -> Self {
        self.highlight_symbol = highlight_symbol;
        self
    }

    #[must_use]
    pub const fn node_closed_symbol(mut self, symbol: &'a str) -> Self {
        self.node_closed_symbol = symbol;
        self
    }

    #[must_use]
    pub const fn node_open_symbol(mut self, symbol: &'a str) -> Self {
        self.node_open_symbol = symbol;
        self
    }

    #[must_use]
    pub const fn node_no_children_symbol(mut self, symbol: &'a str) -> Self {
        self.node_no_children_symbol = symbol;
        self
    }
}

impl<'a> StatefulWidget for Tree<'a> {
    type State = TreeState;

    #[allow(clippy::too_many_lines)]
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        buf.set_style(area, self.style);

        // Get the inner area inside a possible block, otherwise use the full area
        let area = self.block.map_or(area, |b| {
            let inner_area = b.inner(area);
            b.render(area, buf);
            inner_area
        });

        if area.width < 1 || area.height < 1 {
            return;
        }

        let visible = flatten(&state.get_all_opened(), &self.items);
        if visible.is_empty() {
            return;
        }
        let available_height = area.height as usize;

        let selected_index = if state.selected.is_empty() {
            0
        } else {
            visible
                .iter()
                .position(|o| o.identifier == state.selected)
                .unwrap_or(0)
        };

        let mut start = state.offset.min(selected_index);
        let mut end = start;
        let mut height = 0;
        for item in visible.iter().skip(start) {
            if height + item.item.height() > available_height {
                break;
            }

            height += item.item.height();
            end += 1;
        }

        while selected_index >= end {
            height = height.saturating_add(visible[end].item.height());
            end += 1;
            while height > available_height {
                height = height.saturating_sub(visible[start].item.height());
                start += 1;
            }
        }

        state.offset = start;

        let blank_symbol = " ".repeat(self.highlight_symbol.width());

        let mut current_height = 0;
        let has_selection = !state.selected.is_empty();
        #[allow(clippy::cast_possible_truncation)]
        for item in visible.iter().skip(state.offset).take(end - start) {
            #[allow(clippy::single_match_else)] // Keep same as List impl
            let (x, y) = match self.start_corner {
                Corner::BottomLeft => {
                    current_height += item.item.height() as u16;
                    (area.left(), area.bottom() - current_height)
                }
                _ => {
                    let pos = (area.left(), area.top() + current_height);
                    current_height += item.item.height() as u16;
                    pos
                }
            };
            let area = Rect {
                x,
                y,
                width: area.width,
                height: item.item.height() as u16,
            };

            let item_style = self.style.patch(item.item.style);
            buf.set_style(area, item_style);

            let is_selected = state.selected == item.identifier;
            let after_highlight_symbol_x = if has_selection {
                let symbol = if is_selected {
                    self.highlight_symbol
                } else {
                    &blank_symbol
                };
                let (x, _) = buf.set_stringn(x, y, symbol, area.width as usize, item_style);
                x
            } else {
                x
            };

            let after_depth_x = {
                let indent_width = item.depth() * 2;
                let (after_indent_x, _) = buf.set_stringn(
                    after_highlight_symbol_x,
                    y,
                    " ".repeat(indent_width),
                    indent_width,
                    item_style,
                );
                let symbol = if item.item.children.is_empty() {
                    self.node_no_children_symbol
                } else if state.opened.contains(&item.identifier) {
                    self.node_open_symbol
                } else {
                    self.node_closed_symbol
                };
                let max_width = area.width.saturating_sub(after_indent_x - x);
                let (x, _) =
                    buf.set_stringn(after_indent_x, y, symbol, max_width as usize, item_style);
                x
            };

            let max_element_width = area.width.saturating_sub(after_depth_x - x);
            for (j, line) in item.item.text.lines.iter().enumerate() {
                buf.set_line(after_depth_x, y + j as u16, line, max_element_width);
            }
            if is_selected {
                buf.set_style(area, self.highlight_style);
            }
        }
    }
}

impl<'a> Widget for Tree<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut state = TreeState::default();
        StatefulWidget::render(self, area, buf, &mut state);
    }
}
