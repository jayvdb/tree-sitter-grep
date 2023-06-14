#![allow(clippy::into_iter_on_ref, clippy::collapsible_if)]
use std::{borrow::Cow, env, path::PathBuf, process::Command};

use assert_cmd::prelude::*;
use predicates::prelude::*;
use regex::Captures;

#[macro_export]
macro_rules! regex {
    ($re:literal $(,)?) => {{
        static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
        RE.get_or_init(|| regex::Regex::new($re).unwrap())
    }};
}

fn get_fixture_dir_path_from_name(fixture_dir_name: &str) -> PathBuf {
    // per https://andrewra.dev/2019/03/01/testing-in-rust-temporary-files/
    let root_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let mut path: PathBuf = root_dir.into();
    path.push("tests/fixtures");
    path.push(fixture_dir_name);
    path
}

fn parse_command_and_output(command_and_output: &str) -> CommandAndOutput {
    let mut lines = command_and_output.split('\n').collect::<Vec<_>>();
    if lines.is_empty() {
        panic!("Expected at least a command line");
    }
    if lines[0].trim().is_empty() {
        lines.remove(0);
    }
    let command_line = lines.remove(0);
    let indent = regex!(r#"^\s*"#).find(command_line).unwrap().as_str();
    let command_line_args = parse_command_line(strip_indent(command_line, indent));
    if !lines.is_empty() {
        if lines[lines.len() - 1].trim().is_empty() {
            lines.pop();
        }
    }
    let output: String = lines
        .into_iter()
        .map(|line| {
            if line.is_empty() {
                "\n".to_owned()
            } else {
                assert!(line.starts_with(indent));
                format!("{}\n", strip_indent(line, indent))
            }
        })
        .collect();
    CommandAndOutput {
        command_line_args,
        output,
    }
}

struct CommandAndOutput {
    command_line_args: Vec<String>,
    output: String,
}

fn strip_indent<'line>(line: &'line str, indent: &str) -> &'line str {
    &line[indent.len()..]
}

const DYNAMIC_LIBRARY_EXTENSION: &str = if cfg!(target_os = "macos") {
    ".dylib"
} else if cfg!(windows) {
    ".dll"
} else {
    ".so"
};

fn get_dynamic_library_name(library_name: &str) -> String {
    if cfg!(windows) {
        format!("{library_name}{}", DYNAMIC_LIBRARY_EXTENSION)
    } else {
        format!("lib{library_name}{}", DYNAMIC_LIBRARY_EXTENSION)
    }
}

