#include <stdio.h>
#include <stdlib.h>
#include <assert.h>
#include <math.h>
#include <string.h>

#include <cilk/cilk.h>
#include <cilk/cilk_api.h>
#include <pthread.h>

#include "../../ctimer.h"

#define REAL double

// ------ Hierarchical block-based matrix definition --------
typedef enum {
    INTERNAL,
    LEAF,
    MUTABLE_LEAF
} MatrixKind;

typedef struct Matrix Matrix;

typedef struct {
    pthread_mutex_t mutex;
    REAL *data;
    int dim;
} MutableLeaf;

struct Matrix {
    MatrixKind kind;
    union {
        struct {
            Matrix *m00;
            Matrix *m01;
            Matrix *m10;
            Matrix *m11;
        } internal;

        REAL *leaf;
        MutableLeaf mutable_leaf;
    };
};

// matrix cleanup
void matrix_free(Matrix *m) {
    if (!m) return;

    switch (m->kind) {
        case INTERNAL:
            matrix_free(m->internal.m00);
            matrix_free(m->internal.m01);
            matrix_free(m->internal.m10);
            matrix_free(m->internal.m11);
            break;

        case LEAF:
            free(m->leaf);
            break;

        case MUTABLE_LEAF:
            pthread_mutex_destroy(&m->mutable_leaf.mutex);
            free(m->mutable_leaf.data);
            break;
    }

    free(m);
}

// matrix creation // TODO: update value situation???
typedef void (*matrix_init_fn)(REAL *data, int dim, void *ctx);
void init_constant(REAL *data, int dim, void *ctx) {
    REAL value = *(REAL *)ctx;
    for (int i = 0; i < dim * dim; ++i) {
        data[i] = value;
    }
}
void init_random(REAL *data, int dim, void *ctx) {
    (void)ctx;
    for (int i = 0; i < dim * dim; ++i) {
        data[i] = (REAL)rand() / (REAL)RAND_MAX;
    }
}

Matrix *make_leaf(int dim, matrix_init_fn init, void *ctx) { 
    Matrix *m = malloc(sizeof(Matrix));
    if (!m) {
        fprintf(stderr, "leaf matrix malloc err\n");
        return NULL;
    }
    m->kind = LEAF;

    m->leaf = malloc(dim * dim * sizeof(REAL));
    if (!m->leaf) {
        fprintf(stderr, "leaf malloc err\n");
        matrix_free(m);
        return NULL;
    }

    init(m->leaf, dim, ctx);

    return m;
}

Matrix *make_mutable_leaf(int dim, matrix_init_fn init, void *ctx) {
    Matrix *m = malloc(sizeof(Matrix));
    if (!m) {
        fprintf(stderr, "mutable leaf matrix malloc err\n");
        return NULL;
    }
    m->kind = MUTABLE_LEAF;

    m->mutable_leaf.dim = dim;
    m->mutable_leaf.data = malloc(dim * dim * sizeof(REAL));
    if (!m->mutable_leaf.data) {
        fprintf(stderr, "mutable leaf malloc err\n");
        matrix_free(m);
        return NULL;
    }

    init(m->mutable_leaf.data, dim, ctx);
    pthread_mutex_init(&m->mutable_leaf.mutex, NULL);

    return m;
}

Matrix *matrix_new(int depth, int dim, matrix_init_fn init, void *ctx, int result) {
    if (depth <= 0) {
        if (result) {
            return make_mutable_leaf(dim, init, ctx);
        } else {
            return make_leaf(dim, init, ctx);
        }
    }

    Matrix *m = malloc(sizeof(Matrix));
    if (!m) {
        fprintf(stderr, "Matrix malloc err\n");
        return NULL;
    }
    m->kind = INTERNAL;

    m->internal.m00 = matrix_new(depth - 1, dim, init, ctx, result);
    m->internal.m01 = matrix_new(depth - 1, dim, init, ctx, result);
    m->internal.m10 = matrix_new(depth - 1, dim, init, ctx, result);
    m->internal.m11 = matrix_new(depth - 1, dim, init, ctx, result);

    if (!m->internal.m00 || !m->internal.m01 || !m->internal.m10 || !m->internal.m11) {
        fprintf(stderr, "Matrix build err\n");
        matrix_free(m);
        return NULL;
    }
    
    return m;
}

