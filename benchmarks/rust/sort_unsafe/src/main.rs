// Merge-Sort with unsafe buffers
#![allow(non_snake_case)]
#[cfg(not(feature = "test_direct_rec"))]
use velvet::prelude::*;
use std::{env, time::Instant};
use rand::{SeedableRng, rngs::StdRng, distr::{Distribution, Uniform}};
#[cfg(not(feature = "test_direct_rec"))]
include!(concat!(env!("OUT_DIR"), "/velvet_app.rs"));

pub(crate) const SORT_CHUNK: usize = parse_threshold();
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
const MERGE_CHUNK: usize = 2*SORT_CHUNK;

// SIMPLE BUFFER TO ALLOW MUTABILITY
#[derive(Debug, Copy, Clone)]
pub struct MutBuf {
    ptr: *mut i32,
    len: usize,
}
unsafe impl Send for MutBuf {}
unsafe impl Sync for MutBuf {}
impl MutBuf {
    pub fn from_slice(s: &mut [i32]) -> Self {
        Self { ptr: s.as_mut_ptr(), len: s.len() }
    }

    pub(crate) fn read(&self, index: usize) -> i32 {
        assert!(index < self.len);
        unsafe { *self.ptr.add(index) }
    }

    pub fn as_slice(&self) -> &[i32] {
        unsafe { std::slice::from_raw_parts(self.ptr, self.len) }
    }

    pub(crate) fn as_mut_slice(&self) -> &mut [i32] {
        unsafe { std::slice::from_raw_parts_mut(self.ptr, self.len) }
    }

    pub(crate) fn sub_slice(&self, start: usize, end: usize) -> Self {
        assert!(start < end && end <= self.len);
        unsafe { Self { ptr: self.ptr.add(start), len: end-start} }
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
        velvet_unsafe_main(arr, seed);
    } else if app.eq("par_seq") {
        // pin thread!
        let core_ids = core_affinity::get_core_ids().unwrap();
        let res = core_affinity::set_for_current(core_ids[0]);
        if !res {
            eprintln!("Could not pin Root thread id continuing without pinning...");
        } 
        par_unsafe_main(arr, seed);
    } else if app.eq("test_direct") {
        #[cfg(feature = "test_direct_rec")]
        {
            if args.len() < 5 {
                println!("must provide number of workers!");
                return;
            }
            let num_workers: usize = args[4].parse().unwrap();
            test_direct_recursion_unsafe(arr, seed, num_workers);
        }
        #[cfg(not(feature = "test_direct_rec"))]
        println!("Compile with feature \"test_direct_rec\"");
    } else {
        eprintln!("unrecognised app: {}", app);
    }
}

#[cfg(not(feature = "test_direct_rec"))]
#[velvet_main(spawn_sort_unsafe)]
fn velvet_unsafe_main(mut arr: Vec<i32>, seed: usize) {
    let n = arr.len();
    let mut buf = vec![0;n];

    let vec = MutBuf::from_slice(&mut arr);
    let buf = MutBuf::from_slice(&mut buf);

    let start = Instant::now();
    spawn_sort_unsafe(vec.sub_slice(0, n), buf, true);
    let end = start.elapsed();
    
    let version = match velvet_get_queue_name().as_str() {
        "safe" => 3,
        "unsafe" => 27,
        "crossbeam" => 28,
        _ => -10,
    };
    println!("{},{},{},{},{},{}", version, velvet_get_num_workers(), SORT_CHUNK, arr.len(), seed, end.as_secs_f32());
    _check(vec.as_slice().to_vec());
}

fn par_unsafe_main(mut arr: Vec<i32>, seed: usize) {
    let n = arr.len();
    let mut buf = vec![0;n];

    let vec = MutBuf::from_slice(&mut arr);
    let buf = MutBuf::from_slice(&mut buf);

    let start = Instant::now();
    sort_unsafe(vec.sub_slice(0, n), buf, true);
    let end = start.elapsed();
    
    println!("-2,1,{},{},{},{}", SORT_CHUNK, arr.len(), seed, end.as_secs_f32());
    _check(vec.as_slice().to_vec());
}

