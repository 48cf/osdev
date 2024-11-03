#include <kernel/cpu.h>
#include <kernel/intr.h>
#include <kernel/lapic.h>
#include <kernel/print.h>

static int_handler_t handlers[256 - 32];

void
dispatch_isr(struct cpu_context* context)
{
   if (context->int_vec >= 32) {
      int_handler_t handler = handlers[context->int_vec - 32];

      assert(handler != NULL);
      lapic_eoi();

      handler(context);
   } else {
      struct cpu* cpu = get_current_cpu();

      panic("unhandled exception %llu on cpu %u\n", context->int_vec, cpu->id);
   }
}

void
int_set_handler(uint8_t vector, int_handler_t handler)
{
   assert(vector >= 32);

   handlers[vector - 32] = handler;
}
