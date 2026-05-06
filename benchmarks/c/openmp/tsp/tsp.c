#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <omp.h>
#include "../../ctimer.h"
#include <stdatomic.h>
#include <limits.h>

#include "distance_table.h"

typedef __uint128_t uint128;
const int THRESHOLD = 6;
_Atomic size_t MINIMUM = SIZE_MAX;
DistanceTable *DISTANCE = NULL;

void tsp(int hops, int last, uint128 path, int length) {
    const int ntowns = distance_num_towns(DISTANCE);

    if (length + DISTANCE->lower_bounds[ntowns - hops] >= MINIMUM) {
        // stop searching, this path is too long...
        return;
    } else if (hops == ntowns) {
        // found a full route better than current best route
        atomic_store_explicit(&MINIMUM, length, memory_order_relaxed);
        // path_print(&path);
        return;
    }

    // try all cities not on the path, in "nearest-city-first" order
    for (int i = 0; i < ntowns; i++) {
        const int city = DISTANCE->to_city[last * ntowns + i];
        const uint128 city_bit = (uint128)1 << city;

        if (city != last && (path & city_bit) == 0) {
            const int dist = DISTANCE->dist[last * ntowns + i];            
            const uint128 new_path = path | city_bit;
            tsp(hops + 1, city, new_path, length + dist);
        }
    }
}

void tsp_omp(int hops, int last, uint128 path, int length) {
    const int ntowns = distance_num_towns(DISTANCE);

    if (length + DISTANCE->lower_bounds[ntowns - hops] >= MINIMUM) {
        // stop searching, this path is too long...
        return;
    } else if (hops == ntowns) {
        // found a full route better than current best route
        atomic_store_explicit(&MINIMUM, length, memory_order_relaxed);
        // path_print(&path);
        return;
    }

    if (hops > THRESHOLD) {
        tsp(hops, last, path, length);
        return;
    }

    // try all cities not on the path, in "nearest-city-first" order
    for (int i = ntowns-1; i >= 0 ; i--) { //for (int i = 0; i < ntowns ; i++) { //
        const int city = DISTANCE->to_city[last * ntowns + i];
        const uint128 city_bit = (uint128)1 << city;

        if (city != last && (path & city_bit) == 0) {
            const int dist = DISTANCE->dist[last * ntowns + i];            
            const uint128 new_path = path | city_bit;
            #pragma omp task firstprivate(hops, city, new_path, length, dist)
            tsp_omp(hops + 1, city, new_path, length + dist);
        }
    }
    #pragma omp taskwait
}

int main(int argc, char *argv[]) {
    if (argc < 4) {
        fprintf (stderr, "Usage: %s [omp|seq] [number_of_towns] [random_seed]\n", argv[0]);
        return 1;
    }

    ctimer_t t;

    const char *app = argv[1];
    int ntowns = atoi(argv[2]);
    int seed = atoi(argv[3]);

    DistanceTable *dt = distance_table_generate(ntowns, seed);
    DISTANCE = dt;

    uint128 path = (uint128)1 << 0; // City 0 is visited

    if (strcmp(app, "omp") == 0) {
        int workers = omp_get_max_threads();
        #pragma omp parallel
        {
            #pragma omp single
            {
                ctimer_start(&t);
                tsp_omp(1, 0, path, 0);
                ctimer_stop(&t);
                ctimer_measure(&t);
            }
        }
        int result = atomic_load(&MINIMUM);

        printf("11,%d,%d,%d,%d,%ld.%09ld\n", workers,  ntowns, seed, THRESHOLD, (long)t.elapsed.tv_sec, t.elapsed.tv_nsec);

        fprintf(stderr, "OpenMP tsp(%d %d) = %d, threshold = %d, num workers = %d\n", ntowns, seed, result, THRESHOLD, workers);
        ctimer_print(t, "tsp_omp");
    } else if (strcmp(app, "seq") == 0) {
        ctimer_start(&t);
        tsp(1, 0, path, 0);
        ctimer_stop(&t);
        ctimer_measure(&t);
        int result = atomic_load(&MINIMUM);

        printf("10,1,%d,%d,0,%ld.%09ld\n", ntowns, seed, (long)t.elapsed.tv_sec, t.elapsed.tv_nsec);

        fprintf(stderr, "SEQ tsp(%d %d) = %d\n", ntowns, seed, result);
        ctimer_print(t, "tsp_seq");
    } else {
        fprintf(stderr, "Unknown app: %s\n", app);
        return 1;
    }
    return 0;
}
