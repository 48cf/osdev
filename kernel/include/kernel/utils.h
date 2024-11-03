#pragma once

#define is_aligned(x, a) (((x) & ((a) - 1)) == 0)

#define align_down(x, a) ((x) & ~((a) - 1))

#define align_up(x, a)                                                                             \
   ({                                                                                              \
      __auto_type _x = (x);                                                                        \
      __auto_type _a = (a);                                                                        \
      align_down(_x + _a - 1, _a);                                                                 \
   })

#define min(a, b)                                                                                  \
   ({                                                                                              \
      __auto_type _a = (a);                                                                        \
      __auto_type _b = (b);                                                                        \
      _a < _b ? _a : _b;                                                                           \
   })

#define max(a, b)                                                                                  \
   ({                                                                                              \
      __auto_type _a = (a);                                                                        \
      __auto_type _b = (b);                                                                        \
      _a > _b ? _a : _b;                                                                           \
   })
