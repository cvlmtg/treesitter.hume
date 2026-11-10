[
  (package_statement)
  (class_statement)
  (subroutine_declaration_statement)
  (method_declaration_statement)
  (anonymous_subroutine_expression)
  (anonymous_method_expression)
  (phaser_statement)
  (class_phaser_statement)
  (conditional_statement)
  (for_statement)
  (cstyle_for_statement)
  (loop_statement)
  (try_statement)
  (defer_statement)
  (do_expression)
  (anonymous_hash_expression)
  (anonymous_array_expression)
] @context

; An elsif or else holds the next one, so each heads only its own block.
(elsif
  block: (block) @context) @context.start

(else
  block: (block) @context) @context.start

(try_statement
  catch_block: (block) @context)

(try_statement
  finally_block: (block) @context)
