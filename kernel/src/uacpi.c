#include <kernel/intrin.h>
#include <kernel/mmu.h>
#include <kernel/print.h>

#include <limine.h>
#include <uacpi/uacpi.h>

extern volatile struct limine_hhdm_request hhdm_request;
extern volatile struct limine_rsdp_request rsdp_request;

uacpi_status
uacpi_kernel_get_rsdp(uacpi_phys_addr* out_rdsp_address)
{
   struct limine_rsdp_response* rsdp_response = rsdp_request.response;
   struct limine_hhdm_response* hhdm_response = hhdm_request.response;

   assert(hhdm_response != NULL);
   assert(rsdp_response != NULL);

   *out_rdsp_address = (uint64_t)rsdp_response->address - hhdm_response->offset;

   return UACPI_STATUS_OK;
}

uacpi_status
uacpi_kernel_raw_memory_read(uacpi_phys_addr address, uacpi_u8 byte_width, uacpi_u64* out_value)
{
   switch (byte_width) {
      case 1:
         *out_value = _mminb(address + hhdm_offset);
         break;
      case 2:
         *out_value = _mminw(address + hhdm_offset);
         break;
      case 4:
         *out_value = _mminl(address + hhdm_offset);
         break;
      case 8:
         *out_value = _mminq(address + hhdm_offset);
         break;
      default:
         return UACPI_STATUS_INVALID_ARGUMENT;
   }

   return UACPI_STATUS_OK;
}

uacpi_status
uacpi_kernel_raw_memory_write(uacpi_phys_addr address, uacpi_u8 byte_width, uacpi_u64 in_value)
{
   switch (byte_width) {
      case 1:
         _mmoutb(address + hhdm_offset, in_value);
         break;
      case 2:
         _mmoutw(address + hhdm_offset, in_value);
         break;
      case 4:
         _mmoutl(address + hhdm_offset, in_value);
         break;
      case 8:
         _mmoutq(address + hhdm_offset, in_value);
         break;
      default:
         return UACPI_STATUS_INVALID_ARGUMENT;
   }

   return UACPI_STATUS_OK;
}

uacpi_status
uacpi_kernel_raw_io_read(uacpi_io_addr address, uacpi_u8 byte_width, uacpi_u64* out_value)
{
   switch (byte_width) {
      case 1:
         *out_value = _inb(address);
         break;
      case 2:
         *out_value = _inw(address);
         break;
      case 4:
         *out_value = _inl(address);
         break;
      default:
         return UACPI_STATUS_INVALID_ARGUMENT;
   }

   return UACPI_STATUS_OK;
}

uacpi_status
uacpi_kernel_raw_io_write(uacpi_io_addr address, uacpi_u8 byte_width, uacpi_u64 in_value)
{
   switch (byte_width) {
      case 1:
         _outb(address, in_value);
         break;
      case 2:
         _outw(address, in_value);
         break;
      case 4:
         _outl(address, in_value);
         break;
      default:
         return UACPI_STATUS_INVALID_ARGUMENT;
   }

   return UACPI_STATUS_OK;
}

uacpi_status
uacpi_kernel_pci_read(uacpi_pci_address* address,
                      uacpi_size offset,
                      uacpi_u8 byte_width,
                      uacpi_u64* value)
{
   return UACPI_STATUS_UNIMPLEMENTED;
}

uacpi_status
uacpi_kernel_pci_write(uacpi_pci_address* address,
                       uacpi_size offset,
                       uacpi_u8 byte_width,
                       uacpi_u64 value)
{
   return UACPI_STATUS_UNIMPLEMENTED;
}

uacpi_status
uacpi_kernel_io_map(uacpi_io_addr base, uacpi_size len, uacpi_handle* out_handle)
{
   return UACPI_STATUS_UNIMPLEMENTED;
}

void
uacpi_kernel_io_unmap(uacpi_handle handle)
{
}

uacpi_status
uacpi_kernel_io_read(uacpi_handle, uacpi_size offset, uacpi_u8 byte_width, uacpi_u64* value)
{
   return UACPI_STATUS_UNIMPLEMENTED;
}

