use crate::ballistics::{Dispersion, Ballistics};

use serde_derive::{Serialize, Deserialize};
use cgmath::{Vector3, Point3};
use cgmath::prelude::*;
use log::{debug, trace};
use rand::Rng;



fn deg2rad(x: f64) -> f64 {
    x * 3.14159265 / 180.0
}

struct ImpactPath<'a> {
    mesh: &'a Vec<ArmorFace>,
    position: Point3<f64>,
    direction: Vector3<f64>,
    reflected_dir: Vector3<f64>,
}

impl<'a> ImpactPath<'a> {
    pub fn new(target: &'a ShipConfiguration, direction: Vector3<f64>, offset: Point3<f64>) -> Option<(ImpactPath<'a>, ArmorFace, Intersection)> {
        let mut ip = ImpactPath {
            mesh: &target.geometry,
            position: offset - 1000.0 * direction,
            direction: direction,
            reflected_dir: direction, // Unused
        };
        //info!("{:?}", ip.direction);
        let (armorface,first) = ip.next_intersection()?;
        Some((
            ImpactPath {
                mesh: &target.geometry,
                position: first.intersect_point,
                direction: direction,
                reflected_dir: armorface.reflect(&direction),
            },
            armorface,
            first,
        ))
    }

    /// Marks that the previous impact was a ricochet, and returns the next
    /// impact.
    pub fn ricochet(&mut self) -> Option<(ArmorFace, Intersection)> {
        self.direction = self.reflected_dir;
        let (face, intersection) = self.next_intersection()?;
        self.position = intersection.intersect_point;
        self.reflected_dir = face.reflect(&self.direction);
        Some((face, intersection))
    }

    pub fn penetrate(&mut self) -> Option<(ArmorFace, Intersection)> {
        let (face, intersection) = self.next_intersection()?;
        self.position = intersection.intersect_point;
        self.reflected_dir = face.reflect(&self.direction);
        Some((face, intersection))
    }

