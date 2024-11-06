#include <kernel/cpu.h>
#include <kernel/intrin.h>
#include <kernel/print.h>

struct table_register
{
   uint16_t limit;
   uint64_t base;
} __attribute__((packed));

extern void
_fxsave(void* fpu_state);

extern void
_fxrstor(void* fpu_state);

extern void
_xsave(void* fpu_state);

extern void
_xrstor(void* fpu_state);

extern void
_lgdt(struct table_register* gdtr);

extern void
_lidt(struct table_register* idtr);

extern void
_ltr(uint16_t selector);

extern void
_reload_segments(uint16_t cs);

extern uintptr_t _isr_handlers[256];

static size_t fpu_context_size;

static void (*fpu_save)(void* fpu_state) = _fxsave;
static void (*fpu_restore)(void* fpu_state) = _fxrstor;

struct cpu_block bsp_cpu_block;
struct cpu bsp_cpu;

static void
init_cpu_features()
{
   struct cpuid cpuid;

   _cpuid(0x1, 0, &cpuid);

   // check for SSE support
   assert(cpuid.edx & (1 << 24) && (cpuid.edx & (1 << 25)));

   uint64_t cr0 = _read_cr0();

   cr0 &= ~(1 << 2); // CR0.EM
   cr0 &= ~(1 << 5); // CR0.NE

   uint64_t cr4 = _read_cr4();

   cr4 |= (1 << 9); // CR4.OSFXSR

   // check for XSAVE support
   if (cpuid.ecx & (1 << 26)) {
      cr4 |= (1 << 18); // CR4.OSXSAVE
   }

   _write_cr0(cr0);
   _write_cr4(cr4);

   // check for XSAVE and AVX support
   if (cpuid.ecx & (1 << 26)) {
      uint64_t xcr0 = 0;

      xcr0 |= (1 << 0); // x86
      xcr0 |= (1 << 1); // sse

      if (cpuid.ecx & (1 << 28)) {
         xcr0 |= (1 << 2); // avx
      }

      _cpuid(0x7, 0, &cpuid);

      // check for AVX-512 support
      if (cpuid.ebx & (1 << 16)) {
         xcr0 |= (1 << 5); // opmask
         xcr0 |= (1 << 6); // zmm_hi256
         xcr0 |= (1 << 7); // hi16_zmm
      }

      _xsetbv(0, xcr0);
   }

   _cpuid(0x7, 0, &cpuid);

   // check for SMEP support
   if (cpuid.ebx & (1 << 7)) {
      cr4 |= (1 << 20); // CR4.SMEP
   }

   // // check for SMAP support
   if (cpuid.ebx & (1 << 20)) {
      cr4 |= (1 << 21); // CR4.SMAP
   }

   _write_cr4(cr4);
}

static void
setup_gdt(struct cpu* cpu)
{
   uint64_t tss_address = (uint64_t)&cpu->tss;
   uint64_t tss_low = 0x0000890000000000;
   uint64_t tss_high = tss_address >> 32;

   tss_low |= sizeof(cpu->tss) - 1;
   tss_low |= (tss_address & 0xffff) << 16;
   tss_low |= ((tss_address >> 16) & 0xff) << 32;
   tss_low |= ((tss_address >> 24) & 0xff) << 56;

   cpu->gdt[0] = 0;                  // null descriptor
   cpu->gdt[1] = 0x00209b0000000000; // code segment
   cpu->gdt[2] = 0x0020930000000000; // data segment
   cpu->gdt[3] = 0x0020fb0000000000; // user code segment
   cpu->gdt[4] = 0x0020f30000000000; // user data segment
   cpu->gdt[5] = tss_low;            // tss (low)
   cpu->gdt[6] = tss_high;           // tss (high)
}

static void
setup_idt(struct cpu* cpu)
{
   for (size_t i = 0; i < 256; ++i) {
      const uintptr_t handler = _isr_handlers[i];

      cpu->idt[i].offset_low = (uint16_t)(handler & 0xffff);
      cpu->idt[i].cs = SEL_CS64;
      cpu->idt[i].ist = 0;
      cpu->idt[i].flags = 0x8e; // present, interrupt gate
      cpu->idt[i].offset_mid = (uint16_t)((handler >> 16) & 0xffff);
      cpu->idt[i].offset_high = (uint32_t)((handler >> 32) & 0xffffffff);
      cpu->idt[i].reserved = 0;
   }
}

void
cpu_init_early(void)
{
   _wrmsr(IA32_GS_BASE, (uint64_t)&bsp_cpu_block);

   bsp_cpu_block.cpu = &bsp_cpu;
   bsp_cpu.id = 0;

   TAILQ_INIT(&bsp_cpu.timer_queue);

   init_cpu_features();

   struct cpuid cpuid;

   _cpuid(0x1, 0, &cpuid);

   if (cpuid.ecx & (1 << 24)) {
      printf("cpu: tsc deadline timer support on cpu %u\n", bsp_cpu.id);

      bsp_cpu.flags |= CPU_TSC_DEADLINE;
   }

   // check for XSAVE support
   if (cpuid.ecx & (1 << 26)) {
      _cpuid(0xd, 0, &cpuid);

      fpu_context_size = cpuid.ebx;
      fpu_save = _xsave;
      fpu_restore = _xrstor;
   } else {
      fpu_context_size = 512;
   }

   setup_gdt(&bsp_cpu);
   setup_idt(&bsp_cpu);

   struct table_register gdtr = {
      .limit = sizeof(bsp_cpu.gdt) - 1,
      .base = (uint64_t)&bsp_cpu.gdt,
   };

   struct table_register idtr = {
      .limit = sizeof(bsp_cpu.idt) - 1,
      .base = (uint64_t)&bsp_cpu.idt,
   };

   _lgdt(&gdtr);
   _lidt(&idtr);
   _reload_segments(SEL_CS64);
   _ltr(SEL_TSS);
}

void
cpu_save_fpu_context(void* fpu_state)
{
   fpu_save(fpu_state);
}

void
cpu_restore_fpu_context(void* fpu_state)
{
   fpu_restore(fpu_state);
}

size_t
cpu_fpu_context_size(void)
{
   return fpu_context_size;
}