// printing
void print_row (Matrix *m, int dim, int row) {
    switch (m->kind) {
        case LEAF:
            for (int i = 0; i < dim; i++) {
                printf("%f ", m->leaf[row * dim + i]);
            }
            break;

        case MUTABLE_LEAF:
            for (int i = 0; i < dim; i++) {
                printf(" %f ", m->mutable_leaf.data[row * dim + i]);
            }
            break;

        case INTERNAL:
            if (row < dim/2) {
                // top half: 00 | 01
                print_row(m->internal.m00, dim/2, row);
                print_row(m->internal.m01, dim/2, row);
            } else {
                // bottom half: 10 | 11
                print_row(m->internal.m10, dim/2, row-(dim/2));
                print_row(m->internal.m11, dim/2, row-(dim/2));
            }

    }
}

void print_matrix (Matrix *m, int dim) {
    for (int row = 0; row < dim; row++) {
        print_row(m, dim, row);
        printf("\n");
    }
}

void print_matrix_flat (REAL *m, int n) {
    int i, j;
    for (i = 0; i < n; i++) {
        for (j = 0; j < n; j++) {
            printf(" %f ", m[i * n + j]);
        }
        printf("\n");
    }
}

// writing
void write_row(Matrix *m, int dim, int row, REAL *out, int col_offset) {
    switch (m->kind) {
        case LEAF:
            for (int i = 0; i < dim; i++) {
                out[col_offset + i] = m->leaf[row * dim + i];
            }
            break;

        case MUTABLE_LEAF:
            for (int i = 0; i < dim; i++) {
                out[col_offset + i] = m->mutable_leaf.data[row * dim + i];
            }
            break;

        case INTERNAL:
            if (row < dim / 2) {
                // top half: [00 | 01]
                write_row(m->internal.m00, dim/2, row, out, col_offset);
                write_row(m->internal.m01, dim/2, row, out, col_offset + dim/2);
            } else {
                // bottom half: [10 | 11]
                write_row(m->internal.m10, dim/2, row - dim/2, out, col_offset);
                write_row(m->internal.m11, dim/2, row - dim/2, out, col_offset + dim/2);
            }
            break;
    }
}
REAL *matrix_flatten(Matrix *m, int dim) {
    REAL *out = malloc(dim * dim * sizeof(REAL));
    if (!out) return NULL;

    for (int row = 0; row < dim; row++) {
        REAL *moved = out + (row * dim);
        write_row(m, dim, row, moved, 0);
    }

    return out;
}

// checking
int check_matrix_exp(Matrix *m, REAL exp, int dim) {
    int ok = 0;
    switch (m->kind) {
        case LEAF:
            for (int i = 0; i < dim; i++) {
                for (int j = 0; j < dim; j++) {
                    REAL val = m->leaf[i * dim + j];
                    if (val != exp) {
                        printf("MISMATCH!\n val = %f, exp = %f\n LEAF..\n", val, exp);
                        return 1;
                    }
                }
            }
            return 0;

        case MUTABLE_LEAF:
            for (int i = 0; i < dim; i++) {
                for (int j = 0; j < dim; j++) {
                    REAL val = m->mutable_leaf.data[i * dim + j];
                    if (val != exp) {
                        printf("MISMATCH!\n val = %f, exp = %f\n MUTABLE..\n", val, exp);
                        return 1;
                    }
                }
            }
            return 0;

        case INTERNAL:
            ok += check_matrix_exp(m->internal.m00, exp, dim/2) +
            check_matrix_exp(m->internal.m01, exp, dim/2) +
            check_matrix_exp(m->internal.m10, exp, dim/2) +
            check_matrix_exp(m->internal.m11, exp, dim/2);
    }
    return ok;

}

double check_matrix_flat(REAL *A, REAL *B, int n) {
  int i, j;
  double max_error = 0.0;

  for (i = 0; i < n; i++) {
    for (j = 0; j < n; j++) {
      double diff = (A[i * n + j] - B[i * n + j]) / A[i * n + j];
      if (diff < 0)
        diff = -diff;
      if (diff > max_error)
        max_error = diff;
    }
  }

  return max_error;
}

