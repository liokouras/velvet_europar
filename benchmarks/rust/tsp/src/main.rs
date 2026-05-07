#![allow(non_snake_case)]
use std::{env, fs::File, io::{BufWriter, Write}, time::Instant, sync::{atomic::{AtomicUsize, Ordering}, OnceLock}};
use rand::{SeedableRng, rngs::StdRng, distr::{Distribution, Uniform}};

use velvet::prelude::*;
#[cfg(not(feature = "test_direct_rec"))]
include!(concat!(env!("OUT_DIR"), "/velvet_app.rs"));

#[cfg(feature = "rayon")]
use rayon::prelude::*;

static DISTANCE:OnceLock<DistanceTable> = OnceLock::new();
static MINIMUM:AtomicUsize = AtomicUsize::new(usize::MAX);

const THRESHOLD: usize = parse_threshold();
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
        6
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 4 {
        println!("usage for tsp: cargo run [cargo_options] [velvet|seq|rayon] [number_of_towns] [random_seed]");
        println!("example: cargo run --release velvet 20 42");
        return;
    }
    let app = &args[1];
    let ntowns: usize = args[2].parse().unwrap();
    let seed: u64 = args[3].parse().unwrap();

    let distance = DistanceTable::generate(ntowns, seed);
    match DISTANCE.set(distance) {
        Err(_) => {
            println!("could not initialise global distance table variable. exiting...");
            return;
        },
        Ok(()) => (),
    }

    if app.eq("velvet") {
        #[cfg(not(feature = "test_direct_rec"))]
        velvet_main(ntowns, seed);
    } else if app.eq("rayon") {
        #[cfg(feature = "rayon")]
        {
            if args.len() < 5 {
                println!("must provide number of threads for Rayon");
                println!("usage: cargo run [cargo_options][velvet|seq|rayon] [number_of_towns] [random_seed] [number_of_threads]");
                println!("example: cargo run --release rayon 20 42 8");
                return;
            }
            let num_threads: usize = args[4].parse().unwrap();
            rayon_main(ntowns, seed, num_threads);
        }
        #[cfg(not(feature = "rayon"))]
        println!("COMPILE WITH RAYON!");
    } else if app.eq("test_direct") {
        #[cfg(feature = "test_direct_rec")]
        {
            if args.len() < 5 {
                println!("must provide number of workers!");
                return;
            }
            let num_workers: usize = args[4].parse().unwrap();
            test_direct_recursion(ntowns, seed, num_workers);
        }
        #[cfg(not(feature = "test_direct_rec"))]
        println!("Compile with feature \"test_direct_rec\"");
    } else if app.eq("gen_towns") {
        DistanceTable::generate_and_save(ntowns, seed).unwrap();
    } else {
        seq_main(ntowns, seed);
    }
}

fn seq_main(ntowns: usize, seed: u64) {
    // pin thread!
    let core_ids = core_affinity::get_core_ids().unwrap();
    let res = core_affinity::set_for_current(core_ids[0]);
    if !res {
        eprintln!("Could not pin Root thread id continuing without pinning...");
    }

    let path: u128 = 1u128; 
    let length = 0;

    let start = Instant::now();
    tsp_seq(1, 0, path, length);
    let elapsed = start.elapsed();

    println!("0,1,{},{},0,{}", ntowns, seed, elapsed.as_secs_f32());
    eprintln!("ORACLE = {}, in seq time = {}", MINIMUM.load(Ordering::Relaxed), elapsed.as_secs_f32());
}

#[cfg(not(feature = "test_direct_rec"))]
#[velvet_main(tsp_spawn)]
fn velvet_main(ntowns: usize, seed: u64) {
    let path: u128 = 1u128; 
    let length = 0;

    let start = Instant::now();
    tsp_spawn(1, 0, path, length);
    let elapsed = start.elapsed();

    let version = match velvet_get_queue_name().as_str() {
        "safe" => 2,
        "unsafe" => 3,
        "crossbeam" => 4,
        _ => -1,
    };

    println!("{},{},{},{},{},{}", version, velvet_get_num_workers(), ntowns, seed, THRESHOLD, elapsed.as_secs_f32());
    eprintln!("VELVET RESULT = {}, in parallel time = {}",  MINIMUM.load(Ordering::Relaxed), elapsed.as_secs_f32());
}

#[cfg(feature = "rayon")]
fn rayon_main(ntowns: usize, seed: u64, num_threads:usize) {
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

    let path: u128 = 1u128; 
    let length = 0;

    let start = Instant::now();
    tsp_rayon(1, 0, path, length);
    let end = start.elapsed();

    let _version = 1;
    
    println!("{},{},{},{},{},{}", _version, num_threads, ntowns, seed, THRESHOLD, end.as_secs_f32());
    eprintln!("RAYON RESULT = {}, in parallel time = {}", MINIMUM.load(Ordering::Relaxed), end.as_secs_f32());
}

