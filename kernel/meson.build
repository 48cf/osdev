project(
  'kernel',
  'c',
  default_options: ['c_std=gnu23'],
)

kernel_sources = files(
  'src/acpi.c',
  'src/alloc.c',
  'src/cpu.S',
  'src/cpu.c',
  'src/intr.c',
  'src/intrin.S',
  'src/isr.S',
  'src/lapic.c',
  'src/mm.c',
  'src/mmu.c',
  'src/print.c',
  'src/serial.c',
  'src/start.c',
  'src/stdlib.c',
  'src/thread.S',
  'src/thread.c',
  'src/time.c',
  'src/timer.c',
  'src/uacpi.c',
)

kernel_includes = [
  include_directories('include'),
]

flanterm = dependency('flanterm')
freestnd_c_hdrs = dependency('freestnd-c-hdrs')
nanoprintf = dependency('nanoprintf')
uacpi = subproject('uacpi')

kernel_sources += uacpi.get_variable('sources')
kernel_includes += uacpi.get_variable('includes')

kernel_c_args = [
  '-DUACPI_FORMATTED_LOGGING=1',
  '-fdata-sections',
  '-ffunction-sections',
  '-ffreestanding',
  '-flto',
  '-fno-stack-check',
  '-fno-stack-protector',
  '-march=x86-64',
  '-mcmodel=kernel',
  '-mgeneral-regs-only',
  '-mno-red-zone',
  '-nostdinc',
  '-nostdlib',
]

kernel_link_args = [
  '-flto',
  '-nostdlib',
  '-static',
  '-Wl,--build-id=none',
  '-Wl,--gc-sections',
  '-Wl,-m,elf_x86_64',
  '-Wl,-z,max-page-size=0x1000',
  '-Wl,-T,' + meson.current_source_dir() / 'linker-x86_64.lds',
]

kernel = executable(
  'kernel',
  kernel_sources,
  c_args: kernel_c_args,
  link_args: kernel_link_args,
  include_directories: kernel_includes,
  dependencies: [flanterm, freestnd_c_hdrs, nanoprintf],
)
