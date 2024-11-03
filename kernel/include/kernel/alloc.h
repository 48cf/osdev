#pragma once

#include <stddef.h>

void*
alloc(size_t size);

void*
calloc(size_t nmemb, size_t size);

void
free(void* ptr);

void*
realloc(void* ptr, size_t size);