    fn next_intersection(&mut self) -> Option<(ArmorFace, Intersection)> {
        self.mesh.iter()
            .filter_map(|face| {
                let intersection =
                    face.intersect(self.position, self.direction)?;
                Some((face, intersection))
            })
            .filter(|(_, i)| { i.t > 0.00001 })
            .min_by(|(_, a), (_, b)| {
                a.t.partial_cmp(&b.t).unwrap()
            })
            .map(|(face, i)| { ((*face).clone(), (i).clone()) })
    }
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub enum ImpactType {
    Miss,
    NonPenetration,
    Citadel,
    Penetration,
    TorpedoProtection,
    Ricochet,
    OverPenetration,
}

pub trait Bullet {
    fn compute_damage(&self, target: &ShipConfiguration, penetration: f64, speed: f64, direction: Vector3<f64>, offset: Point3<f64>) -> (f64, ImpactType);
}

#[derive(new, Serialize, Deserialize)]
pub struct HeAmmo {
    damage: f64,
    piercing: f64,
}

impl Bullet for HeAmmo {
    fn compute_damage(&self, target: &ShipConfiguration, _penetration: f64, _speed: f64, direction: Vector3<f64>, offset: Point3<f64>) -> (f64, ImpactType) {
        debug!("Computing damage for HE ammo");
        let (_, armorface, intersection) = match ImpactPath::new(target, direction, offset) {
            Some(x) => { x }
            None => {
                debug!("Trajectory was a miss!");
                return (0.0, ImpactType::Miss);
            }
        };
        trace!("First impact: {:?}, {}mm", intersection.intersect_point, armorface.thickness);
        debug!("Impacted {:?}", armorface.armor_type);
        if armorface.thickness > self.piercing {
            debug!("Non-penetration");
            return (0.0, ImpactType::NonPenetration);
        }
        if armorface.armor_type == ArmorType::Citadel {
            debug!("Citadel hit!");
            return (self.damage / 3.0, ImpactType::Citadel);
        }
        return (self.damage / 3.0, ImpactType::Penetration);
    }
}

#[derive(new, Serialize, Deserialize)]
pub struct ApAmmo {
    diameter: f64,
    damage: f64,
    detonator: f64,
    detonator_threshold: f64,
}

impl Bullet for ApAmmo {
    fn compute_damage(&self, target: &ShipConfiguration, penetration: f64, speed: f64, direction: Vector3<f64>, offset: Point3<f64>) -> (f64, ImpactType) {
        let mut penetration = penetration;
        debug!("Computing damage for AP ammo");
        let (mut path, mut armorface, mut intersection) = match ImpactPath::new(target, direction, offset) {
            Some(x) => { x }
            None => {
                debug!("Trajectory was a miss!");
                return (0.0, ImpactType::Miss);
            }
        };
        debug!("Impacted {:?}", armorface.armor_type);

        let mut citadel_count = 0;
        let mut last_pos: Option<Point3<f64>> = None;
        let mut detonator_distance = None;

        let compute_dm = |armorface: ArmorFace, citadel_count| {
            if citadel_count % 2 == 1 {
                return (1.0 * self.damage, ImpactType::Citadel);
            }
            if armorface.armor_type == ArmorType::TorpedoProtectionBelt {
                return (0.0, ImpactType::TorpedoProtection);
            }
            (0.3333 * self.damage, ImpactType::Penetration)
        };

        loop {
            if armorface.armor_type == ArmorType::Citadel {
                citadel_count += 1;
            }

            if let Some(last_pos) = last_pos {
                if detonator_distance != None {
                    // Count down the detonator
                    let distance = (intersection.intersect_point - last_pos).magnitude();
                    detonator_distance = Some(detonator_distance.unwrap() - distance);
                    if detonator_distance.unwrap() < 0.0 {
                        trace!("Detonating due to detonator");
                        return compute_dm(armorface, citadel_count);
                    }
                }
            }

            let ricochet = if armorface.thickness < self.diameter / 14.3 {
                false
            } else if intersection.angle < 30.0 {
                true
            } else if intersection.angle < 45.0 {
                let probability = (intersection.angle - 30.0) / 15.0;
                let mut rng = rand::thread_rng();
                rng.gen::<f64>() < probability
            } else {
                false
            };

            if ricochet {
                let x = match path.ricochet() {
                    Some(x) => x,
                    None => {
                        return (0.0, ImpactType::Ricochet);
                    }
                };
                armorface = x.0;
                intersection = x.1;
            } else {
                // Thickness normalization
                let angle = if 0.0 > intersection.angle - 6.0 { 0.0 } else { intersection.angle - 6.0 };
                let normalized_thickness = armorface.thickness / deg2rad(90.0 - angle).cos();

                penetration -= normalized_thickness;
                if penetration < 0.0 {
                    // Explodes!
                    if last_pos == None {
                        // Non-penetration
                        return (0.0, ImpactType::NonPenetration);
                    }
                    return compute_dm(armorface, citadel_count);
                } else if normalized_thickness > self.detonator_threshold {
                    // Start the timer
                    detonator_distance = Some(speed * self.detonator);
                }
                let x = match path.penetrate() {
                    Some(x) => x,
                    None => {
                        return (0.1 * self.damage, ImpactType::OverPenetration);
                    }
                };
                armorface = x.0;
                intersection = x.1;
            }
            last_pos = Some(intersection.intersect_point);
        }
    }
}

#[derive(Serialize, Deserialize)]
pub enum AmmoType {
    He(HeAmmo),
    Ap(ApAmmo),
}

impl Bullet for AmmoType {
    fn compute_damage(&self, target: &ShipConfiguration, penetration: f64, speed: f64, direction: Vector3<f64>, offset: Point3<f64>) -> (f64, ImpactType) {
        match self {
            AmmoType::He(he) => { he.compute_damage(target, penetration, speed, direction, offset) }
            AmmoType::Ap(ap) => { ap.compute_damage(target, penetration, speed, direction, offset) }
        }
    }
}

#[derive(new, Serialize, Deserialize)]
pub struct Ammo {
    pub bullet: AmmoType,
    pub ballistics: Ballistics,
}

#[derive(new, Serialize, Deserialize)]
pub struct Gun {
    pub dispersion: Dispersion,
    pub ammo: Vec<Ammo>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ArmorType {
    Normal,
    Citadel,
    TorpedoProtectionBelt,
}

impl ArmorType {
    pub fn from_id(id: usize) -> ArmorType {
        if id >= 59 && id <= 67 {
            ArmorType::Citadel
        } else if id == 101 {
            ArmorType::TorpedoProtectionBelt
        } else {
            ArmorType::Normal
        }
    }
}

#[derive(new, Clone, Serialize, Deserialize)]
pub struct ArmorFace {
    pub vertices: [Point3<f64>; 3],
    pub thickness: f64,
    pub armor_type: ArmorType,
}

#[derive(Clone)]
pub struct Intersection {
    pub t: f64,
    pub angle: f64,
    pub intersect_point: Point3<f64>,
}

impl ArmorFace {
    pub fn normal(&self) -> Vector3<f64> {
        (self.vertices[1] - self.vertices[0]).cross(self.vertices[2] - self.vertices[0]).normalize()
    }

