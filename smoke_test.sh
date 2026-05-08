#!/bin/bash

# check for dependencies
HAVE_RUST=false
HAVE_CLANG_OMP=false
HAVE_OPENCILK=false
HAVE_PYTHON_DEPS=false

python3 -c "import numpy, matplotlib, pandas" &>/dev/null 2>&1 && HAVE_PYTHON_DEPS=true

command -v rustc &>/dev/null && command -v cargo &>/dev/null && HAVE_RUST=true
command -v clang &>/dev/null && echo | clang -fopenmp -x c -c -o /dev/null - &>/dev/null 2>&1 && HAVE_CLANG_OMP=true
test -x "$OPENCILK_HOME/bin/clang" && \
  echo | $OPENCILK_HOME/bin/clang -fopencilk -x c -c -o /dev/null - &>/dev/null 2>&1 && HAVE_OPENCILK=true

if [ "$HAVE_RUST" = "false" ]; then
    echo "WARNING: Rust and Cargo not found. These are necessary to run the fundamental benchmarks."
else
    echo "VERIFIED: RUST"
fi
if [ "$HAVE_CLANG_OMP" = "false" ]; then
    echo "WARNING: clang with OpenMP not found. This is necessary for the comparison with OpenMP. Will be skipped..."
else
    echo "VERIFIED: OPENMP"
fi
if [ "$HAVE_OPENCILK" = "false" ]; then
    echo "WARNING: OpenCilk not found. This is necessary for the comparison with Cilk. Will be skipped..."
else
    echo "VERIFIED: CILK"
fi
if [ "$HAVE_PYTHON_DEPS" = "false" ]; then
    echo "WARNING: missing Python dependencies (numpy, matplotlib, and/or pandas), plotting will be skipped"
else
    echo "VERIFIED: PYTHON"
fi