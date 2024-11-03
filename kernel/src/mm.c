#include <kernel/mm.h>
#include <kernel/mmu.h>
#include <kernel/print.h>
#include <kernel/utils.h>

#include <limine.h>
#include <stddef.h>

#define BUDDY_COUNT 32

extern volatile struct limine_memmap_request memmap_request;

static struct page* pfndb;
static size_t pfndb_size;

static uintptr_t boot_alloc_start;
static uintptr_t boot_alloc_end;
static uintptr_t boot_alloc_l2_start;
static uintptr_t boot_alloc_l2_end;

static size_t boot_alloc_offset = 0;
static size_t boot_alloc_l2_offset = 0;

static struct page* free_pages[BUDDY_COUNT];
static size_t free_page_count[BUDDY_COUNT];

static const char* memmap_entry_type[] = {
   [LIMINE_MEMMAP_USABLE] = "usable",
   [LIMINE_MEMMAP_RESERVED] = "reserved",
   [LIMINE_MEMMAP_ACPI_RECLAIMABLE] = "acpi reclaimable",
   [LIMINE_MEMMAP_ACPI_NVS] = "acpi nvs",
   [LIMINE_MEMMAP_BAD_MEMORY] = "bad memory",
   [LIMINE_MEMMAP_BOOTLOADER_RECLAIMABLE] = "bootloader reclaimable",
   [LIMINE_MEMMAP_KERNEL_AND_MODULES] = "kernel and modules",
   [LIMINE_MEMMAP_FRAMEBUFFER] = "framebuffer",
};

static size_t
page_get_pfn(struct page* page)
{
   return page - pfndb;
}

static size_t
order_from_size(size_t size)
{
   assert(size >= PAGE_SIZE);

   return min(__builtin_ctz(size) - PAGE_SIZE_SHIFT, BUDDY_COUNT - 1);
}

static size_t
size_from_order(size_t order)
{
   return (size_t)1 << (order + PAGE_SIZE_SHIFT);
}

static void
free_page(struct page* page)
{
   const size_t order = page->current_order;

   page->next = free_pages[order];
   page->on_free_list = true;

   free_pages[order] = page;
   free_page_count[order]++;
}

static struct page*
alloc_page(size_t order)
{
   if (free_pages[order] == NULL) {
      return NULL;
   }

   struct page* page = free_pages[order];

   page->on_free_list = false;

   free_pages[order] = page->next;
   free_page_count[order]--;

   return page;
}

static void
free_region(uintptr_t base, size_t length)
{
   for (size_t offset = 0; offset < length; offset += PAGE_SIZE) {
      struct page* page = mm_get_page_from_address(base + offset);

      size_t order = min(__builtin_ctz(page_get_pfn(page)), BUDDY_COUNT - 1);

      while (offset + size_from_order(order) > length) {
         order--;
      }

      page->max_order = order;
      page->current_order = order;
      page->on_free_list = false;
   }

   for (size_t offset = 0; offset < length;) {
      struct page* page = mm_get_page_from_address(base + offset);

      page->on_free_list = true;
      offset += size_from_order(page->current_order);

      free_page(page);
   }
}

uintptr_t
allocate_l1_page()
{
   assert(boot_alloc_offset + PAGE_SIZE <= boot_alloc_end - boot_alloc_start);

   const uintptr_t phys = boot_alloc_start + boot_alloc_offset;

   boot_alloc_offset += PAGE_SIZE;

   return phys;
}

uintptr_t
allocate_l2_page()
{
   assert(boot_alloc_l2_offset + PAGE_SIZE_L2 <= boot_alloc_l2_end - boot_alloc_l2_start);

   const uintptr_t phys = boot_alloc_l2_start + boot_alloc_l2_offset;

   boot_alloc_l2_offset += PAGE_SIZE_L2;

   return phys;
}

