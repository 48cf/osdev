#pragma once

#include <stdint.h>

void
lapic_init(void);

void
lapic_eoi(void);

void
lapic_send_ipi(uint32_t lapic_id, uint8_t vector);
