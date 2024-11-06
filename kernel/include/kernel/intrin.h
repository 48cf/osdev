#pragma once

#include <stdint.h>

#define IA32_APIC_BASE 0x1b
#define IA32_TSC_DEADLINE 0x6e0
#define IA32_FS_BASE 0xc0000100
#define IA32_GS_BASE 0xc0000101
#define IA32_KERNEL_GS_BASE 0xc0000102

struct cpuid
{
   uint32_t eax;
   uint32_t ebx;
   uint32_t ecx;
   uint32_t edx;
};

void
_write_cr0(uint64_t value);

void
_write_cr3(uint64_t value);

void
_write_cr4(uint64_t value);

void
_write_cr8(uint64_t value);

uint64_t
_read_cr0(void);

uint64_t
_read_cr2(void);

uint64_t
_read_cr3(void);

uint64_t
_read_cr4(void);

uint64_t
_read_cr8(void);

void
_outb(uint16_t port, uint8_t value);

void
_outw(uint16_t port, uint16_t value);

void
_outl(uint16_t port, uint32_t value);

uint8_t
_inb(uint16_t port);

uint16_t
_inw(uint16_t port);

uint32_t
_inl(uint16_t port);

void
_mmoutb(uintptr_t address, uint8_t value);

void
_mmoutw(uintptr_t address, uint16_t value);

void
_mmoutl(uintptr_t address, uint32_t value);

void
_mmoutq(uintptr_t address, uint64_t value);

uint8_t
_mminb(uintptr_t address);

uint16_t
_mminw(uintptr_t address);

uint32_t
_mminl(uintptr_t address);

uint64_t
_mminq(uintptr_t address);

void
_invlpg(uintptr_t address);

void
_wrmsr(uint32_t msr, uint64_t value);

uint64_t
_rdmsr(uint32_t msr);

void
_xsetbv(uint32_t ecx, uint64_t value);

uint64_t
_xgetbv(uint32_t ecx);

void
_cpuid(uint32_t eax, uint32_t ecx, struct cpuid* result);

uint64_t
_rdtsc(void);

static inline bool
cpuid_is_leaf_supported(uint32_t leaf)
{
   struct cpuid cpuid;

   _cpuid(leaf, 0, &cpuid);

   return cpuid.eax >= leaf;
}

static inline bool
cpuid_is_extended_leaf_supported(uint32_t leaf)
{
   struct cpuid cpuid;

   _cpuid(0x80000000, 0, &cpuid);

   return cpuid.eax >= leaf;
}
