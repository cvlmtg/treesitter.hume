; A function holds its clauses, so each clause heads its own body.
[
  (function_clause)
  (anonymous_function)
  (stab_clause)
  (case)
  (if)
  (receive)
  (after)
  (try)
  (maybe)
  (block)
  (clause)
] @context

; A try's catch section and a maybe's else section are fields, not nodes, so
; their keyword heads the rest of the expression.
(try
  "catch" @context.start) @context

(maybe
  "else" @context.start) @context
