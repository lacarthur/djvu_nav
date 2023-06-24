use std::{fs::{self, File}, io::{BufWriter, Write, BufReader, BufRead, self}, process::Command, time::{Duration, Instant}};
use crossterm::{event::{self, Event, KeyEvent, KeyCode, EnableMouseCapture}, terminal::{enable_raw_mode, EnterAlternateScreen}, execute};
use ratatui::{backend::Backend, Frame, style::{Style, Color}, Terminal};

use crate::{
    nav::{Nav, NavNode, BookmarkLink}, 
    tree_widget::{TreeState, Tree, TreeItem}, djvu::{NavReadingError, get_nav_from_djvu, write_nav_to_djvu}
};

pub const TEMP_FOLDER: &str = "/home/arthur/.cache/djvu_nav";
pub const TEMP_FILE_NAME: &str = "tempfile";

pub struct App {
    filename: String,
    nav: Nav,
    tree_state: TreeState,
    pub state: AppState,
}

#[derive(Debug)]
pub enum AppLifetimeError {
    NavReadingError(NavReadingError),
    TerminalIOError(io::Error),
    TempFileIOError(io::Error),
}

#[derive(Debug, PartialEq, Eq)]
pub enum AppState {
    Quitting,
    Navigating,
    RunningOtherCommand,
}
fn get_tree_from_nav(nav: &Nav) -> Vec<TreeItem> {
    nav.nodes.iter()
        .map(|node| get_tree_from_nav_node(node))
        .collect()
}

fn get_tree_from_nav_node(node: &NavNode) -> TreeItem {
    if node.children.is_empty() {
        TreeItem::new_leaf(node.string.as_str())
    }
    else {
        let children: Vec<_> = node.children.iter()
            .map(|n| get_tree_from_nav_node(n))
            .collect();

        TreeItem::new(node.string.as_str(), children)
    }
}

impl App {
    pub fn new(filename: &str) -> Result<Self, AppLifetimeError> {
        let nav = get_nav_from_djvu(filename)
            .map_err(|e| AppLifetimeError::NavReadingError(e))?;

        let mut tree_state = TreeState::default();

        if !nav.nodes.is_empty() {
            tree_state.select_first();
        }
        
        let state = AppState::Navigating;

        Ok(Self {
            filename: String::from(filename),
            nav,
            tree_state,
            state,
        })
    }

    pub fn ui<B: Backend>(&mut self, f: &mut Frame<B>) {
        let tree = Tree::new(get_tree_from_nav(&self.nav))
            .highlight_style(
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::LightGreen)
            )
            .highlight_symbol("> ");
        f.render_stateful_widget(tree, f.size(), &mut self.tree_state);
    }

    pub fn handle_input<B : Backend + std::io::Write>(&mut self, key: KeyEvent, terminal: &mut Terminal<B>) -> Result<(), AppLifetimeError> {
        match key.code {
            KeyCode::Char('q') => self.state = AppState::Quitting,
            KeyCode::Char('h') => self.move_left(),
            KeyCode::Char('j') => self.move_down(),
            KeyCode::Char('k') => self.move_up(),
            KeyCode::Char('l') => self.move_right(),
            KeyCode::Char('a') => self.edit_currently_selected(terminal).unwrap(),
            KeyCode::Char('w') => self.write().map_err(|e| AppLifetimeError::NavReadingError(e))?,
            _ => (),
        }
        Ok(())
    }

    fn edit_currently_selected<B : Backend + std::io::Write>(&mut self, terminal: &mut Terminal<B>) -> Result<(), AppLifetimeError> {
        // Create temp file with data in it
        fs::create_dir_all(TEMP_FOLDER).map_err(|e| AppLifetimeError::TerminalIOError(e))?;
        let temp_filename = format!("{}/{}", TEMP_FOLDER, TEMP_FILE_NAME);
        let currently_selected_id = &self.tree_state.selected();
        {
            let f = File::create(&temp_filename)
                .map_err(|e| AppLifetimeError::TempFileIOError(e))?;
            let mut writer = BufWriter::new(f);

            let current_node = &self.nav[currently_selected_id];

            writer.write_fmt(format_args!("{}\n{}", current_node.string, current_node.link))
                .map_err(|e| AppLifetimeError::TempFileIOError(e))?;
        }

        // Edit file with EDITOR
        self.state = AppState::RunningOtherCommand;
        let mut command = Command::new("nvim")
            .arg(&temp_filename)
            .spawn()
            .expect("Error spawning nvim");

        command.wait().expect("Error waiting nvim");
        self.state = AppState::Navigating;
        enable_raw_mode().map_err(|e| AppLifetimeError::TerminalIOError(e))?;
        execute!(terminal.backend_mut(), EnterAlternateScreen, EnableMouseCapture)
            .map_err(|e| AppLifetimeError::TerminalIOError(e))?;
        terminal.clear().map_err(|e| AppLifetimeError::TerminalIOError(e))?;
        // Read content of file and change thing

        let tempfile = File::open(&temp_filename).map_err(|e| AppLifetimeError::TempFileIOError(e))?;
        let reader = BufReader::new(tempfile);
        let lines: Vec<String> = reader.lines()
            .map(|result| result.unwrap())
            .collect();

        self.nav[currently_selected_id].string = lines[0].clone();
        self.nav[currently_selected_id].link = BookmarkLink::from_string(&lines[1]);

        Ok(())
    }

    pub fn move_up(&mut self) {
        let items = get_tree_from_nav(&self.nav);
        self.tree_state.key_up(&items);
    }

    pub fn move_down(&mut self) {
        let items = get_tree_from_nav(&self.nav);
        self.tree_state.key_down(&items);
    }

    pub fn move_left(&mut self) {
        let mut temp_state = self.tree_state.clone();
        temp_state.key_left();
        if !temp_state.selected().is_empty() {
            self.tree_state = temp_state;
        }
    }
    
    pub fn move_right(&mut self) {
        self.tree_state.key_right();
    }

    pub fn run<B : Backend + std::io::Write>(
        &mut self,
        terminal: &mut Terminal<B>,
        tick_rate: Duration,
    ) -> Result<(), AppLifetimeError> {
        let mut last_tick = Instant::now();
        loop {
            if self.state == AppState::Quitting {
                return Ok(());
            }
            if self.state == AppState::Navigating {
                terminal.draw(|f| {
                    self.ui(f)
                })
                .map_err(|e| AppLifetimeError::TerminalIOError(e))?;

                let timeout = tick_rate
                    .checked_sub(last_tick.elapsed())
                    .unwrap_or_else(|| Duration::from_secs(0));
                if event::poll(timeout).map_err(|e| AppLifetimeError::TerminalIOError(e))? {
                    if let Event::Key(key) = event::read().map_err(|e| AppLifetimeError::TerminalIOError(e))? {
                        self.handle_input(key, terminal)?;
                    }
                }
            }
            if last_tick.elapsed() >= tick_rate {
                last_tick = Instant::now();
            }
        }
    }

    fn write(&self) -> Result<(), NavReadingError> {
        write_nav_to_djvu(&self.filename, &self.nav)?;
        Ok(())
    }
}