    pub fn reflect(&self, other: &Vector3<f64>) -> Vector3<f64> {
        let v = (Vector3::new(0.0, 0.0, 0.0) - other).normalize();
        let sign = if cgmath::dot(v, self.normal()) < 0.0 {
            -1.0
        } else {
            1.0
        };
        let n = sign * self.normal();
        let scale = 2.0 * cgmath::dot(v, n);
        let reflected = v - scale * n;
        reflected.normalize()
    }

    pub fn intersect(&self, origin: Point3<f64>, direction: Vector3<f64>) -> Option<Intersection> {
        let edge1 = self.vertices[1] - self.vertices[0];
        let edge2 = self.vertices[2] - self.vertices[0];
        let h = direction.cross(edge2);
        let a = cgmath::dot(edge1, h);
        if a.abs() < 0.00001 {
            // Ray parallel to triangle
            return None;
        }

        let f = 1.0 / a;
        let s = origin - self.vertices[0];
        let u = f * cgmath::dot(s, h);
        if u < 0.0 || u > 1.0 {
            return None;
        }

        let q = s.cross(edge1);
        let v = f * cgmath::dot(direction, q);
        if v < 0.0 || u + v > 1.0 {
            return None;
        }

        let t = f * cgmath::dot(edge2, q);

        let a = cgmath::dot(self.normal(), direction);
        let angle = 180.0 / 3.14159265 * (a / direction.magnitude()).acos();
        let angle = {
            if angle < 0.0 {
                -angle
            } else if angle > 90.0 {
                180.0 - angle
            } else {
                angle
            }
        };
        let angle = 90.0 - angle;
        Some(Intersection {
            t: t,
            angle: angle,
            intersect_point: origin + direction * t,
        })
    }
}

#[derive(new, Serialize, Deserialize)]
pub struct ShipConfiguration {
    pub artillery: Vec<Gun>,
    pub geometry: Vec<ArmorFace>,
    pub speed: f64, // m/s
    pub length: f64, // m
    pub name: String,
}

#[derive(Serialize, Deserialize)]
pub enum ShipClass {
    Destroyer,
    Cruiser,
    Battleship,
    AircraftCarrier,
}

#[derive(new, Serialize, Deserialize)]
pub struct Ship {
    pub configurations: Vec<ShipConfiguration>,
    tier: usize,
    pub name: String,
    pub class: ShipClass,
}

impl Ship {
    pub fn can_battle_with(&self, other: &Ship) -> bool {
        let tiers = [
            vec![1],
            vec![2, 3],
            vec![3, 4],
            vec![4, 5],
            vec![5, 6, 7],
            vec![6, 7, 8],
            vec![7, 8, 9],
            vec![8, 9, 10],
            vec![9, 10],
            vec![10],
        ];
        if self.tier == 0 || self.tier > 10 || other.tier == 0 || other.tier > 10 {
            return false;
        }
        let b_battle_tiers = &tiers[other.tier - 1];
        for battle_tier in tiers[self.tier - 1].iter() {
            if b_battle_tiers.contains(battle_tier) {
                return true;
            }
        }
        return false;
    }
}
