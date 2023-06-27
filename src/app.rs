use std::{
    fs::{self, File}, 
    io::{BufWriter, Write, BufReader, BufRead, self, Stdout}, 
    process::Command, 
    time::{Duration, Instant}
};

use crossterm::{
    event::{self, Event, KeyEvent, KeyCode, EnableMouseCapture}, 
    terminal::{enable_raw_mode, EnterAlternateScreen, disable_raw_mode, LeaveAlternateScreen}, 
    execute
};

use ratatui::{backend::CrosstermBackend,  Terminal};

use crate::{
    nav::{Nav, BookmarkLink}, 
    tree_widget::TreeState, djvu::{NavReadingError, get_nav_from_djvu, embed_nav_in_djvu_file}
};

const TEMP_FOLDER: &str = "/home/arthur/.cache/djvu_nav";
const TEMP_FILE_NAME: &str = "tempfile";

pub fn get_temp_file_name() -> String {
    format!("{}/{}", TEMP_FOLDER, TEMP_FILE_NAME)
}

pub struct App {
    terminal: Terminal<CrosstermBackend<Stdout>>,
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
        let terminal = prepare_terminal().map_err(|e| AppLifetimeError::TerminalIOError(e))?;

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
            KeyCode::Char('a') => self.edit_currently_selected().unwrap(),
            KeyCode::Char('w') => self.write().map_err(|e| AppLifetimeError::NavReadingError(e))?,
            _ => (),
        }
        Ok(())
    }

    fn edit_currently_selected(&mut self) -> Result<(), AppLifetimeError> {
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
        execute!(self.terminal.backend_mut(), EnterAlternateScreen, EnableMouseCapture)
            .map_err(|e| AppLifetimeError::TerminalIOError(e))?;
        self.terminal.clear().map_err(|e| AppLifetimeError::TerminalIOError(e))?;
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
        let items = self.nav.get_tree();
        self.tree_state.key_up(&items);
    }

    pub fn move_down(&mut self) {
        let items = self.nav.get_tree();
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
}

impl Drop for App {
    fn drop(&mut self) {
        disable_raw_mode().unwrap();
        execute!(self.terminal.backend_mut(), LeaveAlternateScreen).unwrap();
        self.terminal.show_cursor().unwrap();
    }
}
