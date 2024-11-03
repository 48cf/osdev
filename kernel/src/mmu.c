
#include <kernel/intrin.h>
#include <kernel/mm.h>
#include <kernel/mmu.h>
#include <kernel/print.h>
#include <kernel/utils.h>

#include <limine.h>

#define PTE_P (1ull << 0)
#define PTE_RW (1ull << 1)
#define PTE_US (1ull << 2)
#define PTE_WT (1ull << 3)
#define PTE_CD (1ull << 4)
#define PTE_PS (1ull << 7)
#define PTE_XD (1ull << 63)

// The mask for the physical address in a page table entry
#define PTE_PHYS_MASK 0x000ffffffffff000ull

// Utility macros for extracting the physical address and flags from a page table entry
#define PTE_ADDR(pte) ((pte) & PTE_PHYS_MASK)
#define PTE_FLAGS(pte) ((pte) & ~PTE_PHYS_MASK)

extern volatile struct limine_memmap_request memmap_request;
extern volatile struct limine_hhdm_request hhdm_request;
extern volatile struct limine_kernel_address_request kernel_address_request;

extern char __text_start[];
extern char __text_end[];
extern char __data_start[];
extern char __data_end[];
extern char __rodata_start[];
extern char __rodata_end[];

struct page_table kernel_page_table;

uintptr_t hhdm_offset;
uintptr_t pfndb_offset;

extern uintptr_t
allocate_l1_page();

extern uintptr_t
allocate_l2_page();

enum pte_level
{
   LEVEL_PML4, // PTE at this level points to a PDPT
   LEVEL_PDPT, // PTE at this level either points to a PD or maps a 1 GiB page
   LEVEL_PD,   // PTE at this level either points to a PT or maps a 2 MiB page
   LEVEL_PT,   // PTE at this level maps a 4 KiB page
   LEVEL_COUNT,
};

static void
get_pte_address(uint64_t* entries,
                uintptr_t virt_addr,
                uint8_t start_level,
                uint64_t** out_pte,
                uint8_t* out_level)
{
   const size_t indices[4] = {
      (virt_addr >> 39) & 0x1ff, // PML4 entry index
      (virt_addr >> 30) & 0x1ff, // PDPT entry index
      (virt_addr >> 21) & 0x1ff, // PD entry index
      (virt_addr >> 12) & 0x1ff, // PT entry index
   };

   *out_pte = NULL;

   for (uint8_t level = start_level; level < LEVEL_COUNT; ++level) {
      uint64_t* pte = entries + indices[level];

      if (level == LEVEL_PT || PTE_FLAGS(*pte) & PTE_PS || !(PTE_FLAGS(*pte) & PTE_P)) {
         *out_pte = pte;
         *out_level = level;

         return;
      }

      entries = (uint64_t*)(PTE_ADDR(*pte) + hhdm_offset);
   }
}

static uint64_t
mmu_flags_to_pte_flags(uint64_t flags)
{
   uint64_t pte_flags = 0;

   if (flags & MMU_PRESENT) {
      pte_flags |= PTE_P;
   }

   if (flags & MMU_WRITE) {
      pte_flags |= PTE_RW;
   }

   if (flags & MMU_USER) {
      pte_flags |= PTE_US;
   }

   if (flags & MMU_WRITE_THROUGH) {
      pte_flags |= PTE_WT;
   }

   if (flags & MMU_CACHE_DISABLE) {
      pte_flags |= PTE_CD;
   }

   if (flags & MMU_LARGE_PAGE) {
      pte_flags |= PTE_PS;
   }

   if (!(flags & MMU_EXECUTE)) {
      pte_flags |= PTE_XD;
   }

   return pte_flags;
}

static const size_t level_page_sizes[] = {
   [LEVEL_PDPT] = PAGE_SIZE_L3,
   [LEVEL_PD] = PAGE_SIZE_L2,
   [LEVEL_PT] = PAGE_SIZE,
};

static void
map_page_at_level(struct page_table* pt,
                  uint64_t phys,
                  uintptr_t virt,
                  uint64_t flags,
                  uint8_t level)
{

   assert(level > LEVEL_PML4 && level < LEVEL_COUNT);
   assert(flags & MMU_LARGE_PAGE && level < LEVEL_PT || !(flags & MMU_LARGE_PAGE));
   assert(is_aligned(phys, level_page_sizes[level]));
   assert(is_aligned(virt, level_page_sizes[level]));

   uint64_t* pte;
   uint8_t current_level;

   do {
      get_pte_address(kernel_page_table.entries, virt, LEVEL_PML4, &pte, &current_level);
      assert(pte != NULL);

      if (PTE_FLAGS(*pte) & PTE_P && current_level < level) {
         const uint64_t old_phys = PTE_ADDR(*pte);
         const uintptr_t new_table_phys = allocate_l1_page();

         uint64_t* new_table = (uint64_t*)(new_table_phys + hhdm_offset);
         uint64_t old_flags = PTE_FLAGS(*pte);

         if (current_level == LEVEL_PD) {
            old_flags &= ~PTE_PS;
         }

         for (size_t i = 0; i < 512; ++i) {
            new_table[i] = (old_phys + i * level_page_sizes[current_level + 1]) | old_flags;
         }

         *pte = new_table_phys | PTE_P | PTE_RW | PTE_US;
      } else if (current_level != level) {
         const uintptr_t new_table_phys = allocate_l1_page();

         uint64_t* new_table = (uint64_t*)(new_table_phys + hhdm_offset);

         for (size_t i = 0; i < 512; ++i) {
            new_table[i] = 0;
         }

         *pte = new_table_phys | PTE_P | PTE_RW | PTE_US;
      } else {
         *pte = phys | mmu_flags_to_pte_flags(flags);
      }
   } while (current_level != level);
}

