use std::{process::Command, io::BufWriter};
use std::fs::File;
use std::io::Write;

use nom::{branch::alt, 
          IResult, 
          bytes::complete::{tag, take_till},
          character::{self, complete::multispace0}, combinator::map, Parser, error::{ParseError, ErrorKind}, multi::fold_many0, sequence::delimited,
};

use crate::{nav::{Nav, BookmarkLink, NavNode}, app::{TEMP_FOLDER, TEMP_FILE_NAME}};

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
        parse_djvu_nav(&nav_str)
            .map_err(|e| NavReadingError::NavParsingError(e.to_string()))?.1
    )

}

/// Uses `djvused` to set the outline of the file `filename` to `nav`.
pub fn write_nav_to_djvu(filename: &str, nav: &Nav) -> Result<(), NavReadingError> {
    let nav_s = nav.to_djvu();

    let temp_file_name = format!("{}/{}", TEMP_FOLDER, TEMP_FILE_NAME);
    {
        let temp_file = File::create(&temp_file_name).map_err(|e| NavReadingError::IOError(e))?;
        let mut writer = BufWriter::new(temp_file);
        write!(writer, "{}", nav_s).map_err(|e| NavReadingError::IOError(e))?;
    }

    let sed_command = format!("set-outline {}", &temp_file_name);
    let command_result = Command::new("djvused")
        .args([filename, "-e", &sed_command, "-s", "-v"])
        .output()
        .map_err(|e| NavReadingError::IOError(e))?;

    if !command_result.status.success() {
        return Err(NavReadingError::DjvusedError(command_result.status, String::from_utf8(command_result.stderr).unwrap()));
    }
    Ok(())
}

/// Parse `djvused` output into a `Nav` object.
fn parse_djvu_nav(input: &str) -> IResult<&str, Nav> {
    if input.is_empty() {
        return Ok((input, Nav { nodes: vec![] }));
    }
    let (input, _) = tag("(bookmarks")(input)?;
    let (input, nodes) = fold_many0(
        delimited(
            multispace0,
            parse_nav_node,
            multispace0
        ), 
        Vec::new, 
        |mut acc, item| {
            acc.push(item);
            acc
        })(input)?;
    let (input, _) = tag(")")(input)?;
    Ok((input, Nav { nodes }))
}

/// Parse a string of text accounting for the fact that `djvused` escapes some characters.
fn parse_string_with_escaped_characters(input: &str) -> IResult<&str, String> {
    let mut ret = String::new();
    let mut escape_next = false;
    for (i, c) in input.chars().enumerate() {
        if escape_next {
            match c {
                '"' => ret.push(c),
                '\\' => ret.push(c),
                'n' => ret.push('\n'),
                't' => ret.push('\t'),
                'r' => ret.push('\r'),
                _ => (),
            };
            escape_next = false;
        }
        else {
            if c == '"' {
                return Ok((&input[i..], ret));
            }
            else if c == '\\' {
                escape_next = true;
            }
            else {
                ret.push(c);
            }
        }
    }
    Err(nom::Err::Failure(nom::error::Error {
        input,
        code: ErrorKind::Eof,
    }))
}

fn parse_page_num(input: &str) -> IResult<&str, u32> {
    quoted(hashtag_before(character::complete::u32))(input)
}

fn parse_bookmark_link(input: &str) -> IResult<&str, BookmarkLink> {
    alt((
        map(parse_page_num, |num| BookmarkLink::PageNumber(num)),
        map(quoted(hashtag_before(take_till(|c| c== '"'))), |link| BookmarkLink::PageLink(String::from(link))) 
    ))(input)
}

fn parse_quoted_string(input: &str) -> IResult<&str, String> {
    quoted(parse_string_with_escaped_characters)(input)
}

fn quoted<'a, O, F, E>(mut parser: F) -> impl FnMut(&'a str) -> IResult<&'a str, O, E> 
where
    F: Parser<&'a str, O, E>,
    E: ParseError<&'a str>
{
    move |input: &'a str| {
        let (input, _) = tag("\"")(input)?;
        let (input, ret) = parser.parse(input)?;
        let (input, _) = tag("\"")(input)?;
        Ok((input, ret))
    }
}

fn parse_nav_node(input: &str) -> IResult<&str, NavNode> {
    delimited(tag("("), parse_node_interior, tag(")"))(input)
}

fn parse_node_interior(input: &str) -> IResult<&str, NavNode> {
    let (input, name) = delimited(multispace0, parse_quoted_string, multispace0)(input)?;
    let (input, link) = delimited(multispace0, parse_bookmark_link, multispace0)(input)?;
    let (input, children) = fold_many0(
        delimited(
            multispace0,
            parse_nav_node, 
            multispace0
        ),
        Vec::new, 
        |mut acc: Vec<_>, item| {
            acc.push(item); 
            acc
    })(input)?;

    Ok((input, NavNode { string: name, link, children }))
}

