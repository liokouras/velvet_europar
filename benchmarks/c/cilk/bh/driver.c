#include "bh.h"
#include "../../ctimer.h"

#include <stdlib.h>
#include <stdio.h>
#include <string.h>

#include <cilk/cilk.h>
#include <cilk/cilk_api.h>

const int THRESHOLD = 500;
const int LEAF_CAP = 4;
double dt = 0.1; // time quantum


// FILE IO
int read_input(const char *input_file, Body **out_bodies, int *out_count, double *out_radius) {
    FILE *file = fopen(input_file, "r");
    if (!file) { return -1; }

    int n;
    double radius;

    if (fscanf(file, "%d", &n) != 1) {
        fclose(file);
        return -1;
    }

    if (fscanf(file, "%lf", &radius) != 1) {
        fclose(file);
        return -1;
    }

    Body *bodies = malloc(n * sizeof(Body));
    if (!bodies) {
        fclose(file);
        return -1;
    }

    for (int i = 0; i < n; i++) {
        int id, garb1, garb2, garb3;
        double px, py, vx, vy, mass;

        if (fscanf(
                file,
                "%d %lf %lf %lf %lf %lf %d %d %d",
                &id, &px, &py, &vx, &vy, &mass, &garb1, &garb2, &garb3
            ) != 9) {
            free(bodies);
            fclose(file);
            return -1;
        }

        bodies[i] = body_new(id, mass, px, py, vx, vy);
    }

    fclose(file);

    *out_bodies = bodies;
    *out_count = n;
    *out_radius = radius;

    return 0;
}
int write_output(const char *output_file, Body *bodies, int count, double radius) {
    FILE *writer = fopen(output_file, "w");
    if (writer == NULL) {
        return -1;
    }

    fprintf(writer, "%f\n", radius);

    for (int i = 0; i < count; i++) {
        fprintf(writer, "%d %f %f\n",
                body_id(&bodies[i]),
                body_px(&bodies[i]),
                body_py(&bodies[i]));
    }

    fclose(writer);
    return 0;
}

int par_main(const char *input_file, const char *output_file, int num_iterations) {
    // READ INPUT from input file
    int num_bodies;
    double radius;
    Body *bodies;
    read_input(input_file, &bodies, &num_bodies, &radius);

    ctimer_t full_duration, build_tree, update_force, update_body;
    ctimer_reset(&build_tree);
    ctimer_reset(&update_force);
    ctimer_reset(&update_body);

    // run simulation
    ctimer_start(&full_duration);
    Quadrant quad = quadrant_new(0., 0., radius * 2.);
    
    for (int iter = 0; iter < num_iterations; iter++) {
        // build the Barnes-Hut tree
        ctimer_start(&build_tree);
        Tree *restrict tree = tree_new(quad);
        for (int b = 0; b < num_bodies; b++) {
            Body *og_body = &bodies[b];
            if (body_inside(og_body, &quad)) {
                TreeBody body;
                body.id = b;
                body.mass = body_mass(og_body);
                body.px = body_px(og_body);
                body.py = body_py(og_body);
                tree_insert(tree, body);
            }
        }
        ctimer_stop(&build_tree);
        ctimer_lap(&build_tree);

        // update the forces
        ctimer_start(&update_force);
        tree_traverse_update_par(tree, tree, bodies);
        ctimer_stop(&update_force);
        ctimer_lap(&update_force);

        // update the positions, velocities, and accelerations
        ctimer_start(&update_body);
        for (int b = 0; b < num_bodies; b++) {
            body_update(&bodies[b], dt);
        }
        ctimer_stop(&update_body);
        ctimer_lap(&update_body);
        tree_free(tree);

        ctimer_stop(&full_duration);
        ctimer_measure(&full_duration);
    }

    // write_output(output_file, bodies, num_bodies, radius);

    int workers = __cilkrts_get_nworkers();
    printf("12,%d,%d,%d,%ld.%09ld,%ld.%09ld,%ld.%09ld,%ld.%09ld\n", workers, LEAF_CAP, THRESHOLD, (long)full_duration.elapsed.tv_sec, full_duration.elapsed.tv_nsec, (long)build_tree.elapsed.tv_sec, build_tree.elapsed.tv_nsec, (long)update_force.elapsed.tv_sec, update_force.elapsed.tv_nsec, (long)update_body.elapsed.tv_sec, update_body.elapsed.tv_nsec);
    ctimer_print(full_duration, "full duration");
    ctimer_print(build_tree, "build tree");
    ctimer_print(update_force, "update force");

    free(bodies);
    return 0;
}

