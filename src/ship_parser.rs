use log::{error, warn, debug, trace};
use serde_json::Value;
use serde_json::map::Map;

use crate::ballistics::{Ballistics, Dispersion};
use crate::gun::*;
use crate::download::{download, download_with_params};

use serde_derive::Deserialize;
use std::collections::HashMap;
use cgmath::{Matrix4, Point3};
use std::io::prelude::*;
use std::convert::TryInto;

fn parse_ballistics(ammo: &Map<String, Value>) -> Ballistics {
    Ballistics::new(
        ammo["bulletMass"].as_f64().unwrap(),
        ammo["bulletDiametr"].as_f64().unwrap(),
        ammo["bulletSpeed"].as_f64().unwrap(),
        ammo["bulletAirDrag"].as_f64().unwrap(),
        ammo["bulletKrupp"].as_f64().unwrap()
    )
}

fn parse_ammotype(ammo: &Map<String, Value>) -> Ammo {
    let ammotype = ammo["ammoType"].as_str().expect("Couldn't find ammoType");
    debug!("Found ammo of type {}", ammotype);
    let ballistics = parse_ballistics(ammo);
    if ammotype == "HE" {
        Ammo::new(
            AmmoType::He(HeAmmo::new(
                ammo["alphaDamage"].as_f64().expect("Couldn't find alphaDamage"),
                ammo["alphaPiercingHE"].as_f64().expect("Couldn't find alphaPiercingHE"),
            )),
            ballistics,
        )
    } else if ammotype == "AP" {
        Ammo::new(
            AmmoType::Ap(ApAmmo::new(
                ammo["bulletDiametr"].as_f64().expect("Couldn't find bulletDiametr"),
                ammo["alphaDamage"].as_f64().expect("Couldn't find alphaDamage"),
                ammo["bulletDetonator"].as_f64().expect("Couldn't find bulletDetonator"),
                ammo["bulletDetonatorThreshold"].as_f64().expect("Couldn't find bulletDetonatorThreshold"),
            )),
            ballistics,
        )
    } else if ammotype == "CS" {
        warn!("Found unimplemented ammo type CS!");
        Ammo::new(
            AmmoType::He(HeAmmo::new(1.0, 1.0)), ballistics)
    } else {
        error!("Found unknown ammo type {}!", ammotype);
        panic!()
    }
}

fn parse_artillery(artillery_spec: &Map<String, Value>) -> Vec<Gun> {
    //debug!("{:#?}", artillery_spec);
    let guns = artillery_spec["guns"].as_object().unwrap();
    /*for (key,gun) in guns {
        debug!("{}: {:?}", key, gun);
}*/
    let dispersion = Dispersion::new(
        artillery_spec["minDistH"].as_f64().expect("Couldn't find horizontal"),
        artillery_spec["minDistV"].as_f64().expect("Couldn't find vertical"),
        artillery_spec["maxDist"].as_f64().expect("Couldn't find maxrange"),
        artillery_spec["sigmaCount"].as_f64().expect("Couldn't find sigmaCount")
    );
    guns.iter().map(|(_, gun)| {
        let ammo_list = gun["ammoList"].as_object().expect("Couldn't get ammoList");
        //debug!("{}: {:#?}", key, gun);
        let ammo: Vec<_> = ammo_list.iter().map(|(_, ammo)| {
            parse_ammotype(ammo.as_object().unwrap())
        }).collect();
        Gun::new(
            dispersion.clone(),
            ammo,
        )
    }).collect()
}

#[derive(Deserialize)]
struct ArmorMaterial {
    #[serde(alias = "type")]
    armor_type: usize,
    thickness: usize, // mm
}

#[derive(Deserialize)]
struct ArmorGroup {
    material: String,
    indices: Vec<usize>,
}

#[derive(Deserialize)]
struct ArmorObject {
    vertices: Vec<f64>,
    groups: Vec<ArmorGroup>,
}

#[derive(Deserialize)]
struct GeometryObject {
    armor: ArmorObject,
}

#[derive(Deserialize)]
struct RawGeometry {
    objects: GeometryObject,
    materials: HashMap<String, ArmorMaterial>,
}

