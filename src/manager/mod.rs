pub mod anemometer;
pub mod audiomanager;
pub mod manager;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WindDirection {
    North,
    NorthEast,
    East,
    SouthEast,
    South,
    SouthWest,
    West,
    NorthWest,
    None,
}

#[derive(Clone, Copy, Debug)]
pub struct WindSpeed(f64);
impl WindSpeed {
    pub fn new(rounds_per_sec: f64) -> WindSpeed {
        WindSpeed(rounds_per_sec)
    }

    pub fn round_per_sec(&self) -> f64 {
        self.0
    }
    pub fn km_per_hour(&self) -> f64 {
        //old: self.round_per_sec() * 4.8
        self.round_per_sec() * 16.0
    }
    pub fn meters_per_sec(&self) -> f64 {
        //old: self.round_per_sec() * 4.8 / 3.6
        self.round_per_sec() * 16.0 / 3.6
    }
}

#[derive(Clone, Copy, Debug)]
enum WindPin {
    SPEED = 21,
    N = 20,
    NE = 26,
    E = 16,
    SE = 19,
    S = 13,
    SW = 12,
    W = 6,
    NW = 5,
}
