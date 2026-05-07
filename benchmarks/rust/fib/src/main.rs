#![allow(unused_imports)] // these three allows are to avoid warnings for test_direct_rec/test_no_thresh version..
#![allow(non_snake_case)]
#![allow(dead_code)]
use velvet::prelude::*;
use std::time::Instant;
use std::env;
#[cfg(not(feature = "test_direct_rec"))]
include!(concat!(env!("OUT_DIR"), "/velvet_app.rs"));

const THRESHOLD: u64 = parse_threshold();
const fn parse_threshold() -> u64 {
    if let Some(string) = option_env!("THRESHOLD") {
        let mut res: u64 = 0;
        let mut bytes = string.as_bytes();
        while let [byte, rest @ ..] = bytes {
            assert!(b'0' <= *byte && *byte <= b'9', "invalid digit");
            res *= 10;
            res += (*byte - b'0') as u64;
            bytes = rest;
        }
        res
    } else {
        25
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        println!("usage for fib: cargo run [cargo_options] [velvet|seq] [n]");
        println!("example: cargo run --release velvet 42");
        return;
    }
    let app = &args[1];
    let n: u64 = args[2].parse().unwrap();

    if app.eq("velvet") {
        #[cfg(not(feature = "test_direct_rec"))]
        velvet_main(n);
    } else if app.eq("test_direct") {
        #[cfg(feature = "test_direct_rec")]
        {
            if args.len() < 4 {
                println!("must provide number of workers!");
                return;
            }
            let num_workers: usize = args[3].parse().unwrap();
            test_direct_recursion(n, num_workers);
        }
        #[cfg(not(feature = "test_direct_rec"))]
        println!("Compile with feature \"test_direct_rec\"");
    } else {
        // pin thread!
        let core_ids = core_affinity::get_core_ids().unwrap();
        let res = core_affinity::set_for_current(core_ids[0]);
        if !res {
            eprintln!("Could not pin Root thread id continuing without pinning...");
        }

        let start_seq = Instant::now();
        let oracle = fib_seq(n);
        let end_seq = start_seq.elapsed();
        eprintln!("ORACLE = {}, in seq time = {}", oracle, end_seq.as_secs_f32());
        println!("0,1,{},0,{}", n, end_seq.as_secs_f32());
    }

}

#[cfg(not(feature = "test_direct_rec"))]
#[velvet_main(fib)]
fn velvet_main(n: u64) {
    #[cfg(feature = "test_no_thresh")]
    eprintln!("TESTING NO THRESHOLD VERSION!");
    let start = Instant::now();
    let res = fib(n);
    let end = start.elapsed();
    eprintln!("VELVET RESULT = {}, in parallel time = {}", res, end.as_secs_f32());


   let version = match velvet_get_queue_name().as_str() {
    "safe" => 2,
    "unsafe" => 3,
    "crossbeam" => 4,
    _ => -1,
   };

   #[cfg(not(feature = "test_no_thresh"))]
   let thresh = THRESHOLD;
   #[cfg(feature = "test_no_thresh")]
   let thresh = -1;
   #[cfg(feature = "test_no_thresh")]
   let version = 3;

    println!("{},{},{},{},{}", version, velvet_get_num_workers(), n, thresh, end.as_secs_f32());
}

#[cfg(not(feature = "test_direct_rec"))]
#[spawnable]
fn fib(n: u64) -> u64 {
    #[cfg(not(feature = "test_no_thresh"))]
    if n < THRESHOLD { return fib_seq(n); }
    #[cfg(feature = "test_no_thresh")]
    if n < 2 { return n; }
    let r2 = fib(n-1);
    let r1 = fib(n-2);
    return r1 + r2;
}

fn fib_seq(n: u64) -> u64 {
    if n < 2 { return n; }
    let r1 = fib_seq(n-2);
    let r2 = fib_seq(n-1);
    return r1 + r2;
}

