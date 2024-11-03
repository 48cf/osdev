#include <kernel/cpu.h>
#include <kernel/intr.h>
#include <kernel/intrin.h>
#include <kernel/lapic.h>
#include <kernel/print.h>

static int_handler_t handlers[256 - 32];

void
dispatch_isr(struct cpu_context* ctx)
{
   if (ctx->int_vec >= 32) {
      int_handler_t handler = handlers[ctx->int_vec - 32];

      assert(handler != NULL);
      lapic_eoi();

      handler(ctx);
   } else {
      struct cpu* cpu = get_current_cpu();

      printf("rax = 0x%016llx rbx = 0x%016llx rcx = 0x%016llx\n", ctx->rax, ctx->rbx, ctx->rcx);
      printf("rdx = 0x%016llx rsi = 0x%016llx rdi = 0x%016llx\n", ctx->rdx, ctx->rsi, ctx->rdi);
      printf("rbp = 0x%016llx r8  = 0x%016llx r9  = 0x%016llx\n", ctx->rbp, ctx->r8, ctx->r9);
      printf("r10 = 0x%016llx r11 = 0x%016llx r12 = 0x%016llx\n", ctx->r10, ctx->r11, ctx->r12);
      printf("r13 = 0x%016llx r14 = 0x%016llx r15 = 0x%016llx\n", ctx->r13, ctx->r14, ctx->r15);
      printf("rip = 0x%016llx rsp = 0x%016llx err = 0x%016llx\n",
             ctx->iret.rip,
             ctx->iret.rsp,
             ctx->iret.error_code);

      if (ctx->int_vec == 0xe) {
         uint64_t cr2 = _read_cr2();
         uint64_t cr3 = _read_cr3();

         printf("cr2 = 0x%016llx cr3 = 0x%016llx\n", cr2, cr3);

         if (ctx->iret.error_code & (1 << 3)) {
            panic("page fault caused by reserved bit violation\n");
         } else if (ctx->iret.error_code & (1 << 4)) {
            const char* present = ctx->iret.error_code & (1 << 0) ? "non-" : "";
            const char* mode = ctx->iret.cs & 3 ? "user" : "kernel";
            const char* reason;

            if (ctx->iret.error_code & (1 << 1)) {
               reason = "write to";
            } else if (ctx->iret.error_code & (1 << 4)) {
               reason = "instruction fetch from";
            } else {
               reason = "read from";
            }

            panic("page fault caused by %s a %spresent page from %s mode\n", reason, present, mode);
         }
      }

      panic("unhandled exception %llu on cpu %u\n", ctx->int_vec, cpu->id);
   }
}

void
int_set_handler(uint8_t vector, int_handler_t handler)
{
   assert(vector >= 32);

   handlers[vector - 32] = handler;
}
