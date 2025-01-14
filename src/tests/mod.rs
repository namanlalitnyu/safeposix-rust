#![allow(dead_code)] //suppress warning for these functions not being used in targets other than the
                     // tests

mod fs_tests;
mod ipc_tests;
mod networking_tests;
mod sys_tests;
use rand::Rng;
use std::net::{TcpListener, UdpSocket};

use crate::interface;
use crate::safeposix::{cage::*, filesystem::*};

#[allow(unused_parens)]
#[cfg(test)]
mod setup {

    use crate::interface;
    use crate::safeposix::{cage::*, dispatcher::*, filesystem::*};

    use lazy_static::lazy_static;
    use std::process::Command;
    use std::sync::Mutex;

    // Tests in rust as parallel by default and to make them share resources we are
    // using a global static lock.
    lazy_static! {
        // This has a junk value (a bool).  Could be anything...
        #[derive(Debug)]
        pub static ref TESTMUTEX: Mutex<bool> = {
            Mutex::new(true)
        };
    }

    // Using explicit lifetime to have a safe reference to the lock in the tests.
    pub fn lock_and_init<'a>() -> std::sync::MutexGuard<'a, bool> {
        set_panic_hook();

        //acquiring a lock on TESTMUTEX prevents other tests from running concurrently
        let thelock = TESTMUTEX.lock().unwrap_or_else(|e| {
            //if the lock is poisoned, we need to clear the poison and clean up references
            // to the cage.
            lindrustfinalize();
            //clear the mutex poisoning.
            TESTMUTEX.clear_poison();
            //return the underlying guard.
            e.into_inner()
        });

        interface::RUSTPOSIX_TESTSUITE.store(true, interface::RustAtomicOrdering::Relaxed);

        //setup the lind filesystem, creates a clean filesystem for each test
        lindrustinit(0);
        {
            println!("test_setup()");
            let cage = interface::cagetable_getref(1);
            crate::lib_fs_utils::lind_deltree(&cage, "/");
            assert_eq!(cage.mkdir_syscall("/dev", S_IRWXA), 0);
            assert_eq!(
                cage.mknod_syscall(
                    "/dev/null",
                    S_IFCHR as u32 | 0o777,
                    makedev(&DevNo { major: 1, minor: 3 })
                ),
                0
            );
            assert_eq!(
                cage.mknod_syscall(
                    "/dev/zero",
                    S_IFCHR as u32 | 0o777,
                    makedev(&DevNo { major: 1, minor: 5 })
                ),
                0
            );
            assert_eq!(
                cage.mknod_syscall(
                    "/dev/urandom",
                    S_IFCHR as u32 | 0o777,
                    makedev(&DevNo { major: 1, minor: 9 })
                ),
                0
            );
            assert_eq!(
                cage.mknod_syscall(
                    "/dev/random",
                    S_IFCHR as u32 | 0o777,
                    makedev(&DevNo { major: 1, minor: 8 })
                ),
                0
            );
            assert_eq!(cage.exit_syscall(EXIT_SUCCESS), EXIT_SUCCESS);
        }
        lindrustfinalize();

        //initialize the cage for the test.
        lindrustinit(0);

        //return the lock to the caller which holds it till the end of the test.
        thelock
    }

    fn set_panic_hook() {
        let orig_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |panic_info| {
            // this hook would be triggered whenever a panic occurs
            // good for test cases that panicked inside the non-main thread
            // so the trace information could be printed immediately
            // instead of raising the error when the thread is joined, which might
            // never happen and left the test blocking forever in some test cases
            orig_hook(panic_info);
        }));
    }
}

pub fn str2cbuf(ruststr: &str) -> *mut u8 {
    let cbuflenexpected = ruststr.len();
    let (ptr, len, _) = ruststr.to_string().into_raw_parts();
    assert_eq!(len, cbuflenexpected);
    return ptr;
}

pub fn sizecbuf<'a>(size: usize) -> Box<[u8]> {
    let v = vec![0u8; size];
    v.into_boxed_slice()
    //buf.as_mut_ptr() as *mut u8
}

pub fn cbuf2str(buf: &[u8]) -> &str {
    std::str::from_utf8(buf).unwrap()
}

// The RustPOSIX test suite avoids conflicts caused by repeatedly binding to the
// same ports by generating a random port number within the valid range
// (49152-65535) for each test run. This eliminates the need for waiting between
// tests.

fn is_port_available(port: u16) -> bool {
    TcpListener::bind(("127.0.0.1", port)).is_ok() && UdpSocket::bind(("127.0.0.1", port)).is_ok()
}

pub fn generate_random_port() -> u16 {
    for port in 49152..65535 {
        if is_port_available(port) {
            return port;
        }
    }
    panic!("No available ports found");
}