// ------ Hierarchical block-based matrix multiplication --------
void multiply_stride2(Matrix *c, Matrix *a, Matrix *b, int dim) {
    if (!c || !a || !b) return;

    if (c->kind != LEAF || a->kind != LEAF || b->kind != LEAF) {
        fprintf(stderr, "multiply_stride2 not called on correct leaves!\n");
        return;
    }

    REAL *c_data = c->leaf;
    REAL *a_data = a->leaf;
    REAL *b_data = b->leaf;

    for (int i = 0; i < dim; i += 2) {
        REAL *a0 = a_data + (i * dim);
        REAL *a1 = a_data + ((i + 1) * dim);

        for (int j = 0; j < dim; j += 2) {
            REAL s00 = 0.0;
            REAL s01 = 0.0;
            REAL s10 = 0.0;
            REAL s11 = 0.0;

            for (int k = 0; k < dim; k += 2) {
                REAL *b0 = b_data + (k * dim);
                REAL *b1 = b_data + ((k + 1) * dim);
            
                s00 += *(a0 + k) * *(b0 + j) + *(a0 + k + 1) * *(b1 + j);
                s10 += *(a1 + k) * *(b0 + j) + *(a1 + k + 1) * *(b1 + j);
                s01 += *(a0 + k) * *(b0 + j + 1) + *(a0 + k + 1) * *(b1 + j + 1);
                s11 += *(a1 + k) * *(b0 + j + 1) + *(a1 + k + 1) * *(b1 + j + 1);
            }

            // store results back to C
            REAL *c0 = c_data + (i * dim);
            REAL *c1 = c_data + ((i + 1) * dim);

            *(c0 + j) += s00;
            *(c0 + j + 1) += s01;
            *(c1 + j) += s10;
            *(c1 + j + 1) += s11;
        }
    }
}

void multiply_stride2_lock(Matrix *c, Matrix *a, Matrix *b) {
    if (!c || !a || !b) return;

    if (c->kind != MUTABLE_LEAF || a->kind != LEAF || b->kind != LEAF) {
        fprintf(stderr, "multiply_stride2 not called on correct leaves!\n");
        return;
    }

    int dim = c->mutable_leaf.dim;

    REAL *c_data = c->mutable_leaf.data;
    REAL *a_data = a->leaf;
    REAL *b_data = b->leaf;

    for (int i = 0; i < dim; i += 2) {
        REAL *a0 = a_data + (i * dim);
        REAL *a1 = a_data + ((i + 1) * dim);

        for (int j = 0; j < dim; j += 2) {
            REAL s00 = 0.0;
            REAL s01 = 0.0;
            REAL s10 = 0.0;
            REAL s11 = 0.0;

            for (int k = 0; k < dim; k += 2) {
                REAL *b0 = b_data + (k * dim);
                REAL *b1 = b_data + ((k + 1) * dim);
            
                s00 += *(a0 + k) * *(b0 + j) + *(a0 + k + 1) * *(b1 + j);
                s10 += *(a1 + k) * *(b0 + j) + *(a1 + k + 1) * *(b1 + j);
                s01 += *(a0 + k) * *(b0 + j + 1) + *(a0 + k + 1) * *(b1 + j + 1);
                s11 += *(a1 + k) * *(b0 + j + 1) + *(a1 + k + 1) * *(b1 + j + 1);
            }

            // store results back to C
            pthread_mutex_lock(&c->mutable_leaf.mutex);
            REAL *c0 = c_data + (i * dim);
            REAL *c1 = c_data + ((i + 1) * dim);

            *(c0 + j) += s00;
            *(c0 + j + 1) += s01;
            *(c1 + j) += s10;
            *(c1 + j + 1) += s11;
            
            pthread_mutex_unlock(&c->mutable_leaf.mutex);
        }
    }
}

