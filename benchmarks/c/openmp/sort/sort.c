#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>
#include <math.h>
#include <omp.h>
#include "../../ctimer.h"
#include <limits.h>

// Rust function declaration
void rust_sort_i32(int32_t *ptr, size_t len);

const int DIRECT_THRESHOLD = 2097152;

#define swap_indices(a, b) \
{ \
  int32_t *tmp;\
  tmp = a;\
  a = b;\
  b = tmp;\
}

int load_arr(int32_t *arr, int n, int seed, int check) {
    char filename[256];
    snprintf(filename, sizeof(filename), "../../data/sort_arr_%d_%d.bin", n, seed);

    FILE *f = fopen(filename, "rb");
    if (!f) {
        perror("fopen");
        return 1;
    }
    // check file size
    fseek(f, 0, SEEK_END);
    long filesize = ftell(f);
    rewind(f);
    if (filesize % sizeof(int32_t) != 0) {
        fprintf(stderr, "File size is not a multiple of 4 bytes\n");
        fclose(f);
        return 1;
    }
    int count = filesize / sizeof(int32_t);
    if (count != n) {
        fprintf(stderr, "File does not have %d entries as expected\n", n);
        fprintf(stderr, "File has %d entries\n", count);
        fclose(f);
        return 1;
    }
    // read 
    int read = fread(arr, sizeof(int32_t), count, f);
    if (read != count) {
        fprintf(stderr, "Expected %d ints, read %d\n", count, read);
        fclose(f);
        return 1;
    }

    fclose(f);
    fprintf(stderr, "Read %d ints!\n", count);
    return 0;
}

int check(const int32_t *arr, int len) {
    fprintf(stderr, "Checking arr size: %d\n", len);

    if (arr == NULL || len <= 1) {
        fprintf(stderr, "VACUOUSLY SORTED!\n");
        return 1;
    }

    for (size_t i = 0; i < len - 1; i++) {
        if (arr[i] > arr[i + 1]) {
            fprintf(stderr, "NOT SORTED! discovered at idx %zu\n", i);
            return 0;
        }
    }
    fprintf(stderr, "SORTED!\n");
    return 1;
}

// return the first slot that is equal or LARGER than val
int32_t *binsearch(int32_t val, int32_t *low, int32_t *high) {
    int32_t *mid;

    while (low != high) {
        mid = low + ((high - low + 1) >> 1);
        if (val < *mid) high = mid - 1;
        else low = mid;
    }

    if (*low < val) return low + 1;
    else return low;
}

void seqmerge(int32_t *left_low, int32_t *left_high, int32_t *right_low, int32_t *right_high, int32_t *dest_low) {
    int32_t left, right; 
    
    /*
        * The following 'if' statement is not necessary
        * for the correctness of the algorithm, and is
        * in fact subsumed by the rest of the function.
        * However, it is a few percent faster.  Here is why.
        *
        * The merging loop below has something like
        *   if (a1 < a2) {
        *        *dest++ = a1;
        *        ++low1;
        *        if (end of array) break;
        *        a1 = *low1;
        *   }
        *
        * Now, a1 is needed immediately in the next iteration
        * and there is no way to mask the latency of the load.
        * A better approach is to load a1 *before* the end-of-array
        * check; the problem is that we may be speculatively
        * loading an element out of range.  While this is
        * probably not a problem in practice, yet I don't feel
        * comfortable with an incorrect algorithm.  Therefore,
        * I use the 'fast' loop on the array (except for the last 
        * element) and the 'slow' loop for the rest, saving both
        * performance and correctness.
    */
    if (left_low < left_high-1 && right_low < right_high-1) {
        left = *left_low;
        right = *right_low;
        for (;;) {
            if (left < right) {
                *dest_low++ = left;
                left = *++left_low;
                if (left_low >= left_high-1) break;
            } else {
                *dest_low++ = right;
                right = *++right_low;
                if (right_low >= right_high-1) break;
            }
        }
    }
    
    if (left_low < left_high && right_low < right_high) {
        for (;;) {
            if (left < right) {
                *dest_low++ = left;
                ++left_low;
                if (left_low >= left_high) break;
                left = *left_low;
            } else {
                *dest_low++ = right;
                ++right_low;
                if (right_low >= right_high) break;
                right = *right_low;
            }
        }
    }

    if (left_low >= left_high) {
        memcpy(dest_low, right_low, sizeof(int32_t) * (right_high - right_low));
    } else {
        memcpy(dest_low, left_low, sizeof(int32_t) * (left_high - left_low));
    }
}

int merge(int32_t *left_low, int32_t *left_high, int32_t *right_low, int32_t *right_high, int32_t *dest_low) {
    // want 'left' to be the larger of the two input arrays
    if (right_high - right_low > left_high - left_low) {
        swap_indices(left_low, right_low);
        swap_indices(left_high, right_high);
    }

    int left_len = left_high - left_low;
    int right_len = right_high - right_low;
    if (right_len < 1) {
        // smaller range is empty
        memcpy(dest_low, left_low, sizeof(int32_t) * (left_len));
        return 0;
    }
    if (left_len + right_len <= 2*DIRECT_THRESHOLD) {
        seqmerge(left_low, left_high, right_low, right_high, dest_low);
        return 0;
    }

    // find the middle element of left, and use search for suitable index in right
    int32_t *left_mid = left_low + (left_len / 2);
    int32_t *right_mid = binsearch(*left_mid, right_low, right_high);
    int dest_offset = left_mid - left_low + right_mid - right_low;

    merge(left_low, left_mid, right_low, right_mid, dest_low);
    merge(left_mid, left_high, right_mid, right_high, dest_low + dest_offset);
    
    return 0;
}

