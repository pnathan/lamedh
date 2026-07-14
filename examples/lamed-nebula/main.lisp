;;; lamed-nebula -- a truecolor deep-space still, painted in Lisp.
;;; Shows: value noise + fractional Brownian motion, iq-style domain
;;; warping, a multi-stop color gradient, Lambert-shaded sphere, additive
;;; light compositing, and 24-bit ANSI half-block (U+2580) pixel output
;;; at double vertical resolution.
;;; Also: a from-scratch PNG encoder (CRC-32, Adler-32, stored DEFLATE).
;;; Run (terminal): cargo run -- examples/lamed-nebula/main.lisp
;;;   (Best viewed in a truecolor terminal at >= 104 columns.)
;;; Run (PNG file): cargo run -- --capability CREATE-FS \
;;;                   examples/lamed-nebula/main.lisp nebula.png [scale]
;;;   A trailing path argument opts into a PNG (default 300x192); an
;;;   optional integer SCALE renders at (100*scale)x(64*scale), e.g.
;;;   scale 30 -> 3000x1920. With no argument the program renders to the
;;;   terminal exactly as before. Huge scales are slow (tree-walking
;;;   interpreter, millions of fBm pixels) -- build --release first.

;; ---------------------------------------------------------------------
;; canvas
(def $w 100)          ; pixels across
(def $h 64)           ; pixel rows (two per terminal line)
(def $esc (code-char 27))

;; clamp01 and lerp are TYPED (native): they sit on the per-pixel hot path
;; and are called from the compiled noise/palette kernels. min/max aren't
;; in the compiled subset, so clamp01 uses if-guards -- bit-identical to
;; (max 0.0 (min 1.0 x)) for every finite input.
(defun-typed (clamp01 float64) ((x float64))
  (if (< x 0.0) 0.0 (if (> x 1.0) 1.0 x)))
(defun-typed (lerp float64) ((a float64) (b float64) (u float64))
  (+ a (* (- b a) u)))
;; c255 stays interpreted (it is the byte-membrane: float -> 0..255 int).
(defun c255 (x) (truncate (max 0.0 (min 255.0 x))))

