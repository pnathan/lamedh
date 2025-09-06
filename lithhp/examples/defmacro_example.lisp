; This is an example of defmacro.
; It defines an `unless` macro that is the opposite of `if`.

(defmacro unless (condition true-branch false-branch)
  `(if (not ,condition)
       ,true-branch
       ,false-branch))

(print (unless (= 1 2)
         (concat "1 is not " "equal to 2")
         (concat "1 is " "equal to 2")))
