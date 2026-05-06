#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
#include <cilk/cilk.h>
#include <cilk/cilk_api.h>
#include "../../ctimer.h"

const double THRESHOLD = 1500;

static inline double f(double x) {
    return sin(x) * 0.1 * x;
}

double adapint(double a, double b, double epsilon) {
    double delta = (b - a) / 2.0;
    double deltahalf = delta / 2.0;
    double mid = delta + a;

    double fa = f(a);
    double fb = f(b);
    double fmid = f(mid);

    double total = delta * (fa + fb);
    double left = deltahalf * (fa + fmid);
    double right = deltahalf * (fb + fmid);

    double diff = total - (left + right);
    if (diff < 0.0) {
        diff = -diff;
    }

    if (diff < epsilon) {
        return total;
    }

    double i1 = adapint(mid, b, epsilon);
    double i2 = adapint(a, mid, epsilon);
    return i1 + i2;
}

double adapint_cilk(double a, double b, double epsilon) {
    double delta = (b - a) / 2.0;
    double deltahalf = delta / 2.0;
    double mid = delta + a;

    double fa = f(a);
    double fb = f(b);
    double fmid = f(mid);

    double total = delta * (fa + fb);
    double left = deltahalf * (fa + fmid);
    double right = deltahalf * (fb + fmid);

    double diff = total - (left + right);
    if (diff < 0.0) {
        diff = -diff;
    }

    if (diff < epsilon) {
        return total;
    }

    double i1, i2;

    if (diff <= (double)THRESHOLD) {
        i1 = adapint(mid, b, epsilon);
        i2 = adapint(a, mid, epsilon);
        return i1 + i2;
    }

    cilk_scope {
        i1 = cilk_spawn adapint_cilk(mid, b, epsilon);
        i2 = adapint_cilk(a, mid, epsilon);
    }

    
    return i1 + i2;
}

int main(int argc, char *argv[]) {
    if (argc < 5) {
        fprintf (stderr, "Usage: %s [cilk|seq] [a] [b] [epsilon]\n", argv[0]);
        return 1;
    }

    ctimer_t t;

    const char *app = argv[1];
    double a = strtod(argv[2], NULL);
    double b = strtod(argv[3], NULL);
    double epsilon = strtod(argv[4], NULL);

    if (strcmp(app, "cilk") == 0) {
        int workers = __cilkrts_get_nworkers();
        ctimer_start(&t);
        double result = adapint_cilk(a, b, epsilon);
        ctimer_stop(&t);
        ctimer_measure(&t);

        printf("12,%d,%f,%f,%f,%f,%ld.%09ld\n", workers, a, b, epsilon, THRESHOLD, (long)t.elapsed.tv_sec, t.elapsed.tv_nsec);

        fprintf(stderr, "CILK adapint(%f %f %f) = %f, threshold = %f, num workers = %d\n", a, b, epsilon, result, THRESHOLD, workers);
        ctimer_print(t, "adapint_cilk");
    } else if (strcmp(app, "seq") == 0) {
        ctimer_start(&t);
        double result = adapint(a, b, epsilon);
        ctimer_stop(&t);
        ctimer_measure(&t);

        printf("10,1,%f,%f,%f,0,%ld.%09ld\n", a, b, epsilon, (long)t.elapsed.tv_sec, t.elapsed.tv_nsec);

        fprintf(stderr, "SEQ adapint(%f %f %f) = %f\n", a, b, epsilon, result);
        ctimer_print(t, "adapint_seq");
    } else {
        fprintf(stderr, "Unknown app: %s\n", app);
        return 1;
    }

    return 0;
}