;; T iff FN reached native code (explain-compile's TIER is COMPILED).
(defun compiled-p (fn)
  (equal (cdr (assoc 'tier (explain-compile fn))) 'compiled))

(defun fg (r g b)
  (format nil "~a[38;2;~a;~a;~am" $esc (c255 r) (c255 g) (c255 b)))
(defun bg (r g b)
  (format nil "~a[48;2;~a;~a;~am" $esc (c255 r) (c255 g) (c255 b)))
(def $reset (format nil "~a[0m" $esc))

;; ---------------------------------------------------------------------
;; noise kernel: sin-fract lattice hash -> smoothed value noise -> fbm.
;; ALL THREE ARE TYPED (native code): this is the dominant per-pixel cost
;; -- one nebula() call evaluates fbm three times, i.e. ~60 sines. The
;; typed bodies mirror the interpreted math exactly (same operand order),
;; so results are bit-identical to the tree-walked version and the terminal
;; render stays byte-for-byte unchanged; the compiler just runs it native.
;; The fract idiom is (- z (float (floor z))) because the typed island is
;; strict about int/float mixing.
(defun-typed (hash2 float64) ((x float64) (y float64))
  (let ((z (* 43758.5453123 (sin (+ (* x 12.9898) (* y 78.233))))))
    (- z (float (floor z)))))

;; No let* in the typed subset, so bindings nest; (1+ xi) becomes (+ xi 1.0)
;; and floor's int result is lifted with (float ...) before reuse.
(defun-typed (noise2 float64) ((x float64) (y float64))
  (let ((xi (float (floor x))) (yi (float (floor y))))
    (let ((xf (- x xi)) (yf (- y yi)))
      (let ((u (* xf xf (- 3.0 (* 2.0 xf))))
            (v (* yf yf (- 3.0 (* 2.0 yf))))
            (a (hash2 xi yi))          (b (hash2 (+ xi 1.0) yi))
            (c (hash2 xi (+ yi 1.0)))  (d (hash2 (+ xi 1.0) (+ yi 1.0))))
        (lerp (lerp a b u) (lerp c d u) v)))))

(defun-typed (fbm float64) ((x float64) (y float64))
  ;; Five octaves of value noise.
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

;; TYPED (native). Additive luminance of a 4-point diffraction star.
;; (max 0.0 e) -> (if (< e 0.0) 0.0 e) and (abs d) -> (if (< d 0.0) (- 0.0 d) d),
;; each bit-identical to the interpreted form for finite inputs.
(defun-typed (star-glow float64)
    ((x float64) (y float64) (sx float64) (sy float64)
     (core float64) (spike float64))
  (let ((dx (- x sx)) (dy (- y sy)))
    (let ((d2 (+ (* dx dx) (* dy dy)))
          (adx (if (< dx 0.0) (- 0.0 dx) dx))
          (ady (if (< dy 0.0) (- 0.0 dy) dy)))
      (let ((glow (/ core (+ 1.0 (* 0.16 d2))))
            (arm  (let ((e (- 1.0 (/ (sqrt d2) 17.0)))) (if (< e 0.0) 0.0 e))))
        (let ((sh (* spike (let ((e (- 1.0 (* 0.55 ady)))) (if (< e 0.0) 0.0 e)) arm))
              (sv (* spike (let ((e (- 1.0 (* 0.55 adx)))) (if (< e 0.0) 0.0 e)) arm)))
          (+ glow (* arm (+ sh sv))))))))

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
               ;; pinprick starfield, dimmed where the gas is thick.
               ;; Sampled on the integer 100x64 lattice (floor) so a star is
               ;; one grid cell whether we render one pixel per cell (terminal)
               ;; or supersample for the PNG -- at integer coords floor is the
               ;; identity, so the terminal output is unchanged.
               (sfx (+ 0.0 (floor fx))) (sfy (+ 0.0 (floor fy)))
               (sh (hash2 (+ sfx 91.0) (+ sfy 57.0)))
               (occ (- 1.0 (* 0.8 (clamp01 (* 1.4 density))))))
          (if (> sh 0.9885)
              (let* ((sb (* occ (expt (/ (- sh 0.9885) 0.0115) 2.0)))
                     (warm (hash2 (+ sfx 7.0) (+ sfy 3.0))))
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
;; terminal show -- the default, unchanged path
(defun render-terminal ()
  (princ (format nil "~%  ~a~a~a~a~%" $frame-fg (code-char 9484)
                 (str-repeat (code-char 9472) $w) (code-char 9488)))
  (let ((lines (mapcar #'render-line (iota (/ $h 2)))))
    (for-each (lambda (line) (format t "  ~a~%" line)) lines)
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
    ;; silent self-check: geometry + color ranges hold, AND the number-
    ;; crunching kernels actually reached native code (a regression that
    ;; dropped one back to interpreted would still be correct, just slow --
    ;; this makes it loud). Prints nothing; errors on failure.
    (let ((probe (pixel-at 50 32)))
      (if (and (= (length lines) 32)
               (= (length probe) 3)
               (<= 0 (c255 (car probe)) 255)
               (equal (pal 0.0) (list 4.0 3.0 18.0))
               (compiled-p 'fbm) (compiled-p 'noise2) (compiled-p 'hash2)
               (compiled-p 'star-glow) (compiled-p 'clamp01) (compiled-p 'lerp)
               (compiled-p 'fold-checksums))
          ()
          (error "lamed-nebula self-check failed")))))

;; =====================================================================
;; PNG WRITER  (opt-in: `... examples/lamed-nebula/main.lisp out.png`)
;; ---------------------------------------------------------------------
;; A from-scratch PNG encoder in Lisp: 8-byte signature, IHDR (8-bit
;; truecolor RGB), one IDAT carrying a zlib stream of *uncompressed*
;; ("stored") DEFLATE blocks (one per scanline) with a trailing Adler-32,
;; and IEND. Every chunk carries a CRC-32. No zlib, no host help -- just
;; logand/logior/logxor/ash and mod.
;;
;; ARRAYS, NOT LISTS, on every per-pixel/per-byte path: at 3000x1920 the
;; image is ~5.8M pixels / ~17M bytes, so a cons list there would both
;; churn allocation and (since MAPC/FOR-EACH recurses one eval frame per
;; element) blow the eval-depth limit. Instead each scanline is a reused
;; flat byte ARRAY filled by index with DOTIMES/STORE, streamed straight
;; to the port, with CRC-32 and Adler-32 folded in as running state --
;; so memory stays bounded to one row regardless of image size. Only tiny
;; fixed-size headers (<= 8 bytes) are ever lists.
;;
;; The PNG has no half-block compromise, so it renders at true
;; one-cell-per-pixel resolution; the caller-chosen scale over the 100x64
;; base IS the detail (and the supersampling), so no extra oversampling.

;; ── little integer helpers ────────────────────────────────────────────
(defun be32 (x)                             ; 4 bytes, big-endian, as a list
  (list (logand (ash x -24) 255) (logand (ash x -16) 255)
        (logand (ash x -8) 255)  (logand x 255)))

;; ── CRC-32 (table-driven; IEEE 802.3 polynomial). Table is an ARRAY. ──
(defun crc-entry (n)
  (let ((c n))
    (dotimes (k 8)
      (if (= 1 (logand c 1))
          (setq c (logxor #xedb88320 (ash c -1)))
          (setq c (ash c -1))))
    c))
(def $crc-table (list->array (mapcar #'crc-entry (iota 256))))

(defun crc-step (c b)                       ; fold one byte into a CRC state
  (logxor (fetch $crc-table (logand (logxor c b) 255)) (ash c -8)))

(defun crc-feed (c lst)                     ; fold a SMALL fixed list of bytes
  (let ((cc c))
    (for-each (lambda (b) (setq cc (crc-step cc b))) lst)
    cc))

(defun crc32 (bytes)                        ; CRC-32 of a SMALL list (headers)
  (logxor (crc-feed #xffffffff bytes) #xffffffff))

;; TYPED (native): fold a WHOLE scanline array into the running CRC-32 and
;; Adler-32 in one compiled tail-recursive pass. STATE is a 3-int array
;; [crc, adler-a, adler-b]; TABLE is the CRC table; both mutate in place.
;; This replaces what used to be a per-byte interpreted DOTIMES -- at high
;; scale that was ~17M tree-walked iterations; here it is one native loop
;; per row (the shift `(ash c -8)` is a literal constant, so it compiles).
(defun-typed (fold-checksums int64)
    ((table (array int64)) (state (array int64)) (row (array int64))
     (i int64) (n int64))
  (if (>= i n)
      0
      (progn
        (store state 0
               (logxor (fetch table
                              (logand (logxor (fetch state 0) (fetch row i)) 255))
                       (ash (fetch state 0) -8)))
        (store state 1 (mod (+ (fetch state 1) (fetch row i)) 65521))
        (store state 2 (mod (+ (fetch state 2) (fetch state 1)) 65521))
        (fold-checksums table state row (+ i 1) n))))

;; ── chunk framing (for the tiny IHDR/IEND chunks): len|type|data|CRC ──
(defun png-chunk (type-bytes data)
  (append (be32 (length data))
          type-bytes data
          (be32 (crc32 (append type-bytes data)))))

(def $type-ihdr (list 73 72 68 82))         ; "IHDR"
(def $type-idat (list 73 68 65 84))         ; "IDAT"
(def $type-iend (list 73 69 78 68))         ; "IEND"

(defun write-list! (p lst)
  "Write a SMALL fixed-size byte list to port P."
  (ports:write-bytes! p (list->array lst)))

;; ── the streaming, array-based renderer ───────────────────────────────
(defun render-png (path scale)
  (require 'ports)
  ;; Open the port FIRST so the CREATE-FS capability check fails fast,
  ;; before the (potentially very long) render.
  (ports:with-open-port (p (ports:open-output path))
    (let* ((width  (* $w scale))
           (height (* $h scale))
           (invsc  (/ 1.0 (+ 0.0 scale)))
           (rowlen (+ 1 (* 3 width)))        ; filter byte + RGB triples
           (nlen   (logxor rowlen 65535))    ; stored-block ~LEN (rowlen<64KiB)
           ;; IDAT payload length is exactly computable up front, so we can
           ;; write the chunk length prefix before streaming the body:
           ;; 2 zlib-header bytes + 5 block-header bytes/scanline + raw + 4.
           (idat-len (+ 2 (* height 5) (* height rowlen) 4))
           (row (make-array rowlen))         ; ONE reused scanline buffer
           (cs  (make-array 3))              ; [crc, adler-a, adler-b] for the
                                             ; native fold-checksums kernel
           (crc #xffffffff) (aa 1) (ab 0))   ; running CRC / Adler state
      (format t "Painting ~ax~a (scale ~a) ...~%" width height scale)
      ;; signature + IHDR chunk
      (write-list! p (list 137 80 78 71 13 10 26 10))
      (write-list! p (png-chunk $type-ihdr
                                (append (be32 width) (be32 height)
                                        (list 8 2 0 0 0))))  ; 8-bit truecolor
      ;; IDAT: length prefix, then type + body streamed while CRC accrues
      (write-list! p (be32 idat-len))
      (setq crc (crc-feed crc $type-idat))
      (write-list! p $type-idat)
      (setq crc (crc-feed crc (list 120 1)))     ; zlib header 0x78 0x01
      (write-list! p (list 120 1))
      (dotimes (py height)
        ;; stored-block header for this scanline
        (let ((hdr (list (if (= py (- height 1)) 1 0)   ; BFINAL on last row
                         (logand rowlen 255) (logand (ash rowlen -8) 255)
                         (logand nlen 255)   (logand (ash nlen -8) 255))))
          (setq crc (crc-feed crc hdr))
          (write-list! p hdr))
        ;; fill the row array by index (filter byte 0, then RGB)
        (let ((sy (* invsc (+ 0.0 py))) (o 1))
          (store row 0 0)
          (dotimes (px width)
            (let ((c (pixel-at (* invsc (+ 0.0 px)) sy)))
              (store row o        (c255 (car c)))
              (store row (+ o 1)  (c255 (nth 1 c)))
              (store row (+ o 2)  (c255 (nth 2 c)))
              (setq o (+ o 3))))
          ;; fold this scanline's bytes into CRC and Adler natively: load the
          ;; running state, run the compiled kernel over the whole row, read
          ;; it back. (Native loop instead of a per-byte interpreted DOTIMES.)
          (store cs 0 crc) (store cs 1 aa) (store cs 2 ab)
          (fold-checksums $crc-table cs row 0 rowlen)
          (setq crc (fetch cs 0) aa (fetch cs 1) ab (fetch cs 2))
          (ports:write-bytes! p row)))
      ;; Adler-32 trailer, then the IDAT chunk CRC
      (let ((adler (logior (ash ab 16) aa)))
        (setq crc (crc-feed crc (be32 adler)))
        (write-list! p (be32 adler)))
      (write-list! p (be32 (logxor crc #xffffffff)))
      ;; IEND
      (write-list! p (png-chunk $type-iend ()))
      ;; self-check + report (total = sig 8 + IHDR 25 + IDAT 12+len + IEND 12)
      (if (and (< 0 width) (< 0 height) (< rowlen 65536))
          (format t "Wrote ~a (~ax~a, ~a bytes).~%"
                  path width height (+ idat-len 57))
          (error "lamed-nebula PNG self-check failed")))))

;; ---------------------------------------------------------------------
;; dispatch:
;;   no argument            -> paint the terminal (default, unchanged)
;;   PATH                   -> small PNG at scale 3  (300x192)
;;   PATH SCALE             -> PNG at (100*SCALE)x(64*SCALE); e.g. 30 -> 3000x1920
(if (and (boundp '*ARGV*) *ARGV* (car *ARGV*))
    (render-png (car *ARGV*)
                (if (nth 1 *ARGV*)
                    (max 1 (truncate (string->number (nth 1 *ARGV*))))
                    3))
    (render-terminal))
