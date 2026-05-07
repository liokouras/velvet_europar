#![allow(non_snake_case)]
use velvet::prelude::*;
#[cfg(feature = "rayon")]
use rayon::prelude::*;

use std::{fs::File, io::{self, BufRead, Write}, sync::{Arc, RwLock}, time::{Duration, Instant}};
#[allow(unused_imports)]
use super::{par_body::Body, LEAF_CAP, par_tree::{BHTree, BODIES, TreeNode}, quad::Quadrant};

pub(crate) static THRESHOLD: usize = parse_threshold();
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
        500
    }
}

// read input and make bodies
pub(super) fn read_input_par(input_file: &str) -> io::Result<(Vec<RwLock<Body>>, Vec<Body>, f64)> {
    let file = File::open(input_file)?;
    let mut lines = io::BufReader::new(file).lines();

    let n: usize = lines.next().unwrap()?.parse().expect("Invalid number of particles");
    let radius: f64 = lines.next().unwrap()?.parse().expect("Invalid radius of universe");

    let mut locked_bodies: Vec<RwLock<Body>> = Vec::with_capacity(n);
    let mut raw_bodies: Vec<Body> = Vec::with_capacity(n);
    for _ in 0..n {
        if let Some(Ok(line)) = lines.next() {
            let mut iter = line.split_whitespace();
            let id = iter.next().unwrap().parse().expect("Invalid id");
            let px = iter.next().unwrap().parse().expect("Invalid px");
            let py = iter.next().unwrap().parse().expect("Invalid py");
            let vx = iter.next().unwrap().parse().expect("Invalid vx");
            let vy = iter.next().unwrap().parse().expect("Invalid vy");
            let mass = iter.next().unwrap().parse().expect("Invalid mass");
            locked_bodies.push(RwLock::new(Body::new(id, mass, px, py, vx, vy)));
            raw_bodies.push(Body::new(id, mass, px, py, vx, vy));
        }
    }

    Ok((locked_bodies, raw_bodies, radius))
}

// write output
pub(super) fn _write_output_par(output_file: &str, radius: f64, bodies: &Vec<RwLock<Body>>) -> io::Result<()>  {
    let mut writer = File::create(output_file)?;
    writeln!(writer, "{}", radius)?;
    for body in bodies.iter() {
        let body = body.read().unwrap();
        writeln!(writer, "{} {} {}", body.id(), body.px(), body.py())?;
    }
    Ok(())
}

pub(super) fn extract_bodies(bodies: &Vec<RwLock<Body>>) -> Vec<Body> {
    let mut raw_bodies: Vec<Body> = Vec::with_capacity(bodies.len());
    for body in bodies {
        let body = body.read().unwrap();
        raw_bodies.push(body.clone());
    }
    raw_bodies
}

pub(crate) fn par_main(input_file: &str, _output_file: &str, num_iterations: u32) {
    // pin thread!
    let core_ids = core_affinity::get_core_ids().unwrap();
    let res = core_affinity::set_for_current(core_ids[0]);
    if !res {
        eprintln!("Could not pin Root thread id continuing without pinning...");
    }
    
    let dt: f64 = 0.1; // time quantum

    let (locked_bodies, mut raw_bodies, radius) = read_input_par(input_file).unwrap();
    BODIES.set(locked_bodies).unwrap();

    // run simulation
    let mut duration_tree = Duration::default();
    let mut duration_update_forces = Duration::default();
    let mut duration_update_bodies = Duration::default();
    let start = Instant::now();
    let quad = Quadrant::new(0., 0., radius * 2.);
    for iter in 0..num_iterations {
        // build the Barnes-Hut tree
        let start_tree = Instant::now();
        let mut tree = BHTree::new(quad);
        if iter > 0 {
            raw_bodies = extract_bodies(BODIES.get().unwrap());
        }
        for body in &raw_bodies {
            if body.inside(&quad) {
                tree.insert((body.id(), body.mass(), body.px(), body.py()));
            }
        }
        let root = Arc::new(tree);
        duration_tree += start_tree.elapsed();

        // update the forces, positions, velocities, and accelerations
        let start_update_forces = Instant::now();
        root.traverse_update_force(root.clone());
        duration_update_forces += start_update_forces.elapsed();

        let start_update_bodies = Instant::now();
        for body in BODIES.get().unwrap().iter() {
            body.write().unwrap().update(dt);
        }
        duration_update_bodies += start_update_bodies.elapsed();
    }
    let duration = start.elapsed();
    println!("-1,1,{},0,{},{},{},{}", LEAF_CAP, duration.as_secs_f32(), duration_tree.as_secs_f32(), duration_update_forces.as_secs_f32(), duration_update_bodies.as_secs_f32());

    // let output_filename = _output_file.replace(".txt", "_par.txt");
    // _write_output_par(&output_filename, radius, BODIES.get().unwrap()).unwrap();
}

