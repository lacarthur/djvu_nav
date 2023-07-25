use nom::{
    IResult, Parser,
    multi::fold_many0,
    bytes::complete::tag,
    sequence::{delimited, preceded},
    character::complete::{multispace0, u32},
    error::{ErrorKind, ParseError},
};

use crate::nav::{Nav, BookmarkLink, NavNode};

/// Parse `djvused` output into a `Nav` object.
pub fn parse_djvu_nav(input: &str) -> IResult<&str, Nav> {
    if input.is_empty() {
        return Ok((input, Nav { nodes: vec![] }));
    }

    let (input, _) = tag("(bookmarks")(input)?;

    let (input, nodes) = parse_nav_nodes(input)?;

    let (input, _) = tag(")")(input)?;
    Ok((input, Nav { nodes }))
}

/// Parse a string of text accounting for the fact that `djvused` escapes some characters.
fn parse_string_with_escaped_characters(input: &str) -> IResult<&str, String> {
    let mut ret = String::new();
    let mut escape_next = false;
    let mut size_to_skip = 0;
    for c in input.chars() {
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
                return Ok((&input[size_to_skip..], ret));
            }
            else if c == '\\' {
                escape_next = true;
            }
            else {
                ret.push(c);
            }
        }
        size_to_skip += c.len_utf8();
    }
    Err(nom::Err::Failure(nom::error::Error {
        input,
        code: ErrorKind::Eof,
    }))
}

fn parse_page_num(input: &str) -> IResult<&str, u32> {
    preceded(tag("#"), u32)(input)
}

fn parse_bookmark_link(input: &str) -> IResult<&str, BookmarkLink> {
    let (input, quoted_string) = parse_quoted_string(input)?;

    if let Ok(("", page_num)) = parse_page_num(&quoted_string) {
        Ok((input, BookmarkLink::PageNumber(page_num)))
    } else if let Some('#') = quoted_string.chars().next() {
        Ok((input, BookmarkLink::PageLink(String::from(&quoted_string[1..]))))
    } else {
        Err(nom::Err::Failure(nom::error::Error {
            input,
            code: ErrorKind::Tag,
        }))
    }
}

fn parse_quoted_string(input: &str) -> IResult<&str, String> {
    quoted(parse_string_with_escaped_characters)(input)
}

fn quoted<'a, O, F, E>(parser: F) -> impl FnMut(&'a str) -> IResult<&'a str, O, E> 
where
    F: Parser<&'a str, O, E>,
    E: ParseError<&'a str>
{
    delimited(tag("\""), parser, tag("\""))
}

fn parse_nav_nodes(input: &str) -> IResult<&str, Vec<NavNode>> {
    let (input, children) = fold_many0(
        delimited(
            multispace0, 
            parse_nav_node,
            multispace0,
        ), 
        Vec::new,
        |mut acc: Vec<_>, item| {
            acc.push(item);
            acc
        })(input)?;

    Ok((input, children))
}

fn parse_nav_node(input: &str) -> IResult<&str, NavNode> {
    delimited(tag("("), parse_node_interior, tag(")"))(input)
}

fn parse_node_interior(input: &str) -> IResult<&str, NavNode> {
    let (input, name) = delimited(multispace0, parse_quoted_string, multispace0)(input)?;
    let (input, link) = delimited(multispace0, parse_bookmark_link, multispace0)(input)?;
    let (input, children) = parse_nav_nodes(input)?;

    Ok((input, NavNode { string: name, link, children }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn page_number() {
        assert_eq!(
            parse_page_num("#756").unwrap(),
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
    fn quoted_string() {
        assert_eq!(
            parse_quoted_string(r##""test\\n\\n\\n\"hh\" jlkj""##).unwrap(),
            ("", String::from(r##"test\n\n\n"hh" jlkj"##))
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

    #[test]
    fn nav_node_utf8() {
        assert_eq!(
            parse_nav_node("(\"4.2 CONVEXITY—ALGEBRAIC\" \"#90\" )").unwrap().1,
            NavNode {
                string: "4.2 CONVEXITY—ALGEBRAIC".to_string(),
                link: BookmarkLink::PageNumber(90),
                children: vec![],
            }
        )
    }
}
