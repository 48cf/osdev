#include <kernel/intrin.h>
#include <kernel/serial.h>

void
serial_init(uint16_t port)
{
   _outb(port + 3, 0x80);
   _outb(port + 0, 0x01);
   _outb(port + 1, 0x00);
   _outb(port + 3, 0x03);
   _outb(port + 2, 0xc7);
   _outb(port + 4, 0x0b);
}

void
serial_out(uint16_t port, char ch)
{
   while (!(_inb(port + 5) & (1 << 6))) {
      asm volatile("pause");
   }

   _outb(port, ch);
}
