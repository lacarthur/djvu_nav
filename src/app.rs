use std::{
    fs::File, 
    io::{BufWriter, Write, BufReader, BufRead, self, Stdout}, 
    process::Command, 
    time::{Duration, Instant}, path::PathBuf
};

use crossterm::{
    event::{self, Event, KeyEvent, KeyCode, EnableMouseCapture}, 
    terminal::{enable_raw_mode, EnterAlternateScreen, disable_raw_mode, LeaveAlternateScreen}, 
    execute
};

use ratatui::{backend::CrosstermBackend, Terminal};

use crate::{
    nav::{Nav, BookmarkLink}, 
    tree_widget::{TreeState, TreeView}, 
    djvu::{NavReadingError, get_nav_from_djvu, embed_nav_in_djvu_file}
};

const APP_NAME: &str = "nav_edit";
const TEMP_FILE_NAME: &str = "tempfile";

const EDITOR: &str = "nvim";

pub fn get_temp_file_name() -> Result<PathBuf, TempFileError> {
    let xdg_dirs = xdg::BaseDirectories::with_prefix(format!("{}", APP_NAME))
        .map_err(|e| TempFileError::XDGSpecificError(e))?;
    // this does not create the cache file, but it creates the directories necessary to create it.
    xdg_dirs.place_cache_file(TEMP_FILE_NAME)
        .map_err(|e| TempFileError::SystemIOError(e))
}

pub struct App {
    terminal: Terminal<CrosstermBackend<Stdout>>,
    filename: String,
    nav: Nav,
    tree_state: TreeState,
    pub state: AppState,
}

#[derive(Debug)]
pub enum TempFileError {
    SystemIOError(io::Error),
    XDGSpecificError(xdg::BaseDirectoriesError),
}

#[derive(Debug)]
pub enum AppLifetimeError {
    NavReadingError(NavReadingError),
    ExternalProgramError(io::Error),
    TerminalIOError(io::Error),
    TempFileError(TempFileError),
}

#[derive(Debug, PartialEq, Eq)]
pub enum AppState {
    Quitting,
    Navigating,
    RunningOtherCommand,
}

fn prepare_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>, io::Error> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

impl App {
    pub fn new(filename: &str) -> Result<Self, AppLifetimeError> {
        let terminal = prepare_terminal()
            .map_err(|e| AppLifetimeError::TerminalIOError(e))?;

        let nav = get_nav_from_djvu(filename)
            .map_err(|e| AppLifetimeError::NavReadingError(e))?;

        let mut tree_state = TreeState::default();

        if !nav.nodes.is_empty() {
            tree_state.select_first();
        }
        
        let state = AppState::Navigating;

        Ok(Self {
            terminal,
            filename: String::from(filename),
            nav,
            tree_state,
            state,
        })
    }

    pub fn handle_input(
        &mut self, 
        key: KeyEvent, 
    ) -> Result<(), AppLifetimeError> {
        match key.code {
            KeyCode::Char('q') => self.state = AppState::Quitting,
            KeyCode::Char('h') => self.move_left(),
            KeyCode::Char('j') => self.move_down(),
            KeyCode::Char('k') => self.move_up(),
            KeyCode::Char('l') => self.move_right(),
            KeyCode::Char('i') => self.edit_currently_selected()?,
            KeyCode::Char('w') => self.write().map_err(|e| AppLifetimeError::NavReadingError(e))?,
            KeyCode::Char('o') => self.add_new_entry_below(),
            KeyCode::Char('d') => self.delete_currently_selected(),
            _ => (),
        }
        Ok(())
    }

