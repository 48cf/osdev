#pragma once

#include <stddef.h>
#include <stdint.h>

struct page
{
   struct page* next;

   uint32_t max_order : 5;
   uint32_t current_order : 5;
   uint32_t on_free_list : 1;
   uint32_t reserved : 21;
};

void
mm_init_early(void);

void
mm_init(void);

struct page*
mm_alloc_page(uint8_t order);

void
mm_free_page(struct page* page);

size_t
mm_get_page_size(struct page* page);

uintptr_t
mm_get_page_address(struct page* page);

struct page*
mm_get_page_from_address(uintptr_t address);
