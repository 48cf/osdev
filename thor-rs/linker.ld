ENTRY(kernel_main)

PHDRS
{
  text PT_LOAD FLAGS(5);
  rodata PT_LOAD FLAGS(4);
  note PT_NOTE FLAGS(6);
  data PT_LOAD FLAGS(6);
  percpu PT_LOAD FLAGS(6);
}

SECTIONS
{
  . = 0xffffffff80000000;

  .text : {
    *(.text .text.*)
  } :text

  . = ALIGN(CONSTANT(MAXPAGESIZE));

  .rodata : {
    *(.rodata .rodata.*)

    LD_PERCPU_INIT_FNS_START = .;
    *(.percpu.init .percpu.init.*)
    LD_PERCPU_INIT_FNS_END = .;
  } :rodata

  .note.gnu.build-id : {
    *(.note.gnu.build-id)
  } :rodata

  . = ALIGN(CONSTANT(MAXPAGESIZE));

  .data : {
    *(.data .data.*)
  } :data

  .note.kernel : {
    *(.note.kernel)
  } :data :note

  .bss : {
    *(.bss .bss.*)
    *(COMMON)
  } :data

  . = ALIGN(CONSTANT(MAXPAGESIZE));

  .percpu : {
    LD_PERCPU_START = .;
    *(.percpu.head)
    *(.percpu.tail)
    . = ALIGN(CONSTANT(MAXPAGESIZE));
    LD_PERCPU_END = .;
  } :percpu

  /DISCARD/ : {
    *(.eh_frame*)
    *(.note .note.*)
  }
}
