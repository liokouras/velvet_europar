#include "distance_table.h"

#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <limits.h>
#include <math.h>

static Coord *read_towns_from_file(const char *filename, int ntowns) {
    FILE *f = fopen(filename, "r");
    if (!f) {
        perror("fopen");
        return NULL;
    }

    Coord *towns = malloc(sizeof(Coord) * ntowns);
    if (!towns) {
        fclose(f);
        return NULL;
    }

    for (int i = 0; i < ntowns; i++) {
        if (fscanf(f, "%d %d", &towns[i].x, &towns[i].y) != 2) {
            fprintf(stderr, "Error reading town %d\n", i);
            free(towns);
            fclose(f);
            return NULL;
        }
    }

    fclose(f);
    return towns;
}

static int isqrt_int(int x) {
    return (int)sqrt((double)x);
}

static void put_min(int *vec, int len, int pos) {
    int minpos = pos;
    int min = INT_MAX;

    for (int i = pos; i < len; i++) {
        if (vec[i] == 0)
            vec[i] = INT_MAX;
        if (vec[i] < min) {
            min = vec[i];
            minpos = i;
        }
    }

    int tmp = vec[pos];
    vec[pos] = vec[minpos];
    vec[minpos] = tmp;
}

static void sort_vec(int *vec, int len) {
    for (int i = 0; i < len; i++) {
        put_min(vec, len, i);
    }
}

static int calc_lower_bound(int hops, const int *table) {
    int res = 0;
    for (int i = 0; i < hops; i++) {
        res += table[i];
    }
    return res;
}

DistanceTable *distance_table_generate(int ntowns, int seed) {
    DistanceTable *dt = malloc(sizeof(DistanceTable));
    if (!dt) return NULL;

    char filename[256];
    snprintf(filename, sizeof(filename), "../../data/dist_tab_%d_%d.txt", ntowns, seed);
    Coord *towns = read_towns_from_file(filename, ntowns);
    if (!towns){
        free(dt);
        return NULL;
    }

    dt->ntowns = ntowns;
    dt->to_city = calloc(ntowns * ntowns, sizeof(int));
    dt->dist = calloc(ntowns * ntowns, sizeof(int));
    dt->lower_bounds = calloc(ntowns, sizeof(int));

    int *temp_dist = malloc(sizeof(int) * ntowns);
    int *min_dists = malloc(sizeof(int) * ntowns * ntowns);
    int min_dist_count = 0;
    int dx, dy, d, tmp, x;

    for (int i = 0; i < ntowns; i++) {
        for (int j = 0; j < ntowns; j++) {
            dx = towns[i].x - towns[j].x;
            dy = towns[i].y - towns[j].y;
            d = isqrt_int((int)(dx*dx + dy*dy));

            temp_dist[j] = d;

            if (i != j && d != 0) {
                min_dists[min_dist_count++] = d;
            }
        }

        // Sort pairs[i]: nearest city first.
        for (int j = 0; j < ntowns; j++) {
            tmp = INT_MAX;
            x = 0;

            for (int k = 0; k < ntowns; k++) {
                if (temp_dist[k] < tmp) {
                    tmp = temp_dist[k];
                    x = k;
                }
            }

            temp_dist[x] = INT_MAX;
            dt->to_city[i * ntowns + j] = x;
            dt->dist[i * ntowns + j] = tmp;
        }
    }

    sort_vec(min_dists, min_dist_count);

    for (int i = 0; i < ntowns; i++) {
        dt->lower_bounds[i] = calc_lower_bound(i, min_dists);
    }

    free(towns);
    free(temp_dist);
    free(min_dists);

    return dt;
}

int distance_num_towns(const DistanceTable *restrict dt) {
    return dt->ntowns;
}

int distance_lower_bound(const DistanceTable *restrict dt, int idx) {
    return dt->lower_bounds[idx];
}

__attribute__((always_inline)) inline int distance_to_city(const DistanceTable *restrict dt, int source, int finish) {
    return dt->to_city[source * dt->ntowns + finish];
}

__attribute__((always_inline)) inline int distance_dist(const DistanceTable *restrict dt, int source, int finish) {
    return dt->dist[source * dt->ntowns + finish];
}

void distance_table_free(DistanceTable *dt) {
    if (!dt) return;
    free(dt->lower_bounds);
    free(dt->to_city);
    free(dt->dist);
    free(dt);
}