fn tsp_seq(hops: usize, last: usize, path: u128, length: usize) {
    let distance = DISTANCE.get().unwrap();
    let ntowns = distance.ntowns;

    if length + distance.lower_bounds[ntowns - hops] >= MINIMUM.load(Ordering::Relaxed) {
        // stop searching, this path is too long...
        return;
    } else if hops == ntowns {
        // found a full route better than current best route,
        MINIMUM.store(length, Ordering::Relaxed);
        return;
    }
    
    // try all cities not on the path, in "nearest-city-first" order
    for i in 0..ntowns {
        let city = distance.to_city[last * ntowns +  i];
        let city_bit = 1u128 << city;

        if city != last && (path & city_bit) == 0 {
            let dist = distance.dist[last * ntowns + i];
            let new_path = path | city_bit; 
            tsp_seq(hops + 1, city, new_path, length + dist);
        }
    }
}

#[cfg(not(feature = "test_direct_rec"))]
#[spawnable]
fn tsp_spawn(hops: usize, last: usize, path: u128, length: usize) {
    let distance = DISTANCE.get().unwrap();
    let ntowns = distance.ntowns;

    if length + distance.lower_bounds[ntowns - hops] >= MINIMUM.load(Ordering::Relaxed) {
        // stop searching, this path is too long...
        return;
    } else if hops == ntowns {
        // found a full route better than current best route,
        MINIMUM.store(length, Ordering::Relaxed);
        return;
    }

    if hops > THRESHOLD {
        return tsp_seq(hops, last, path, length);
    }
    
    // try all cities not on the path, in "nearest-city-first" order
    for i in (0..ntowns).rev() {
        let city = distance.to_city[last * ntowns +  i];
        let city_bit = 1u128 << city;

        if city != last && (path & city_bit) == 0 {
            let dist = distance.dist[last * ntowns + i];
            let new_path = path | city_bit; 
            tsp_spawn(hops + 1, city, new_path, length + dist);
        }
    }
}

#[cfg(feature = "rayon")]
fn tsp_rayon (hops: usize, last: usize, path: u128, length: usize) {
    let distance = DISTANCE.get().unwrap();
    let ntowns = distance.ntowns;

    if length + distance.lower_bounds[ntowns - hops] >= MINIMUM.load(Ordering::Relaxed) {
        // stop searching, this path is too long...
        return;
    } else if hops == ntowns {
        // found a full route better than current best route,
        MINIMUM.store(length, Ordering::Relaxed);
        return;
    }

    if hops > THRESHOLD {
        return tsp_seq(hops, last, path, length);
    }

    // try all cities not on the path, in "nearest-city-first" order
    (0..ntowns).into_par_iter().for_each(|i| {
        let city = distance.to_city[last * ntowns +  i];
        let city_bit = 1u128 << city;
        
        if city != last && (path & city_bit) == 0 {
            let dist = distance.dist[last * ntowns + i];
            let new_path = path | city_bit; 
            tsp_rayon(hops + 1, city, new_path, length + dist);
        }
    });
}

struct Coord {
    x: usize,
    y: usize,
}

struct DistanceTable {
    ntowns: usize,
    lower_bounds: Vec<usize>,
    to_city: Vec<usize>,
    dist: Vec<usize>,
}

impl DistanceTable {
    fn generate(ntowns: usize, seed:u64) -> DistanceTable {
        let mut to_city = vec![0;ntowns * ntowns];
        let mut dist = vec![0;ntowns * ntowns];
        let mut lower_bounds =  vec![0;ntowns];

        let mut temp_dist = vec![0;ntowns];
        let mut towns = Vec::with_capacity(ntowns);
        let mut min_dists = vec![0;ntowns * ntowns];

        let mut dx;
        let mut dy;
        let mut x = 0;
        let mut min_dist_count = 0;
        let mut tmp;

        let range: Uniform<usize> = Uniform::try_from(0..100).unwrap();
        let mut rng = StdRng::seed_from_u64(seed); 
        for _ in 0..ntowns {
            let x = range.sample(&mut rng);
            let y = range.sample(&mut rng);
            towns.push(Coord{x, y}); 
        }

        for i in 0..ntowns {
            for j in 0..ntowns {
                dx = towns[i].x as i32 - towns[j].x as i32;
                dy = towns[i].y as i32 - towns[j].y as i32;
                let dist = (dx * dx + dy * dy).isqrt();
                temp_dist[j] = dist as usize;
                if i != j && temp_dist[j] != 0 {
                    min_dists[min_dist_count] = temp_dist[j];
                    min_dist_count += 1;
                }
            }

            // Sort pairs[i]: nearest city first.
            for j in 0..ntowns {
                tmp = usize::MAX;
                for k in 0..ntowns {
                    if temp_dist[k] < tmp {
                        tmp = temp_dist[k];
                        x = k;
                    }
                }
                temp_dist[x] = usize::MAX;
                to_city[i*ntowns + j] = x;
                dist[i*ntowns + j] = tmp;
            }
        }

        DistanceTable::sort(&mut min_dists);

        for i in 0..ntowns {
            lower_bounds[i] = DistanceTable::calc_lower_bound(i, &min_dists);
        }


        DistanceTable{ ntowns, lower_bounds, to_city, dist }
    }

    fn sort(vec: &mut Vec<usize>) {
        for i in 0..vec.len() {
            DistanceTable::put_min(vec, i);
        }
    }

