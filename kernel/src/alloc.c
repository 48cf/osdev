#include <kernel/alloc.h>
#include <kernel/mm.h>
#include <kernel/mmu.h>
#include <kernel/print.h>
#include <kernel/stdlib.h>
#include <kernel/utils.h>

static struct page* bump_page = NULL;
static uintptr_t bump_offset = 0;

void*
alloc(size_t size)
{
   size = align_up(size, alignof(max_align_t));

   if (bump_page == NULL || bump_offset + size > PAGE_SIZE) {
      bump_page = mm_alloc_page(0);

      if (bump_page == NULL) {
         return NULL;
      }

      bump_offset = 0;
   }

   uintptr_t address = mm_get_page_address(bump_page) + bump_offset;

   bump_offset += size;

   void* ptr = (void*)(address + hhdm_offset);

   memset(ptr, 0, size);

   return ptr;
}

void*
calloc(size_t nmemb, size_t size)
{
   void* ptr = alloc(nmemb * size);

   if (ptr != NULL) {
      memset(ptr, 0, nmemb * size);
   }

   return ptr;
}

void
free(void* ptr)
{
   // ...
}

void*
realloc(void* ptr, size_t size)
{
   if (size == 0) {
      free(ptr);
      return NULL;
   }

   void* new_ptr = alloc(size);

   if (ptr != NULL) {
      memcpy(new_ptr, ptr, size);
      free(ptr);
   }

   return new_ptr;
}
