;;; lamed-nebula -- a truecolor deep-space still, painted in Lisp.
;;; Shows: value noise + fractional Brownian motion, iq-style domain
;;; warping, a multi-stop color gradient, Lambert-shaded sphere, additive
;;; light compositing, and 24-bit ANSI half-block (U+2580) pixel output
;;; at double vertical resolution.
;;; Run: cargo run -- examples/lamed-nebula/main.lisp
;;; (Best viewed in a truecolor terminal at >= 104 columns.)

;; ---------------------------------------------------------------------
;; canvas
(def $w 100)          ; pixels across
(def $h 64)           ; pixel rows (two per terminal line)
(def $esc (code-char 27))

(defun clamp01 (x) (max 0.0 (min 1.0 x)))
(defun c255 (x) (truncate (max 0.0 (min 255.0 x))))
(defun lerp (a b u) (+ a (* (- b a) u)))

(defun fg (r g b)
  (format nil "~a[38;2;~a;~a;~am" $esc (c255 r) (c255 g) (c255 b)))
(defun bg (r g b)
  (format nil "~a[48;2;~a;~a;~am" $esc (c255 r) (c255 g) (c255 b)))
(def $reset (format nil "~a[0m" $esc))

;; ---------------------------------------------------------------------
;; noise kernel: sin-fract lattice hash -> smoothed value noise -> fbm
(defun fract (x) (- x (floor x)))

(defun hash2 (x y)
  (fract (* 43758.5453123 (sin (+ (* x 12.9898) (* y 78.233))))))

(defun noise2 (x y)
  (let* ((xi (floor x)) (yi (floor y))
         (xf (- x xi))  (yf (- y yi))
         (u (* xf xf (- 3.0 (* 2.0 xf))))
         (v (* yf yf (- 3.0 (* 2.0 yf))))
         (a (hash2 xi yi))       (b (hash2 (1+ xi) yi))
         (c (hash2 xi (1+ yi)))  (d (hash2 (1+ xi) (1+ yi))))
    (lerp (lerp a b u) (lerp c d u) v)))

(defun fbm (x y)
  "Five octaves of value noise."
  (+ (* 0.5     (noise2 x y))
     (* 0.25    (noise2 (* 2.0 x) (* 2.0 y)))
     (* 0.125   (noise2 (* 4.0 x) (* 4.0 y)))
     (* 0.0625  (noise2 (* 8.0 x) (* 8.0 y)))
     (* 0.03125 (noise2 (* 16.0 x) (* 16.0 y)))))

;; ---------------------------------------------------------------------
;; palette: deep indigo -> violet -> magenta -> rose -> amber -> gold
(def $stops
  (list (list 0.00   4.0   3.0  18.0)
        (list 0.30  34.0  16.0  74.0)
        (list 0.52 104.0  30.0 128.0)
        (list 0.70 198.0  62.0 110.0)
        (list 0.85 246.0 142.0  74.0)
        (list 1.00 255.0 238.0 186.0)))

(defun pal (u)
  "Piecewise-linear gradient through $stops; returns (r g b)."
  (let ((uu (clamp01 u)))
    (pal-walk uu $stops)))

(defun pal-walk (u stops)
  (let* ((s0 (car stops)) (rest (cdr stops)))
    (if (null rest)
        (cdr s0)
        (let ((s1 (car rest)))
          (if (<= u (car s1))
              (let ((k (/ (- u (car s0)) (max 0.0001 (- (car s1) (car s0))))))
                (list (lerp (nth 1 s0) (nth 1 s1) k)
                      (lerp (nth 2 s0) (nth 2 s1) k)
                      (lerp (nth 3 s0) (nth 3 s1) k)))
              (pal-walk u rest))))))

;; ---------------------------------------------------------------------
;; scene: nebula + starfield + two suns + one banded planet
(def $star1x 23.0) (def $star1y 15.0)      ; hero sun (upper left)
(def $star2x 63.0) (def $star2y  7.0)      ; distant companion
(def $pcx 78.0) (def $pcy 45.0) (def $pr 12.5)  ; planet

;; light direction: from planet toward the hero sun, tilted out of plane
(def $lx -0.67) (def $ly -0.38) (def $lz 0.64)

(defun nebula (x y)
  "Domain-warped fbm density and warp channel: returns (density qy)."
  (let* ((px (* 0.058 x)) (py (* 0.058 y))
         (qx (fbm px py))
         (qy (fbm (+ px 5.2) (+ py 1.3)))
         (v  (fbm (+ px (* 2.6 qx)) (+ py (* 2.6 qy))))
         ;; a soft diagonal band of gas falling left-to-right
         (nx (/ x 100.0)) (ny (/ y 64.0))
         (c (- ny (+ 0.28 (* 0.38 nx))))
         (band (exp (/ (* c c) -0.055)))
         (d (* (expt (clamp01 v) 1.45) (+ 0.30 (* 1.05 band)))))
    (list (clamp01 d) qy)))