    fn put_min(vec: &mut Vec<usize>, pos: usize) {
        let mut minpos = pos;
        let mut min = usize::MAX;
        for i in pos..vec.len() {
            if vec[i] == 0 {
                vec[i] = usize::MAX;
            }
            if vec[i] < min {
                minpos = i;
                min = vec[i];
            }
        }
        let tmp = vec[pos];
        vec[pos] = vec[minpos];
        vec[minpos] = tmp;
    }

    fn calc_lower_bound(hops: usize, table: &Vec<usize>) -> usize {
        let mut res = 0;
        for i in 0..hops {
            res += table[i] as usize;
        }
        res
    }

    fn generate_and_save(ntowns: usize, seed:u64) -> std::io::Result<()> {        
        let range: Uniform<usize> = Uniform::try_from(0..100).unwrap();
        let mut rng = StdRng::seed_from_u64(seed); 

        let filename = format!("../../data/dist_tab_{}_{}.txt", ntowns, seed);
        let file = File::create(&filename)?;
        let mut writer = BufWriter::new(file);

        for _ in 0..ntowns {
            let x = range.sample(&mut rng) as i32;
            let y = range.sample(&mut rng) as i32;
            writeln!(writer, "{} {}", x, y)?;
        }
        Ok(())
    }
}

// ---------------------- FOR CHECKING EFFECT OF DIRECT RECURSION ----------
#[cfg(feature = "test_direct_rec")]
pub(crate) enum __Frame__ {
    Stolen(std::sync::Arc<std::sync::Mutex<Option<__Frame__>>>),
    InputTspSpawn(usize, usize, usize, u128, usize),
}
#[cfg(feature = "test_direct_rec")]
impl velvet::Identifiable for __Frame__ {
    fn get_id(&self) -> usize {
        if let __Frame__::InputTspSpawn(uid, ..) = self {
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
                __Frame__::InputTspSpawn(_, a0, a1, a2, a3) => {
                    tsp_spawn(worker, a0, a1, a2, a3);
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
fn test_direct_recursion(ntowns: usize, seed: u64, num_workers: usize) {
    let mut __root__worker__ = velvet::VelvetWorker::prepare_workers(num_workers, 64, __velvet_steal__);
    __root__worker__.wait();
    let path: u128 = 1u128;
    let length = 0;
    let start = Instant::now();
    tsp_spawn(&mut __root__worker__, 1, 0, path, length);
    let elapsed = start.elapsed();
    let version = match velvet_get_queue_name().as_str() {
        "safe" => 7,
        "unsafe" => -1,
        "crossbeam" => -1,
        _ => -1,
    };
    let _threshold = THRESHOLD;
    println!("{},{},{},{},{},{}", version, num_workers, ntowns, seed, _threshold, elapsed.as_secs_f32());
    eprintln!("VELVET RESULT WITHOUT DIRECT RECURSION OPT = {}, in parallel time = {}",  MINIMUM.load(Ordering::Relaxed), elapsed.as_secs_f32());
}
#[cfg(feature = "test_direct_rec")]
fn tsp_spawn(
    __worker__: &mut velvet::VelvetWorker<crate::__Frame__>,
    hops: usize,
    last: usize,
    path: u128,
    length: usize,
) {
    let mut __checkpoint__ = __worker__.get_seq();
    let mut __count__ = 0;
    let distance = DISTANCE.get().unwrap();
    let ntowns = distance.ntowns;
    if length + distance.lower_bounds[ntowns - hops] >= MINIMUM.load(Ordering::Relaxed) {
        return;
    } else if hops == ntowns {
        MINIMUM.store(length, Ordering::Relaxed);
        return;
    }
    if hops > THRESHOLD {
        return tsp_seq(hops, last, path, length);
    }
    for i in (1..ntowns).rev() {
        let city = distance.to_city[last * ntowns + i];
        let city_bit = 1u128 << city;
        if city != last && (path & city_bit) == 0 {
            let dist = distance.dist[last * ntowns + i];
            let new_path = path | city_bit;
            let __uid__ = __worker__.get_seq();
            __worker__
                .spawn(
                    crate::__Frame__::InputTspSpawn(
                        __uid__,
                        hops + 1,
                        city,
                        new_path,
                        length + dist,
                    ),
                );
            __count__ += 1;
        }
    }
    let i = 0;
    let city = distance.to_city[last * ntowns + i];
    let city_bit = 1u128 << city;
    if city != last && (path & city_bit) == 0 {
        let dist = distance.dist[last * ntowns + i];
        let new_path = path | city_bit;
        tsp_spawn(__worker__, hops + 1, city, new_path, length + dist);
    }

    while __count__ > 0 {
        let __SYNC__ = __worker__.sync(__checkpoint__ + __count__);
        let __SYNC_RES__ = match __SYNC__ {
            __Frame__::InputTspSpawn(_, a0, a1, a2, a3) => {
                tsp_spawn(__worker__, a0, a1, a2, a3)
            }
            __Frame__::Stolen(ptr) => {
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
        };
        __count__ -= 1;
    }
}