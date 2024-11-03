.section .note.GNU-stack,"",%progbits
.section .text

.global _read_cr0
.type _read_cr0,"function"
_read_cr0:
   mov %cr0, %rax
   ret
.size _read_cr0, .-_read_cr0

.global _read_cr2
.type _read_cr2,"function"
_read_cr2:
   mov %cr2, %rax
   ret
.size _read_cr2, .-_read_cr2

.global _read_cr3
.type _read_cr3,"function"
_read_cr3:
   mov %cr3, %rax
   ret
.size _read_cr3, .-_read_cr3

.global _read_cr4
.type _read_cr4,"function"
_read_cr4:
   mov %cr4, %rax
   ret
.size _read_cr4, .-_read_cr4

.global _read_cr8
.type _read_cr8,"function"
_read_cr8:
   mov %cr8, %rax
   ret
.size _read_cr8, .-_read_cr8

.global _write_cr0
.type _write_cr0,"function"
_write_cr0:
   mov %rdi, %cr0
   ret
.size _write_cr0, .-_write_cr0

.global _write_cr3
.type _write_cr3,"function"
_write_cr3:
   mov %rdi, %cr3
   ret
.size _write_cr3, .-_write_cr3

.global _write_cr4
.type _write_cr4,"function"
_write_cr4:
   mov %rdi, %cr4
   ret
.size _write_cr4, .-_write_cr4

.global _write_cr8
.type _write_cr8,"function"
_write_cr8:
   mov %rdi, %cr8
   ret
.size _write_cr8, .-_write_cr8

.global _outb
.type _outb,"function"
_outb:
   mov %di, %dx
   mov %sil, %al
   out %al, %dx
   ret
.size _outb, .-_outb

.global _outw
.type _outw,"function"
_outw:
   mov %di, %dx
   mov %si, %ax
   out %ax, %dx
   ret
.size _outw, .-_outw

.global _outl
.type _outl,"function"
_outl:
   mov %di, %dx
   mov %esi, %eax
   out %eax, %dx
   ret
.size _outl, .-_outl

.global _inb
.type _inb,"function"
_inb:
   mov %di, %dx
   in %dx, %al
   ret
.size _inb, .-_inb

.global _inw
.type _inw,"function"
_inw:
   mov %di, %dx
   in %dx, %ax
   ret
.size _inw, .-_inw

.global _inl
.type _inl,"function"
_inl:
   mov %di, %dx
   in %dx, %eax
   ret
.size _inl, .-_inl

.global _mmoutb
.type _mmoutb,"function"
_mmoutb:
   mov %sil, (%rdi)
   ret
.size _mmoutb, .-_mmoutb

.global _mmoutw
.type _mmoutw,"function"
_mmoutw:
   mov %si, (%rdi)
   ret
.size _mmoutw, .-_mmoutw

.global _mmoutl
.type _mmoutl,"function"
_mmoutl:
   mov %esi, (%rdi)
   ret
.size _mmoutl, .-_mmoutl

.global _mmoutq
.type _mmoutq,"function"
_mmoutq:
   mov %rsi, (%rdi)
   ret
.size _mmoutq, .-_mmoutq

.global _mminb
.type _mminb,"function"
_mminb:
   mov (%rdi), %al
   ret
.size _mminb, .-_mminb

.global _mminw
.type _mminw,"function"
_mminw:
   mov (%rdi), %ax
   ret
.size _mminw, .-_mminw

.global _mminl
.type _mminl,"function"
_mminl:
   mov (%rdi), %eax
   ret
.size _mminl, .-_mminl

.global _mminq
.type _mminq,"function"
_mminq:
   mov (%rdi), %rax
   ret
.size _mminq, .-_mminq

.global _invlpg
.type _invlpg,"function"
_invlpg:
   invlpg (%rdi)
   ret
.size _invlpg, .-_invlpg

.global _wrmsr
.type _wrmsr,"function"
_wrmsr:
   mov %rsi, %rdx
   mov %rsi, %rax
   mov %edi, %ecx
   shr $32, %rdx
   wrmsr
   ret
.size _wrmsr, .-_wrmsr

.global _rdmsr
.type _rdmsr,"function"
_rdmsr:
   mov %edi, %ecx
   rdmsr
   shl $32, %rdx
   mov %eax, %eax
   or %rdx, %rax
   ret
.size _rdmsr, .-_rdmsr

.global _xsetbv
.type _xsetbv,"function"
_xsetbv:
   mov %edi, %ecx
   mov %rsi, %rdx
   mov %rsi, %rax
   shr $32, %rdx
   xsetbv
   ret
.size _xsetbv, .-_xsetbv

.global _xgetbv
.type _xgetbv,"function"
_xgetbv:
   mov %edi, %ecx
   xgetbv
   sal $32, %rdx
   mov %eax, %eax
   or %rdx, %rax
   ret
.size _xgetbv, .-_xgetbv

.global _cpuid
.type _cpuid,"function"
_cpuid:
   mov %rdx, %r8
   mov %edi, %eax
   mov %esi, %ecx
   cpuid
   mov %eax, 0(%r8)
   mov %ebx, 4(%r8)
   mov %ecx, 8(%r8)
   mov %edx, 12(%r8)
   ret
.size _cpuid, .-_cpuid
