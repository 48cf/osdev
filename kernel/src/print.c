#define NANOPRINTF_IMPLEMENTATION
#define NANOPRINTF_USE_FIELD_WIDTH_FORMAT_SPECIFIERS 1
#define NANOPRINTF_USE_PRECISION_FORMAT_SPECIFIERS 1
#define NANOPRINTF_USE_FLOAT_FORMAT_SPECIFIERS 0
#define NANOPRINTF_USE_LARGE_FORMAT_SPECIFIERS 1
#define NANOPRINTF_USE_BINARY_FORMAT_SPECIFIERS 1
#define NANOPRINTF_USE_WRITEBACK_FORMAT_SPECIFIERS 0

#include <kernel/cpu.h>
#include <kernel/print.h>
#include <kernel/serial.h>

#include <nanoprintf.h>

static void
_putc(int ch, void* context)
{
   serial_out(COM1_PORT, ch);
}

size_t
sprintf(char* buf, size_t count, const char* fmt, ...)
{
   va_list args;

   va_start(args, fmt);
   size_t result = vsprintf(buf, count, fmt, args);
   va_end(args);

   return result;
}

size_t
vsprintf(char* buf, size_t count, const char* fmt, va_list args)
{
   return npf_vsnprintf(buf, count, fmt, args);
}

void
printf(const char* fmt, ...)
{
   va_list args;

   va_start(args, fmt);
   npf_vpprintf(_putc, NULL, fmt, args);
   va_end(args);
}

void
vprintf(const char* fmt, va_list args)
{
   npf_vpprintf(_putc, NULL, fmt, args);
}

void
panic(const char* fmt, ...)
{
   va_list args;

   va_start(args, fmt);
   vpanic(fmt, args);
   va_end(args);
}

void
vpanic(const char* fmt, va_list args)
{
   asm volatile("cli");

   struct cpu* cpu = get_current_cpu();

   vprintf(fmt, args);
   printf("panic on cpu %zu, oops...\n", cpu->id);

   for (;;) {
      asm volatile("hlt");
   }
}
