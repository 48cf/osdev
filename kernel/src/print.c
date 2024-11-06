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

#include <backends/fb.h>
#include <flanterm.h>
#include <limine.h>
#include <nanoprintf.h>

extern volatile struct limine_framebuffer_request framebuffer_request;

static uint8_t font_data[] = {
#embed "font.bin"
};

static uint32_t ansi_colors[] = {
   0x000000, // black
   0x800000, // red
   0x008000, // green
   0x808000, // yellow
   0x000080, // blue
   0x800080, // magenta
   0x008080, // cyan
   0xc0c0c0, // white
};

static uint32_t ansi_colors_bright[] = {
   0x808080, // gray
   0xff0000, // bright red
   0x00ff00, // bright green
   0xffff00, // bright yellow
   0x0000ff, // bright blue
   0xff00ff, // bright magenta
   0x00ffff, // bright cyan
   0xffffff, // bright white
};

static void
_putc(int ch, void* context)
{
   serial_out(COM1_PORT, ch);
}

static struct flanterm_context* flanterm_ctx;

static void
_putc_flanterm(int ch, void* context)
{
   char c = ch;

   flanterm_write(flanterm_ctx, &c, 1);
   serial_out(COM1_PORT, c);
}

static void (*putc_fn)(int, void*) = _putc;

void
print_fb_init(void)
{
   struct limine_framebuffer_response* framebuffer_response = framebuffer_request.response;

   if (framebuffer_response == NULL || framebuffer_response->framebuffer_count == 0) {
      return;
   }

   struct limine_framebuffer* fb = framebuffer_response->framebuffers[0];

   putc_fn = _putc_flanterm;
   flanterm_ctx = flanterm_fb_init(NULL,
                                   NULL,
                                   fb->address,
                                   fb->width,
                                   fb->height,
                                   fb->pitch,
                                   fb->red_mask_size,
                                   fb->red_mask_shift,
                                   fb->green_mask_size,
                                   fb->green_mask_shift,
                                   fb->blue_mask_size,
                                   fb->blue_mask_shift,
                                   NULL,
                                   ansi_colors,
                                   ansi_colors_bright,
                                   &ansi_colors[0],
                                   &ansi_colors[7],
                                   &ansi_colors_bright[0],
                                   &ansi_colors_bright[7],
                                   font_data,
                                   8,
                                   16,
                                   2,
                                   1,
                                   1,
                                   0);
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
   npf_vpprintf(putc_fn, NULL, fmt, args);
   va_end(args);
}

void
vprintf(const char* fmt, va_list args)
{
   npf_vpprintf(putc_fn, NULL, fmt, args);
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

   vprintf(fmt, args);
   printf("panic on cpu %zu, oops...\n", pcb_current_get_cpu()->id);

   for (;;) {
      asm volatile("hlt");
   }
}
