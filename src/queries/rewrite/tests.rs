use super::*;

fn context(text: &str) -> String {
    for_hume("context.scm", text).expect("rewrites")
}

fn folds(text: &str) -> String {
    for_hume("folds.scm", text).expect("rewrites")
}

#[test]
fn a_wildcard_child_of_a_body_goes() {
    assert_eq!(
        context("(if_expression\n  consequence: (_\n    (_) @context.end)) @context\n"),
        "(if_expression\n  consequence: (_)) @context\n"
    );
}

#[test]
fn a_fielded_child_goes_with_its_field() {
    assert_eq!(
        context("(struct_item\n  name: (_) @context.end) @context\n"),
        "(struct_item) @context\n"
    );
}

#[test]
fn an_unfielded_child_goes_and_its_siblings_stay() {
    assert_eq!(
        context("(preproc_if (_) (_) @context.end) @context\n"),
        "(preproc_if (_)) @context\n"
    );
}

#[test]
fn a_child_of_an_intermediate_node_goes() {
    assert_eq!(
        context("(keyframe_block (block (_) @context.end)) @context\n"),
        "(keyframe_block (block)) @context\n"
    );
}

#[test]
fn a_named_child_goes() {
    assert_eq!(
        context("(struct_expression\n  (type_identifier) @context.end) @context\n"),
        "(struct_expression) @context\n"
    );
}

#[test]
fn a_child_goes_with_its_own_children() {
    assert_eq!(
        context("(else_clause\n  (block\n    (_)) @context.end) @context\n"),
        "(else_clause) @context\n"
    );
}

#[test]
fn an_anonymous_token_goes_with_its_quantifier() {
    assert_eq!(
        context("(annotation \".end annotation\" @context.end) @context\n"),
        "(annotation) @context\n"
    );
    assert_eq!(
        context("(param \".end param\"? @context.end) @context\n"),
        "(param) @context\n"
    );
}

#[test]
fn a_fielded_alternation_goes() {
    assert_eq!(
        context(
            "(if consequence: [ (statement_list) (field_declaration_list) ] @context.end) @context\n"
        ),
        "(if) @context\n"
    );
}

#[test]
fn a_bare_wildcard_goes() {
    assert_eq!(
        context("(function_definition parameters: (_) _ @context.end) @context\n"),
        "(function_definition parameters: (_)) @context\n"
    );
}

#[test]
fn a_quantified_child_goes() {
    assert_eq!(
        context("(_ ([ (a) (b) ] (_)+ @context.end)) @context\n"),
        "(_ ([ (a) (b) ])) @context\n"
    );
}

/// The node encloses `@context`, so only the capture goes.
#[test]
fn a_node_holding_another_capture_loses_only_the_capture() {
    assert_eq!(
        context("(haskell (header module: (module) @context)) @context.end\n"),
        "(haskell (header module: (module) @context))\n"
    );
}

#[test]
fn the_anchor_before_a_removed_sibling_goes() {
    assert_eq!(
        context("(if_statement (else) @context . (_) @context.end)\n"),
        "(if_statement (else) @context)\n"
    );
}

#[test]
fn a_predicate_naming_the_capture_goes() {
    assert_eq!(
        context("(call (identifier) @context.end (#eq? @context.end \"x\")) @context\n"),
        "(call) @context\n"
    );
}

#[test]
fn a_supertype_child_reads_as_one_name() {
    assert_eq!(
        context("(case (expression/case) @context.end) @context\n"),
        "(case) @context\n"
    );
}

#[test]
fn strings_and_comments_are_left_alone() {
    let text = "; @context.end in a comment\n(string \"@context.end\") @context\n";
    assert_eq!(context(text), text);
}

/// Cutting back to the comment's end would put the closing paren inside
/// the comment.
#[test]
fn a_removed_child_after_a_comment_keeps_the_comment_line() {
    assert_eq!(
        context("(block\n  ; end\n  (_) @context.end) @context\n"),
        "(block\n  ; end\n  ) @context\n"
    );
}

#[test]
fn repetition_leaves_folds() {
    assert_eq!(
        folds("[\n  (a)\n  (b)+\n] @fold\n\n[ (c) (d) ]* @fold\n"),
        "[\n  (a)\n  (b)\n] @fold\n\n[ (c) (d) ]? @fold\n"
    );
}

#[test]
fn a_quantifier_in_a_string_stays() {
    let text = "((x) @fold (#match? @fold \"a+\"))\n";
    assert_eq!(folds(text), text);
}

#[test]
fn other_files_are_unchanged() {
    let text = "(x)+ @y (_ (_) @context.end)\n";
    assert_eq!(for_hume("highlights.scm", text).expect("rewrites"), text);
}

#[test]
fn an_unbalanced_query_is_an_error() {
    assert!(for_hume("context.scm", "(a (b) @context.end\n").is_err());
}
