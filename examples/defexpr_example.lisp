; This is an example of defexpr.
; It defines a f-expression `quote-args` that returns its arguments without evaluating them.

(defexpr quote-args (args)
  args)

(print (quote-args (+ 1 2) "hello"))