#[cfg(not(feature = "test_direct_rec"))]
#[velvet_main(traverse_spawn)]
pub(crate) fn velvet_main(input_file: &str, _output_file: &str, num_iterations: u32) {
    let dt: f64 = 0.1; // time quantum

    let (locked_bodies, mut raw_bodies, radius) = read_input_par(input_file).unwrap();
    BODIES.set(locked_bodies).unwrap();

    // run simulation
    let mut duration_tree = Duration::default();
    let mut duration_update_forces = Duration::default();
    let mut duration_update_bodies = Duration::default();
    let start = Instant::now();
    let quad = Quadrant::new(0., 0., radius * 2.);
    for iter in 0..num_iterations {
        // build the Barnes-Hut tree
        let start_tree = Instant::now();
        let mut tree = BHTree::new(quad);
        if iter > 0 {
            raw_bodies = extract_bodies(BODIES.get().unwrap());
        }
        for body in &raw_bodies {
            if body.inside(&quad) {
                tree.insert((body.id(), body.mass(), body.px(), body.py()));
            }
        }
        let root = Arc::new(tree);
        duration_tree += start_tree.elapsed();

        // update the forces, positions, velocities, and accelerations
        let start_update_forces = Instant::now();
        root.clone().traverse_spawn(root);
        duration_update_forces += start_update_forces.elapsed();

        let start_update_bodies = Instant::now();
        for body in BODIES.get().unwrap().iter() {
            body.write().unwrap().update(dt);
        }
        duration_update_bodies += start_update_bodies.elapsed();
    }
    let duration = start.elapsed();

    let version = match velvet_get_queue_name().as_str() {
        "safe" => 2,
        "unsafe" => 3,
        "crossbeam" => 4,
        _ => -1,
    };
    println!("{},{},{},{},{},{},{},{}", version, velvet_get_num_workers(), LEAF_CAP, THRESHOLD, duration.as_secs_f32(), duration_tree.as_secs_f32(), duration_update_forces.as_secs_f32(), duration_update_bodies.as_secs_f32());
    // let output_filename = _output_file.replace(".txt", "_velvet.txt");
    // _write_output_par(&output_filename, radius)
}

#[cfg(feature = "rayon")]
pub(crate) fn rayon_main(input_file: &str, _output_file: &str, num_iterations: u32, itertype: usize, num_threads: usize) {
    let dt: f64 = 0.1; // time quantum

    let (locked_bodies, mut raw_bodies, radius) = read_input_par(input_file).unwrap();
    BODIES.set(locked_bodies).unwrap();

    // DO RAYON SETUP
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

    // run simulation
    let mut duration_tree = Duration::default();
    let mut duration_update_forces = Duration::default();
    let mut duration_update_bodies = Duration::default();
    let start = Instant::now();
    let quad = Quadrant::new(0., 0., radius * 2.);
    for iter in 0..num_iterations {
        // build the Barnes-Hut tree
        let start_tree = Instant::now();
        let mut tree = BHTree::new(quad);
        if iter > 0 {
            raw_bodies = extract_bodies(BODIES.get().unwrap());
        }
        for body in &raw_bodies {
            if body.inside(&quad) {
                tree.insert((body.id(), body.mass(), body.px(), body.py()));
            }
        }
        let root = Arc::new(tree);
        duration_tree += start_tree.elapsed();

        if itertype == 1 { // tree iter
            // update the forces;
            let start_update_forces = Instant::now();
            let bodies = BODIES.get().unwrap();
            root.iter().par_bridge().for_each(
                |tree| {
                    if let TreeNode::Leaf(bodyvec) = &tree.body {
                        bodyvec.par_iter().for_each(|body| {
                            let (fx, fy) = root.compute_force_seq(body);
                            bodies[body.0].write().unwrap().set_force(fx, fy);
                        });                    
                    }
                }
            );
            duration_update_forces += start_update_forces.elapsed();
        } else if itertype == 5 { // par iter
             // update the forces;
            let start_update_forces = Instant::now();
            root.traverse_rayon(&root);
            duration_update_forces += start_update_forces.elapsed();
        } else if itertype == 6 {
            let start_update_forces = Instant::now();
            BODIES.get().unwrap().par_iter().for_each(
                |body| {
                    let mut body = body.write().unwrap(); // TODO: can do without locking?
                    let (fx, fy) = root.compute_force_seq(&(body.id(), body.mass(), body.px(), body.py()));
                    body.set_force(fx, fy);
                }
            );
            duration_update_forces += start_update_forces.elapsed();
        }

        // update the positions, velocities, and accelerations
        let start_update_bodies = Instant::now();
        for body in BODIES.get().unwrap() {
            body.write().unwrap().update(dt);
        }
        duration_update_bodies += start_update_bodies.elapsed();
    }
    let duration = start.elapsed();

    let _version = itertype;
    // #[cfg(feature = "pin_rayon")]
    // let _version = if itertype == 1 { 6 } else { 7 };

    println!("{},{},{},{},{},{},{},{}", _version, num_threads, LEAF_CAP, THRESHOLD, duration.as_secs_f32(), duration_tree.as_secs_f32(), duration_update_forces.as_secs_f32(), duration_update_bodies.as_secs_f32());

    // let output_filename = _output_file.replace(".txt", "_rayon.txt");
    // _write_output_par(&output_filename, radius)
}

