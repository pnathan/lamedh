;;;; lamedh.asd -- ASDF system definition for the SBCL port of Lamedh.

(asdf:defsystem "lamedh"
  :description "A standalone Common Lisp (SBCL) implementation of the Lamedh Lisp 1.5 dialect."
  :author "Lamedh contributors"
  :license "AGPL-3.0"
  :pathname "src"
  :serial t
  :components ((:file "package")
               (:file "reader")
               (:file "runtime")
               (:file "printer")
               (:file "builtins")
               (:file "extra")
               (:file "io")
               (:file "bootstrap")
               (:file "cli")))