fn sort_unsafe(src: MutBuf, buf: MutBuf, usebuf: bool) {
    let buf_left;
    let buf_right;
    let left;
    let right;
    if usebuf && src.len <= SORT_CHUNK {
        src.as_mut_slice().sort();
        return;
    } else {
        let mid = src.len/2;
        left = src.sub_slice(0, mid);
        right = src.sub_slice(mid, src.len);
        buf_left = buf.sub_slice(0, mid);
        buf_right = buf.sub_slice(mid, buf.len);
        sort_unsafe(left, buf_left, !usebuf);
        sort_unsafe(right, buf_right, !usebuf);
    }

    if usebuf {
        merge_unsafe(buf_left, buf_right, src);
    } else {
        merge_unsafe(left, right, buf);
    }
}

fn merge_unsafe(mut left: MutBuf, mut right: MutBuf, dest: MutBuf) {
    if dest.len <= MERGE_CHUNK {
        merge_seq_unsafe(left, right, dest);
        return;
    }

    // let 'left' be the larger of the two sub-arrays
    if left.len < right.len { 
        let tmp_left = left;
        left = right;
        right = tmp_left;
    };

    // find the middle element of left, and use std's binary_search to find suitable index in right
    let a_mid = left.len / 2;
    let b_mid = match right.as_slice().binary_search(&left.read(a_mid)) {
        Ok(i) => i,
        Err(i) => i,
    };

    // recurse, splitting at the newly found 'mid' indexes (in the sense of value, not array size)
    merge_unsafe(left.sub_slice(0, a_mid), right.sub_slice(0, b_mid), dest.sub_slice(0, a_mid+b_mid));
    merge_unsafe(left.sub_slice(a_mid, left.len), right.sub_slice(b_mid, right.len), dest.sub_slice(a_mid+b_mid, dest.len));
}

fn merge_seq_unsafe(left: MutBuf, right: MutBuf, dest: MutBuf) {
    let left = left.as_slice();
    let right = right.as_slice();
    let dest = dest.as_mut_slice();
    
    if right.is_empty() {
        dest.copy_from_slice(left);
        return;
    }

    let mut i = 0; // index for src_left
    let mut j = 0; // index for src_right
    let mut k = 0; // index for dest

    let left_len = left.len();
    let right_len = right.len();

    while i < left_len && j < right_len {
        let l = left[i];
        let r = right[j];
        if l <= r {
            dest[k] = l;
            i += 1;
        } else {
            dest[k] = r;
            j += 1;
        }
        k += 1;
    }

    if i < left_len {
        dest[k..].copy_from_slice(&left[i..]);
    } else if j < right_len {
        dest[k..].copy_from_slice(&right[j..]);
    }
}

#[cfg(not(feature = "test_direct_rec"))]
#[spawnable]
fn spawn_sort_unsafe(src: MutBuf, buf: MutBuf, usebuf: bool) {
    let buf_left;
    let buf_right;
    let left;
    let right;
    if usebuf && src.len <= SORT_CHUNK {
        src.as_mut_slice().sort();
        return;
    } else {
        let mid = src.len/2;
        left = src.sub_slice(0, mid);
        right = src.sub_slice(mid, src.len);
        buf_left = buf.sub_slice(0, mid);
        buf_right = buf.sub_slice(mid, buf.len);
        spawn_sort_unsafe(left, buf_left, !usebuf);
        spawn_sort_unsafe(right, buf_right, !usebuf);
    }

    if usebuf {
        spawn_merge_unsafe(__worker__, buf_left, buf_right, src);
    } else {
        spawn_merge_unsafe(__worker__, left, right, buf);
    }
}

