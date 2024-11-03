#pragma once

#include <stdint.h>

#define PAGE_SIZE 0x1000ull
#define PAGE_SIZE_SHIFT 12

#define PAGE_SIZE_L2 0x200000ull
#define PAGE_SIZE_L2_SHIFT 21

#define PAGE_SIZE_L3 0x40000000ull
#define PAGE_SIZE_L3_SHIFT 30

#define MMU_PRESENT (1 << 0)
#define MMU_READ (1 << 1)
#define MMU_WRITE (1 << 2)
#define MMU_EXECUTE (1 << 3)
#define MMU_USER (1 << 4)
#define MMU_WRITE_THROUGH (1 << 5)
#define MMU_CACHE_DISABLE (1 << 6)
#define MMU_LARGE_PAGE (1 << 7)

struct page_table
{
   uint64_t cr3;
   uint64_t* entries;
};

extern struct page_table kernel_page_table;

extern uintptr_t hhdm_offset;
extern uintptr_t pfndb_offset;

void
mmu_init(void);

void
mmu_create_page_table(struct page_table* pt);

void
mmu_map_page(struct page_table* pt, uint64_t phys, uintptr_t virt, uint64_t flags);

void
mmu_map_page_l2(struct page_table* pt, uint64_t phys, uintptr_t virt, uint64_t flags);

void
mmu_map_page_l3(struct page_table* pt, uint64_t phys, uintptr_t virt, uint64_t flags);

void
mmu_unmap_page(struct page_table* pt, uintptr_t virt);

void
mmu_unmap_page_l2(struct page_table* pt, uintptr_t virt);

void
mmu_unmap_page_l3(struct page_table* pt, uintptr_t virt);

void
mmu_switch_page_table(struct page_table* pt);
