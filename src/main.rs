mod nav;
mod djvu;
mod app;
mod tree_widget;

use crate::app::App;

use std::{io, time::Duration};

fn main() -> Result<(), io::Error> {
    let filename = "resources/with_index.djvu";
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
