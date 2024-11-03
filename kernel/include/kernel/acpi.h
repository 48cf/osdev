#pragma once

#include <uacpi/acpi.h>
#include <uacpi/tables.h>
#include <uacpi/uacpi.h>

void
acpi_init(void);

bool
acpi_get_table(const char signature[4], size_t index, uacpi_table* out);