#[cfg(not(feature = "test_direct_rec"))]
#[spawnable]
fn spawn_merge_unsafe(mut left: MutBuf, mut right: MutBuf, dest: MutBuf) {
    if dest.len <= MERGE_CHUNK {
        merge_seq_unsafe(left, right, dest);
        return;
    }

    // let 'left' be the larger of the two sub-arrays
    if left.len < right.len { 
        let tmp_left = left;
        left = right;
        right = tmp_left;
    };

    // find the middle element of left, and use std's binary_search to find suitable index in right
    let a_mid = left.len / 2;
    let b_mid = match right.as_slice().binary_search(&left.read(a_mid)) {
        Ok(i) => i,
        Err(i) => i,
    };

    // recurse, splitting at the newly found 'mid' indexes (in the sense of value, not array size)
    spawn_merge_unsafe(left.sub_slice(0, a_mid), right.sub_slice(0, b_mid), dest.sub_slice(0, a_mid+b_mid));
    spawn_merge_unsafe(left.sub_slice(a_mid, left.len), right.sub_slice(b_mid, right.len), dest.sub_slice(a_mid+b_mid, dest.len));
}

// ---------------------- FOR CHECKING EFFECT OF DIRECT RECURSION ----------
#[cfg(feature = "test_direct_rec")]
pub(crate) enum __Frame__ {
    Stolen(std::sync::Arc<std::sync::Mutex<Option<__Frame__>>>),
    InputSpawnSortUnsafe(usize, crate::MutBuf, crate::MutBuf, bool),
    InputSpawnMergeUnsafe(usize, crate::MutBuf, crate::MutBuf, crate::MutBuf),
}
#[cfg(feature = "test_direct_rec")]
impl velvet::Identifiable for __Frame__ {
    fn get_id(&self) -> usize {
        if let __Frame__::InputSpawnSortUnsafe(uid, ..) = self {
            return *uid;
        }
        if let __Frame__::InputSpawnMergeUnsafe(uid, ..) = self {
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
                __Frame__::InputSpawnSortUnsafe(_, a0, a1, a2) => {
                    crate::spawn_sort_unsafe(worker, a0, a1, a2);
                    *lock = None;
                }
                __Frame__::InputSpawnMergeUnsafe(_, a0, a1, a2) => {
                    crate::spawn_merge_unsafe(worker, a0, a1, a2);
                    *lock = None;
                }
                _ => panic!("WRONG STOLEN WORK FRAME!"),
            }
            return;
        }
        n = (n + 1) % len;
    }
}

#[cfg(feature = "test_direct_rec")]
fn test_direct_recursion_unsafe (mut arr: Vec<i32>, seed: usize, num_workers: usize) {
    let mut __root__worker__ = velvet::VelvetWorker::prepare_workers(
        num_workers,
        64usize,
        crate::__velvet_steal__,
    );
    __root__worker__.wait();
    let n = arr.len();
    let mut buf = vec![0;n];

    let vec = MutBuf::from_slice(&mut arr);
    let buf = MutBuf::from_slice(&mut buf);

    let start = Instant::now();
    spawn_sort_unsafe(&mut __root__worker__, vec.sub_slice(0, n), buf, true);
    let end = start.elapsed();

    let version = 8;
    println!("{},{},{},{},{},{}", version, num_workers, SORT_CHUNK, arr.len(), seed, end.as_secs_f32());
    _check(vec.as_slice().to_vec());
}

