#pragma once

#include <stddef.h>
#include <stdint.h>

#define CPU_X2APIC (1 << 0)

#define SEL_CS64 0x8
#define SEL_DS64 0x10
#define SEL_USER_CS64 0x18
#define SEL_USER_DS64 0x20
#define SEL_TSS 0x28

#define READ_CPU_LOCAL(_type, _offset)                                                             \
   ({                                                                                              \
      _type _ret;                                                                                  \
      asm volatile("mov %%gs:(%c1), %0" : "=r"(_ret) : "i"(_offset));                              \
      _ret;                                                                                        \
   })

#define WRITE_CPU_LOCAL(_offset, _value)                                                           \
   do {                                                                                            \
      asm volatile("mov %0, %%gs:(%c1)" : : "r"(_value), "i"(_offset));                            \
   } while (0)

struct idt_entry
{
   uint16_t offset_low;
   uint16_t cs;
   uint8_t ist;
   uint8_t flags;
   uint16_t offset_mid;
   uint32_t offset_high;
   uint32_t reserved;
};

struct tss
{
   uint32_t reserved0;
   uint64_t rsp0;
   uint64_t rsp1;
   uint64_t rsp2;
   uint64_t reserved1;
   uint64_t ist[7];
   uint64_t reserved2;
   uint16_t reserved3;
   uint16_t iopb_offset;
} __attribute__((packed));

struct iret_frame
{
   uint64_t error_code;
   uint64_t rip;
   uint64_t cs;
   uint64_t rflags;
   uint64_t rsp;
   uint64_t ss;
};

struct cpu_context
{
   uint64_t ds;
   uint64_t es;
   uint64_t rax;
   uint64_t rbx;
   uint64_t rcx;
   uint64_t rdx;
   uint64_t rsi;
   uint64_t rdi;
   uint64_t rbp;
   uint64_t r8;
   uint64_t r9;
   uint64_t r10;
   uint64_t r11;
   uint64_t r12;
   uint64_t r13;
   uint64_t r14;
   uint64_t r15;
   uint64_t int_vec;

   struct iret_frame iret;
};

struct cpu
{
   uint32_t id;
   uint32_t flags;
   uint64_t lapic_base;
   uint32_t lapic_id;
   uint64_t gdt[7];
   struct idt_entry idt[256];
   struct tss tss;
   uint64_t tsc_frequency;
   uint64_t tsc_ratio_n;
   uint64_t tsc_ratio_p;
};

struct cpu_block
{
   struct cpu* cpu;
   struct thread* current_thread;
};

static inline struct cpu*
pcb_current_get_cpu(void)
{
   return READ_CPU_LOCAL(struct cpu*, offsetof(struct cpu_block, cpu));
}

static inline struct thread*
pcb_current_get_thread(void)
{
   return READ_CPU_LOCAL(struct thread*, offsetof(struct cpu_block, current_thread));
}

static inline void
pcb_current_set_thread(struct thread* thread)
{
   WRITE_CPU_LOCAL(offsetof(struct cpu_block, current_thread), thread);
}

void
cpu_init_early(void);

void
cpu_save_fpu_context(void* fpu_state);

void
cpu_restore_fpu_context(void* fpu_state);

size_t
cpu_fpu_context_size(void);
