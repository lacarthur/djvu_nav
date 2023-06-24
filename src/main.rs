mod nav;
mod djvu;
mod app;
mod tree_widget;

use crate::app::App;

use std::{io, time::Duration};
use crossterm::{
    terminal::{enable_raw_mode, EnterAlternateScreen, disable_raw_mode, LeaveAlternateScreen}, 
    execute,
};

use ratatui::{
    backend::CrosstermBackend, 
    Terminal,
};

fn main() -> Result<(), io::Error> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let filename = "resources/example2.djvu";
    let tick_rate = Duration::from_millis(250);
    match App::new(filename) {
        Ok(mut application) => {
            let res = application.run(&mut terminal, tick_rate);
            disable_raw_mode()?;
            execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
            terminal.show_cursor()?;

            if let Err(err) = res {
                println!("{err:?}");
            }
        },
        Err(e) => {
            disable_raw_mode()?;
            execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
            terminal.show_cursor()?;
            println!("{e:?}");
        }
    }
    Ok(())
}
