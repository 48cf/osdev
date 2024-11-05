#include <kernel/acpi.h>
#include <kernel/intrin.h>
#include <kernel/mmu.h>
#include <kernel/print.h>
#include <kernel/time.h>

#define CALIBRATION_RUNS 10
#define ACPI_TIMER_FREQUENCY 3579545
#define USEC_PER_SEC 1000000

static uint64_t
pm_timer_ticks_to_us(uint64_t ticks)
{
   return (ticks * USEC_PER_SEC) / ACPI_TIMER_FREQUENCY;
}

static uint32_t
read_pm_timer(struct acpi_fadt* fadt)
{
   // could use uacpi_read_gas for this but it seems overly complicated
   // for what essentially could just be a single inl/mminl so we'll just do it manually
   if (fadt->x_pm_tmr_blk.address_space_id == UACPI_ADDRESS_SPACE_SYSTEM_IO) {
      return _inl((uint16_t)fadt->x_pm_tmr_blk.address);
   } else {
      return _mminl(fadt->x_pm_tmr_blk.address + hhdm_offset);
   }
}

static uint64_t
pm_timer_wait(struct acpi_fadt* fadt, uint64_t us)
{
retry:
   uint64_t tsc = _rdtsc();
   uint64_t start = read_pm_timer(fadt);

   while (true) {
      uint64_t end = read_pm_timer(fadt);

      // if the timer rolls over we can just try again
      // this happens roughly every couple seconds with 24-bit timers
      if (end < start) {
         goto retry;
      }

      if (pm_timer_ticks_to_us(end - start) >= us) {
         return _rdtsc() - tsc;
      }
   }
}

static void
calibrate_tsc(void)
{
   struct acpi_fadt* fadt;

   assert(uacpi_table_fadt(&fadt) == UACPI_STATUS_OK);

   bool use_pm_timer = true;

   if (fadt == NULL || fadt->pm_tmr_len != 4) {
      use_pm_timer = false;
   }

   if (use_pm_timer) {
      printf("time: pm timer resolution: %llu bits\n", fadt->flags & ACPI_TMR_VAL_EXT ? 32 : 24);

      assert(fadt->x_pm_tmr_blk.address_space_id == UACPI_ADDRESS_SPACE_SYSTEM_IO ||
             fadt->x_pm_tmr_blk.address_space_id == UACPI_ADDRESS_SPACE_SYSTEM_MEMORY);

   retry:
      uint64_t times[CALIBRATION_RUNS];
      uint64_t lowest_tsc = UINT64_MAX;

      for (size_t i = 0; i < CALIBRATION_RUNS; ++i) {
         uint64_t tsc = pm_timer_wait(fadt, USEC_PER_SEC / 1000);

         printf("time: tsc: %llu ticks per 1ms\n", tsc);

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
         } else {
            printf("time: %llu is an outlier (%llu%%)\n", times[i], deviaton);
         }
      }

      if (mean_count < 5) {
         printf("time: calibration failed, retrying\n");
         goto retry;
      }

      mean /= mean_count;
      mean /= 1000; // ms -> us

      uint64_t last_tsc = _rdtsc();
      uint64_t last_pm = read_pm_timer(fadt);

      while (true) {
         if (_rdtsc() - last_tsc >= mean * USEC_PER_SEC) {
            uint64_t pm = read_pm_timer(fadt);

            if (pm < last_pm) {
               printf("time: pm timer rolled over\n");

               last_pm -= (fadt->flags & ACPI_TMR_VAL_EXT) ? (1ull << 32) : (1ull << 24);
            }

            printf("tsc: %llu ticks, pm: %lluus\n",
                   _rdtsc() - last_tsc,
                   pm_timer_ticks_to_us(pm - last_pm));

            last_tsc = _rdtsc();
            last_pm = pm;
         }
      }
   } else {
      struct uacpi_table hpet_table;

      assert(acpi_get_table(ACPI_HPET_SIGNATURE, 0, &hpet_table));
      assert(hpet_table.virt_addr != 0);

      assert(!"hpet calibration not implemented");
   }

   assert(false);
}

void
time_init(void)
{
   assert(cpuid_is_extended_leaf_supported(0x80000007));

   struct cpuid cpuid;

   _cpuid(0x80000007, 0, &cpuid);

   if (!(cpuid.edx & (1 << 8))) {
      panic("time: invariant tsc not supported\n");
   }

   calibrate_tsc();
}

uint64_t
time_get_ticks(void)
{
   return 0;
}

struct timespec
time_get_time(void)
{
   return (struct timespec){ 0, 0 };
}
