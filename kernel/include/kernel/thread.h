#pragma once

#include <kernel/cpu.h>

#include <stdbool.h>
#include <stdint.h>

struct thread_context
{
   uint64_t rsi;
   uint64_t rdi;
   uint64_t rbx;
   uint64_t rsp;
   uint64_t rbp;
   uint64_t r12;
   uint64_t r13;
   uint64_t r14;
   uint64_t r15;
   void* fpu_state;
};

struct thread
{
   uint64_t id;
   uintptr_t fs_base;
   struct page* kernel_stack;
   struct page* user_stack;
   struct cpu* last_cpu;
   struct thread_context context;
};

int
thread_init(struct thread* thread);

int
thread_init_context(struct thread* thread, bool is_user, void (*entry)(void*), void* arg);

void
thread_free(struct thread* thread);

void
thread_switch(struct thread* from, struct thread* to);
