#![allow(non_snake_case)]
#![allow(unused_variables)]
#[cfg(not(feature = "test_direct_rec"))]
use velvet::prelude::*;
use std::{env, time::Instant};
use rand::{SeedableRng, rngs::StdRng, distr::{Distribution, Uniform}};
#[cfg(not(feature = "test_direct_rec"))]
include!(concat!(env!("OUT_DIR"), "/velvet_app.rs"));

mod par_merge;

#[cfg(feature = "rayon")]
mod par_rayon;

pub(crate) const DIRECT_THRESHOLD: usize = parse_threshold();
const fn parse_threshold() -> usize {
    if let Some(string) = option_env!("THRESHOLD") {
        let mut res: usize = 0;
        let mut bytes = string.as_bytes();
        while let [byte, rest @ ..] = bytes {
            assert!(b'0' <= *byte && *byte <= b'9', "invalid digit");
            res *= 10;
            res += (*byte - b'0') as usize;
            bytes = rest;
        }
        res
    } else {
        2097152
    }
}

pub(crate) fn gen_vec(n: usize, seed: usize) -> Vec<i32> {
    let range: Uniform<i32> = Uniform::try_from(i32::MIN..i32::MAX).unwrap();
    let mut rng = StdRng::seed_from_u64(seed as u64); 

    (0..n).map(|_| range.sample(&mut rng)).collect()
}
pub(crate) fn _check(sorted: Vec<i32>) {
    eprintln!("checking vec size: {}", sorted.len());
    // println!("sorted = {:?}", sorted);
    let mut prev = sorted[0];
    for i in 1..sorted.len() {
        if prev == 0 && sorted[i] == 0 {
            eprintln!("DOUBLE ZEROES!! at idx {}", i);
            return;
        }
        if sorted[i] < prev {
            eprintln!("NOT SORTED! discovered at idx = {}", i);
            return;
        } else {
            prev = sorted[i];
        }
    }
    eprintln!("SORTED!");
}

// sort src in-place, using buf if split is necessary
fn merge_sort(src: &mut [i32], buf: &mut [i32]) {
    if src.len() <= DIRECT_THRESHOLD {
        src.sort();
        return;
    }

    // split and sort (using buffer)
    let mid = src.len() / 2;
    let (left_src, right_src) = src.split_at_mut(mid);
    let (left_buf, right_buf) = buf.split_at_mut(mid);
    let (left_sorted, right_sorted) = (sort_into(left_src, left_buf), sort_into(right_src, right_buf));

    // merge buffers into src
    merge(left_sorted, right_sorted, src);
}

// sort src into dest
fn sort_into<'dest>(src: &mut [i32], dest: &'dest mut [i32]) -> &'dest [i32] {
    let mid = src.len() / 2;
    let (left_src, right_src) = src.split_at_mut(mid);
    
    // sort each half
    let (left_dest, right_dest) = dest.split_at_mut(mid);
    merge_sort(left_src, left_dest);
    merge_sort(right_src, right_dest);
    

    // merge the sorted halves into dest
    merge(left_src, right_src, dest);
    dest
}