int merge_omp(int32_t *left_low, int32_t *left_high, int32_t *right_low, int32_t *right_high, int32_t *dest_low) {
    // want 'left' to be the larger of the two input arrays
    if (right_high - right_low > left_high - left_low) {
        swap_indices(left_low, right_low);
        swap_indices(left_high, right_high);
    }

    int left_len = left_high - left_low;
    int right_len = right_high - right_low;
    if (right_len < 1) {
        // smaller range is empty
        memcpy(dest_low, left_low, sizeof(int32_t) * (left_len));
        return 0;
    }
    if (left_len + right_len <= 2*DIRECT_THRESHOLD) {
        seqmerge(left_low, left_high, right_low, right_high, dest_low);
        return 0;
    }

    // find the middle element of left, and use search for suitable index in right
    int32_t *left_mid = left_low + (left_len / 2);
    int32_t *right_mid = binsearch(*left_mid, right_low, right_high);
    int dest_offset = left_mid - left_low + right_mid - right_low;

    #pragma omp task
    merge_omp(left_low, left_mid, right_low, right_mid, dest_low);
    merge_omp(left_mid, left_high, right_mid, right_high, dest_low + dest_offset);
    #pragma omp taskwait // sync
    
    return 0;
}

int sort(int32_t *arr, int32_t *buf, int len, int usebuf) {
    if (len < DIRECT_THRESHOLD && usebuf == 1) {
        rust_sort_i32(arr, len);
    } else {
        int mid = len/2 + 1;
        sort(arr, buf, mid, 1 - usebuf);
        sort(arr+mid, buf+mid, len-mid, 1 - usebuf);

        int32_t *left_high, *left_low, *right_high, *right_low, *dest_low;
        if (usebuf == 1) {
            left_low = buf;
            left_high = buf+mid;
            right_low = buf+mid;
            right_high = buf+len;
            dest_low = arr;
        } else {
            left_low = arr;
            left_high = arr+mid;
            right_low = arr+mid;
            right_high = arr+len;
            dest_low = buf;
        }
        merge(left_low, left_high, right_low, right_high, dest_low);
    }
    return 0;
}

int sort_omp(int32_t *arr, int32_t *buf, int len, int usebuf) {
    if (len < DIRECT_THRESHOLD && usebuf == 1) {
        rust_sort_i32(arr, len);
    } else {
        int mid = len/2 + 1;
        #pragma omp task
        sort_omp(arr, buf, mid, 1 - usebuf);
        sort_omp(arr+mid, buf+mid, len-mid, 1 - usebuf);
        #pragma omp taskwait 

        int32_t *left_high, *left_low, *right_high, *right_low, *dest_low;
        if (usebuf == 1) {
            left_low = buf;
            left_high = buf+mid;
            right_low = buf+mid;
            right_high = buf+len;
            dest_low = arr;
        } else {
            left_low = arr;
            left_high = arr+mid;
            right_low = arr+mid;
            right_high = arr+len;
            dest_low = buf;
        }
        merge_omp(left_low, left_high, right_low, right_high, dest_low);
    }
    return 0;
}

int main(int argc, char *argv[]) {
    if (argc < 4) {
        fprintf (stderr, "Usage: %s [omp|seq] [number_of_elements] [random_seed]\n", argv[0]);
        return 1;
    }

    ctimer_t t;

    const char *app = argv[1];
    int n = atoi(argv[2]);
    int seed = atoi(argv[3]);

    int32_t *arr = malloc(n * sizeof(int32_t));
    if (!arr) {
        fprintf(stderr, "Arr malloc error\n");
        return 1;
    }
    int read = load_arr(arr, n, seed, 0);
    if (read != 0) {
        fprintf(stderr, "Failed to read array!\n");
        free(arr);
        return 1;
    }
    int32_t *buf = malloc(n * sizeof(int32_t));
    if (!buf) {
        fprintf(stderr, "Buf malloc error\n");
        free(arr);
        return 1;
    }

    if (strcmp(app, "omp") == 0) {
        int workers = omp_get_max_threads();
        #pragma omp parallel
        {
            #pragma omp single
            {
                ctimer_start(&t);
                sort_omp(arr, buf, n, 1);
                ctimer_stop(&t);
                ctimer_measure(&t);
            }
        }

        printf("11,%d,%d,%d,%d,%ld.%09ld\n", workers,  n, seed, DIRECT_THRESHOLD, (long)t.elapsed.tv_sec, t.elapsed.tv_nsec);
        
        fprintf(stderr, "OpenMP sort(%d %d), threshold = %d, num workers = %d\n", n, seed, DIRECT_THRESHOLD, workers);
        check(arr, n);
        ctimer_print(t, "sort_omp");
    } else if (strcmp(app, "seq") == 0) {
        ctimer_start(&t);
        sort(arr, buf, n, 1);
        ctimer_stop(&t);
        ctimer_measure(&t);

        printf("10,1,%d,%d,0,%ld.%09ld\n", n, seed, (long)t.elapsed.tv_sec, t.elapsed.tv_nsec);
        
        fprintf(stderr, "SEQ sort(%d %d)\n", n, seed);
        check(arr, n);
        ctimer_print(t, "sort_seq");
    } else {
        fprintf(stderr, "Unknown app: %s\n", app);
    }

    free(arr);
    free(buf);
    return 0;
}