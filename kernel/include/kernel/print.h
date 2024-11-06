#pragma once

#include <stdarg.h>
#include <stddef.h>

void
print_fb_init(void);

size_t
sprintf(char* buf, size_t count, const char* fmt, ...);

size_t
vsprintf(char* buf, size_t count, const char* fmt, va_list args);

void
printf(const char* fmt, ...);

void
vprintf(const char* fmt, va_list args);

void
panic(const char* fmt, ...);

void
vpanic(const char* fmt, va_list args);

#define assert(expr)                                                                               \
   do {                                                                                            \
      if (!(expr)) {                                                                               \
         panic("assertion failed at %s:%d: %s\n", __FILE__, __LINE__, #expr);                      \
      }                                                                                            \
   } while (0)