    fn edit_currently_selected(&mut self) -> Result<(), AppLifetimeError> {
        if self.tree_state.selected().is_empty() {
            return Ok(());
        }
        // Create temp file with data in it
        let temp_filename = get_temp_file_name().map_err(|e| AppLifetimeError::TempFileError(e))?;
        let currently_selected_id = &self.tree_state.selected();
        {
            let f = File::create(&temp_filename)
                .map_err(|e| AppLifetimeError::TempFileError(TempFileError::SystemIOError(e)))?;
            let mut writer = BufWriter::new(f);

            let current_node = &self.nav[currently_selected_id];

            writer.write_fmt(format_args!("{}\n{}", current_node.string, current_node.link))
                .map_err(|e| AppLifetimeError::TempFileError(TempFileError::SystemIOError(e)))?;
        }

        // Edit file with EDITOR
        self.state = AppState::RunningOtherCommand;
        let mut command = Command::new(EDITOR)
            .arg(&temp_filename)
            .spawn()
            .map_err(|e| AppLifetimeError::ExternalProgramError(e))?;

        command.wait().map_err(|e| AppLifetimeError::ExternalProgramError(e))?;
        self.state = AppState::Navigating;

        enable_raw_mode().map_err(|e| AppLifetimeError::TerminalIOError(e))?;
        execute!(self.terminal.backend_mut(), EnterAlternateScreen, EnableMouseCapture)
            .map_err(|e| AppLifetimeError::TerminalIOError(e))?;
        self.terminal.clear().map_err(|e| AppLifetimeError::TerminalIOError(e))?;

        let tempfile = File::open(&temp_filename)
            .map_err(|e| AppLifetimeError::TempFileError(TempFileError::SystemIOError(e)))?;
        let reader = BufReader::new(tempfile);
        let lines: Vec<String> = reader.lines()
            .map(|result| result.unwrap())
            .collect();

        self.nav[currently_selected_id].string = lines[0].clone();
        self.nav[currently_selected_id].link = BookmarkLink::from_string(&lines[1]);

        Ok(())
    }

    fn delete_currently_selected(&mut self) {
        if self.tree_state.selected().is_empty() {
            return;
        }

        let selected = self.tree_state.selected().to_owned();
        let father = &selected[..selected.len() - 1];
        let last = selected[selected.len() - 1];

        if last == 0 && self.nav.num_children(father) == 1 {
            self.tree_state.select(father);
        } else if last == self.nav.num_children(father) - 1 {
            let mut new_select = father.to_owned();
            new_select.push(last - 1);
            self.tree_state.select(new_select);
        }

        self.nav.delete_entry(&selected);
    }

    pub fn move_up(&mut self) {
        self.tree_state.key_up(&self.nav);
    }

    pub fn move_down(&mut self) {
        self.tree_state.key_down(&self.nav);
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

    pub fn run(
        &mut self,
        tick_rate: Duration,
    ) -> Result<(), AppLifetimeError> {
        let mut last_tick = Instant::now();
        loop {
            if self.state == AppState::Quitting {
                return Ok(());
            }
            if self.state == AppState::Navigating {
                self.terminal.draw(|f| {
                    self.nav.ui(f, &mut self.tree_state)
                })
                .map_err(|e| AppLifetimeError::TerminalIOError(e))?;

                let timeout = tick_rate
                    .checked_sub(last_tick.elapsed())
                    .unwrap_or_else(|| Duration::from_secs(0));
                if event::poll(timeout).map_err(|e| AppLifetimeError::TerminalIOError(e))? {
                    if let Event::Key(key) = event::read().map_err(|e| AppLifetimeError::TerminalIOError(e))? {
                        self.handle_input(key)?;
                    }
                }
            }
            if last_tick.elapsed() >= tick_rate {
                last_tick = Instant::now();
            }
        }
    }

    fn write(&self) -> Result<(), NavReadingError> {
        embed_nav_in_djvu_file(&self.filename, &self.nav)?;
        Ok(())
    }

    fn add_new_entry_below(&mut self) {
        let is_selected_open = self.tree_state.is_open(self.tree_state.selected());
        if is_selected_open {
            self.nav.new_first_child(&self.tree_state.selected());
        } else {
            self.nav.new_sibling_below(&self.tree_state.selected());
        }

        self.move_down();
    }

    // fn delete_selected_entry(&mut self) {
    //     self.nav.delete_entry(&self.tree_state.selected());
    //     // TODO : handle updating the state
    // }
}

impl Drop for App {
    fn drop(&mut self) {
        disable_raw_mode().unwrap();
        execute!(self.terminal.backend_mut(), LeaveAlternateScreen).unwrap();
        self.terminal.show_cursor().unwrap();
    }
}