#[cfg(feature = "test_direct_rec")]
fn spawn_sort_unsafe(
    __worker__: &mut velvet::VelvetWorker<crate::__Frame__>,
    src: MutBuf,
    buf: MutBuf,
    usebuf: bool,
) {
    let buf_left;
    let buf_right;
    let left;
    let right;
    if usebuf && src.len <= SORT_CHUNK {
        src.as_mut_slice().sort();
        return;
    } else {
        let mid = src.len / 2;
        left = src.sub_slice(0, mid);
        right = src.sub_slice(mid, src.len);
        buf_left = buf.sub_slice(0, mid);
        buf_right = buf.sub_slice(mid, buf.len);
        let __0__ = __worker__.get_seq();
        __worker__
            .spawn(
                crate::__Frame__::InputSpawnSortUnsafe(__0__, left, buf_left, !usebuf),
            );
        let __1__ = __worker__.get_seq();
         __worker__
            .spawn(
                crate::__Frame__::InputSpawnSortUnsafe(__1__,right, buf_right, !usebuf),
            );
        let __SYNC__ = __worker__.sync(__1__);
        let __SYNC_RES__ = match __SYNC__ {
            crate::__Frame__::InputSpawnSortUnsafe(_, a0, a1, a2) => {
                spawn_sort_unsafe(__worker__, a0, a1, a2)
            }
            crate::__Frame__::Stolen(ptr) => {
                let mut try_lock = ptr.try_lock();
                loop {
                    if let Ok(mut _value) = try_lock {
                        break;
                    } else {
                        __worker__.steal();
                        try_lock = ptr.try_lock();
                    }
                }
            }
            _ => {
                panic!("WRONG FRAME POPPED!");
            }
        };
        let __SYNC__ = __worker__.sync(__0__);
        let __SYNC_RES__ = match __SYNC__ {
            crate::__Frame__::InputSpawnSortUnsafe(_, a0, a1, a2) => {
                spawn_sort_unsafe(__worker__, a0, a1, a2)
            }
            crate::__Frame__::Stolen(ptr) => {
                let mut try_lock = ptr.try_lock();
                loop {
                    if let Ok(mut _value) = try_lock {
                        break;
                    } else {
                        __worker__.steal();
                        try_lock = ptr.try_lock();
                    }
                }
            }
            _ => {
                panic!("WRONG FRAME POPPED!");
            }
        };
    }
    if usebuf {
        spawn_merge_unsafe(__worker__, buf_left, buf_right, src);
    } else {
        spawn_merge_unsafe(__worker__, left, right, buf);
    }
}
#[cfg(feature = "test_direct_rec")]
fn spawn_merge_unsafe(
    __worker__: &mut velvet::VelvetWorker<crate::__Frame__>,
    mut left: MutBuf,
    mut right: MutBuf,
    dest: MutBuf,
) {
    if dest.len <= MERGE_CHUNK {
        merge_seq_unsafe(left, right, dest);
        return;
    }
    if left.len < right.len {
        let tmp_left = left;
        left = right;
        right = tmp_left;
    }
    let a_mid = left.len / 2;
    let b_mid = match right.as_slice().binary_search(&left.read(a_mid)) {
        Ok(i) => i,
        Err(i) => i,
    };
    let __0__ = __worker__.get_seq();
    __worker__
        .spawn(
            crate::__Frame__::InputSpawnMergeUnsafe(
                __0__,
                left.sub_slice(0, a_mid),
                right.sub_slice(0, b_mid),
                dest.sub_slice(0, a_mid + b_mid),
            ),
        );
    let __1__ = __worker__.get_seq();
    __worker__
        .spawn(
            crate::__Frame__::InputSpawnMergeUnsafe(
                __1__,
                left.sub_slice(a_mid, left.len),
                right.sub_slice(b_mid, right.len),
                dest.sub_slice(a_mid + b_mid, dest.len),
            ),
        );
    let __SYNC__ = __worker__.sync(__1__);
    let __SYNC_RES__ = match __SYNC__ {
        crate::__Frame__::InputSpawnMergeUnsafe(_, a0, a1, a2) => {
            spawn_merge_unsafe(__worker__, a0, a1, a2)
        }
        crate::__Frame__::Stolen(ptr) => {
            let mut try_lock = ptr.try_lock();
            loop {
                if let Ok(mut _value) = try_lock {
                    break;
                } else {
                    __worker__.steal();
                    try_lock = ptr.try_lock();
                }
            }
        }
        _ => {
            panic!("WRONG FRAME POPPED!");
        }
    };
    let __SYNC__ = __worker__.sync(__0__);
    let __SYNC_RES__ = match __SYNC__ {
        crate::__Frame__::InputSpawnMergeUnsafe(_, a0, a1, a2) => {
            spawn_merge_unsafe(__worker__, a0, a1, a2)
        }
        crate::__Frame__::Stolen(ptr) => {
            let mut try_lock = ptr.try_lock();
            loop {
                if let Ok(mut _value) = try_lock {
                    break;
                } else {
                    __worker__.steal();
                    try_lock = ptr.try_lock();
                }
            }
        }
        _ => {
            panic!("WRONG FRAME POPPED!");
        }
    };
}