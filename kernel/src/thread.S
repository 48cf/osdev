.section .note.GNU-stack,"",%progbits
.section .text

.global _do_context_switch
.type _do_context_switch,"function"
_do_context_switch:
   mov %rsi, 0x0(%rdi)
   mov %rdi, 0x8(%rdi)
   mov %rbx, 0x10(%rdi)
   mov %rsp, 0x18(%rdi)
   mov %rbp, 0x20(%rdi)
   mov %r12, 0x28(%rdi)
   mov %r13, 0x30(%rdi)
   mov %r14, 0x38(%rdi)
   mov %r15, 0x40(%rdi)
   mov 0x40(%rsi), %r15
   mov 0x38(%rsi), %r14
   mov 0x30(%rsi), %r13
   mov 0x28(%rsi), %r12
   mov 0x20(%rsi), %rbp
   mov 0x18(%rsi), %rsp
   mov 0x10(%rsi), %rbx
   mov 0x8(%rsi), %rdi
   mov 0x0(%rsi), %rsi
   ret
.size _do_context_switch, .-_do_context_switch

.global _enter_userspace
.type _enter_userspace,"function"
_enter_userspace:
   mov $0x23, %ax
   mov %ax, %es
   mov %ax, %ds
   swapgs
   iretq
.size _enter_userspace, .-_enter_userspace
