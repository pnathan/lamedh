#!/bin/bash
# Benchmark runner for lamedh fibonacci

RUN_MS=${1:-5000}
WARMUP_MS=${2:-1000}
N=${3:-30}

LAMEDH="../../../target/release/lamedh"

# Build lamedh if needed
if [ ! -f "$LAMEDH" ]; then
    echo "Building lamedh..." >&2
    (cd ../../.. && cargo build --release) >&2
fi

# Create a temporary benchmark wrapper
cat > /tmp/lamedh_fib_bench.lisp << EOF
(defun fibonacci (n)
  "Calculate the nth Fibonacci number using naive recursion"
  (cond
    ((eq n 0) 0)
    ((eq n 1) 1)
    (t (+ (fibonacci (- n 1)) (fibonacci (- n 2))))))

(defun fibonacci-sum (n)
  "Calculate sum of fibonacci numbers from 1 to n-1"
  (prog (result i)
    (setq result 0)
    (setq i 1)
    loop
    (cond ((< i n) (go continue)))
    (return result)
    continue
    (setq result (+ result (fibonacci i)))
    (setq i (+ i 1))
    (go loop)))

;; Warmup
(print "Warmup...")
(fibonacci-sum $N)

;; Actual benchmark
(print "Running benchmark...")
(fibonacci-sum $N)
EOF

# Run the benchmark and time it
START_TIME=$(date +%s%N)
RESULT=$($LAMEDH -i /tmp/lamedh_fib_bench.lisp 2>/dev/null | tail -1)
END_TIME=$(date +%s%N)

ELAPSED_NS=$((END_TIME - START_TIME))
ELAPSED_MS=$(echo "scale=6; $ELAPSED_NS / 1000000" | bc)

# Output in CSV format (simple version - just one iteration)
echo "$ELAPSED_MS,0.0,$ELAPSED_MS,$ELAPSED_MS,1,$RESULT"

rm /tmp/lamedh_fib_bench.lisp