int seq_main(const char *input_file, const char *output_file, int num_iterations) {
    // READ INPUT from input file
    int num_bodies;
    double radius;
    Body *bodies;
    read_input(input_file, &bodies, &num_bodies, &radius);

    ctimer_t full_duration, build_tree, update_force, update_body;
    ctimer_reset(&build_tree);
    ctimer_reset(&update_force);
    ctimer_reset(&update_body);

    // run simulation
    ctimer_start(&full_duration);
    Quadrant quad = quadrant_new(0., 0., radius * 2.);
    for (int iter = 0; iter < num_iterations; iter++) {
        // build the Barnes-Hut tree
        ctimer_start(&build_tree);
        Tree *restrict tree = tree_new(quad);
        for (int b = 0; b < num_bodies; b++) {
            Body *og_body = &bodies[b];
            if (body_inside(og_body, &quad)) {
                TreeBody body;
                body.id = b;
                body.mass = body_mass(og_body);
                body.px = body_px(og_body);
                body.py = body_py(og_body);
                tree_insert(tree, body);
            }
        }
        ctimer_stop(&build_tree);
        ctimer_lap(&build_tree);

        // update the forces
        ctimer_start(&update_force);
        tree_traverse_update_seq(tree, tree, bodies);
        ctimer_stop(&update_force);
        ctimer_lap(&update_force);

        // update the positions, velocities, and accelerations
        ctimer_start(&update_body);
        for (int b = 0; b < num_bodies; b++) {
            body_update(&bodies[b], dt);
        }
        ctimer_stop(&update_body);
        ctimer_lap(&update_body);
        tree_free(tree);
    }
    ctimer_stop(&full_duration);
    ctimer_measure(&full_duration);

    // write_output(output_file, bodies, num_bodies, radius);

    printf("10,1,%d,%d,%ld.%09ld,%ld.%09ld,%ld.%09ld,%ld.%09ld\n", LEAF_CAP, THRESHOLD, (long)full_duration.elapsed.tv_sec, full_duration.elapsed.tv_nsec, (long)build_tree.elapsed.tv_sec, build_tree.elapsed.tv_nsec, (long)update_force.elapsed.tv_sec, update_force.elapsed.tv_nsec, (long)update_body.elapsed.tv_sec, update_body.elapsed.tv_nsec);
    ctimer_print(full_duration, "full duration");
    ctimer_print(build_tree, "build tree");
    ctimer_print(update_force, "update force");

    free(bodies);
    return 0;
}

int main(int argc, char *argv[]) {
    if (argc < 5) {
        fprintf (stderr, "Usage: %s [cilk|seq] [input_file] [output_file] [num_iters]\n", argv[0]);
        return 1;
    }

    const char *app = argv[1];
    const char *input_file = argv[2];
    const char *output_file = argv[3];
    int num_iterations = atoi(argv[4]);


    if (strcmp(app, "cilk") == 0) {
        par_main(input_file, output_file, num_iterations);
    } else if (strcmp(app, "seq") == 0) {
       seq_main(input_file, output_file, num_iterations);
    } else {
        fprintf(stderr, "Unknown app: %s\n", app);
        return 1;
    }

    return 0;
}