#include <kernel/acpi.h>
#include <kernel/print.h>

#include <uacpi/acpi.h>
#include <uacpi/uacpi.h>

static char temp_buffer[4096];

void
acpi_init()
{
   assert(uacpi_setup_early_table_access(temp_buffer, sizeof(temp_buffer)) == UACPI_STATUS_OK);
}

bool
acpi_get_table(const char signature[4], size_t index, uacpi_table* out)
{
   if (uacpi_table_find_by_signature(signature, out) != UACPI_STATUS_OK) {
      return false;
   }

   while (index != 0) {
      if (uacpi_table_find_next_with_same_signature(out) != UACPI_STATUS_OK) {
         return false;
      }

      index--;
   }

   return true;
}
