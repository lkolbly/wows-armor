#[macro_use]
extern crate derive_new;

use log::{info, debug};
use std::collections::HashMap;
use cgmath::{Vector3, Point3};
use std::time::{Instant};

mod shiplist;
mod download;
mod ballistics;
mod gun;
mod ship_parser;
use crate::shiplist::get_ship_list;
use crate::gun::*;
use crate::ballistics::Dispersion;
use crate::ship_parser::download_vehicle;

fn deg2rad(x: f64) -> f64 {
    x * 3.14159265 / 180.0
}

fn simulate_attack(gun: &Ammo, target: &ShipConfiguration, range: f64, azimuth: f64, offset: Point3<f64>) -> (f64, ImpactType) {
    let trajectory = gun.ballistics.calculate_flight_at_range(range);
    debug!("At range {}, calculated path {:?}", range, trajectory);
    let direction = Vector3::new(
        deg2rad(azimuth).cos() * deg2rad(trajectory.impact_angle).cos(),
        deg2rad(trajectory.impact_angle).sin(),
        deg2rad(azimuth).cos() * deg2rad(trajectory.impact_angle).cos(),
    );
    gun.bullet.compute_damage(target, trajectory.penetration, trajectory.velocity, direction, offset)
}

fn take_shot(dispersion: &Dispersion, gun: &Ammo, target: &ShipConfiguration, range: f64, azimuth: f64, offset: Point3<f64>) -> (f64, ImpactType) {
    let offset = offset + dispersion.generate_offset(azimuth, range);
    simulate_attack(gun, target, range, azimuth, offset)
}

fn volley(count: usize, dispersion: &Dispersion, gun: &Ammo, target: &ShipConfiguration, range: f64, azimuth: f64, offset: Point3<f64>) -> (f64, HashMap<ImpactType, usize>) {
    let mut map = HashMap::new();
    let mut total_damage = 0.0;
    for _ in 0..count {
        let (damage, t) = take_shot(dispersion, gun, target, range, azimuth, offset);
        total_damage += damage;
        *map.entry(t).or_insert(0) += 1;
    }
    (total_damage / count as f64, map)
}

fn main() {
    env_logger::init();
    //env_logger::from_env(env_logger::Env::default().default_filter_or("debug")).init();

    let vehicles = match std::fs::File::open("ships.dat") {
        Ok(f) => {
            bincode::deserialize_from(f).unwrap()
        }
        _ => {
            let ids = get_ship_list();
            let vehicles: Vec<_> = ids.iter().filter_map(|id| { download_vehicle(&id) }).collect();

            // Serialize all the found vehicles into a file
            {
                //let serialized = bincode::serialize(&vehicles).unwrap();
                let f = std::fs::File::create("ships.dat").unwrap();
                //f.write_all(&serialized[..]).unwrap();
                bincode::serialize_into(f, &vehicles).unwrap();
            }
            vehicles
        }
    };

    // Search the vehicles for a name
    let pensacola: Vec<_> = vehicles.iter().filter(|s| { s.name.contains("Pensacola") }).collect();
    let mut total_battles = 0;
    for vehicle in vehicles.iter() {
        if vehicle.can_battle_with(pensacola[0]) {
            total_battles += 1;
            info!("Can battle with {}", vehicle.name);
        }
    }
    info!("Pensacola can battle with {} ships", total_battles);

    /*for id in ids {
        download_vehicle(&id);
    }*/
    /*info!("{}", ids[0]);
    download_vehicle(&ids[0]);*/
    let mut total_battles = 0;
    for vehicle_a in vehicles.iter() {
        for vehicle_b in vehicles.iter() {
            if vehicle_a.can_battle_with(vehicle_b) {
                total_battles += 1;
            }
        }
    }
    info!("Found {} possible battles", total_battles);

    let dd = download_vehicle("pasd014").unwrap();
    let bb = download_vehicle("pasb006").unwrap();
    //download_vehicle("pjsb799");
    let x = simulate_attack(&dd.configurations[0].artillery[0].ammo[0], &bb.configurations[0], 10000.0, 30.0, Point3::new(0.0, 0.0, 0.0));
    info!("{:?}", x);
    let now = Instant::now();
    for i in 0..36 {
        let (damage, occurrences) = volley(100, &bb.configurations[0].artillery[0].dispersion, &bb.configurations[0].artillery[0].ammo[0], &bb.configurations[0], 10000.0, i as f64 * 10.0, Point3::new(0.0, 0.0, 0.0));
        info!("{} degrees: {} w/ {} misses/{} penetrations", i as f64 * 10.0, damage, occurrences.get(&ImpactType::Miss).unwrap_or(&0), occurrences.get(&ImpactType::Penetration).unwrap_or(&0));
    }
    info!("Computed 3600 shots in {:?}, {} shots/sec", now.elapsed(), 3600.0 / now.elapsed().as_secs_f64());
}
