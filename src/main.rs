mod nav;
mod djvu;
mod app;
mod tree_widget;

use crate::app::App;

use std::{io, time::Duration};

use clap::{Command, Arg};

fn main() -> Result<(), io::Error> {
    let cmd = Command::new("nav_edit")
        .version("1.0.0")
        .about("Edit NAV section of djvu files.")
        .arg(
            Arg::new("filename")
                .required(true)
        );

    let args = cmd.get_matches();
    let filename = args.get_one::<String>("filename").unwrap();
    let tick_rate = Duration::from_millis(250);
    match App::new(filename) {
        Ok(mut application) => {
            let res = application.run(tick_rate);
            if let Err(err) = res {
                println!("{err:?}");
            }
        },
        Err(err) => {
            println!("{err:?}");
        }
    }
    Ok(())
}
