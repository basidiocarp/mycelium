use tree_sitter::{Node, Parser, TreeCursor};

use super::shell::{has_unsupported_shell_quoting, needs_shell_parser_fallback};

pub(super) fn parser_allows_rewrite_shape(cmd: &str) -> bool {
    let trimmed = cmd.trim();
    if trimmed.is_empty()
        || trimmed.contains('\n')
        || trimmed.contains('\r')
        || has_unsupported_shell_quoting(trimmed)
    {
        return false;
    }

    let mut parser = Parser::new();
    if parser
        .set_language(&tree_sitter_bash::LANGUAGE.into())
        .is_err()
    {
        return false;
    }

    let Some(tree) = parser.parse(trimmed, None) else {
        return false;
    };

    let root = tree.root_node();
    !root.has_error() && parsed_tree_is_safe(root, trimmed.as_bytes())
}

pub(super) fn rewrite_shape_requires_parser(cmd: &str) -> bool {
    needs_shell_parser_fallback(cmd)
}

fn parsed_tree_is_safe(root: Node<'_>, source: &[u8]) -> bool {
    let mut cursor = root.walk();
    walk_tree(root, source, &mut cursor)
}

fn walk_tree(node: Node<'_>, source: &[u8], cursor: &mut TreeCursor<'_>) -> bool {
    if node.is_error() || node.is_missing() {
        return false;
    }

    if node.is_named() && !named_node_is_safe(node, source) {
        return false;
    }

    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if !walk_tree(child, source, cursor) {
                cursor.goto_parent();
                return false;
            }

            if !cursor.goto_next_sibling() {
                break;
            }
        }
        cursor.goto_parent();
    }

    true
}

fn named_node_is_safe(node: Node<'_>, source: &[u8]) -> bool {
    match node.kind() {
        "program"
        | "list"
        | "command"
        | "word"
        | "string"
        | "raw_string"
        | "concatenation"
        | "variable_assignment"
        | "simple_expansion"
        | "expansion"
        | "special_variable_name"
        | "variable_name"
        | "file_descriptor"
        | "number" => true,
        "command_name" => command_name_is_safe(node, source),
        "pipeline"
        | "redirected_statement"
        | "file_redirect"
        | "herestring_redirect"
        | "heredoc_redirect"
        | "subshell"
        | "process_substitution"
        | "command_substitution"
        | "arithmetic_expansion"
        | "function_definition"
        | "compound_statement"
        | "brace_expression"
        | "subscript"
        | "array"
        | "binary_expression"
        | "unary_expression"
        | "postfix_expression"
        | "parenthesized_expression"
        | "for_statement"
        | "c_style_for_statement"
        | "while_statement"
        | "if_statement"
        | "elif_clause"
        | "else_clause"
        | "case_statement"
        | "case_item"
        | "test_command"
        | "declaration_command"
        | "unset_command"
        | "negated_command"
        | "coproc"
        | "comment" => false,
        _ => true,
    }
}

fn command_name_is_safe(node: Node<'_>, source: &[u8]) -> bool {
    node.utf8_text(source)
        .map(|text| {
            !matches!(
                text,
                "function" | "declare" | "local" | "readonly" | "typeset"
            )
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::parser_allows_rewrite_shape;

    #[test]
    fn parser_allows_safe_compound_list() {
        assert!(parser_allows_rewrite_shape(
            "git log --grep 'feat|fix' && cargo test"
        ));
    }

    #[test]
    fn parser_rejects_redirects_and_shell_grouping() {
        assert!(!parser_allows_rewrite_shape("git status > out.txt"));
        assert!(!parser_allows_rewrite_shape(
            "git status && (cargo test; git status)"
        ));
        assert!(!parser_allows_rewrite_shape("{ git status; cargo test; }"));
    }

    #[test]
    fn parser_rejects_substitutions_and_extended_forms() {
        assert!(!parser_allows_rewrite_shape(
            "echo $(git status && cargo test)"
        ));
        assert!(!parser_allows_rewrite_shape("git status <<< foo"));
        assert!(!parser_allows_rewrite_shape(
            "git diff <(cat old) <(cat new)"
        ));
        assert!(!parser_allows_rewrite_shape(r"rg $'foo\nbar' src"));
    }
}