fn parse_command_line(command_line: &str) -> Vec<String> {
    assert!(command_line.starts_with('$'));
    shlex::split(&command_line[1..])
        .unwrap()
        .iter()
        .map(|arg| {
            regex!(r#"lib(\S+)\.so$"#)
                .replace(arg, |captures: &Captures| {
                    get_dynamic_library_name(&captures[1])
                })
                .into_owned()
        })
        .collect()
}

fn assert_sorted_output(fixture_dir_name: &str, command_and_output: &str) {
    let CommandAndOutput {
        mut command_line_args,
        output,
    } = parse_command_and_output(command_and_output);
    let command_name = command_line_args.remove(0);
    Command::cargo_bin(command_name)
        .unwrap()
        .args(command_line_args)
        .current_dir(get_fixture_dir_path_from_name(fixture_dir_name))
        .assert()
        .success()
        .stdout(predicate::function(|actual_output| {
            do_sorted_lines_match(actual_output, &output)
        }));
}

fn massage_windows_line(line: &str) -> String {
    if cfg!(windows) {
        let line = strip_trailing_carriage_return(line);
        let line = normalize_match_path(&line);
        line.into_owned()
    } else {
        line.to_owned()
    }
}

fn strip_trailing_carriage_return(line: &str) -> Cow<'_, str> {
    regex!(r#"\r$"#).replace(line, "")
}

fn normalize_match_path(line: &str) -> Cow<'_, str> {
    regex!(r#"^[^:]+:"#).replace(line, |captures: &Captures| captures[0].replace('\\', "/"))
}

fn do_sorted_lines_match(actual_output: &str, expected_output: &str) -> bool {
    let mut actual_lines = actual_output
        .split('\n')
        .map(massage_windows_line)
        .collect::<Vec<_>>();
    actual_lines.sort();
    let mut expected_lines = expected_output.split('\n').collect::<Vec<_>>();
    expected_lines.sort();
    actual_lines == expected_lines
}

fn assert_failure_output(fixture_dir_name: &str, command_and_output: &str) {
    let CommandAndOutput {
        mut command_line_args,
        output,
    } = parse_command_and_output(command_and_output);
    let command_name = command_line_args.remove(0);
    Command::cargo_bin(command_name)
        .unwrap()
        .args(command_line_args)
        .current_dir(get_fixture_dir_path_from_name(fixture_dir_name))
        .assert()
        .failure()
        .stderr(predicate::function(|stderr: &str| {
            let stderr = massage_error_output(stderr);
            stderr == output
        }));
}

fn assert_non_match_output(fixture_dir_name: &str, command_and_output: &str) {
    let CommandAndOutput {
        mut command_line_args,
        output,
    } = parse_command_and_output(command_and_output);
    let command_name = command_line_args.remove(0);
    Command::cargo_bin(command_name)
        .unwrap()
        .args(command_line_args)
        .current_dir(get_fixture_dir_path_from_name(fixture_dir_name))
        .assert()
        .success()
        .stdout(predicate::function(|stdout: &str| {
            let stdout = massage_error_output(stdout);
            stdout == output
        }));
}

fn massage_error_output(output: &str) -> String {
    if cfg!(windows) {
        output.replace(".exe", "")
    } else {
        output.to_owned()
    }
    .split('\n')
    .map(|line| line.trim_end())
    .collect::<Vec<_>>()
    .join("\n")
}

fn build_example(example_name: &str) {
    // CargoBuild::new().example(example_name).exec().unwrap();
    Command::new("cargo")
        .args(["build", "--example", example_name])
        .status()
        .expect("Build example command failed");
}

#[test]
fn test_query_inline() {
    assert_sorted_output(
        "rust_project",
        r#"
            $ tree-sitter-grep --query-source '(function_item) @function_item' --language rust
            src/helpers.rs:1:pub fn helper() {}
            src/lib.rs:3:pub fn add(left: usize, right: usize) -> usize {
            src/lib.rs:4:    left + right
            src/lib.rs:5:}
            src/lib.rs:12:    fn it_works() {
            src/lib.rs:13:        let result = add(2, 2);
            src/lib.rs:14:        assert_eq!(result, 4);
            src/lib.rs:15:    }
            src/stop.rs:1:fn stop_it() {}
        "#,
    );
}

#[test]
fn test_query_inline_short_option() {
    assert_sorted_output(
        "rust_project",
        r#"
            $ tree-sitter-grep -q '(function_item) @function_item' --language rust
            src/helpers.rs:1:pub fn helper() {}
            src/lib.rs:3:pub fn add(left: usize, right: usize) -> usize {
            src/lib.rs:4:    left + right
            src/lib.rs:5:}
            src/lib.rs:12:    fn it_works() {
            src/lib.rs:13:        let result = add(2, 2);
            src/lib.rs:14:        assert_eq!(result, 4);
            src/lib.rs:15:    }
            src/stop.rs:1:fn stop_it() {}
        "#,
    );
}

#[test]
fn test_vimgrep_mode() {
    assert_sorted_output(
        "rust_project",
        r#"
            $ tree-sitter-grep --query-source '(function_item) @function_item' --language rust --vimgrep
            src/helpers.rs:1:1:pub fn helper() {}
            src/lib.rs:3:1:pub fn add(left: usize, right: usize) -> usize {
            src/lib.rs:12:5:    fn it_works() {
            src/stop.rs:1:1:fn stop_it() {}
       "#,
    );
}

#[test]
fn test_query_file() {
    assert_sorted_output(
        "rust_project",
        r#"
            $ tree-sitter-grep --query-file ./function-item.scm --language rust
            src/helpers.rs:1:pub fn helper() {}
            src/lib.rs:3:pub fn add(left: usize, right: usize) -> usize {
            src/lib.rs:4:    left + right
            src/lib.rs:5:}
            src/lib.rs:12:    fn it_works() {
            src/lib.rs:13:        let result = add(2, 2);
            src/lib.rs:14:        assert_eq!(result, 4);
            src/lib.rs:15:    }
            src/stop.rs:1:fn stop_it() {}
       "#,
    );
}

#[test]
fn test_query_file_short_option() {
    assert_sorted_output(
        "rust_project",
        r#"
            $ tree-sitter-grep -Q ./function-item.scm --language rust
            src/helpers.rs:1:pub fn helper() {}
            src/lib.rs:3:pub fn add(left: usize, right: usize) -> usize {
            src/lib.rs:4:    left + right
            src/lib.rs:5:}
            src/lib.rs:12:    fn it_works() {
            src/lib.rs:13:        let result = add(2, 2);
            src/lib.rs:14:        assert_eq!(result, 4);
            src/lib.rs:15:    }
            src/stop.rs:1:fn stop_it() {}
       "#,
    );
}

#[test]
fn test_specify_single_file() {
    assert_sorted_output(
        "rust_project",
        r#"
            $ tree-sitter-grep --query-source '(function_item) @function_item' --language rust src/lib.rs
            src/lib.rs:3:pub fn add(left: usize, right: usize) -> usize {
            src/lib.rs:4:    left + right
            src/lib.rs:5:}
            src/lib.rs:12:    fn it_works() {
            src/lib.rs:13:        let result = add(2, 2);
            src/lib.rs:14:        assert_eq!(result, 4);
            src/lib.rs:15:    }
        "#,
    );
}

#[test]
fn test_specify_single_file_preserves_leading_dot_slash() {
    assert_sorted_output(
        "rust_project",
        r#"
            $ tree-sitter-grep --query-source '(function_item) @function_item' --language rust ./src/lib.rs
            ./src/lib.rs:3:pub fn add(left: usize, right: usize) -> usize {
            ./src/lib.rs:4:    left + right
            ./src/lib.rs:5:}
            ./src/lib.rs:12:    fn it_works() {
            ./src/lib.rs:13:        let result = add(2, 2);
            ./src/lib.rs:14:        assert_eq!(result, 4);
            ./src/lib.rs:15:    }
        "#,
    );
}

#[test]
fn test_specify_multiple_files() {
    assert_sorted_output(
        "rust_project",
        r#"
            $ tree-sitter-grep --query-source '(function_item) @function_item' --language rust src/lib.rs ./src/helpers.rs
            ./src/helpers.rs:1:pub fn helper() {}
            src/lib.rs:3:pub fn add(left: usize, right: usize) -> usize {
            src/lib.rs:4:    left + right
            src/lib.rs:5:}
            src/lib.rs:12:    fn it_works() {
            src/lib.rs:13:        let result = add(2, 2);
            src/lib.rs:14:        assert_eq!(result, 4);
            src/lib.rs:15:    }
        "#,
    );
}

#[test]
fn test_invalid_query_inline() {
    assert_failure_output(
        "rust_project",
        r#"
            $ tree-sitter-grep --query-source '(function_itemz) @function_item' --language rust
            error: invalid query
        "#,
    );
}

#[test]
fn test_invalid_query_file() {
    assert_failure_output(
        "rust_project",
        r#"
            $ tree-sitter-grep --query-file ./function-itemz.scm --language rust
            error: invalid query
        "#,
    );
}

#[test]
fn test_no_query_or_filter_specified() {
    assert_failure_output(
        "rust_project",
        r#"
            $ tree-sitter-grep --language rust
            error: the following required arguments were not provided:
              <--query-file <PATH_TO_QUERY_FILE>|--query-source <QUERY_SOURCE>|--filter <FILTER>>

            Usage: tree-sitter-grep --language <LANGUAGE> <--query-file <PATH_TO_QUERY_FILE>|--query-source <QUERY_SOURCE>|--filter <FILTER>> [PATHS]...

            For more information, try '--help'.
        "#,
    );
}

#[test]
fn test_invalid_language_name() {
    assert_failure_output(
        "rust_project",
        r#"
            $ tree-sitter-grep --query-source '(function_item) @function_item' --language rustz
            error: invalid value 'rustz' for '--language <LANGUAGE>'
              [possible values: rust, typescript, javascript]

              tip: a similar value exists: 'rust'

            For more information, try '--help'.
        "#,
    );
}

#[test]
fn test_invalid_query_file_path() {
    assert_failure_output(
        "rust_project",
        r#"
            $ tree-sitter-grep --query-file ./nonexistent.scm --language rust
            error: couldn't read query file "./nonexistent.scm"
        "#,
    );
}

#[test]
fn test_auto_language_single_known_language_encountered() {
    assert_sorted_output(
        "rust_project",
        r#"
            $ tree-sitter-grep --query-source '(function_item) @function_item'
            src/helpers.rs:1:pub fn helper() {}
            src/lib.rs:3:pub fn add(left: usize, right: usize) -> usize {
            src/lib.rs:4:    left + right
            src/lib.rs:5:}
            src/lib.rs:12:    fn it_works() {
            src/lib.rs:13:        let result = add(2, 2);
            src/lib.rs:14:        assert_eq!(result, 4);
            src/lib.rs:15:    }
            src/stop.rs:1:fn stop_it() {}
        "#,
    );
}

#[test]
fn test_auto_language_multiple_parseable_languages() {
    assert_sorted_output(
        "mixed_project",
        r#"
            $ tree-sitter-grep --query-source '(arrow_function) @arrow_function'
            javascript_src/index.js:1:const js_foo = () => {}
            typescript_src/index.tsx:1:const foo = () => {}
        "#,
    );
}

#[test]
fn test_auto_language_single_parseable_languages() {
    assert_sorted_output(
        "mixed_project",
        r#"
            $ tree-sitter-grep --query-source '(function_item) @function_item'
            rust_src/lib.rs:1:fn foo() {}
        "#,
    );
}

#[test]
fn test_capture_name() {
    assert_sorted_output(
        "rust_project",
        r#"
            $ tree-sitter-grep --query-source '(function_item name: (identifier) @name) @function_item' --language rust --capture function_item
            src/helpers.rs:1:pub fn helper() {}
            src/lib.rs:3:pub fn add(left: usize, right: usize) -> usize {
            src/lib.rs:4:    left + right
            src/lib.rs:5:}
            src/lib.rs:12:    fn it_works() {
            src/lib.rs:13:        let result = add(2, 2);
            src/lib.rs:14:        assert_eq!(result, 4);
            src/lib.rs:15:    }
            src/stop.rs:1:fn stop_it() {}
        "#,
    );
}

#[test]
fn test_predicate() {
    assert_sorted_output(
        "rust_project",
        r#"
            $ tree-sitter-grep --query-source '(function_item name: (identifier) @name (#eq? @name "add")) @function_item' --language rust --capture function_item
            src/lib.rs:3:pub fn add(left: usize, right: usize) -> usize {
            src/lib.rs:4:    left + right
            src/lib.rs:5:}
        "#,
    );
}

#[test]
fn test_no_matches() {
    assert_sorted_output(
        "rust_project",
        r#"
            $ tree-sitter-grep --query-source '(function_item name: (identifier) @name (#eq? @name "addz")) @function_item' --language rust
        "#,
    );
}

#[test]
fn test_invalid_capture_name() {
    assert_failure_output(
        "rust_project",
        r#"
            $ tree-sitter-grep --query-source '(function_item) @function_item' --language rust --capture function_itemz
            error: invalid capture name 'function_itemz'
        "#,
    );
}

#[test]
fn test_unknown_option() {
    assert_failure_output(
        "rust_project",
        r#"
            $ tree-sitter-grep --query-sourcez '(function_item) @function_item' --language rust
            error: unexpected argument '--query-sourcez' found

              tip: a similar argument exists: '--query-source'

            Usage: tree-sitter-grep <--query-file <PATH_TO_QUERY_FILE>|--query-source <QUERY_SOURCE>|--filter <FILTER>> <PATHS|--query-file <PATH_TO_QUERY_FILE>|--query-source <QUERY_SOURCE>|--capture <CAPTURE_NAME>|--language <LANGUAGE>|--filter <FILTER>|--filter-arg <FILTER_ARG>|--vimgrep>

            For more information, try '--help'.
        "#,
    );
}

#[test]
fn test_filter_plugin() {
    build_example("filter_before_line_10");

    assert_sorted_output(
        "rust_project",
        r#"
            $ tree-sitter-grep --query-source '(function_item) @function_item' --language rust --filter ../../../target/debug/examples/libfilter_before_line_10.so
            src/helpers.rs:1:pub fn helper() {}
            src/lib.rs:3:pub fn add(left: usize, right: usize) -> usize {
            src/lib.rs:4:    left + right
            src/lib.rs:5:}
            src/stop.rs:1:fn stop_it() {}
        "#,
    );
}

#[test]
fn test_filter_plugin_with_argument() {
    build_example("filter_before_line_number");

    assert_sorted_output(
        "rust_project",
        r#"
            $ tree-sitter-grep --query-source '(function_item) @function_item' --language rust --filter ../../../target/debug/examples/libfilter_before_line_number.so --filter-arg 2
            src/helpers.rs:1:pub fn helper() {}
            src/stop.rs:1:fn stop_it() {}
        "#,
    );
}

#[test]
fn test_filter_plugin_expecting_argument_not_received() {
    build_example("filter_before_line_number");

    assert_failure_output(
        "rust_project",
        r#"
            $ tree-sitter-grep --query-source '(function_item) @function_item' --language rust --filter ../../../target/debug/examples/libfilter_before_line_number.so
            error: plugin expected '--filter-arg <ARGUMENT>'
        "#,
    );
}

#[test]
fn test_filter_plugin_unparseable_argument() {
    build_example("filter_before_line_number");

    assert_failure_output(
        "rust_project",
        r#"
            $ tree-sitter-grep --query-source '(function_item) @function_item' --language rust --filter ../../../target/debug/examples/libfilter_before_line_number.so --filter-arg abc
            error: plugin couldn't parse argument "abc"
        "#,
    );
}

#[test]
fn test_filter_plugin_no_query() {
    build_example("filter_function_items_before_line_10");

    assert_sorted_output(
        "rust_project",
        r#"
            $ tree-sitter-grep --language rust --filter ../../../target/debug/examples/libfilter_function_items_before_line_10.so
            src/helpers.rs:1:pub fn helper() {}
            src/lib.rs:3:pub fn add(left: usize, right: usize) -> usize {
            src/lib.rs:4:    left + right
            src/lib.rs:5:}
            src/stop.rs:1:fn stop_it() {}
        "#,
    );
}

#[test]
fn test_query_inline_and_query_file_path() {
    assert_failure_output(
        "rust_project",
        r#"
            $ tree-sitter-grep --query-source '(function_item) @function_item' --query-file ./function-item.scm --language rust
            error: the argument '--query-source <QUERY_SOURCE>' cannot be used with '--query-file <PATH_TO_QUERY_FILE>'

            Usage: tree-sitter-grep --language <LANGUAGE> <--query-file <PATH_TO_QUERY_FILE>|--query-source <QUERY_SOURCE>|--filter <FILTER>> [PATHS]...

            For more information, try '--help'.
        "#,
    );
}

#[test]
fn test_help_option() {
    assert_non_match_output(
        "rust_project",
        r#"
            $ tree-sitter-grep --help
            Usage: tree-sitter-grep [OPTIONS] <--query-file <PATH_TO_QUERY_FILE>|--query-source <QUERY_SOURCE>|--filter <FILTER>> [PATHS]...

            Arguments:
              [PATHS]...

            Options:
              -Q, --query-file <PATH_TO_QUERY_FILE>
              -q, --query-source <QUERY_SOURCE>
              -c, --capture <CAPTURE_NAME>
              -l, --language <LANGUAGE>              [possible values: rust, typescript, javascript]
              -f, --filter <FILTER>
              -a, --filter-arg <FILTER_ARG>
                  --vimgrep
              -h, --help                             Print help
        "#,
    );
}
