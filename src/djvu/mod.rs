use std::{
    process::Command,
    fs::File, 
    io::{BufWriter, Write},
};

use crate::{
    nav::Nav,
    app::{TEMP_FOLDER, TEMP_FILE_NAME},
};

pub mod parser;

#[derive(Debug)]
pub enum NavReadingError {
    IOError(std::io::Error),
    DjvusedError(std::process::ExitStatus, String),
    InvalidUtf8Error(std::string::FromUtf8Error),
    NavParsingError(String),
}

/// Uses `djvused` to get the outline of the file with path `filename`, and parse it into a `Nav`
/// object.
pub fn get_nav_from_djvu(filename: &str) -> Result<Nav, NavReadingError> {
    let nav_str = String::from_utf8(
        Command::new("djvused")
            .args([filename, "-u", "-e", "print-outline"])
            .output()
            .map_err(|e| NavReadingError::IOError(e))?
            .stdout
        ).map_err(|e| NavReadingError::InvalidUtf8Error(e))?;

    Ok(
        parser::parse_djvu_nav(&nav_str)
            .map_err(|e| NavReadingError::NavParsingError(e.to_string()))?.1
    )
}

/// Write `nav` to a temp file so that it can be used by `djvused` later on.
fn write_nav_to_temp_file(filename: &str, nav: &Nav) -> Result<(), std::io::Error> {
    let nav_s = nav.to_djvu();

    let temp_file = File::create(filename)?;
    let mut writer = BufWriter::new(temp_file);
    write!(writer, "{}", nav_s)?;
    Ok(())
}

/// Uses `djvused` to set the outline of the file `filename` to `nav`.
pub fn embed_nav_in_djvu_file(filename: &str, nav: &Nav) -> Result<(), NavReadingError> {
    let temp_file_name = format!("{}/{}", TEMP_FOLDER, TEMP_FILE_NAME);
    write_nav_to_temp_file(&temp_file_name, nav).map_err(|e| NavReadingError::IOError(e))?;

    let sed_command = format!("set-outline {}", &temp_file_name);
    let command_result = Command::new("djvused")
        .args([filename, "-e", &sed_command, "-s", "-v"])
        .output()
        .map_err(|e| NavReadingError::IOError(e))?;

    if !command_result.status.success() {
        return Err(NavReadingError::DjvusedError(
                command_result.status, 
                String::from_utf8(command_result.stderr
        ).unwrap()));
    }
    Ok(())
}
