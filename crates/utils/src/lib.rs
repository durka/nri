/// Just like println!, but prints to stderr
#[macro_export]
macro_rules! errorln {
    ($($arg:tt)*) => {{
        use std::io::Write;
        match writeln!(&mut ::std::io::stderr(), $($arg)* ) {
            Ok(_) => {},
            Err(x) => panic!("Unable to write to stderr: {}", x),
        }
    }}
}

#[macro_export]
macro_rules! group_attr {
    (#[cfg($attr:meta)] $($yes:tt)*) => { group_attr!(internal () #[cfg($attr)] $($yes)*); };

    ($name:ident ($($krates:ident)*) #[cfg($attr:meta)] extern crate $krate:ident; $($yes:tt)*) => {
        #[cfg($attr)] extern crate $krate;
        group_attr!($name ($($krates)* $krate) #[cfg($attr)] $($yes)*);
    };

    ($name:ident ($($krates:ident)*) #[cfg($attr:meta)] #[macro_use] extern crate $krate:ident; $($yes:tt)*) => {
        #[cfg($attr)] #[macro_use] extern crate $krate;
        group_attr!($name ($($krates)* $krate) #[cfg($attr)] $($yes)*);
    };

    ($name:ident ($($krates:ident)*) #[cfg($attr:meta)] extern crate $krate:ident as $alias:ident; $($yes:tt)*) => {
        #[cfg($attr)] extern crate $krate as $alias;
        group_attr!($name ($($krates)* $alias) #[cfg($attr)] $($yes)*);
    };

    ($name:ident ($($krates:ident)*) #[cfg($attr:meta)] #[macro_use] extern crate $krate:ident as $alias:ident; $($yes:tt)*) => {
        #[cfg($attr)] #[macro_use] extern crate $krate as $alias;
        group_attr!($name ($($krates)* $alias) #[cfg($attr)] $($yes)*);
    };

    ($name:ident ($($krates:ident)*) #[cfg($attr:meta)] $($yes:item)*) => {
        #[cfg($attr)]
        mod $name {
            #[allow(unused_imports)] use super::{$($krates),*};
            #[allow(unused_imports)] use super::*;
            $($yes)*
        }
    
        #[cfg($attr)]
        pub use $name::*;
    };
}

pub mod prof {
    extern crate hprof;
    use std::cell::RefCell;

    pub use self::hprof::enter;

    thread_local! {
        // Thread-local profiler object (FIXME no doc comments on thread locals)
        pub static PROF: RefCell<Option<hprof::Profiler>> = RefCell::new(None)
    }

    #[macro_export]
    macro_rules! prof {
        ($b:expr) => { prof!(stringify!($b), $b) };
        //($n:expr, $b:expr) => ($b)
        ($n:expr, $b:expr) => {{
            $crate::prof::PROF.with(|wrapped_prof| {
                let appease_borrowck = wrapped_prof.borrow();
                let g = match *appease_borrowck {
                    Some(ref prof) => prof.enter($n),
                    None => $crate::prof::enter($n)
                };
                let ret = { $b }; //~ ALLOW let_unit_value
                drop(g);
                ret
            })
        }}
    }
}

pub use prof::PROF;

