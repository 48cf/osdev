#pragma once

#include <stddef.h>

void*
memcpy(void* dest, const void* src, size_t count);

void*
memset(void* dest, int value, size_t count);

void*
memmove(void* dest, const void* src, size_t count);

int
memcmp(const void* buf1, const void* buf2, size_t count);

size_t
strlen(const char* str);

size_t
strnlen(const char* str, size_t max_len);

int
strcmp(const char* str1, const char* str2);

int
strncmp(const char* str1, const char* str2, size_t count);