#[cfg(feature = "rayon")]
pub(crate) fn rayon_iterative(input_file: &str, _output_file: &str, num_iterations: u32, itertype: usize, num_threads: usize) {
    let dt: f64 = 0.1; // time quantum

    let (_, mut raw_bodies, radius) = read_input_par(input_file).unwrap();

    // DO RAYON SETUP
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

    // run simulation
    let mut duration_tree = Duration::default();
    let mut duration_update_forces = Duration::default();
    let mut duration_update_bodies = Duration::default();
    let start = Instant::now();
    let quad = Quadrant::new(0., 0., radius * 2.);
    for _ in 0..num_iterations {
        // build the Barnes-Hut tree
        let start_tree = Instant::now();
        let mut tree = BHTree::new(quad);
        for body in &raw_bodies {
            if body.inside(&quad) {
                tree.insert((body.id(), body.mass(), body.px(), body.py()));
            }
        }
        duration_tree += start_tree.elapsed();

        let start_update_forces = Instant::now();
        raw_bodies.par_iter_mut().for_each(
            |body| {
                let (fx, fy) = tree.compute_force_seq(&(body.id(), body.mass(), body.px(), body.py()));
                body.set_force(fx, fy);
            }
        );
        duration_update_forces += start_update_forces.elapsed();

        // update the positions, velocities, and accelerations
        let start_update_bodies = Instant::now();
        for body in raw_bodies.iter_mut() {
            body.update(dt);
        }
        duration_update_bodies += start_update_bodies.elapsed();
    }
    let duration = start.elapsed();

    let _version = itertype;
    
    println!("{},{},{},{},{},{},{},{}", _version, num_threads, LEAF_CAP, THRESHOLD, duration.as_secs_f32(), duration_tree.as_secs_f32(), duration_update_forces.as_secs_f32(), duration_update_bodies.as_secs_f32());

    // let output_filename = _output_file.replace(".txt", "_rayon.txt");
    // _write_output_par(&output_filename, radius)
}

// FOR CHECKING EFFECT OF DIRECT RECURSION
#[cfg(feature = "test_direct_rec")]
pub(crate) fn test_direct_recursion(input_file: &str, _output_file: &str, num_iterations: u32, num_workers: usize) {
    let mut __root__worker__ = velvet::VelvetWorker::prepare_workers(num_workers, 64, crate::__velvet_steal__);
    __root__worker__.wait();
    let dt: f64 = 0.1;
    let (locked_bodies, mut raw_bodies, radius) = read_input_par(input_file)
        .unwrap();
    BODIES.set(locked_bodies).unwrap();
    let mut duration_tree = Duration::default();
    let mut duration_update_forces = Duration::default();
    let mut duration_update_bodies = Duration::default();
    let start = Instant::now();
    let quad = Quadrant::new(0., 0., radius * 2.);
    for iter in 0..num_iterations {
        let start_tree = Instant::now();
        let mut tree = BHTree::new(quad);
        if iter > 0 {
            raw_bodies = extract_bodies(BODIES.get().unwrap());
        }
        for body in &raw_bodies {
            if body.inside(&quad) {
                tree.insert((body.id(), body.mass(), body.px(), body.py()));
            }
        }
        let root = Arc::new(tree);
        duration_tree += start_tree.elapsed();
        let start_update_forces = Instant::now();
        root.clone().traverse_spawn(&mut __root__worker__, root);
        duration_update_forces += start_update_forces.elapsed();
        let start_update_bodies = Instant::now();
        for body in BODIES.get().unwrap().iter() {
            body.write().unwrap().update(dt);
        }
        duration_update_bodies += start_update_bodies.elapsed();
    }
    let duration = start.elapsed();
    let version = match velvet_get_queue_name().as_str() {
        "safe" => 7,
        "unsafe" => 8,
        "crossbeam" => 9,
        _ => -1,
    };

    let _threshold = THRESHOLD;
    println!("{},{},{},{},{},{},{},{}",
        version,
        num_workers,
        LEAF_CAP,
        _threshold,
        duration.as_secs_f32(),
        duration_tree.as_secs_f32(),
        duration_update_forces.as_secs_f32(),
        duration_update_bodies.as_secs_f32(),
    );
}