fn hashtag_before<'a, O, F, E>(mut parser: F) -> impl FnMut(&'a str) -> IResult<&'a str, O, E>
where
    F: Parser<&'a str, O, E>,
    E: ParseError<&'a str>
{
    move |input: &'a str| {
        let (input, _) = tag("#")(input)?;
        parser.parse(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn page_number() {
        assert_eq!(
            parse_page_num("\"#756\"").unwrap(),
            ("", 756)
        );
    }

    #[test]
    fn bookmark_link_parser_number() {
        assert_eq!(
            parse_bookmark_link("\"#756\"").unwrap(), 
            ("", BookmarkLink::PageNumber(756))
        );
    }

    #[test]
    fn bookmark_link_parser_link() {
        assert_eq!(
            parse_bookmark_link("\"#page0008.djvu\"").unwrap(),
            ("", BookmarkLink::PageLink(String::from("page0008.djvu")))
        );
    }

    #[test]
    fn bookmark_name_parser() {
        assert_eq!(
            parse_quoted_string("\"test test\"").unwrap(),
            ("", String::from("test test"))
        )
    }

    #[test]
    fn nav_node_parser1() {
        assert_eq!(
            parse_nav_node("(\"TOC\" \"#3\")").unwrap(),
            ("",
            NavNode {
                string: "TOC".to_string(),
                link: BookmarkLink::PageNumber(3),
                children: vec![],
            })
        )
    }

    #[test]
    fn nav_node_parser2() {
        assert_eq!(
            parse_nav_node("(\"Chapter 2 - Blabla\" \"#15\"
        (\"Subchapter 2.1 - Blabla\" \"#17\"
            (\"Subchapter 2.1.1 - Blabla\" \"#20\")
        )
    )").unwrap(),
            ("",
             NavNode {
                 string: "Chapter 2 - Blabla".to_string(),
                 link: BookmarkLink::PageNumber(15),
                 children: vec![
                     NavNode {
                         string: "Subchapter 2.1 - Blabla".to_string(),
                         link: BookmarkLink::PageNumber(17),
                         children: vec![
                             NavNode {
                                 string: "Subchapter 2.1.1 - Blabla".to_string(),
                                 link: BookmarkLink::PageNumber(20),
                                 children: vec![],
                             }
                         ]
                     }
                 ]
             })
        )
    }

    #[test]
    fn nav_node_parser3() {
        let s = r##"("Introduction"
   "#21"
   ("Historical Skentch"
    "#21" )
   ("Bombellis test"
    "#23" )
   ("Some terminology and notation"
    "#26" )
   ("Practice"
    "#27" )
   ("Equivalence of Symbolic and geometric arithmetic"
    "#28" ) )"##;

        assert_eq!(
            parse_nav_node(s).unwrap(),
            ("",
             NavNode {
                string: "Introduction".to_string(),
                link: BookmarkLink::PageNumber(21),
                children: vec![
                    NavNode {
                        string: "Historical Skentch".to_string(),
                        link: BookmarkLink::PageNumber(21),
                        children: vec![],
                    },
                    NavNode {
                        string: "Bombellis test".to_string(),
                        link: BookmarkLink::PageNumber(23),
                        children: vec![],
                    },
                    NavNode {
                        string: "Some terminology and notation".to_string(),
                        link: BookmarkLink::PageNumber(26),
                        children: vec![],
                    },
                    NavNode {
                        string: "Practice".to_string(),
                        link: BookmarkLink::PageNumber(27),
                        children: vec![],
                    },
                    NavNode {
                        string: "Equivalence of Symbolic and geometric arithmetic".to_string(),
                        link: BookmarkLink::PageNumber(28),
                        children: vec![],
                    }
                ],
            })
        )
    }

    #[test]
    fn nav_node_parser4() {
        assert_eq!(
            parse_nav_node("(\"Chapter 2 - \\\"Blabla\\\"\" \"#15\"
        (\"Subchapter 2.1 - Blabla\" \"#17\"
            (\"Subchapter 2.1.1 - Blabla\" \"#20\")
        )
    )").unwrap(),
            ("",
             NavNode {
                 string: "Chapter 2 - \"Blabla\"".to_string(),
                 link: BookmarkLink::PageNumber(15),
                 children: vec![
                     NavNode {
                         string: "Subchapter 2.1 - Blabla".to_string(),
                         link: BookmarkLink::PageNumber(17),
                         children: vec![
                             NavNode {
                                 string: "Subchapter 2.1.1 - Blabla".to_string(),
                                 link: BookmarkLink::PageNumber(20),
                                 children: vec![],
                             }
                         ]
                     }
                 ]
             })
        )
    }
}