static void
unmap_page_at_level(struct page_table* pt, uintptr_t virt, uint8_t level)
{
   assert(level > LEVEL_PML4 && level < LEVEL_COUNT);
   assert(is_aligned(virt, level_page_sizes[level]));

   uint64_t* pte;
   uint8_t current_level;

   get_pte_address(kernel_page_table.entries, virt, LEVEL_PML4, &pte, &current_level);

   assert(pte != NULL);
   assert(current_level == level);

   *pte &= ~PTE_P;
}

static void
map_kernel_section(struct limine_kernel_address_response* kernel_addr_response,
                   char* start,
                   char* end,
                   uint64_t flags)
{
   const uintptr_t start_page = align_down((uintptr_t)start, PAGE_SIZE);
   const uintptr_t end_page = align_up((uintptr_t)end, PAGE_SIZE);

   for (uintptr_t addr = start_page; addr < end_page; addr += PAGE_SIZE) {
      mmu_map_page(&kernel_page_table,
                   addr - kernel_addr_response->virtual_base + kernel_addr_response->physical_base,
                   addr,
                   flags);
   }
}

void
mmu_init()
{
   struct limine_memmap_response* memmap_response = memmap_request.response;
   struct limine_hhdm_response* hhdm_response = hhdm_request.response;
   struct limine_kernel_address_response* kernel_address_response = kernel_address_request.response;

   assert(memmap_response != NULL);
   assert(hhdm_response != NULL);
   assert(kernel_address_response != NULL);

   hhdm_offset = hhdm_response->offset;
   pfndb_offset = 0xffff900000000000;

   uintptr_t highest_addr = 0;

   for (size_t i = 0; i < memmap_response->entry_count; ++i) {
      struct limine_memmap_entry* entry = memmap_response->entries[i];

      if (entry->type != LIMINE_MEMMAP_USABLE) {
         continue;
      }

      if (entry->base + entry->length > highest_addr) {
         highest_addr = entry->base + entry->length;
      }
   }

   const size_t page_count = highest_addr / PAGE_SIZE;
   const size_t pfndb_size = page_count * sizeof(struct page);

   kernel_page_table.cr3 = allocate_l1_page();
   kernel_page_table.entries = (uint64_t*)(kernel_page_table.cr3 + hhdm_response->offset);

   for (size_t i = 0; i < 512; ++i) {
      kernel_page_table.entries[i] = 0;
   }

   for (size_t i = 0; i < 512; ++i) {
      const uint64_t phys = i * PAGE_SIZE_L3;

      mmu_map_page_l3(&kernel_page_table,
                      phys,
                      phys + hhdm_response->offset,
                      MMU_PRESENT | MMU_READ | MMU_WRITE);
   }

   if (pfndb_size >= PAGE_SIZE_L2) {
      const size_t l2_page_count = align_up(pfndb_size, PAGE_SIZE_L2) / PAGE_SIZE_L2;

      for (size_t i = 0; i < l2_page_count; ++i) {
         const uintptr_t phys = allocate_l2_page();

         mmu_map_page_l2(&kernel_page_table,
                         phys,
                         pfndb_offset + i * PAGE_SIZE_L2,
                         MMU_PRESENT | MMU_READ | MMU_WRITE);
      }
   } else {
      const size_t page_count = align_up(pfndb_size, PAGE_SIZE) / PAGE_SIZE;

      for (size_t i = 0; i < page_count; ++i) {
         const uintptr_t phys = allocate_l1_page();

         mmu_map_page(&kernel_page_table,
                      phys,
                      pfndb_offset + i * PAGE_SIZE,
                      MMU_PRESENT | MMU_READ | MMU_WRITE);
      }
   }

   map_kernel_section(
      kernel_address_response, __text_start, __text_end, MMU_PRESENT | MMU_READ | MMU_EXECUTE);

   map_kernel_section(
      kernel_address_response, __data_start, __data_end, MMU_PRESENT | MMU_READ | MMU_WRITE);

   map_kernel_section(
      kernel_address_response, __rodata_start, __rodata_end, MMU_PRESENT | MMU_READ);
}

void
mmu_create_page_table(struct page_table* pt)
{
}

void
mmu_map_page(struct page_table* pt, uint64_t phys, uintptr_t virt, uint64_t flags)
{
   map_page_at_level(pt, phys, virt, flags, LEVEL_PT);
}

void
mmu_map_page_l2(struct page_table* pt, uint64_t phys, uintptr_t virt, uint64_t flags)
{
   map_page_at_level(pt, phys, virt, flags | MMU_LARGE_PAGE, LEVEL_PD);
}

void
mmu_map_page_l3(struct page_table* pt, uint64_t phys, uintptr_t virt, uint64_t flags)
{
   map_page_at_level(pt, phys, virt, flags | MMU_LARGE_PAGE, LEVEL_PDPT);
}

void
mmu_unmap_page(struct page_table* pt, uintptr_t virt)
{
   unmap_page_at_level(pt, virt, LEVEL_PT);
   _invlpg(virt);
}

void
mmu_unmap_page_l2(struct page_table* pt, uintptr_t virt)
{
   unmap_page_at_level(pt, virt, LEVEL_PD);

   for (size_t i = 0; i < 512; ++i) {
      _invlpg(virt + i * PAGE_SIZE);
   }
}

void
mmu_unmap_page_l3(struct page_table* pt, uintptr_t virt)
{
   unmap_page_at_level(pt, virt, LEVEL_PDPT);
   _write_cr3(pt->cr3); // quicker than invalidating every page in the 1 GiB region
}

void
mmu_switch_page_table(struct page_table* pt)
{
   _write_cr3(pt->cr3);
}
