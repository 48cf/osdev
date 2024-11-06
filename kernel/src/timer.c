#include <kernel/cpu.h>
#include <kernel/lapic.h>
#include <kernel/print.h>
#include <kernel/time.h>
#include <kernel/timer.h>

static void
enqueue_timer(struct timer* timer)
{
   struct timer* it;

   TAILQ_FOREACH(it, &timer->cpu->timer_queue, list_entry)
   {
      if (it->deadline > timer->deadline) {
         TAILQ_INSERT_BEFORE(it, timer, list_entry);
         return;
      }
   }

   TAILQ_INSERT_TAIL(&timer->cpu->timer_queue, timer, list_entry);

   __atomic_store_n(&timer->is_armed, true, __ATOMIC_RELEASE);
}

static void
dequeue_timer(struct timer* timer)
{
   if (!__atomic_load_n(&timer->is_armed, __ATOMIC_RELAXED)) {
      return;
   }

   __atomic_store_n(&timer->is_armed, false, __ATOMIC_RELEASE);

   TAILQ_REMOVE(&timer->cpu->timer_queue, timer, list_entry);
}

void
timer_init(struct timer* timer)
{
   timer->cpu = NULL;
   timer->deadline = 0;
   timer->is_armed = false;
}

void
timer_arm(struct timer* timer, uint64_t deadline)
{
   uint64_t ticks = time_get_ticks();

   if (__atomic_load_n(&timer->is_armed, __ATOMIC_RELAXED)) {
      dequeue_timer(timer);
   }

   struct cpu* cpu = pcb_current_get_cpu();
   struct timer* last = TAILQ_LAST(&cpu->timer_queue, timer_queue);

   timer->cpu = cpu;
   timer->deadline = deadline;

   enqueue_timer(timer);

   if (last == NULL || last->deadline < timer->deadline) {
      lapic_timer_arm_one_shot(timer->deadline, 32);
   }
}

void
timer_disarm(struct timer* timer)
{
   dequeue_timer(timer);

   struct timer* last = TAILQ_LAST(&timer->cpu->timer_queue, timer_queue);

   if (last != NULL) {
      lapic_timer_arm_one_shot(last->deadline, 32);
   } else {
      lapic_timer_disarm();
   }
}
