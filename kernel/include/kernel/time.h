#pragma once

#include <stdint.h>

#define NSEC_PER_USEC ((uint64_t)1000)
#define NSEC_PER_MSEC ((uint64_t)NSEC_PER_USEC * 1000)
#define NSEC_PER_SEC ((uint64_t)NSEC_PER_MSEC * 1000)

typedef int64_t time_t;

struct timespec
{
   time_t sec;
   time_t nsec;
};

static inline struct timespec
timespec_from_nsec(uint64_t ns)
{
   return (struct timespec){
      .sec = ns / NSEC_PER_SEC,
      .nsec = ns % NSEC_PER_SEC,
   };
}

static inline struct timespec
timespec_add(struct timespec a, struct timespec b)
{
   struct timespec result;

   result.sec = a.sec + b.sec;
   result.nsec = a.nsec + b.nsec;

   while (result.nsec >= NSEC_PER_SEC) {
      result.sec++;
      result.nsec -= NSEC_PER_SEC;
   }

   return result;
}

static inline struct timespec
timespec_sub(struct timespec a, struct timespec b)
{
   struct timespec result;

   result.sec = a.sec - b.sec;
   result.nsec = a.nsec - b.nsec;

   while (result.nsec < 0) {
      result.sec--;
      result.nsec += NSEC_PER_SEC;
   }

   if (result.sec < 0) {
      result.sec = 0;
      result.nsec = 0;
   }

   return result;
}

static inline time_t
timespec_to_nsec(struct timespec ts)
{
   return (uint64_t)ts.sec * NSEC_PER_SEC + ts.nsec;
}

void
time_init(void);

uint64_t
time_get_ticks(void);

uint64_t
time_get_nanos(void);

uint64_t
time_nanos_to_ticks(uint64_t ns);
