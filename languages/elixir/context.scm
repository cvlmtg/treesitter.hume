(binary_operator
  left: (_)
  right: (_) @context)

(pair
  key: (_)
  value: (_) @context)

((unary_operator
  operand: (call
    target: (identifier)
    (arguments
      (_)))) @_op
  (#match? @_op "@[A-Za-z0-9_]+")) @context

(stab_clause
  right: (body
    (_) @context.end)) @context

(call
  target: (identifier)
  (do_block
    (_) @context.end)) @context
