.altmacro
.section .note.GNU-stack,"",%progbits
.section .text

.macro make_isr vec
.type isr_\vec,"function"
isr_\vec:
.if !(\vec == 8 || \vec == 10 || \vec == 11 || \vec == 12 || \vec == 13 || \vec == 14 || \vec == 17 || \vec == 21)
   push $0 // fake error code
.endif
   push $\vec
   jmp _isr_common
.size isr_\vec, .-isr_\vec
.endm

.macro isr_handler vec
.quad isr_\vec
.endm

.type _isr_common,"function"
.global dispatch_isr
_isr_common:
   // check if we need to swapgs by testing the code segment selector
   testq $3, 24(%rsp)
   jz 1f
   swapgs
1:
   push %r15
   push %r14
   push %r13
   push %r12
   push %r11
   push %r10
   push %r9
   push %r8
   push %rbp
   push %rdi
   push %rsi
   push %rdx
   push %rcx
   push %rbx
   push %rax
   xor %eax, %eax
   mov %es, %ax
   push %rax
   mov %ds, %ax
   push %rax
   mov %rsp, %rdi
   call dispatch_isr
   pop %rax
   mov %ax, %ds
   pop %rax
   mov %ax, %es
   pop %rax
   pop %rbx
   pop %rcx
   pop %rdx
   pop %rsi
   pop %rdi
   pop %rbp
   pop %r8
   pop %r9
   pop %r10
   pop %r11
   pop %r12
   pop %r13
   pop %r14
   pop %r15
   // pop the int vec number and error code from the stack
   add $16, %rsp
   // check if we need to swapgs before returning to user mode
   testq $3, 16(%rsp)
   jz 1f
   swapgs
1:
   iretq
.size _isr_common, .-_isr_common

.set i, 0
.rept 256
make_isr %i
.set i, i+1
.endr

.section .rodata
.global _isr_handlers
.type _isr_handlers,"object"
_isr_handlers:
.set i, 0
.rept 256
isr_handler %i
.set i, i+1
.endr
