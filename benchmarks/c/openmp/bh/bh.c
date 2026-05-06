#include "bh.h"
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include <math.h>
#include <omp.h>

const double THETA = 0.5;
const double G = 6.67e-11;

Tree *tree_new(Quadrant q) {
    Tree *t = malloc(sizeof(Tree));
    if (!t) return NULL;

    t->quad = q;
    t->body.type = TREE_LEAF;
    t->body.leaf.bodies = malloc(LEAF_CAP * sizeof(TreeBody));
    if (!t->body.leaf.bodies) {
        free(t);
        return NULL;
    }
    t->body.leaf.count = 0;
    return t;
}

void tree_free(Tree *t) {
    if (!t) return;

    if (t->body.type == TREE_LEAF) {
        free(t->body.leaf.bodies);
    } else {
        for (size_t i = 0; i < 4; i++) {
            tree_free(t->body.aggregate.children[i]);
        }
    }

    free(t);
}

void append_leaf(Tree *restrict tree, TreeBody body) {
    if (tree->body.type == TREE_LEAF) {
        // assumes space!
        tree->body.leaf.bodies[tree->body.leaf.count++] = body;
    }
}

void split_leaf(Tree *restrict tree) {
    Quadrant nw_quad = quadrant_nw(&tree->quad);
    Quadrant ne_quad = quadrant_ne(&tree->quad);
    Quadrant sw_quad = quadrant_sw(&tree->quad);
    Quadrant se_quad = quadrant_se(&tree->quad);

    // Save old leaf data
    TreeBody *old_bodies = tree->body.leaf.bodies;
    int old_count = tree->body.leaf.count;

    tree->body.type = TREE_AGGREGATE;
    tree->body.aggregate.children[0] = tree_new(nw_quad);
    tree->body.aggregate.children[1] = tree_new(ne_quad);
    tree->body.aggregate.children[2] = tree_new(sw_quad);
    tree->body.aggregate.children[3] = tree_new(se_quad);

    // Re-distribute existing bodies
    for (int i = 0; i < old_count; i++) {
        TreeBody body = old_bodies[i];
        if (quadrant_contains(&nw_quad, body.px, body.py)) {
            append_leaf(tree->body.aggregate.children[0], body);
        } else if (quadrant_contains(&ne_quad, body.px, body.py)) {
            append_leaf(tree->body.aggregate.children[1], body);
        } else if (quadrant_contains(&sw_quad, body.px, body.py)) {
            append_leaf(tree->body.aggregate.children[2], body);
        } else if (quadrant_contains(&se_quad, body.px, body.py)) {
            append_leaf(tree->body.aggregate.children[3], body);
        }
    }

    // Free the old leaf bodies array
    free(old_bodies);
}

void compute_leaf_aggregate(Tree *restrict tree, TreeBody body, double *restrict out_mass, double *restrict out_px, double *restrict out_py) {
    if (tree->body.type != TREE_LEAF) {
        *out_mass = 0.0;
        *out_px = 0.0;
        *out_py = 0.0;
        return;
    }

    double mass = body.mass;
    double px = body.px * body.mass;
    double py = body.py * body.mass;

    for (int i = 0; i < tree->body.leaf.count; i++) {
        TreeBody b = tree->body.leaf.bodies[i];
        mass += b.mass;
        px += b.px * b.mass;
        py += b.py * b.mass;
    }

    px /= mass;
    py /= mass;

    *out_mass = mass;
    *out_px = px;
    *out_py = py;
}

void put_body(Tree *restrict tree, TreeBody body) {
    if (tree->body.type != TREE_AGGREGATE) return;

    AggregateNode *agg = &tree->body.aggregate;

    for (int i = 0; i < 4; i++) {
        Tree *child = agg->children[i];
        if (quadrant_contains(&child->quad, body.px, body.py)) {
            tree_insert(child, body);
            return;
        }
    }
}