void matmul_seq(Matrix *c, Matrix *a, Matrix *b, int inner_dim) {
    if (!c || !a || !b) return;

    switch (a->kind) {
        case LEAF:
            assert(b->kind == LEAF);
            switch (c->kind) {
                case LEAF:
                    multiply_stride2(c, a, b, inner_dim);
                    break;
                case INTERNAL:
                    fprintf(stderr, "C-matrix is internal when it should be a leaf!\n");
                    return;
                case MUTABLE_LEAF:
                    fprintf(stderr, "C-matrix is mutable_leaf when it should be a leaf!\n");
                    return;
            }
            break;
        case INTERNAL:
            assert(b->kind == INTERNAL);
            if (c->kind != INTERNAL) {
                fprintf(stderr, "C-matrix is a leaf when it should be internal!\n");
                return;
            }

            // recursive multiplication
            matmul_seq(c->internal.m00, a->internal.m00, b->internal.m00, inner_dim);
            matmul_seq(c->internal.m00, a->internal.m01, b->internal.m10, inner_dim);

            matmul_seq(c->internal.m01, a->internal.m00, b->internal.m01, inner_dim);
            matmul_seq(c->internal.m01, a->internal.m01, b->internal.m11, inner_dim);

            matmul_seq(c->internal.m10, a->internal.m10, b->internal.m00, inner_dim);
            matmul_seq(c->internal.m10, a->internal.m11, b->internal.m10, inner_dim);

            matmul_seq(c->internal.m11, a->internal.m10, b->internal.m01, inner_dim);
            matmul_seq(c->internal.m11, a->internal.m11, b->internal.m11, inner_dim);
            break;

        default:
            fprintf(stderr, "A & B matrices are incompatible dims!\n");
            return;
    }
}

void matmul_par(int task, Matrix *c, Matrix *a, Matrix *b) {
    if (!c || !a || !b) return;

    if (task == 0) {
        multiply_stride2_lock(c, a, b);
        return;
    }

    switch (a->kind) {
        case INTERNAL:
            assert(b->kind == INTERNAL);
            if (c->kind != INTERNAL) {
                fprintf(stderr, "C-matrix is a leaf when it should be internal!\n");
                return;
            }

            // recursive multiplication
            cilk_spawn matmul_par(task-1, c->internal.m00, a->internal.m00, b->internal.m00);
            cilk_spawn matmul_par(task-1, c->internal.m00, a->internal.m01, b->internal.m10);
            
            cilk_spawn matmul_par(task-1, c->internal.m01, a->internal.m00, b->internal.m01);
            cilk_spawn matmul_par(task-1, c->internal.m01, a->internal.m01, b->internal.m11);

            cilk_spawn matmul_par(task-1,c->internal.m10, a->internal.m10, b->internal.m00);
            cilk_spawn matmul_par(task-1,c->internal.m10, a->internal.m11, b->internal.m10);

            cilk_spawn matmul_par(task-1,c->internal.m11, a->internal.m10, b->internal.m01);
            matmul_par(task-1,c->internal.m11, a->internal.m11, b->internal.m11);
            cilk_sync;

            break;

        default:
            fprintf(stderr, "matmul_par called on leaf matrices!\n");
            return;
    }
}

void matmul_par_seq(int task, Matrix *c, Matrix *a, Matrix *b) {
    if (!c || !a || !b) return;

    if (task == 0) {
        multiply_stride2_lock(c, a, b);
        return;
    }

    switch (a->kind) {
        case INTERNAL:
            assert(b->kind == INTERNAL);
            if (c->kind != INTERNAL) {
                fprintf(stderr, "C-matrix is a leaf when it should be internal!\n");
                return;
            }

            // recursive multiplication
            matmul_par_seq(task-1, c->internal.m00, a->internal.m00, b->internal.m00);
            matmul_par_seq(task-1, c->internal.m00, a->internal.m01, b->internal.m10);

            matmul_par_seq(task-1, c->internal.m01, a->internal.m00, b->internal.m01);
            matmul_par_seq(task-1, c->internal.m01, a->internal.m01, b->internal.m11);

            matmul_par_seq(task-1, c->internal.m10, a->internal.m10, b->internal.m00);
            matmul_par_seq(task-1, c->internal.m10, a->internal.m11, b->internal.m10);

            matmul_par_seq(task-1, c->internal.m11, a->internal.m10, b->internal.m01);
            matmul_par_seq(task-1, c->internal.m11, a->internal.m11, b->internal.m11);
            break;

        default:
            fprintf(stderr, "matmul_par called on leaf matrices!\n");
            return;
    }

}

