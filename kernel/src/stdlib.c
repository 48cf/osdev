#include <kernel/stdlib.h>

#include <stdint.h>

void*
memcpy(void* dest, const void* src, size_t count)
{
   uint8_t* dest_u8 = (uint8_t*)dest;
   const uint8_t* src_u8 = (const uint8_t*)src;

   for (size_t i = 0; i < count; ++i) {
      dest_u8[i] = src_u8[i];
   }

   return dest;
}

void*
memset(void* dest, int value, size_t count)
{
   uint8_t* dest_u8 = (uint8_t*)dest;

   for (size_t i = 0; i < count; ++i) {
      dest_u8[i] = (uint8_t)value;
   }

   return dest;
}

void*
memmove(void* dest, const void* src, size_t count)
{
   uint8_t* dest_u8 = (uint8_t*)dest;
   const uint8_t* src_u8 = (const uint8_t*)src;

   if (src > dest) {
      for (size_t i = 0; i < count; ++i) {
         dest_u8[i] = src_u8[i];
      }
   } else if (src < dest) {
      for (size_t i = count; i > 0; --i) {
         dest_u8[i - 1] = src_u8[i - 1];
      }
   }

   return dest;
}

int
memcmp(const void* buf1, const void* buf2, size_t count)
{
   const uint8_t* buf1_u8 = (const uint8_t*)buf1;
   const uint8_t* buf2_u8 = (const uint8_t*)buf2;

   for (size_t i = 0; i < count; ++i) {
      if (buf1_u8[i] != buf2_u8[i]) {
         return buf1_u8[i] < buf2_u8[i] ? -1 : 1;
      }
   }

   return 0;
}

size_t
strlen(const char* str)
{
   size_t len = 0;

   while (str[len] != '\0') {
      len++;
   }

   return len;
}

size_t
strnlen(const char* str, size_t max_len)
{
   size_t len = 0;

   while (len < max_len && str[len] != '\0') {
      len++;
   }

   return len;
}

int
strcmp(const char* str1, const char* str2)
{
   while (*str1 != '\0' && *str1 == *str2) {
      str1++;
      str2++;
   }

   if (*str1 != *str2) {
      return *(const unsigned char*)str1 < *(const unsigned char*)str2 ? -1 : 1;
   }

   return 0;
}

int
strncmp(const char* str1, const char* str2, size_t count)
{
   while (count != 0 && *str1 != '\0' && *str1 == *str2) {
      str1++;
      str2++;
      count--;
   }

   if (count == 0) {
      return 0;
   }

   return *(const unsigned char*)str1 < *(const unsigned char*)str2 ? -1 : 1;
}