fn merge(left: &[i32], right: &[i32], dest: &mut [i32]) {
    let max = Ord::max(*left.last().unwrap(), *right.last().unwrap());
    let mut left = left.iter();
    let mut left_n = *left.next().unwrap();
    let mut right = right.iter();
    let mut right_n = *right.next().unwrap();
    for d in dest.iter_mut() {
        if left_n < right_n {
            *d = left_n;
            left_n = match left.next() {
                Some(val) => *val,
                None => max,
            }
        } else {
            *d = right_n;
            right_n = match right.next() {
                Some(val) => *val,
                None => max,
            }
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 4 {
        println!("usage for mergesort: cargo run [cargo_options] [velvet|seq|rayon] [vec size] [random seed]");
        println!("example: cargo run --release velvet 100000 42");
        return;
    }

    let app = &args[1];
    let n: usize = args[2].parse().unwrap();
    let seed: usize = args[3].parse().unwrap();

    let arr = gen_vec(n, seed);

    if app.eq("velvet") {
        #[cfg(not(feature = "test_direct_rec"))]
        velvet_par_merge_glob_main(arr, seed);
    } else if app.eq("par_seq") {
        // pin thread!
        let core_ids = core_affinity::get_core_ids().unwrap();
        let res = core_affinity::set_for_current(core_ids[0]);
        if !res {
            eprintln!("Could not pin Root thread id continuing without pinning...");
        } 
        par_merge_seq_glob_main(arr, seed);
    } else if app.eq("rayon") {
        #[cfg(feature = "rayon")]
        {
            if args.len() < 5 {
                println!("must provide number of threads for Rayon");
                println!("usage for merge-sort: cargo run [cargo_options] [velvet|seq|rayon] [vec size] [random seed] [numer_of_threads]");
                println!("example: cargo run --release rayon-demo 100000 42 8");
                return;
            }
            let num_threads: usize = args[4].parse().unwrap();
            rayon_main(arr, seed, num_threads);
        }
        #[cfg(not (feature = "rayon"))]
        println!("COMPILE WITH RAYON!");
    } else if app.eq("test_direct") {
        #[cfg(feature = "test_direct_rec")]
        {
            if args.len() < 5 {
                println!("must provide number of workers!");
                return;
            }
            let num_workers: usize = args[4].parse().unwrap();
            test_direct_recursion(arr, seed, num_workers);
        }
        #[cfg(not(feature = "test_direct_rec"))]
        println!("Compile with feature \"test_direct_rec\"");
    } else if app.eq("gen_arr") {
        use std::{fs::{self,File}, io::{BufWriter, Write}};
        const CHUNK_INTS: usize = 1_000_000;
        let filename = format!("../../data/sort_arr_{}_{}.bin", n, seed);
        let file = File::create(&filename).unwrap();
        let mut writer = BufWriter::with_capacity(8 * 1024 * 1024, file);

        for chunk in arr.chunks(CHUNK_INTS) {
            let bytes = unsafe {
                std::slice::from_raw_parts(
                    chunk.as_ptr() as *const u8,
                    chunk.len() * std::mem::size_of::<i32>(),
                )
            };

            writer.write_all(bytes).unwrap();
        }
        writer.flush().unwrap();
        eprintln!("wrote array to {}", filename);

        let md = fs::metadata(&filename);
        match md {
            Err(e) => println!("{}", e),
            Ok(m) => {
                let size = m.len();
                let ints = size / 4;
                let exp = ints == arr.len().try_into().unwrap();
                println!("bytes = {}", size);
                println!("i32 count = {}, as expected = {}", ints, exp);
            }
        }
    } else {
        // pin thread!
        let core_ids = core_affinity::get_core_ids().unwrap();
        let res = core_affinity::set_for_current(core_ids[0]);
        if !res {
            eprintln!("Could not pin Root thread id continuing without pinning...");
        } 
        seq_main(arr, seed);
    }
}

fn seq_main(mut arr: Vec<i32>, seed: usize){
    let n: usize = arr.len();
    let mut buf: Vec<i32> = (0..n).map(|_| 0).collect();
    let start = Instant::now();
    merge_sort(&mut arr, &mut buf);
    let end = start.elapsed();

    println!("0,1,0,{},{},{}", n, seed, end.as_secs_f32());
    _check(arr);
}

#[cfg(not(feature = "test_direct_rec"))]
#[velvet_main(sort_parmerge_glob_spawn)]
fn velvet_par_merge_glob_main(arr: Vec<i32>, seed: usize) {
    let len = arr.len();

    use std::sync::atomic;
    par_merge::VEC.set(arr).unwrap();
    let vec: Vec<atomic::AtomicIsize> = (0..len).map(|_| atomic::AtomicIsize::new(0)).collect();
    par_merge::VEC_SORTED.set(vec).unwrap();
    let vec: Vec<atomic::AtomicIsize> = (0..len).map(|_| atomic::AtomicIsize::new(0)).collect();
    par_merge::VEC_MERGED.set(vec).unwrap();

    let start = Instant::now();
    par_merge::sort_parmerge_glob_spawn(0, len, true);
    let end = start.elapsed();
    
    let version = match velvet_get_queue_name().as_str() {
        "safe" => 2,
        "unsafe" => 20,
        "crossbeam" => 21,
        _ => -10,
    };
    println!("{},{},{},{},{},{}", version, velvet_get_num_workers(), DIRECT_THRESHOLD, len, seed, end.as_secs_f32());
    let sorted: Vec<i32> = par_merge::VEC_SORTED.get().unwrap().iter().map(|x| x.load(std::sync::atomic::Ordering::Relaxed) as i32).collect();
    _check(sorted);
}

fn par_merge_seq_glob_main(arr: Vec<i32>, seed: usize) {
    let len = arr.len();
    
    use std::sync::atomic;
    par_merge::VEC.set(arr).unwrap();
    let vec: Vec<atomic::AtomicIsize> = (0..len).map(|_| atomic::AtomicIsize::new(0)).collect();
    par_merge::VEC_SORTED.set(vec).unwrap();
    let vec: Vec<atomic::AtomicIsize> = (0..len).map(|_| atomic::AtomicIsize::new(0)).collect();
    par_merge::VEC_MERGED.set(vec).unwrap();

    let start = Instant::now();
    par_merge::sort_par_glob(0, len, true);
    let end = start.elapsed();
    
    println!("-1,1,{},{},{},{}", DIRECT_THRESHOLD, len, seed, end.as_secs_f32());
    let sorted: Vec<i32> = par_merge::VEC_SORTED.get().unwrap().iter().map(|x| x.load(std::sync::atomic::Ordering::Relaxed) as i32).collect();
    _check(sorted);
}

#[cfg(feature = "rayon")]
fn rayon_main(mut arr: Vec<i32>, seed: usize, num_threads: usize){
    let cores = core_affinity::get_core_ids().unwrap();
    rayon::ThreadPoolBuilder::new()
        .num_threads(num_threads)
        .start_handler(move |index| {
            let core_id = cores[index % cores.len()];
            let res = core_affinity::set_for_current(core_id);
            if !res {
                eprintln!("Could not pin worker thread id {:?}, continuing without pinning...", core_id.id);
            }
        })
        .build_global()
        .unwrap();

    let n = arr.len();
    let mut buf: Vec<i32> = vec![0; n];
    
    let start = Instant::now();
    par_rayon::sort(&mut arr, &mut buf[..], true);
    let end = start.elapsed();

    _check(arr);
    let _version = 1;

    println!("{},{},{},{},{},{}", _version, num_threads, DIRECT_THRESHOLD, n, seed, end.as_secs_f32());

}

#[cfg(feature = "test_direct_rec")]
fn test_direct_recursion(arr: Vec<i32>, seed: usize, num_workers: usize){ 
    // velvet_par_merge_glob_main
    let mut __root__worker__ = velvet::VelvetWorker::prepare_workers(num_workers, 64, __velvet_steal__);
    __root__worker__.wait();
    let len = arr.len();
    use std::sync::atomic;
    par_merge::VEC.set(arr).unwrap();
    let vec: Vec<atomic::AtomicIsize> = (0..len)
        .map(|_| atomic::AtomicIsize::new(0))
        .collect();
    par_merge::VEC_SORTED.set(vec).unwrap();
    let vec: Vec<atomic::AtomicIsize> = (0..len)
        .map(|_| atomic::AtomicIsize::new(0))
        .collect();
    par_merge::VEC_MERGED.set(vec).unwrap();
    let start = Instant::now();
    par_merge::sort_parmerge_glob_spawn(&mut __root__worker__, 0, len, true);
    let end = start.elapsed();

    let version = 7;
    println!("{},{},{},{},{},{}", version, num_workers, DIRECT_THRESHOLD, len, seed, end.as_secs_f32());
    let sorted: Vec<i32> = par_merge::VEC_SORTED.get().unwrap().iter().map(|x| x.load(std::sync::atomic::Ordering::Relaxed) as i32).collect();
    _check(sorted);
}


// ---------------------- FOR CHECKING EFFECT OF DIRECT RECURSION ----------
#[cfg(feature = "test_direct_rec")]
pub(crate) enum __Frame__ {
    Stolen(std::sync::Arc<std::sync::Mutex<Option<__Frame__>>>),
    InputSortParmergeGlobSpawn(usize, usize, usize, bool),
    InputMergeParGlobSpawn(usize, (usize, usize), (usize, usize), (usize, usize), bool)
}
#[cfg(feature = "test_direct_rec")]
impl velvet::Identifiable for __Frame__ {
    fn get_id(&self) -> usize {
        if let __Frame__::InputSortParmergeGlobSpawn(uid, ..) = self {
            return *uid;
        }
        if let __Frame__::InputMergeParGlobSpawn(uid, ..) = self {
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
                __Frame__::InputSortParmergeGlobSpawn(_, a0, a1, a2) => {
                    par_merge::sort_parmerge_glob_spawn(worker, a0, a1, a2);
                    *lock = None;
                }
                __Frame__::InputMergeParGlobSpawn(_, a0, a1, a2, a3) => {
                    par_merge::merge_par_glob_spawn(worker, a0, a1, a2, a3);
                    *lock = None;
                }
                _ => panic!("WRONG STOLEN WORK FRAME!"),
            }
            return;
        }
        n = (n + 1) % len;
    }
}