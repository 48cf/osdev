mod x86_64;

cfg_select! {
    target_arch = "x86_64" => {
        use x86_64 as arch_impl;
    }
    _ => {
        panic!("Unsupported architecture");
    }
}

// pub mod image;

// pub use x86_64::*;
