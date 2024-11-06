#pragma once

#include <kernel/queue.h>

#include <stdbool.h>
#include <stdint.h>

struct timer
{
   TAILQ_ENTRY(timer) list_entry;
   struct cpu* cpu;
   uint64_t deadline;
   bool is_armed;
};

void
timer_init(struct timer* timer);

void
timer_arm(struct timer* timer, uint64_t deadline);

void
timer_disarm(struct timer* timer);
