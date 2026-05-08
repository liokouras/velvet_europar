#!/bin/bash

python3 -c "import numpy, matplotlib, pandas" &>/dev/null 2>&1 && HAVE_PYTHON_DEPS=true || HAVE_PYTHON_DEPS=false

if [ "$HAVE_PYTHON_DEPS" = "false" ]; then
    echo "WARNING: missing Python dependencies (numpy, matplotlib, and/or pandas),  cannot do data processing"
fi

cd data_processing

mkdir -p figs

python3 scripts/main.py all data/ > processing_output.txt