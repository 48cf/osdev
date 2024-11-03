.section .note.GNU-stack,"",%progbits
.section .text

.global _fxsave
.type _fxsave,"function"
_fxsave:
   fxsave (%rdi)
   ret
.size _fxsave, .-_fxsave

.global _fxrstor
.type _fxrstor,"function"
_fxrstor:
   fxrstor (%rdi)
   ret
.size _fxrstor, .-_fxrstor

.global _xsave
.type _xsave,"function"
_xsave:
   xsave (%rdi)
   ret
.size _xsave, .-_xsave

.global _xrstor
.type _xrstor,"function"
_xrstor:
   xrstor (%rdi)
   ret
.size _xrstor, .-_xrstor

.global _lgdt
.type _lgdt,"function"
_lgdt:
   lgdt (%rdi)
   ret
.size _lgdt, .-_lgdt

.global _lidt
.type _lidt,"function"
_lidt:
   lidt (%rdi)
   ret
.size _lidt, .-_lidt

.global _ltr
.type _ltr,"function"
_ltr:
   ltr %di
   ret
.size _ltr, .-_ltr

.global _reload_segments
.type _reload_segments,"function"
_reload_segments:
   swapgs
   mov %si, %ds
   mov %si, %es
   mov %si, %fs
   mov %si, %gs
   mov %si, %ss
   pushq $8
   pushq $1f
   lretq
1:
   swapgs
   ret
.size _reload_segments, .-_reload_segments
