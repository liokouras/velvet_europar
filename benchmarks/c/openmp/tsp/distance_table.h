#ifndef DISTANCE_TABLE_H
#define DISTANCE_TABLE_H

#include <stddef.h>
#include <stdint.h>

typedef struct {
    int x;
    int y;
} Coord;

typedef struct {
    int ntowns;
    int *lower_bounds;  // length ntowns
    int *to_city;       // length ntowns * ntowns
    int *dist;          // length ntowns * ntowns
} DistanceTable;

DistanceTable *distance_table_generate(int ntowns, int seed);
void distance_table_free(DistanceTable *dt);
int distance_num_towns(const DistanceTable *restrict dt);
int distance_lower_bound(const DistanceTable *restrict dt, int idx);
int distance_to_city(const DistanceTable *restrict dt, int source, int finish);
int distance_dist(const DistanceTable *restrict dt, int source, int finish);

#endif