// ---------------------- FOR CHECKING EFFECT OF DIRECT RECURSION ----------
#[cfg(feature = "test_direct_rec")]
pub(crate) enum __Frame__ {
    Stolen(std::sync::Arc<std::sync::Mutex<Option<__Frame__>>>),
    InputFib(usize, u64),
    OutputFib(u64),
}
#[cfg(feature = "test_direct_rec")]
impl velvet::Identifiable for __Frame__ {
    fn get_id(&self) -> usize {
        if let __Frame__::InputFib(uid, ..) = self {
            return *uid;
        }
        return 0;
    }
}
#[cfg(feature = "test_direct_rec")]
fn __velvet_steal__(worker: &mut velvet::VelvetWorker<__Frame__>) {
    let stealers = &worker.stealers;
    let len = stealers.len();
    let mut n = worker.get_random(len);
    let result_slot = std::sync::Arc::new(std::sync::Mutex::new(None));
    let mut lock = result_slot.lock().unwrap();
    for _ in 0..len {
        let maybe_frame = stealers[n].steal(__Frame__::Stolen(result_slot.clone()));
        if let Some(frame) = maybe_frame {
            match frame {
                __Frame__::InputFib(_, a0) => {
                    let result = fib(worker, a0);
                    *lock = Some(__Frame__::OutputFib(result));
                }
                _ => panic!("WRONG STOLEN WORK FRAME!"),
            }
            return;
        }
        n = (n + 1) % len;
    }
}
#[cfg(feature = "test_direct_rec")]
fn test_direct_recursion(n: u64, num_workers: usize) {
    let mut __root__worker__ = velvet::VelvetWorker::prepare_workers(num_workers, 64, __velvet_steal__);
    __root__worker__.wait();
    
    let start = Instant::now();
    let res = fib(&mut __root__worker__, n);
    let end = start.elapsed();
    eprintln!("res = {}", res);

    let version = 7;
    #[cfg(not(feature = "test_no_thresh"))]
    let thresh = THRESHOLD;
    #[cfg(feature = "test_no_thresh")]
    let thresh = -1;
    #[cfg(feature = "test_no_thresh")]
    let version = 8;
    
    println!("{},{},{},{},{}", version, num_workers, n, thresh, end.as_secs_f32());
}

#[cfg(feature = "test_direct_rec")]
fn fib(__worker__: &mut velvet::VelvetWorker<__Frame__>, n: u64) -> u64 {
    #[cfg(not(feature = "test_no_thresh"))]
    if n < THRESHOLD { return fib_seq(n); }
    #[cfg(feature = "test_no_thresh")]
    if n < 2 { return n; }
    let __0__ = __worker__.get_seq();
    __worker__.spawn(__Frame__::InputFib(__0__, n - 1));
    let __1__ = __worker__.get_seq();
    __worker__.spawn(__Frame__::InputFib(__1__, n - 2));
    let __SYNC__ = __worker__.sync(__1__);
    let __SYNC_RES__ = match __SYNC__ {
        __Frame__::InputFib(_, a0) => fib(__worker__, a0),
        __Frame__::Stolen(ptr) => {
            let mut try_lock = ptr.try_lock();
            loop {
                if let Ok(mut _value) = try_lock {
                    if let Some(__Frame__::OutputFib(result)) = (*_value).take() {
                        break result
                    } else {
                        panic!("WRONG STOLEN RESULT FRAME!");
                    }
                } else {
                    __worker__.steal();
                    try_lock = ptr.try_lock();
                }
            }
        }
        _ => panic!("WRONG FRAME POPPED!"),
    };
    let r1 = __SYNC_RES__;
    let __SYNC__ = __worker__.sync(__0__);
    let __SYNC_RES__ = match __SYNC__ {
        __Frame__::InputFib(_, a0) => fib(__worker__, a0),
        __Frame__::Stolen(ptr) => {
            let mut try_lock = ptr.try_lock();
            loop {
                if let Ok(mut _value) = try_lock {
                    if let Some(__Frame__::OutputFib(result)) = (*_value).take() {
                        break result
                    } else {
                        panic!("WRONG STOLEN RESULT FRAME!");
                    }
                } else {
                    __worker__.steal();
                    try_lock = ptr.try_lock();
                }
            }
        }
        _ => panic!("WRONG FRAME POPPED!"),
    };
    let r2 = __SYNC_RES__;
    return r1 + r2;
}
