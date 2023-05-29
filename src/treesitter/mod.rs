use grep::matcher::Match;
use tree_sitter::{Language, Parser, Query, QueryCursor};

pub fn get_parser(language: Language) -> Parser {
    let mut parser = Parser::new();
    parser
        .set_language(language)
        .expect("Error loading grammar");
    parser
}

pub fn get_query(source: &str, language: Language) -> Query {
    Query::new(language, source).unwrap()
}

pub fn get_matches(
    query: &Query,
    capture_index: u32,
    file_text_as_bytes: &[u8],
    language: Language,
) -> Vec<Match> {
    let mut query_cursor = QueryCursor::new();
    let file_text =
        std::str::from_utf8(file_text_as_bytes).expect("Expected file text to be valid UTF-8");
    let tree = get_parser(language).parse(file_text, None).unwrap();
    query_cursor
        .matches(query, tree.root_node(), file_text_as_bytes)
        .flat_map(|match_| {
            match_
                .nodes_for_capture_index(capture_index)
                .collect::<Vec<_>>()
        })
        .map(|node| {
            let range = node.range();

            Match::new(range.start_byte, range.end_byte)
        })
        .collect()
}
