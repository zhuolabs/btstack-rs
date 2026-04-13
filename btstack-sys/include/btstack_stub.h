#ifndef BTSTACK_STUB_H
#define BTSTACK_STUB_H

#ifdef __cplusplus
extern "C" {
#endif

void btstack_rs_init(void);
void btstack_rs_tick(void);
int btstack_rs_counter(void);

#ifdef __cplusplus
}
#endif

#endif /* BTSTACK_STUB_H */
