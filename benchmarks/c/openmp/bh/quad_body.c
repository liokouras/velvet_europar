#include "bh.h"
#include <stdint.h>
#include <stddef.h>

Body body_new(int id, double mass, double px, double py, double vx, double vy) {
    Body b;
    b.id = id;
    b.mass = mass;
    b.px = px;
    b.py = py;
    b.vx = vx;
    b.vy = vy;
    b.fx = 0.0;
    b.fy = 0.0;
    return b;
}
int body_id(Body *b) { return b->id; }
double body_mass(Body *b) { return b->mass; }
double body_px(Body *b) { return b->px; }
double body_py(Body *b) { return b->py; }
void body_set_force(Body *b, double fx, double fy) {
    b->fx = fx;
    b->fy = fy;
}
void body_update(Body *b, double dt) {
    b->vx += dt * b->fx / b->mass;
    b->vy += dt * b->fy / b->mass;
    b->px += dt * b->vx;
    b->py += dt * b->vy;
    b->fx = 0.;
    b->fy = 0.;
}
int body_inside(Body *b, Quadrant *q) {
    return quadrant_contains(q, b->px, b->py);
}

Quadrant quadrant_new(double x_mid, double y_mid, double length) {
    Quadrant q;
    q.x_mid = x_mid;
    q.y_mid = y_mid;
    q.length = length;
    return q;
}
Quadrant quadrant_nw(Quadrant *q) {
    return quadrant_new(
        q->x_mid - q->length / 4.,
        q->y_mid + q->length / 4.,
        q->length / 2.
    );
}
Quadrant quadrant_ne(Quadrant *q) {
    return quadrant_new(
        q->x_mid + q->length / 4.,
        q->y_mid + q->length / 4.,
        q->length / 2.
    );
}
Quadrant quadrant_sw(Quadrant *q) {
    return quadrant_new(
        q->x_mid - q->length / 4.,
        q->y_mid - q->length / 4.,
        q->length / 2.
    );
}
Quadrant quadrant_se(Quadrant *q) {
    return quadrant_new(
        q->x_mid + q->length / 4.,
        q->y_mid - q->length / 4.,
        q->length / 2.
    );
}
double quadrant_length(const Quadrant *q) {
    return q->length;
}
int quadrant_contains(Quadrant *q, double x, double y) {
    double half = q->length/2;
    if (x <= (q->x_mid + half) && 
        x >= (q->x_mid - half) &&
        y <= (q->y_mid + half) && 
        y >= (q->y_mid - half)) {
            return 1;
        }
    return 0;
}