(defun star-glow (x y sx sy core spike)
  "Additive luminance of a 4-point diffraction star at (sx, sy)."
  (let* ((dx (- x sx)) (dy (- y sy))
         (d2 (+ (* dx dx) (* dy dy)))
         (glow (/ core (+ 1.0 (* 0.16 d2))))
         (dd (sqrt d2))
         (arm (max 0.0 (- 1.0 (/ dd 17.0))))
         (sh (* spike (max 0.0 (- 1.0 (* 0.55 (abs dy)))) arm))
         (sv (* spike (max 0.0 (- 1.0 (* 0.55 (abs dx)))) arm)))
    (+ glow (* arm (+ sh sv)))))

(defun planet-color (dx dy density)
  "Lambert-shaded banded gas giant; dx dy relative to center."
  (let* ((r $pr)
         (nz2 (- 1.0 (/ (+ (* dx dx) (* dy dy)) (* r r))))
         (nz (sqrt (max 0.0001 nz2)))
         (nx (/ dx r)) (ny (/ dy r))
         (lam (max 0.0 (+ (* nx $lx) (* ny $ly) (* nz $lz))))
         (shade (+ 0.06 (* 1.08 (expt lam 1.2))))
         ;; latitude bands, bent by curvature and stirred by noise
         (lat (* dy (+ 1.0 (* 0.35 nx nx))))
         (turb (fbm (+ (* 0.11 dx) 30.0) (+ (* 0.23 lat) 9.0)))
         (bandv (+ 0.5 (* 0.5 (sin (+ (* 0.62 lat) (* 5.0 turb))))))
         (bandv (* bandv bandv (- 3.0 (* 2.0 bandv))))  ; sharpen bands
         (br (lerp 158.0 244.0 bandv))     ; deep rust <-> warm cream
         (bgc (lerp 84.0 212.0 bandv))
         (bb (lerp 58.0 166.0 bandv))
         ;; thin cyan atmosphere at the lit rim
         (rim (* (expt (- 1.0 nz) 5.0) (+ 0.15 lam))))
    (list (+ (* br shade) (* 70.0 rim) (* 6.0 density))
          (+ (* bgc shade) (* 120.0 rim) (* 4.0 density))
          (+ (* bb shade) (* 200.0 rim) (* 10.0 density)))))

(defun pixel-at (x y)
  "Full scene composite for one pixel; returns (r g b)."
  (let* ((fx (+ 0.0 x)) (fy (+ 0.0 y))
         (pdx (- fx $pcx)) (pdy (- fy $pcy))
         (pd (sqrt (+ (* pdx pdx) (* pdy pdy))))
         (neb (nebula fx fy))
         (density (car neb)) (qy (nth 1 neb)))
    (if (< pd $pr)
        ;; foreground planet occludes everything behind it
        (planet-color pdx pdy density)
        ;; open space: gas, shadow tint, stars, suns, atmosphere halo
        (let* ((base (pal (clamp01 (* 1.28 density))))
               (teal (* 0.9 (clamp01 qy) (- 1.0 density)))
               (r (+ (car base)  (* 14.0 teal)))
               (g (+ (nth 1 base) (* 46.0 teal)))
               (b (+ (nth 2 base) (* 60.0 teal)))
               ;; pinprick starfield, dimmed where the gas is thick
               (sh (hash2 (+ fx 91.0) (+ fy 57.0)))
               (occ (- 1.0 (* 0.8 (clamp01 (* 1.4 density))))))
          (if (> sh 0.9885)
              (let* ((sb (* occ (expt (/ (- sh 0.9885) 0.0115) 2.0)))
                     (warm (hash2 (+ fx 7.0) (+ fy 3.0))))
                (if (> warm 0.5)
                    (setq r (+ r (* 255.0 sb)) g (+ g (* 226.0 sb))
                          b (+ b (* 178.0 sb)))
                    (setq r (+ r (* 172.0 sb)) g (+ g (* 204.0 sb))
                          b (+ b (* 255.0 sb)))))
              ())
          ;; the two suns
          (let ((l1 (star-glow fx fy $star1x $star1y 2.6 1.1))
                (l2 (star-glow fx fy $star2x $star2y 0.9 0.45)))
            (setq r (+ r (* 255.0 l1) (* 168.0 l2))
                  g (+ g (* 240.0 l1) (* 198.0 l2))
                  b (+ b (* 198.0 l1) (* 255.0 l2))))
          ;; soft blue halo hugging the planet's limb, strongest sunward
          (if (< pd (+ $pr 3.5))
              (let* ((lit (clamp01 (+ 0.5 (* 0.5 (/ (+ (* pdx $lx) (* pdy $ly))
                                                    (max 0.001 pd))))))
                     (halo (* (+ 0.2 (* 0.8 lit))
                              0.55 (exp (* -1.1 (- pd $pr))))))
                (setq r (+ r (* 70.0 halo))
                      g (+ g (* 120.0 halo))
                      b (+ b (* 215.0 halo))))
              ())
          ;; gentle vignette pulls the eye to the middle
          (let* ((vx (/ (- fx 50.0) 50.0)) (vy (/ (- fy 32.0) 32.0))
                 (vig (- 1.0 (* 0.30 (clamp01 (- (+ (* vx vx) (* vy vy)) 0.25))))))
            (list (* r vig) (* g vig) (* b vig)))))))

