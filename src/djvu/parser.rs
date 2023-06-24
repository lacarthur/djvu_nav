use nom::{
    IResult, Parser,
    multi::fold_many0,
    bytes::complete::{tag, take_till},
    sequence::delimited,
    character::complete::{multispace0, u32},
    error::{ErrorKind, ParseError},
    branch::alt,
    combinator::map,
};

use crate::nav::{Nav, BookmarkLink, NavNode};

/// Parse `djvused` output into a `Nav` object.
pub fn parse_djvu_nav(input: &str) -> IResult<&str, Nav> {
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
    quoted(hashtag_before(u32))(input)
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
