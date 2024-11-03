#pragma once

#include <kernel/cpu.h>

typedef void (*int_handler_t)(struct cpu_context*);

void
int_set_handler(uint8_t vector, int_handler_t handler);