;; ---------------------------------------------------------------------
;; renderer: two pixel rows per terminal line via the upper half block
(def $hb (code-char 9600))                 ; U+2580
(def $frame-fg (fg 96.0 78.0 142.0))       ; dim violet frame

(defun str-repeat (s n)
  (let ((out ""))
    (dotimes (i n) (setq out (concat out s)))
    out))

(defun render-line (j)
  (let ((line (concat $frame-fg (code-char 9474) $reset))) ; │
    (dotimes (x $w)
      (let ((top (pixel-at x (* 2 j)))
            (bot (pixel-at x (1+ (* 2 j)))))
        (setq line (concat line
                           (fg (car top) (nth 1 top) (nth 2 top))
                           (bg (car bot) (nth 1 bot) (nth 2 bot))
                           $hb))))
    (concat line $reset $frame-fg (code-char 9474) $reset)))

;; ---------------------------------------------------------------------
;; plaque: gradient rule and title
(defun gradient-rule (width)
  (let ((line ""))
    (dotimes (x width)
      (let ((c (pal (/ (+ 0.0 x) (- width 1.0)))))
        (setq line (concat line
                           (fg (* 0.85 (car c)) (* 0.85 (nth 1 c))
                               (* 0.85 (nth 2 c)))
                           (code-char 9473)))))            ; ━
    (concat line $reset)))

(defun gradient-title (letters lo hi)
  "Color each string in LETTERS along pal(lo..hi), double-spaced."
  (let ((n (length letters)) (i 0) (line ""))
    (for-each
     (lambda (ch)
       (let ((c (pal (+ lo (* (- hi lo) (/ (+ 0.0 i) (- n 1.0)))))))
         (setq line (concat line (fg (car c) (nth 1 c) (nth 2 c)) ch
                            (if (< i (- n 1)) "  " "")))
         (setq i (1+ i))))
     letters)
    (concat line $reset)))

(def $dim (fg 128.0 118.0 152.0))
(def $lamed (code-char 1500))              ; ל

;; ---------------------------------------------------------------------
;; show
(princ (format nil "~%  ~a~a~a~a~%" $frame-fg (code-char 9484)
               (str-repeat (code-char 9472) $w) (code-char 9488)))
(def $lines (mapcar #'render-line (iota (/ $h 2))))
(for-each (lambda (line) (format t "  ~a~%" line)) $lines)
(princ (format nil "  ~a~a~a~a~a~%" $frame-fg (code-char 9492)
               (str-repeat (code-char 9472) $w) (code-char 9496) $reset))
(format t "  ~a~%" (gradient-rule (+ $w 2)))
(format t "~%~a~a~a~a~a~a~%"
        (str-repeat " " 50) $dim (code-char 183)
        (concat " " (fg 255.0 238.0 186.0) $lamed $dim " ") (code-char 183)
        $reset)
(format t "~a~a~%" (str-repeat " " 27)
        (gradient-title (list "T" "H" "E" " " " " "L" "A" "M" "E" "D"
                              " " " " "N" "E" "B" "U" "L" "A")
                        0.45 1.0))
(format t "~a~adomain-warped fBm ~a value noise ~a one frame, no loops left running~a~%~%"
        (str-repeat " " 19) $dim (code-char 183) (code-char 183) $reset)

;; silent self-check: geometry and color ranges hold; errors if not.
(let ((probe (pixel-at 50 32)))
  (if (and (= (length $lines) 32)
           (= (length probe) 3)
           (<= 0 (c255 (car probe)) 255)
           (equal (pal 0.0) (list 4.0 3.0 18.0)))
      ()
      (error "lamed-nebula self-check failed")))
