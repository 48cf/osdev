#include <kernel/alloc.h>
#include <kernel/errno.h>
#include <kernel/intrin.h>
#include <kernel/mm.h>
#include <kernel/mmu.h>
#include <kernel/print.h>
#include <kernel/stdlib.h>
#include <kernel/thread.h>

extern void
_do_context_switch(struct thread_context* from, struct thread_context* to);

extern void
_enter_userspace(void* arg, void (*entry)(void*), uintptr_t rsp);

static uintptr_t
get_stack_top(struct page* stack_page)
{
   return mm_get_page_address(stack_page) + mm_get_page_size(stack_page) + hhdm_offset;
}

int
thread_init(struct thread* thread)
{
   int result = 0;

   struct page* stack_page = mm_alloc_page(4); // 1<<(12+4) = 64 KiB
   struct page* user_stack_page = mm_alloc_page(4);

   if (stack_page == NULL || user_stack_page == NULL) {
      result = -ENOMEM;
      goto out;
   }

   thread->kernel_stack = stack_page;
   thread->user_stack = user_stack_page;
   thread->context.fpu_state = NULL;

out:
   if (result != 0) {
      if (stack_page != NULL) {
         mm_free_page(stack_page);
      }

      if (user_stack_page != NULL) {
         mm_free_page(user_stack_page);
      }
   }

   return result;
}

int
thread_init_context(struct thread* thread, bool is_user, void (*entry)(void*), void* arg)
{
   int result = 0;

   memset(&thread->context, 0, sizeof(struct thread_context));

   if (is_user && thread->context.fpu_state == NULL) {
      void* fpu_state = alloc(cpu_fpu_context_size());

      if (fpu_state == NULL) {
         result = -ENOMEM;
         goto out;
      }

      thread->context.fpu_state = fpu_state;
   }

   uint64_t* stack = (uint64_t*)get_stack_top(thread->kernel_stack);

   *(--stack) = 0;

   if (is_user) {
      *(--stack) = SEL_USER_DS64 | 3;
      *(--stack) = get_stack_top(thread->user_stack);
      *(--stack) = 0x202; // IF + reserved bit
      *(--stack) = SEL_USER_CS64 | 3;
      *(--stack) = (uint64_t)entry;
      *(--stack) = (uint64_t)_enter_userspace;
   } else {
      *(--stack) = (uint64_t)entry;
   }

   thread->context.rsp = (uint64_t)stack;
   thread->context.rdi = (uint64_t)arg;

out:
   return result;
}

void
thread_free(struct thread* thread)
{
   if (thread->context.fpu_state != NULL) {
      free(thread->context.fpu_state);
   }

   thread->context.fpu_state = NULL;
}

void
thread_switch(struct thread* from, struct thread* to)
{
   _wrmsr(IA32_FS_BASE, to->fs_base);

   get_current_cpu()->tss.rsp0 = get_stack_top(to->kernel_stack);

   if (from->context.fpu_state != NULL) {
      cpu_save_fpu_context(from->context.fpu_state);
   }

   if (to->context.fpu_state != NULL) {
      cpu_restore_fpu_context(to->context.fpu_state);
   }

   _do_context_switch(&from->context, &to->context);
}
