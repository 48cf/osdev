#include <kernel/acpi.h>
#include <kernel/alloc.h>
#include <kernel/cpu.h>
#include <kernel/intr.h>
#include <kernel/intrin.h>
#include <kernel/lapic.h>
#include <kernel/mm.h>
#include <kernel/mmu.h>
#include <kernel/print.h>
#include <kernel/serial.h>
#include <kernel/thread.h>
#include <kernel/time.h>
#include <kernel/timer.h>

#include <limine.h>

struct limine_framebuffer_request framebuffer_request = {
   .id = LIMINE_FRAMEBUFFER_REQUEST,
   .response = NULL,
};

struct limine_memmap_request memmap_request = {
   .id = LIMINE_MEMMAP_REQUEST,
   .response = NULL,
};

struct limine_hhdm_request hhdm_request = {
   .id = LIMINE_HHDM_REQUEST,
   .response = NULL,
};

struct limine_rsdp_request rsdp_request = {
   .id = LIMINE_RSDP_REQUEST,
   .response = NULL,
};

struct limine_kernel_address_request kernel_address_request = {
   .id = LIMINE_KERNEL_ADDRESS_REQUEST,
   .response = NULL,
};

static struct thread main_thread;
static struct thread test_thread;

static void
test_thread_func(void* arg)
{

   printf("test: hello world from test thread\n");

   while (true) {
      struct timespec start_ts = timespec_from_nsec(time_get_nanos());

      printf("test: current time is %lu.%09lu\n", start_ts.sec, start_ts.nsec);

      struct timer timer;
      struct timespec end_ts = timespec_add(start_ts, timespec_from_nsec(NSEC_PER_MSEC * 10));

      printf("test: setting timer to expire at %lu.%09lu\n", end_ts.sec, end_ts.nsec);

      timer_init(&timer);
      timer_arm(&timer, time_nanos_to_ticks(timespec_to_nsec(end_ts)));

      asm volatile("sti");
      asm volatile("hlt");

      struct timespec ts = timespec_from_nsec(time_get_nanos());

      printf("test: timer expired at %lu.%09lu\n", ts.sec, ts.nsec);
   }

   printf("test: switching back to main thread\n");
   thread_switch(&test_thread, &main_thread);
}

void
_start(void)
{
   cpu_init_early();
   serial_init(COM1_PORT);

   struct limine_hhdm_response* hhdm_response = hhdm_request.response;

   assert(hhdm_response != NULL);

   hhdm_offset = hhdm_response->offset;

   print_fb_init();
   acpi_init();
   time_init();
   lapic_init();
   mm_init_early();
   mmu_init();
   mmu_switch_page_table(&kernel_page_table);
   mm_init();

   thread_init(&main_thread);
   thread_init(&test_thread);
   thread_init_context(&test_thread, false, test_thread_func, NULL);
   thread_switch(&main_thread, &test_thread);

   printf("main: back to main thread\n");

   for (;;) {
      asm volatile("hlt");
   }
}