impl RawGeometry {
    pub fn to_armor_faces(self, matrix: Matrix4<f64>) -> Vec<ArmorFace> {
        let mut vertices = vec!();
        for i in 0..self.objects.armor.vertices.len()/3 {
            let pnt = Point3::new(
                self.objects.armor.vertices[i*3+0],
                self.objects.armor.vertices[i*3+1],
                self.objects.armor.vertices[i*3+2],
            );
            let pnt = Point3::from_homogeneous(matrix * pnt.to_homogeneous());
            vertices.push(Point3::new(pnt.x, pnt.y, pnt.z));
        }

        let mut faces = vec!();
        for group in self.objects.armor.groups {
            let material = &self.materials[&group.material];
            for i in 0..group.indices.len()/3 {
                faces.push(ArmorFace::new(
                    [
                        vertices[group.indices[i*3+0]],
                        vertices[group.indices[i*3+1]],
                        vertices[group.indices[i*3+2]],
                    ],
                    material.thickness as f64,
                    ArmorType::from_id(material.armor_type),
                ));
            }
        }
        faces
    }
}

fn parse_armor(url: &str, hull_components: &Map<String, Value>) -> Vec<ArmorFace> {
    let mut params = Map::new();
    for (k,v) in hull_components {
        debug!("Hull has component {}: {}", k, v);
        params.insert(k.to_string(), v[0].clone());
    }

    let page = download_with_params(&url, "armor", &Value::Object(params).to_string());
    let scheme: Vec<_> = page.lines().filter(|line| {
        line.contains("var scheme")
    }).collect();
    if scheme.len() != 1 {
        error!("Expected to find exactly one scheme variable! Found {}", scheme.len());
        panic!();
    }
    let armor = scheme[0].split("=").skip(1).collect::<Vec<_>>().join("=");
    let armor: Value = serde_json::from_str(&armor[1..armor.len()-1]).unwrap();

    let mut faces = vec!();
    for (_,v) in armor.as_object().unwrap() {
        let url = format!("https://gamemodels3d.com/games/worldofwarships/data/current/armor/{}", v["model"].as_str().unwrap());
        let model = download(&url);
        if model.len() == 0 {
            // Sometimes we get 404 for some reason
            continue;
        }

        let mut m = [0.0; 16];
        let transform = v["transform"].as_array().unwrap();
        for i in 0..4 {
            let col = transform[i].as_array().unwrap();
            for j in 0..4 {
                m[i*4 + j] = col[j].as_f64().expect(&format!("Couldn't get {}th element of column {}", j, i));
            }
        }
        let m = Matrix4::new(
            m[0*4 + 0],
            m[0*4 + 1],
            m[0*4 + 2],
            m[0*4 + 3],

            m[1*4 + 0],
            m[1*4 + 1],
            m[1*4 + 2],
            m[1*4 + 3],

            m[2*4 + 0],
            m[2*4 + 1],
            m[2*4 + 2],
            m[2*4 + 3],

            m[3*4 + 0],
            m[3*4 + 1],
            m[3*4 + 2],
            m[3*4 + 3],
        );
        //debug!("Got matrix: {:?}", m);
        let geometry: RawGeometry = serde_json::from_str(&model).unwrap();
        faces.append(&mut geometry.to_armor_faces(m));
    }
    debug!("Mesh has {} faces", faces.len());

    // Get the bounding box
    let mins = [
        faces.iter().map(|face| { face.vertices.iter() }).flatten().map(|p| {p.x}).fold(1./0., f64::min),
        faces.iter().map(|face| { face.vertices.iter() }).flatten().map(|p| {p.y}).fold(1./0., f64::min),
        faces.iter().map(|face| { face.vertices.iter() }).flatten().map(|p| {p.z}).fold(1./0., f64::min),
    ];
    let maxs = [
        faces.iter().map(|face| { face.vertices.iter() }).flatten().map(|p| {p.x}).fold(-1./0., f64::max),
        faces.iter().map(|face| { face.vertices.iter() }).flatten().map(|p| {p.y}).fold(-1./0., f64::max),
        faces.iter().map(|face| { face.vertices.iter() }).flatten().map(|p| {p.z}).fold(-1./0., f64::max),
    ];
    debug!("Bounding box: {:?} to {:?}", mins, maxs);

    // Dump the mesh as a .obj to debug
    {
        let mut f = std::fs::File::create("test.obj").unwrap();
        for face in faces.iter() {
            for v in face.vertices.iter() {
                f.write_all(format!("v {} {} {}\n", v.x, v.y, v.z).as_bytes()).unwrap();
            }
        }
        for i in 0..faces.len() {
            f.write_all(format!("f {} {} {}\n", i*3+1, i*3+2, i*3+3).as_bytes()).unwrap();
        }
    }
    faces
}

