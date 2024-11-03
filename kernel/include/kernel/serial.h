#pragma once

#include <stdint.h>

#define COM1_PORT 0x3f8

void
serial_init(uint16_t port);

void
serial_out(uint16_t port, char ch);
