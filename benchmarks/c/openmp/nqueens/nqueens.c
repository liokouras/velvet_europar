#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <omp.h>
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

size_t nqueens_omp(char *board, size_t row, size_t size) {
    if (row > THRESHOLD) return nqueens(board, row, size);

    if (row >= size) return 1;

    size_t solutions = 0;


    size_t *count = (size_t *) alloca(size * sizeof(size_t));
    (void) memset(count, 0, size * sizeof (size_t));

    for (size_t q = 0; q < size; q++) {
        size_t i;

        // incremental conflict check
        for (i = 0; i < row; i++) {
            size_t p = (size_t)board[i] - q;
            size_t d = row - i;

            if (p == 0 || p == d || p == -d) break;
        }

        if (i != row) continue;

        // parallel recursion: copy board
        #pragma omp task shared(solutions) firstprivate(row, q, size)
        {
            char *new_board = (char *) alloca((row + 1) * sizeof (char));
            memcpy(new_board, board, row * sizeof (char));
            new_board[row] = (char)q;
            size_t res = nqueens_omp(new_board, row + 1, size);
                
            #pragma omp atomic
            solutions += res;
        }
    }

    #pragma omp taskwait
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

    if (strcmp(app, "omp") == 0) {
        int workers = omp_get_max_threads();
        size_t result;
        #pragma omp parallel 
        {
            #pragma omp single
            {
                ctimer_start(&t);
                result = nqueens_omp(board, 0, n);
                ctimer_stop(&t);
                ctimer_measure(&t);
            }
        }

        printf("11,%d,%d,%zu,%ld.%09ld\n", workers, n, THRESHOLD, (long)t.elapsed.tv_sec, t.elapsed.tv_nsec);

        fprintf(stderr, "OpenMP nqueens(%d) = %zu, threshold = %zu, num workers = %d\n", n, result, THRESHOLD, workers);
        ctimer_print(t, "nqueens_omp");
    } else if (strcmp(app, "seq") == 0) {
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