void
mm_init_early()
{
   struct limine_memmap_response* memmap_response = memmap_request.response;
   struct limine_memmap_entry* biggest_entry = NULL;

   assert(memmap_response != NULL);

   uintptr_t highest_addr = 0;

   for (size_t i = 0; i < memmap_response->entry_count; ++i) {
      struct limine_memmap_entry* entry = memmap_response->entries[i];

      printf("mm: %p-%p: %s\n",
             entry->base,
             entry->base + entry->length,
             memmap_entry_type[entry->type]);

      if (entry->type != LIMINE_MEMMAP_USABLE) {
         continue;
      }

      if (biggest_entry == NULL || entry->length > biggest_entry->length) {
         biggest_entry = entry;
      }

      if (entry->base + entry->length > highest_addr) {
         highest_addr = entry->base + entry->length;
      }
   }

   assert(biggest_entry != NULL);

   pfndb_size = (highest_addr / PAGE_SIZE) * sizeof(struct page);

   printf("mm: physical page frame database size: %zu KiB\n", pfndb_size / 1024);
   printf("mm: highest usable physical address: %p\n", highest_addr);
   printf("mm: biggest usable memory region: %p-%p\n",
          biggest_entry->base,
          biggest_entry->base + biggest_entry->length);

   if (pfndb_size >= PAGE_SIZE_L2) {
      boot_alloc_l2_end = align_down(biggest_entry->base + biggest_entry->length, PAGE_SIZE_L2);
      boot_alloc_l2_start = boot_alloc_l2_end - align_up(pfndb_size, PAGE_SIZE_L2);

      boot_alloc_start = biggest_entry->base;
      boot_alloc_end = boot_alloc_l2_start;

      if (boot_alloc_l2_end != biggest_entry->base + biggest_entry->length) {
         printf("mm: wasting %zu KiB of memory due to alignment\n",
                (biggest_entry->base + biggest_entry->length - boot_alloc_l2_end) / 1024);
      }
   } else {
      boot_alloc_start = biggest_entry->base;
      boot_alloc_end = biggest_entry->base + biggest_entry->length;
   }
}

void
mm_init()
{
   pfndb = (struct page*)pfndb_offset;

   free_region(boot_alloc_start + boot_alloc_offset,
               boot_alloc_end - boot_alloc_start - boot_alloc_offset);

   struct limine_memmap_response* memmap_response = memmap_request.response;

   assert(memmap_response != NULL);

   for (size_t i = 0; i < memmap_response->entry_count; ++i) {
      struct limine_memmap_entry* entry = memmap_response->entries[i];

      if (entry->type != LIMINE_MEMMAP_USABLE || entry->base == boot_alloc_start) {
         continue;
      }

      free_region(entry->base, entry->length);
   }

   size_t total_memory_size = 0;

   for (size_t i = 0; i < BUDDY_COUNT; ++i) {
      total_memory_size += free_page_count[i] * size_from_order(i);
   }

   printf("mm: %zu KiB of memory available\n", total_memory_size / 1024);
}

struct page*
mm_alloc_page(uint8_t order)
{
   assert(order < BUDDY_COUNT);

   const uint8_t desired_order = order;

   while (free_pages[order] == NULL) {
      if (order == BUDDY_COUNT - 1) {
         return NULL;
      }

      order++;
   }

   for (; order != desired_order; --order) {
      struct page* page = alloc_page(order);
      struct page* buddy = page + (1 << order) / 2;

      assert(page->current_order == order);
      assert(buddy->current_order == order - 1);

      page->current_order -= 1;

      free_page(page);
      free_page(buddy);
   }

   struct page* page = alloc_page(order);

   assert(page->current_order == desired_order);

   return page;
}

void
mm_free_page(struct page* page)
{
   while (page->current_order != page->max_order) {
      struct page* buddy =
         mm_get_page_from_address(mm_get_page_address(page) ^ size_from_order(page->current_order));

      if (buddy->current_order != page->current_order || !buddy->on_free_list) {
         break;
      }

      if (free_pages[page->current_order] == buddy) {
         free_pages[page->current_order] = buddy->next;
      } else {
         struct page* it = free_pages[page->current_order];

         while (it->next != buddy) {
            it = it->next;
         }

         it->next = buddy->next;
      }

      if (buddy < page) {
         page = buddy;
      }

      page->current_order += 1;
   }

   free_page(page);
}

size_t
mm_get_page_size(struct page* page)
{
   return size_from_order(page->current_order);
}

uintptr_t
mm_get_page_address(struct page* page)
{
   assert(page != NULL);

   return page_get_pfn(page) << PAGE_SIZE_SHIFT;
}

struct page*
mm_get_page_from_address(uintptr_t address)
{
   assert(is_aligned(address, PAGE_SIZE));

   return &pfndb[address >> PAGE_SIZE_SHIFT];
}
