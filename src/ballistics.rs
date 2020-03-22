use serde_derive::{Serialize, Deserialize};
use rand_distr::{Normal, Distribution};
use cgmath::Vector3;

#[derive(Debug, Serialize, Deserialize)]
pub struct Ballistics {
    mass: f64, // kg
    diameter: f64, // m
    muzzle_speed: f64, // m/s
    drag: f64, // coefficient
    krupp: f64,
}

impl Ballistics {
    pub fn new(
        mass: f64,
        diameter: f64,
        muzzle_speed: f64,
        drag: f64,
        krupp: f64,
    ) -> Ballistics {
        Ballistics{
            mass: mass,
            diameter: diameter,
            muzzle_speed: muzzle_speed,
            drag: drag,
            krupp: krupp,
        }
    }
}

fn deg2rad(x: f64) -> f64 { x * 3.14159265 / 180. }

#[derive(new, Debug, Serialize, Deserialize)]
pub struct BallisticFlight {
    pub distance: f64,
    pub velocity: f64,
    pub time_aloft: f64,
    pub impact_angle: f64, // deg
    pub penetration: f64,
}

impl Ballistics {
    pub fn calculate_flight_at_range(&self, range: f64) -> BallisticFlight {
        let mut guess = 22.5;
        for _ in 0..12 {
            let flight = self.calculate_flight(guess);
            let error = flight.distance - range;
            if error.abs() < 1.0 {
                return flight;
            }
            guess = guess * range / flight.distance;
        }
        self.calculate_flight(guess)
    }

    pub fn calculate_flight(&self, angle: f64) -> BallisticFlight {
        //debug!("Calculating {}-degree flight for {:?}", angle);
        let temp_sea_level = 288.;
        let temp_lapse_rate = 0.0065;
        let press_sea_level = 101325.0;
        let gravity = 9.81;
        let air_mass = 0.0289644;
        let r = 8.31447;

        let cw_1 = 1.;
        let cw_2 = 100. + 1000. / 3. * self.diameter;
        let k = 0.5 * self.drag * (self.diameter / 2.) * (self.diameter / 2.) * 3.14159265 / self.mass;

        let mut vx = self.muzzle_speed * deg2rad(angle).cos();
        let mut vy = self.muzzle_speed * deg2rad(angle).sin();
        let mut x = 0.;
        let mut y = 0.;
        let mut t = 0.;
        let dt = 0.05;
        while y >= 0.0 {
            x += dt * vx;
            y += dt * vy;

            let temperature = temp_sea_level - temp_lapse_rate * y;
            let pressure = press_sea_level * (temperature / temp_sea_level).powf(gravity * air_mass / r / temp_lapse_rate);
            let rho_g = pressure * air_mass / r / temperature;

            vx -= dt * k * rho_g * (cw_1 * vx * vx + cw_2 * vx);
            vy -= dt * gravity + dt * k * rho_g * (cw_1 * vy * vy + cw_2 * vy);

            t += dt;
        }

        let v = (vx*vx + vy*vy).sqrt();
        let c_pen = 0.5561613 * self.krupp / 2400.;
        let penetration = c_pen * v.powf(1.1) * self.mass.powf(0.55) / (self.diameter * 1000.).powf(0.65);
        BallisticFlight::new(
            x, v, t, vy.atan2(vx) * 180. / 3.14159265, penetration
        )
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Dispersion {
    horizontal: f64,
    vertical: f64,
    maxrange: f64,
    sigma: f64,
}

fn bounded_gauss(sigma: f64) -> f64 {
    let normal = Normal::new(0.0, sigma).unwrap();
    loop {
        let v = normal.sample(&mut rand::thread_rng());
        if v > -0.5 && v < 0.5 {
            return v;
        }
    }
}

impl Dispersion {
    pub fn new(
        horizontal: f64,
        vertical: f64,
        maxrange: f64,
        sigma: f64,
    ) -> Dispersion {
        Dispersion {
            horizontal: horizontal,
            vertical: vertical,
            maxrange: maxrange,
            sigma: sigma,
        }
    }

    /// Returns a randomly generated offset based on dispersion
    pub fn generate_offset(&self, azimuth: f64, range: f64) -> Vector3<f64> {
        let distance_factor = range / self.maxrange;
        let x = self.horizontal * bounded_gauss(self.sigma) * distance_factor;
        let y = self.vertical * bounded_gauss(self.sigma) * distance_factor;
        //info!("{} {} {} {} {},{}", self.horizontal, self.vertical, distance_factor, self.sigma, x, y);
        Vector3::new(
            x * deg2rad(azimuth).cos() - y * deg2rad(azimuth).sin(),
            0.0,
            x * deg2rad(azimuth).sin() + y * deg2rad(azimuth).cos(),
        )
    }
}