fn find_size(faces: &Vec<ArmorFace>) -> [f64; 3] {
    let mins = [
        faces.iter().map(|face| { face.vertices.iter() }).flatten().map(|p| {p.x}).fold(1./0., f64::min),
        faces.iter().map(|face| { face.vertices.iter() }).flatten().map(|p| {p.y}).fold(1./0., f64::min),
        faces.iter().map(|face| { face.vertices.iter() }).flatten().map(|p| {p.z}).fold(1./0., f64::min),
    ];
    let maxs = [
        faces.iter().map(|face| { face.vertices.iter() }).flatten().map(|p| {p.x}).fold(-1./0., f64::max),
        faces.iter().map(|face| { face.vertices.iter() }).flatten().map(|p| {p.y}).fold(-1./0., f64::max),
        faces.iter().map(|face| { face.vertices.iter() }).flatten().map(|p| {p.z}).fold(-1./0., f64::max),
    ];
    [
        maxs[0] - mins[0],
        maxs[1] - mins[1],
        maxs[2] - mins[2],
    ]
}

fn parse_hull(url: &str, ship_spec: &Value, components: &Map<String, Value>) -> ShipConfiguration {
    let hull_spec = ship_spec["components"].as_object().unwrap();

    for (key, value) in hull_spec {
        debug!("Found component {}: {}", key, value);
    }

    let hull = &components[hull_spec["hull"].as_array().unwrap()[0].as_str().unwrap()];
    let max_speed = hull["maxSpeed"].as_f64().unwrap() / 1.944; // Scaling factor to get m/s, as far as I can tell

    let name = hull["name"].as_str().unwrap();

    let artillery = if hull_spec.contains_key("artillery") {
        let artillery = &hull_spec["artillery"];
        let artillery = artillery.as_array().unwrap();
        if artillery.len() != 1 {
            warn!("Found an artillery of length {}!", artillery.len());
        }
        let artillery = &artillery[0];
        debug!("Parsing artillery: {:?}", artillery);
        parse_artillery(components[artillery.as_str().unwrap()].as_object().unwrap())
    } else {
        vec!()
    };
    let geometry = parse_armor(url, hull_spec);

    let size = find_size(&geometry);
    let length = size[2] * 1.53; // Scaling factor to get meters

    ShipConfiguration::new(
        artillery,
        geometry,
        max_speed,
        length,
        name.to_string(),
    )
}

pub fn download_vehicle(vehicle_id: &str) -> Option<Ship> {
    trace!("Downloading vehicle_id={}", vehicle_id);
    let url = format!("https://gamemodels3d.com/games/worldofwarships/vehicles/{}", vehicle_id);
    let page = download(&url);

    let vehicle: Vec<_> = page.lines().filter(|line| {
        line.contains("var _vehicle")
    }).collect();
    if vehicle.len() != 1 {
        panic!("Expected vehicle length to be 1!");
    }
    let spec = vehicle[0].split("=").skip(1).collect::<Vec<_>>().join("=");
    //println!("Spec: {}", spec);
    let v: Value = serde_json::from_str(&spec[1..spec.len()-1]).unwrap();
    let vehicle_components = v["Components"].as_object().unwrap();
    let hulls = v["ShipUpgradeInfo"]["_Hull"].as_object().unwrap();
    let mut configs = vec!();
    for (key, value) in hulls {
        debug!("Found hull {}", key);
        let hull = parse_hull(&url, value, &vehicle_components);
        configs.push(hull);
    }

    let name = v["name"].as_str().unwrap();
    let class = v["class"].as_str().unwrap();
    let class = if class == "destroyer" {
        ShipClass::Destroyer
    } else if class == "cruiser" {
        ShipClass::Cruiser
    } else if class == "battleship" {
        ShipClass::Battleship
    } else if class == "aircarrier" {
        ShipClass::AircraftCarrier
    } else if class == "auxiliary" || class == "submarine" {
        // Ignore these
        return None;
    } else {
        error!("Found unknown ship class {} for {}", class, name);
        panic!();
    };

    Some(Ship::new(
        configs,
        v["level"].as_i64().unwrap().try_into().unwrap(),
        v["name"].as_str().unwrap().to_string(),
        class,
    ))
}
