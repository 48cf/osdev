#include <kernel/acpi.h>
#include <kernel/cpu.h>
#include <kernel/intrin.h>
#include <kernel/mmu.h>
#include <kernel/print.h>
#include <kernel/time.h>
#include <kernel/utils.h>

#include <uacpi/io.h>

#define CALIBRATION_RUNS 10
#define ACPI_TIMER_FREQUENCY 3579545 // 3.579545 MHz

static uint64_t
acpi_timer_ticks_to_us(uint64_t ticks)
{
   return (ticks * NSEC_PER_SEC) / ACPI_TIMER_FREQUENCY / 1000;
}

static uint64_t
acpi_timer_wait(struct acpi_fadt* fadt, uint64_t us)
{
   uint64_t start;
   uint64_t end;

   uacpi_gas_read(&fadt->x_pm_tmr_blk, &start);

   uint64_t start_tsc = _rdtsc();

   while (true) {
      uacpi_gas_read(&fadt->x_pm_tmr_blk, &end);

      uint64_t tsc = _rdtsc();

      // if the timer rolls over we can just try again
      // this happens roughly every couple seconds with 24-bit timers
      if (end < start) {
         start += (uint64_t)1 << (fadt->flags & ACPI_TMR_VAL_EXT ? 32 : 24);
      }

      if (acpi_timer_ticks_to_us(end - start) >= us) {
         return tsc - start_tsc;
      }
   }
}

static void
calibrate_tsc(struct cpu* cpu)
{
   struct acpi_fadt* fadt;

   assert(uacpi_table_fadt(&fadt) == UACPI_STATUS_OK);

   bool use_pm_timer = true;

   if (fadt == NULL || fadt->pm_tmr_len != 4) {
      use_pm_timer = false;
   }

   if (use_pm_timer) {
      printf("time: acpi timer resolution: %llu bits\n", fadt->flags & ACPI_TMR_VAL_EXT ? 32 : 24);

      assert(fadt->x_pm_tmr_blk.address_space_id == UACPI_ADDRESS_SPACE_SYSTEM_IO ||
             fadt->x_pm_tmr_blk.address_space_id == UACPI_ADDRESS_SPACE_SYSTEM_MEMORY);

   retry:
      uint64_t times[CALIBRATION_RUNS];
      uint64_t lowest_tsc = UINT64_MAX;

      for (size_t i = 0; i < CALIBRATION_RUNS; ++i) {
         uint64_t tsc = acpi_timer_wait(fadt, (NSEC_PER_SEC / 1000 / 1000) * 25) / 25;

         times[i] = tsc;

         if (tsc < lowest_tsc) {
            lowest_tsc = tsc;
         }
      }

      uint64_t mean = 0;
      size_t mean_count = 0;

      for (size_t i = 0; i < CALIBRATION_RUNS; ++i) {
         uint64_t deviaton = times[i] * 100 / lowest_tsc;

         if (deviaton >= 95 && deviaton <= 105) {
            mean += times[i];
            mean_count++;
         }
      }

      if (mean_count < 5) {
         goto retry;
      }

      mean /= mean_count;

      cpu->tsc_frequency = mean * 1000;
      cpu->tsc_ratio_p = sizeof(uint64_t) * 8 - 1 - __builtin_clzll(cpu->tsc_frequency);
      cpu->tsc_ratio_n = ((uint64_t)1000000000 << cpu->tsc_ratio_p) / cpu->tsc_frequency;
   } else {
      struct uacpi_table hpet_table;

      assert(acpi_get_table(ACPI_HPET_SIGNATURE, 0, &hpet_table));
      assert(hpet_table.virt_addr != 0);

      panic("time: hpet calibration not yet implemented\n");
   }
}

void
time_init(void)
{
   struct cpu* cpu = pcb_current_get_cpu();

   assert(cpuid_is_extended_leaf_supported(0x80000007));

   struct cpuid cpuid;

   _cpuid(0x80000007, 0, &cpuid);

   if (!(cpuid.edx & (1 << 8))) {
      panic("time: invariant tsc not supported\n");
   }

   calibrate_tsc(cpu);
}

uint64_t
time_get_ticks(void)
{
   return _rdtsc();
}

uint64_t
time_get_nanoseconds(void)
{
   struct cpu* cpu = pcb_current_get_cpu();

   return ((unsigned __int128)time_get_ticks() * cpu->tsc_ratio_n) >> cpu->tsc_ratio_p;
}

struct timespec
time_get_time(void)
{
   uint64_t nanos = time_get_nanoseconds();

   return (struct timespec){
      .sec = nanos / NSEC_PER_SEC,
      .nsec = nanos % NSEC_PER_SEC,
   };
}
