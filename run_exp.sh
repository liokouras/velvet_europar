#!/bin/bash

# ** MUST RUN FROM INSIDE RUNSCRIPTS FOLDER **
cd benchmarks/runscripts
# create timestamped dirs
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
mkdir -p "../output/out_$TIMESTAMP"
mkdir -p "../output/dump_$TIMESTAMP"
OUT_DIR="$(realpath "../output/out_$TIMESTAMP")"
DUMP_DIR="$(realpath "../output/dump_$TIMESTAMP")"

# ensure existence of zout and data folders
mkdir -p "../c/zout"
mkdir -p "../data"

# check for dependencies
HAVE_RUST=false
HAVE_CLANG_OMP=false
HAVE_OPENCILK=false

command -v rustc &>/dev/null && command -v cargo &>/dev/null && HAVE_RUST=true
command -v clang &>/dev/null && echo | clang -fopenmp -x c -c -o /dev/null - &>/dev/null 2>&1 && HAVE_CLANG_OMP=true
test -x "$OPENCILK_HOME/bin/clang" && \
  echo | $OPENCILK_HOME/bin/clang -fopencilk -x c -c -o /dev/null - &>/dev/null 2>&1 && HAVE_OPENCILK=true

if [ "$HAVE_RUST" = "false" ]; then
    echo "WARNING: Rust and Cargo not found. These are necessary to run the fundamental benchmarks."
fi
if [ "$HAVE_CLANG_OMP" = "false" ]; then
    echo "WARNING: clang with OpenMP not found. This is necessary for the comparison with OpenMP. Will be skipped..."
fi
if [ "$HAVE_OPENCILK" = "false" ]; then
    echo "WARNING: OpenCilk not found. This is necessary for the comparison with Cilk. Will be skipped..."
fi

# decide whether to run 'full' or 'reduced' setup
RUN_MODE="reduced"
CORES_OVERRIDE=""
for arg in "$@"; do
    if [ "$arg" = "full" ]; then
        RUN_MODE="full"
    elif [[ "$arg" =~ ^--cores=([0-9]+)$ ]]; then
        CORES_OVERRIDE="${BASH_REMATCH[1]}"
    fi
done

if [ -n "$CORES_OVERRIDE" ]; then
    PHYS_CORES="$CORES_OVERRIDE"
else
    PHYS_CORES=$(lscpu | grep "^Core(s) per socket:" | awk '{print $NF}')
    SOCKETS=$(lscpu | grep "^Socket(s):" | awk '{print $NF}')
    PHYS_CORES=$((PHYS_CORES * SOCKETS))
fi

if [ "$RUN_MODE" = "full" ]; then
    MAX_CORES=$PHYS_CORES
else
    MAX_CORES=$(( PHYS_CORES < 64 ? PHYS_CORES : 64 ))
fi
echo "Running $RUN_MODE setup with $MAX_CORES cores"


# call app-specific scripts with same out-dir and relevant dep info
bash adapint.sh "$OUT_DIR" "$DUMP_DIR" "$HAVE_RUST" "$HAVE_CLANG_OMP" "$HAVE_OPENCILK" "$MAX_CORES" "$RUN_MODE"

bash bh.sh "$OUT_DIR" "$DUMP_DIR" "$HAVE_RUST" "$HAVE_CLANG_OMP" "$HAVE_OPENCILK" "$MAX_CORES" "$RUN_MODE"

bash fib.sh "$OUT_DIR" "$DUMP_DIR" "$HAVE_RUST" "$MAX_CORES" "$RUN_MODE"

bash matmul.sh "$OUT_DIR" "$DUMP_DIR" "$HAVE_RUST" "$HAVE_CLANG_OMP" "$HAVE_OPENCILK" "$MAX_CORES" "$RUN_MODE"

bash nqueens.sh "$OUT_DIR" "$DUMP_DIR" "$HAVE_RUST" "$HAVE_CLANG_OMP" "$HAVE_OPENCILK" "$MAX_CORES" "$RUN_MODE"

bash sort.sh "$OUT_DIR" "$DUMP_DIR" "$HAVE_RUST" "$HAVE_CLANG_OMP" "$HAVE_OPENCILK" "$MAX_CORES" "$RUN_MODE"

bash tsp.sh "$OUT_DIR" "$DUMP_DIR" "$HAVE_RUST" "$HAVE_CLANG_OMP" "$HAVE_OPENCILK" "$MAX_CORES" "$RUN_MODE"

# do the stat runs
if [ "$HAVE_RUST" = "true" ]; then
    STATS_DIR="${OUT_DIR}/stats"
    mkdir -p "$STATS_DIR"
    bash stats.sh "$STATS_DIR"
fi

# process data
python3 -c "import numpy, matplotlib, pandas" &>/dev/null 2>&1 && HAVE_PYTHON_DEPS=true || HAVE_PYTHON_DEPS=false

mkdir -p ../../data_processing/figs

if [ "$HAVE_PYTHON_DEPS" = "false" ]; then
    echo "WARNING: missing Python dependencies (numpy, matplotlib, and/or pandas), data processing is being skipped..."
elif [ "$HAVE_RUST" = "false" ]; then
    echo "WARNING: Rust experiments were not run, so there is no data to process..."
elif [ "$HAVE_CLANG_OMP" = "false" -a "$HAVE_OPENCILK" = "false" ]; then
    echo "WARNING: both C-versions were skipped, only plotting Rust data (tables & Fig. 1)"
    python3 ../../data_processing/scripts/main.py rust-only "$OUT_DIR" > ../../data_processing/processing_output.txt
    echo "Processing output printed to data_processing/processing_output.txt"
elif [ "$HAVE_CLANG_OMP" = "false" -o "$HAVE_OPENCILK" = "false" ]; then
    echo "WARNING: at least one C-version was skipped, there will be gaps in the C-comparison figure (Fig. 2)"
    python3 ../../data_processing/scripts/main.py all "$OUT_DIR" > ../../data_processing/processing_output.txt
    echo "Processing output printed to data_processing/processing_output.txt"
else
    echo "Processing data...."
    python3 ../../data_processing/scripts/main.py all "$OUT_DIR" > ../../data_processing/processing_output.txt
    echo "Processing output printed to data_processing/processing_output.txt"
fi