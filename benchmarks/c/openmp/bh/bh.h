#ifndef BH_H
#define BH_H

#include <stdint.h>
#include <stddef.h>

extern const int THRESHOLD;
extern const int LEAF_CAP;
extern const double G;

typedef struct {
    int id;
    double mass;
    double px;
    double py;
    double vx;
    double vy;
    double fx;
    double fy;
} Body;

typedef struct {
    double x_mid;
    double y_mid;
    double length;
} Quadrant;


typedef enum {
    TREE_LEAF,
    TREE_AGGREGATE
} TreeNodeType;

typedef struct Tree Tree; // forward declaration

typedef struct {
    double mass;
    double px;
    double py;
    int count;
    Tree *children[4]; // NW, NE, SW, SE
} AggregateNode;

typedef struct {
    int id;
    double mass;
    double px;
    double py;
} TreeBody;

typedef struct {
    TreeNodeType type;
    union {
        struct {
            TreeBody *bodies;
            int count;
        } leaf;

        AggregateNode aggregate;
    };
} TreeNode;

struct Tree {
    TreeNode body;
    Quadrant quad;
};

typedef struct {
    double x;
    double y;
} Force;


Body body_new(int id, double mass, double px, double py, double vx, double vy);
int body_id(Body *b);
double body_mass(Body *b);
double body_px(Body *b);
double body_py(Body *b);
void body_set_force(Body *b, double fx, double fy);
void body_update(Body *b, double dt);
int body_inside(Body *b, Quadrant *q);

Quadrant quadrant_new(double x_mid, double y_mid, double length);
Quadrant quadrant_nw(Quadrant *q);
Quadrant quadrant_ne(Quadrant *q);
Quadrant quadrant_sw(Quadrant *q);
Quadrant quadrant_se(Quadrant *q);
double quadrant_length(const Quadrant *q);
int quadrant_contains(Quadrant *q, double x, double y);

Tree *tree_new(Quadrant q);
void tree_insert(Tree *restrict t, TreeBody b);
void tree_traverse_update_par(const Tree *t, const Tree *r, Body *restrict bodies);
void tree_traverse_update_seq(const Tree *t, const Tree *r, Body *restrict bodies);
void tree_free(Tree *t);

#endif