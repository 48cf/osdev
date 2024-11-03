OUTPUT_FORMAT(elf64-x86-64)
ENTRY(_start)

PHDRS {
  text    PT_LOAD;
  rodata  PT_LOAD;
  data    PT_LOAD;
}

SECTIONS {
  . = 0xffffffff80000000;

  .text : {
    __text_start = .;
    *(.text .text.*)
    __text_end = .;
  } :text

  . = ALIGN(CONSTANT(MAXPAGESIZE));

  .rodata : {
    __rodata_start = .;
    *(.rodata .rodata.*)
    __rodata_end = .;
  } :rodata

  . = ALIGN(CONSTANT(MAXPAGESIZE));

  .data : {
    __data_start = .;
    *(.data .data.*)
  } :data

  .bss : {
    *(.bss .bss.*)
    *(COMMON)
    __data_end = .;
  } :data

  /DISCARD/ : {
    *(.eh_frame*)
    *(.note .note.*)
  }
}
