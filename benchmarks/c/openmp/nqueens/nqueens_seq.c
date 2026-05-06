#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "../../ctimer.h"

const size_t THRESHOLD = 6;

size_t nqueens(char *board, size_t row, size_t size) {
    if (row >= size) return 1;

    size_t solutions = 0;

    for (size_t q = 0; q < size; q++) {
        size_t i;

        // incremental conflict check
        for (i = 0; i < row; i++) {
            size_t p = (size_t)board[i] - q;
            size_t d = row - i;

            if (p == 0 || p == d || p == -d) break;
        }

        if (i != row) continue;


        // sequential recursion: reuse board
        board[row] = (char)q;
        solutions += nqueens(board, row + 1, size);
    }

    return solutions;
}

int main(int argc, char *argv[]) {
    if (argc < 3) {
        fprintf (stderr, "Usage: %s [omp|seq] [n]\n", argv[0]);
        return 1;
    }

    ctimer_t t;

    const char *app = argv[1];
    int n = atoi(argv[2]);

    char *board = calloc(n, sizeof(unsigned char));
    if (strcmp(app, "seq") == 0) {
        ctimer_start(&t);
        size_t result = nqueens(board, 0, n);
        ctimer_stop(&t);
        ctimer_measure(&t);

        printf("10,1,%d,0,%ld.%09ld\n", n, (long)t.elapsed.tv_sec, t.elapsed.tv_nsec);

        fprintf(stderr, "SEQ nqueens(%d) = %zu\n", n, result);
        ctimer_print(t, "nqueens_seq");
    } else {
        fprintf(stderr, "Unknown app: %s\n", app);
        return 1;
    }

    free(board);
    return 0;
}