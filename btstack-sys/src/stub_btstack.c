#include "btstack_stub.h"

static int g_counter = 0;

void btstack_rs_init(void) {
    g_counter = 0;
}

void btstack_rs_tick(void) {
    g_counter += 1;
}

int btstack_rs_counter(void) {
    return g_counter;
}