void tree_insert(Tree *restrict tree, TreeBody body) {
    if (tree->body.type == TREE_LEAF) {
        if (tree->body.leaf.count < LEAF_CAP) {
            // leaf has space: just append
            tree->body.leaf.bodies[tree->body.leaf.count++] = body;
        } else {
            // leaf is full: convert to aggregate
            double mass, px, py;
            compute_leaf_aggregate(tree, body, &mass, &px, &py);

            // replace leaf with aggregate
            split_leaf(tree);
            tree->body.aggregate.mass = mass;
            tree->body.aggregate.px = px;
            tree->body.aggregate.py = py;
            tree->body.aggregate.count = LEAF_CAP + 1;

            // recursively insert body
            put_body(tree, body);
        }
    } else {
        // aggregate node: update center-of-mass and count
        AggregateNode *restrict agg = &tree->body.aggregate;
        double new_mass = agg->mass + body.mass;
        agg->px = (agg->px * agg->mass + body.px * body.mass) / new_mass;
        agg->py = (agg->py * agg->mass + body.py * body.mass) / new_mass;
        agg->mass = new_mass;
        agg->count += 1;

        put_body(tree, body);
    }
}

Force compute_force_seq(const Tree *restrict root, const Body *restrict body) {
    const double eps = 30000.0;
    Force total = {0.0, 0.0};

    if (root->body.type == TREE_LEAF) {
        // Leaf node: compute force from each body directly
        for (int i = 0; i < root->body.leaf.count; i++) {
            const TreeBody leaf = root->body.leaf.bodies[i];

            if (leaf.id != body->id) {
                double dx = leaf.px - body->px;
                double dy = leaf.py - body->py;

                double sq = dx * dx + dy * dy;
                double dist = sqrt(sq);

                double f = (G * body->mass * leaf.mass) / (dist * dist + eps * eps);

                total.x += f * dx / dist;
                total.y += f * dy / dist;
            }
        }
        return total;
    } else {
        // Aggregate
        const AggregateNode *restrict agg = &root->body.aggregate;

        const double s = root->quad.length; //quadrant_length(&root->quad);
        double dx = agg->px - body->px;
        double dy = agg->py - body->py;
        double sq = dx * dx + dy * dy;
        double dist = sqrt(sq);
        double ratio = s / dist;

        if (ratio < THETA) {
            // b is far away, use aggregate
            double f = (G * body->mass * agg->mass) / (dist * dist + eps * eps);
            total.x = f * dx / dist;
            total.y = f * dy / dist;
            return total;
        } else {
            // b is close, recurse
            for (int i = 0; i < 4; i++) {
                Force f = compute_force_seq(agg->children[i], body);
                total.x += f.x;
                total.y += f.y;
            }
            return total;
        }
    }

}

void tree_traverse_update_seq(const Tree *restrict tree, const Tree *restrict root, Body *restrict bodies) {
    if (tree->body.type == TREE_LEAF) {
        for (int i = 0; i < tree->body.leaf.count; i++) {
            int id = tree->body.leaf.bodies[i].id;
            Force f = compute_force_seq(root, &bodies[id]);
            body_set_force(&bodies[id], f.x, f.y);
        }
    } else {
        for (int i = 0; i < 4; i++) {
            tree_traverse_update_seq(tree->body.aggregate.children[i], root, bodies);
        }
    }

}

void tree_traverse_update_par(const Tree *restrict tree, const Tree *restrict root, Body *restrict bodies) {
    if (tree->body.type == TREE_LEAF) {
        for (int i = 0; i < tree->body.leaf.count; i++) {
            int id = tree->body.leaf.bodies[i].id;
            Force f = compute_force_seq(root, &bodies[id]);
            body_set_force(&bodies[id], f.x, f.y);
        }
    } else {
        if (tree->body.aggregate.count < THRESHOLD) {
            tree_traverse_update_seq(tree, root, bodies);
            return;
        }
        #pragma omp task
        tree_traverse_update_par(tree->body.aggregate.children[0], root, bodies);
        #pragma omp task
        tree_traverse_update_par(tree->body.aggregate.children[1], root, bodies);
        #pragma omp task
        tree_traverse_update_par(tree->body.aggregate.children[2], root, bodies);

        tree_traverse_update_par(tree->body.aggregate.children[3], root, bodies);
        #pragma omp taskwait
    }
}