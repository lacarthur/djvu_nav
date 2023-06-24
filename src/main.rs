mod nav;
mod djvu;
mod app;
mod tree_widget;

use crate::app::App;

use std::{io::{self, Stdout}, time::Duration};
use crossterm::{
    terminal::{enable_raw_mode, EnterAlternateScreen, disable_raw_mode, LeaveAlternateScreen}, 
    execute,
};

use ratatui::{
    backend::{CrosstermBackend, Backend}, 
    Terminal,
};

/// Prepare terminal for TUI application, and return a handle to stdout.
fn prepare_terminal() -> Result<Stdout, io::Error> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    Ok(stdout)
}

/// Cleanup terminal to allow for graceful exit of the application
fn terminal_cleanup<B : Backend + io::Write>(terminal: &mut Terminal<B>) -> Result<(), io::Error> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

fn main() -> Result<(), io::Error> {
    let stdout = prepare_terminal()?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let filename = "resources/no_index.djvu";
    let tick_rate = Duration::from_millis(250);
    match App::new(filename) {
        Ok(mut application) => {
            let res = application.run(&mut terminal, tick_rate);
            terminal_cleanup(&mut terminal)?;

            if let Err(err) = res {
                println!("{err:?}");
            }
        },
        Err(e) => {
            terminal_cleanup(&mut terminal)?;
            println!("{e:?}");
        }
    }
    Ok(())
}
