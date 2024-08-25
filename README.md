# djvu_nav

djvu_nav is a TUI program to edit the `NAV` section of `.djvu` file. It works by leveraging [`djvused`](https://djvu.sourceforge.net/doc/man/djvused.html) with a small parser made with [`nom`](https://github.com/rust-bakery/nom). The interface is made with [`ratatui`](https://github.com/ratatui/ratatui) and [`crossterm`](https://github.com/crossterm-rs/crossterm), and a bespoke treeview widget inspired by [`tui-rs-tree-widget`](https://github.com/EdJoPaTo/tui-rs-tree-widget).

The editor used to edit the names of the sections is hardcoded as `nvim`, this should probably change to be something like `$EDITOR` in the future.

![videodjvu_nav](https://github.com/user-attachments/assets/a4ad0848-74b7-4767-b68b-5e856ab6b225)