int main(int argc, char *argv[]) {
    if (argc < 4) {
        fprintf (stderr, "Usage: %s [cilk|seq] [depth] [dim] \n", argv[0]);
        return 1;
    }

    ctimer_t t;

    const char *app = argv[1];
    int depth = atoi(argv[2]);
    int dim = atoi(argv[3]);

    int full_dim = dim * pow(2, depth);

    REAL one = 1.0;
    Matrix *A = matrix_new(depth, dim, init_constant, &one, 0);
    if (!A) {
        fprintf(stderr, "Matrix build error\n");
        return 1;
    }

    REAL two = 2.0;
    Matrix *B = matrix_new(depth, dim, init_constant, &two, 0);
    if (!B) {
        fprintf(stderr, "Matrix build error\n");
        matrix_free(A);
        return 1;
    }

    Matrix *C;
    REAL zero = 0.0;
    if (strcmp(app, "cilk") == 0) {
        C = matrix_new(depth, dim, init_constant, &zero, 1); // with lock at leaves
        if (!C) {
            fprintf(stderr, "Matrix build error\n");
            matrix_free(A);
            matrix_free(B);
            return 1;
        }

        int workers = __cilkrts_get_nworkers();
        ctimer_start(&t);
        matmul_par(depth, C, A, B);
        ctimer_stop(&t);
        ctimer_measure(&t);

        printf("12,%d,%d,%d,%ld.%09ld\n", workers, depth, dim, (long)t.elapsed.tv_sec, t.elapsed.tv_nsec);
        
        fprintf(stderr, "CILK matmul(%d %d), num workers = %d\n", depth, dim, workers);
        int check = check_matrix_exp(C, full_dim*2, full_dim);
        if (check != 0) {
            fprintf(stderr, "there is an error...\n");
        } else {
            fprintf(stderr, "check succes!\n");
        }
        ctimer_print(t, "matmul_cilk");
    } else if (strcmp(app, "par") == 0) {
        C = matrix_new(depth, dim, init_constant, &zero, 1); // with lock at leaves
        if (!C) {
            fprintf(stderr, "Matrix build error\n");
            matrix_free(A);
            matrix_free(B);
            return 1;
        }

        ctimer_start(&t);
        matmul_par_seq(depth, C, A, B);
        ctimer_stop(&t);
        ctimer_measure(&t);

        printf("-11,1,%d,%d,%ld.%09ld\n", depth, dim, (long)t.elapsed.tv_sec, t.elapsed.tv_nsec);
        
        fprintf(stderr, "PAR SEQ matmul(%d %d)\n", depth, dim);
        int check = check_matrix_exp(C, full_dim*2, full_dim);
        if (check != 0) {
            fprintf(stderr, "there is an error...\n");
        } else {
            fprintf(stderr, "check succes!\n");
        }
        ctimer_print(t, "matmul_par_seq");
    } else if (strcmp(app, "seq") == 0) {
        C = matrix_new(depth, dim, init_constant, &zero, 0); // without lock at leaves
        if (!C) {
            fprintf(stderr, "Matrix build error\n");
            matrix_free(A);
            matrix_free(B);
            return 1;
        }

        ctimer_start(&t);
        matmul_seq(C, A, B, dim);
        ctimer_stop(&t);
        ctimer_measure(&t);

        printf("10,1,%d,%d,%ld.%09ld\n", depth, dim, (long)t.elapsed.tv_sec, t.elapsed.tv_nsec);
        
        fprintf(stderr, "SEQ matmul(%d %d)\n", depth, dim);
        int check = check_matrix_exp(C, full_dim*2, full_dim);
        if (check != 0) {
            fprintf(stderr, "there is an error...\n");
        } else {
            fprintf(stderr, "check succes!\n");
        }
        ctimer_print(t, "matmul_seq");
    }

    matrix_free(C);
    matrix_free(B);
    matrix_free(A);

    return 0;
}