uacpi_status
uacpi_kernel_io_write(uacpi_handle, uacpi_size offset, uacpi_u8 byte_width, uacpi_u64 value)
{
   return UACPI_STATUS_UNIMPLEMENTED;
}

void*
uacpi_kernel_map(uacpi_phys_addr addr, uacpi_size len)
{
   struct limine_hhdm_response* hhdm_response = hhdm_request.response;

   assert(hhdm_response != NULL);

   return (void*)(addr + hhdm_response->offset);
}

void
uacpi_kernel_unmap(void* addr, uacpi_size len)
{
}

void*
uacpi_kernel_alloc(uacpi_size size)
{
   return NULL;
}

void*
uacpi_kernel_calloc(uacpi_size count, uacpi_size size)
{
   return NULL;
}

void
uacpi_kernel_free(void* mem)
{
}

void
uacpi_kernel_log(uacpi_log_level level, const uacpi_char* fmt, ...)
{
   va_list args;

   va_start(args, fmt);
   uacpi_kernel_vlog(level, fmt, args);
   va_end(args);
}

void
uacpi_kernel_vlog(uacpi_log_level level, const uacpi_char* fmt, uacpi_va_list args)
{
   vprintf(fmt, args);
}

uacpi_u64
uacpi_kernel_get_ticks(void)
{
   return 0;
}

void
uacpi_kernel_stall(uacpi_u8 usec)
{
}

void
uacpi_kernel_sleep(uacpi_u64 msec)
{
}

uacpi_handle
uacpi_kernel_create_mutex(void)
{
   return UACPI_NULL;
}

void
uacpi_kernel_free_mutex(uacpi_handle)
{
}

uacpi_handle
uacpi_kernel_create_event(void)
{
   return UACPI_NULL;
}

void
uacpi_kernel_free_event(uacpi_handle)
{
}

uacpi_thread_id
uacpi_kernel_get_thread_id(void)
{
   return UACPI_NULL;
}

uacpi_bool
uacpi_kernel_acquire_mutex(uacpi_handle, uacpi_u16)
{
   return UACPI_FALSE;
}

void
uacpi_kernel_release_mutex(uacpi_handle)
{
}

uacpi_bool
uacpi_kernel_wait_for_event(uacpi_handle, uacpi_u16)
{
   return UACPI_FALSE;
}

void
uacpi_kernel_signal_event(uacpi_handle)
{
}

void
uacpi_kernel_reset_event(uacpi_handle)
{
}

uacpi_status
uacpi_kernel_handle_firmware_request(uacpi_firmware_request*)
{
   return UACPI_STATUS_UNIMPLEMENTED;
}

uacpi_status
uacpi_kernel_install_interrupt_handler(uacpi_u32 irq,
                                       uacpi_interrupt_handler,
                                       uacpi_handle ctx,
                                       uacpi_handle* out_irq_handle)
{
   return UACPI_STATUS_UNIMPLEMENTED;
}

uacpi_status
uacpi_kernel_uninstall_interrupt_handler(uacpi_interrupt_handler, uacpi_handle irq_handle)
{
   return UACPI_STATUS_UNIMPLEMENTED;
}

uacpi_handle
uacpi_kernel_create_spinlock(void)
{
   return UACPI_NULL;
}

void
uacpi_kernel_free_spinlock(uacpi_handle)
{
}

uacpi_cpu_flags
uacpi_kernel_lock_spinlock(uacpi_handle)
{
   return 0;
}

void
uacpi_kernel_unlock_spinlock(uacpi_handle, uacpi_cpu_flags)
{
}

uacpi_status
uacpi_kernel_schedule_work(uacpi_work_type, uacpi_work_handler, uacpi_handle ctx)
{
   return UACPI_STATUS_UNIMPLEMENTED;
}

uacpi_status
uacpi_kernel_wait_for_work_completion(void)
{
   return UACPI_STATUS_UNIMPLEMENTED;
}
