#include <kernel/cpu.h>
#include <kernel/intrin.h>
#include <kernel/lapic.h>
#include <kernel/print.h>

#include <stdbool.h>

#define LAPIC_REG_ID 0x20
#define LAPIC_REG_EOI 0xb0
#define LAPIC_REG_LVT_TIMER 0x320

static inline void
lapic_write(struct cpu* cpu, size_t reg, uint64_t value)
{
   if (cpu->flags & CPU_X2APIC) {
      _wrmsr(0x800 + (reg >> 4), value);
   } else {
      _mmoutl(cpu->lapic_base + reg, (uint32_t)(value & 0xffffffff));
   }
}

static inline uint64_t
lapic_read(struct cpu* cpu, size_t reg)
{
   if (cpu->flags & CPU_X2APIC) {
      return _rdmsr(0x800 + (reg >> 4));
   } else {
      return _mminl(cpu->lapic_base + reg);
   }
}

static inline bool
is_x2apic_supported(void)
{
   struct cpuid cpuid;

   _cpuid(0x1, 0x0, &cpuid);

   return cpuid.ecx & (1 << 21);
}

void
lapic_init()
{
   struct cpu* cpu = pcb_current_get_cpu();

   uint64_t ia32_apic_base = _rdmsr(IA32_APIC_BASE);

   if (is_x2apic_supported()) {
      ia32_apic_base |= (1 << 10);

      printf("lapic: enabling x2apic for cpu %u\n", cpu->id);
      _wrmsr(IA32_APIC_BASE, ia32_apic_base);
   } else {
      printf("lapic: x2apic not supported, using xapic for cpu %u\n", cpu->id);
   }

   if (ia32_apic_base & (1 << 10)) {
      cpu->flags |= CPU_X2APIC;
   }

   cpu->lapic_base = ia32_apic_base & 0xfffff000;

   if (cpu->flags & CPU_X2APIC) {
      cpu->lapic_id = lapic_read(cpu, LAPIC_REG_ID);
   } else {
      cpu->lapic_id = lapic_read(cpu, LAPIC_REG_ID) >> 24;
   }

   printf("lapic: cpu %u has lapic id %u\n", cpu->id, cpu->lapic_id);
}

void
lapic_eoi()
{
   lapic_write(pcb_current_get_cpu(), LAPIC_REG_EOI, 0);
}

void
lapic_send_ipi(uint32_t lapic_id, uint8_t vector)
{
   // @todo: Implement this
}

void
lapic_timer_arm_one_shot(uint64_t ticks, uint8_t vector)
{
   struct cpu* cpu = pcb_current_get_cpu();

   if (cpu->flags & CPU_TSC_DEADLINE) {
      lapic_write(cpu, LAPIC_REG_LVT_TIMER, (uint32_t)vector | (0b10 << 17));
      _wrmsr(IA32_TSC_DEADLINE, ticks);
   } else {
      panic("lapic: implement regular lapic timer\n");
   }
}

void
lapic_timer_disarm(void)
{
   struct cpu* cpu = pcb_current_get_cpu();

   if (cpu->flags & CPU_TSC_DEADLINE) {
      _wrmsr(IA32_TSC_DEADLINE, 0);
   } else {
      panic("lapic: implement regular lapic timer\n");
   }
}
