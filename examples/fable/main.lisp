;;; fable -- Aesop's proofs: four fables run as simulations, and each
;;; moral is filed as a THEOREM about the run. Where honesty demands it,
;;; a counterfactual is run too: remove the flaw and the story must come
;;; out the other way, or the moral was never earned.
;;; Shows: records + record-with as story state, closures as temperament
;;; and as a fox's private heart, variants folded into a chronicle,
;;; and conditions/restarts as the ending a policy chooses.
;;; Run: cargo run -- examples/fable/main.lisp

;; ---------------------------------------------------------------- the book
(def $morals ())

(defun moral (title lesson holds)
  "File LESSON in the book, with whether the run actually bore it out."
  (push (list title lesson holds) $morals)
  holds)

;; ============================================================== fable i
;; THE TORTOISE AND THE HARE
;; A runner is a record; a temperament is a function from the runner and
;; the rival's position to the runner's next self.

(defrecord runner (name symbol) (pos int64) (nap int64))

(defun plod (me rival-pos)
  "One pace, every tick, no opinions."
  (record-with me 'pos (+ (runner-pos me) 1)))

(defun swagger (me rival-pos)
  "Five paces a stride -- but any ten-length lead earns a long nap."
  (cond ((> (runner-nap me) 0)
         (record-with me 'nap (- (runner-nap me) 1)))
        ((>= (- (runner-pos me) rival-pos) 10)
         (record-with me 'nap 40))
        (t (record-with me 'pos (+ (runner-pos me) 5)))))

(defun sprint (me rival-pos)
  "The counterfactual hare: all speed, no naps."
  (record-with me 'pos (+ (runner-pos me) 5)))

(defun race (hare-mind hare tortoise goal tick)
  "First to GOAL. Ties go to the tortoise, as a courtesy of the road."
  (cond ((>= (runner-pos tortoise) goal) (list 'tortoise tick (runner-pos hare)))
        ((>= (runner-pos hare) goal) (list 'hare tick (runner-pos tortoise)))
        (t (race hare-mind
                 (funcall hare-mind hare (runner-pos tortoise))
                 (plod tortoise (runner-pos hare))
                 goal (+ tick 1)))))

(defun run-race (hare-mind)
  (race hare-mind (make-runner 'hare 0 0) (make-runner 'tortoise 0 0) 100 0))

(def $slow-story (run-race #'swagger))
(def $counter-story (run-race #'sprint))

(format t "i.   the ~a wins at tick ~a; the hare is found asleep at pace ~a~%"
        (car $slow-story) (cadr $slow-story) (caddr $slow-story))
(format t "     (counterfactual: a hare that never naps wins -- so it was~%")
(format t "      the napping, not the speed, that lost the race)~%")

(moral 'the-tortoise-and-the-hare
       "Slow and steady wins the race."
       (and (equal (car $slow-story) 'tortoise)
            (equal (car $counter-story) 'hare)))

;; ============================================================== fable ii
;; THE BOY WHO CRIED WOLF
;; The village is a fold over cries. Trust is spent by lying and is the
;; only currency that summons help.

(defvariant cry
  (false-alarm)
  (wolf))

(defrecord village (trust int64) (sheep int64))

(defun hear (v c)
  (variant-case c
    (false-alarm ()
      (format t "     the villagers come running; there is no wolf; they go home~%")
      (if (> (village-trust v) 0)
          (record-with v 'trust (- (village-trust v) 1))
          v))
    (wolf ()
      (if (> (village-trust v) 0)
          (progn (format t "     a real wolf! the villagers arrive in time~%") v)
          (progn (format t "     a real wolf! ... nobody comes~%")
                 (record-with v 'sheep 0))))))

(defun tend (events)
  (reduce #'hear events (make-village 2 40)))

(format t "ii.  the boy who cried wolf:~%")
(def $lied-to (tend (list (false-alarm) (false-alarm) (wolf))))
(def $honest (tend (list (wolf))))

(moral 'the-boy-who-cried-wolf
       "Nobody believes a liar, even when he tells the truth."
       (and (= (village-sheep $lied-to) 0)     ; same wolf, flock lost
            (= (village-sheep $honest) 40)))   ; same wolf, flock saved

;; ============================================================== fable iii
;; THE ANT AND THE GRASSHOPPER
;; The grasshopper's knock is a CONDITION; the ant's answer is a RESTART.
;; Both endings are reachable from the same winter -- the ending belongs
;; to whoever holds the restart, which is the most Aesop fact in this file.

(defun knock (songs)
  "Detection only: the fiddler is at the door with songs and no grain."
  (error (concat "a thin fiddler knocks, offering "
                 (princ-to-string songs) " songs for supper")))

(defun winter (ant-grain need policy)
  "POLICY is share-the-larder or a-lesson-instead. Returns
(ant's-grain grasshopper's-grain) when the thaw comes."
  (restart-case
      (handler-bind ((error (lambda (c) (invoke-restart policy))))
        (knock 30))
    (share-the-larder () (list (- ant-grain need) need))
    (a-lesson-instead () (list ant-grain 0))))

(defun survives-p (grain need) (>= grain need))

(def $kind-ending (winter 30 10 'share-the-larder))
(def $stern-ending (winter 30 10 'a-lesson-instead))

(format t "iii. kind winter:  ant keeps ~a, grasshopper gets ~a~%"
        (car $kind-ending) (cadr $kind-ending))
(format t "     stern winter: ant keeps ~a, grasshopper gets ~a~%"
        (car $stern-ending) (cadr $stern-ending))

(moral 'the-ant-and-the-grasshopper
       "There is a time for work and a time for play -- and in no ending do songs become grain."
       (and (survives-p (car $kind-ending) 10)        ; ant lives either way
            (survives-p (car $stern-ending) 10)
            (survives-p (cadr $kind-ending) 10)       ; mercy is reachable
            (not (survives-p (cadr $stern-ending) 10)))) ; so is Aesop's cut

;; ============================================================== fable iv
;; THE FOX AND THE GRAPES
;; A fox is a closure with a private heart. Nothing outside can touch the
;; heart; only failure can. The grapes never change. The verdict does.

(def $sweetness 9)                       ; bound once; never touched again

(defun make-fox (leap)
  (let ((heart (list->array (list 0))))  ; grudges, privately held
    (lambda (msg arg)
      (cond ((equal msg 'jump)
             (if (>= leap arg)
                 'got-them
                 (progn (put! heart 0 (+ 1 (ref heart 0))) 'missed)))
            ((equal msg 'appraise)
             (if (> (ref heart 0) 2)
                 'sour                   ; the heart outvotes the tongue
                 (if (> arg 5) 'sweet 'meh)))))))

(def $short-fox (make-fox 6))
(def $before (funcall $short-fox 'appraise $sweetness))
(dotimes (i 3) (funcall $short-fox 'jump 8))     ; three leaps at height 8
(def $after (funcall $short-fox 'appraise $sweetness))

(def $tall-fox (make-fox 9))
(def $tall-jump (funcall $tall-fox 'jump 8))
(def $tall-verdict (funcall $tall-fox 'appraise $sweetness))

(format t "iv.  before leaping, the fox calls the grapes ~a;~%" $before)
(format t "     after three misses, the same fox calls the same grapes ~a~%" $after)
(format t "     (a taller fox ~a and calls them ~a)~%" $tall-jump $tall-verdict)

(moral 'the-fox-and-the-grapes
       "It is easy to despise what you cannot reach."
       (and (equal $before 'sweet)
            (equal $after 'sour)
            (= $sweetness 9)                     ; the grapes never changed
            (equal $tall-jump 'got-them)
            (equal $tall-verdict 'sweet)))

;; ---------------------------------------------------------- the colophon
(format t "~%--- the book of proven morals ---~%")
(for-each (lambda (entry)
            (format t "~a ~a~%     ~a~%"
                    (if (caddr entry) "[proved]" "[FAILED]")
                    (car entry) (cadr entry)))
          (reverse $morals))

;; self-check: four fables told, every moral borne out by its own run.
(if (and (= (length $morals) 4)
         (reduce (lambda (acc entry) (and acc (caddr entry))) $morals t))
    (print 'ok)
    (error "fable self-check failed: a moral was asserted but not earned"))
