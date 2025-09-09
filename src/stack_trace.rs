use crate::{KERNEL_SYMBOL_MODULE, MODULE_REQUEST, arch_x86_64, kernel_virt_begin};

pub struct StackTrace {
    rbp: Option<u64>,
}

impl StackTrace {
    // always inlined since otherwise we'll be copying the rbp of this function,
    // which is useless
    #[inline(always)]
    pub fn new() -> Self {
        Self {
            rbp: Some(arch_x86_64::rbp()),
        }
    }

    /// Get the next address in the stack trace.
    /// Note: the address here is of the RIP at which the call instruction to the function was called.
    // Note: this must be inlined, otherwise we might enter an infinite loop since calling .next()
    // changes the rbp constatnly
    #[inline(always)]
    pub unsafe fn next(&mut self) -> Option<u64> {
        let rbp = self.rbp?;
        let as_ptr = rbp as *const u64;
        let addr = unsafe { *(as_ptr.offset(1)) };
        self.rbp = Some(unsafe { *as_ptr });
        //qemu_println!("rbp: {:#x?}", self.rbp);
        if self.rbp == Some(0) {
            // we reached the end of the trace, which means that
            // addr is invalid
            self.rbp = None;
        }
        Some(addr)
    }

    /// Look up the symbol of a function from a certain return address
    /// ## Safety:
    /// Must ensure that the KERNEL_SYMBOL_MODULE is loaded
    // todo: instead of making it unsafe, just make sure that it is intialized and return an error if it i not.
    pub unsafe fn lookup_symbol_from_return_addr(ret_addr: u64) -> Option<&'static str> {
        for addr in (kernel_virt_begin()..ret_addr).rev() {
            // safety: we require our called to ensure that the kerenl symbol module is loaded
            if let Some(sym) = unsafe { lookup_symbol(addr) } {
                return Some(sym);
            }
        }

        None
    }

    // inline always since otherwise we'll look the name of this function
    #[inline(always)]
    pub unsafe fn lookup_current_function() -> Option<&'static str> {
        unsafe { Self::lookup_symbol_from_return_addr(arch_x86_64::rip()) }
    }
}

/// Lookup a name of a symbol from an address.
/// ## Safety:
/// must ensure that the KERNEL_SYMBOL_MODULE is loaded
// todo: make less ugly
// todo: binary search?
pub unsafe fn lookup_symbol(addr: u64) -> Option<&'static str> {
    //qemu_println!("looking up addr: {:#x}", addr);
    let modules = MODULE_REQUEST.get_response().unwrap();
    let symbols_module = modules
        .modules()
        .iter()
        .find(|f| f.path().to_bytes().ends_with(KERNEL_SYMBOL_MODULE.path()))
        .unwrap();
    // the symbol module is just a file in the following format:
    // addr | SYMBOL_TYPE | symbol_name
    // so we just parse that basically
    let bytes = unsafe {
        core::slice::from_raw_parts(symbols_module.addr(), symbols_module.size() as usize)
    };
    let mut lines = bytes.split(|s| *s == b'\n');
    while let Some(line) = lines.next() {
        // skip the type of the symbol
        if line.is_empty() {
            continue;
        }
        let mut split = line.splitn(3, |c| c.is_ascii_whitespace());
        let sym_addr = split.next().unwrap();
        let _type = split.next();
        let name = split.next();
        let addr_as_num = u64::from_str_radix(str::from_utf8(sym_addr).unwrap(), 16).unwrap();
        if addr_as_num > addr {
            break;
        }
        if addr_as_num == addr && name.is_some() {
            return Some(str::from_utf8(name.unwrap()).unwrap());
        }
    }
    None
}
