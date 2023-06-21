use bmp280::{Bmp280, Bmp280Builder};
use rppal::gpio::{Gpio, InputPin};
use std::error::Error;
use super::WindSpeed;
use super::anemometer::Anemometer;
use super::WindDirection;
use super::WindPin;

#[derive(Debug)]
pub struct CurrentManagerResult {
    pub bmp280_temp: f32,
    pub bmp280_pressure: f32,
    pub dht22_temp: f32,
    pub dht22_humidity: f32,
    pub rainfall: bool,
    pub anemometer_speed: WindSpeed,
    pub weathervane_position: WindDirection,
}

impl CurrentManagerResult {
    pub fn new() -> CurrentManagerResult {
        CurrentManagerResult {
            bmp280_temp: 0.0,
            bmp280_pressure: 0.0,
            dht22_temp: 0.0,
            dht22_humidity: 0.0,
            rainfall: false,
            anemometer_speed: WindSpeed::new(0.0),
            weathervane_position: WindDirection::None,
        }
    }
}

pub struct Manager {
    rainfall: InputPin,
    bmp280_controller: Bmp280,
    anemometer: Anemometer,
    weathervane_n_pin: InputPin,
    weathervane_ne_pin: InputPin,
    weathervane_e_pin: InputPin,
    weathervane_se_pin: InputPin,
    weathervane_s_pin: InputPin,
    weathervane_sw_pin: InputPin,
    weathervane_w_pin: InputPin,
    weathervane_nw_pin: InputPin,
    dht22_fs_temp: &'static str,
    dht22_fs_humidity: &'static str,
    pub current_result: CurrentManagerResult,
    button: InputPin
}

impl Manager {
    pub fn new() -> Result<Manager, Box<dyn Error>> {
        let gpio = Gpio::new()?;
        let rainfall = gpio.get(14)?.into_input_pulldown();

        let bmp280_controller = Bmp280Builder::new()
            .path("/dev/i2c-1")
            .address(0x76)
            .build()?;

        let anemometer_speed_pin = gpio.get(WindPin::SPEED as u8)?.into_input_pullup();
        let anemometer = Anemometer::new(anemometer_speed_pin);

        //weathervane - kierunki wiatru
        let weathervane_n_pin = gpio.get(WindPin::N as u8)?.into_input_pullup();
        let weathervane_ne_pin = gpio.get(WindPin::NE as u8)?.into_input_pullup();
        let weathervane_e_pin = gpio.get(WindPin::E as u8)?.into_input_pullup();
        let weathervane_se_pin = gpio.get(WindPin::SE as u8)?.into_input_pullup();
        let weathervane_s_pin = gpio.get(WindPin::S as u8)?.into_input_pullup();
        let weathervane_sw_pin = gpio.get(WindPin::SW as u8)?.into_input_pullup();
        let weathervane_w_pin = gpio.get(WindPin::W as u8)?.into_input_pullup();
        let weathervane_nw_pin = gpio.get(WindPin::NW as u8)?.into_input_pullup();
        
        let current_result = CurrentManagerResult::new();

        //Dostęp do danych z DHT22 poprzez sterownik Linuksa
        let dht22_fs_temp = "/sys/bus/iio/devices/iio:device0/in_temp_input";
        let dht22_fs_humidity = "/sys/bus/iio/devices/iio:device0/in_humidityrelative_input";

        let button = gpio.get(1)?.into_input_pullup();

        Ok(Manager {
            rainfall,
            bmp280_controller,
            anemometer,
            weathervane_n_pin,
            weathervane_ne_pin,
            weathervane_e_pin,
            weathervane_se_pin,
            weathervane_s_pin,
            weathervane_sw_pin,
            weathervane_w_pin,
            weathervane_nw_pin,
            dht22_fs_temp,
            dht22_fs_humidity,
            current_result,
            button
        })
    }

    pub fn prepare(&mut self) {
        self.anemometer.start_thread();
    }

    pub fn get_bmp280(&mut self) {
        match self.bmp280_controller.pressure_kpa() {
            Ok(it) => self.current_result.bmp280_pressure = it * 10.0,
            Err(_) => (),
        };
        match self.bmp280_controller.temperature_celsius() {
            Ok(it) => self.current_result.bmp280_temp = it,
            Err(_) => (),
        };
    }

    pub fn get_dht22(&mut self) {
        let _ = std::fs::read_to_string(self.dht22_fs_temp).and_then(|it| {
            let _ = it.trim_end().parse::<f32>().and_then(|parsed| {
                self.current_result.dht22_temp = parsed / 1000.0;
                Ok(())
            });
            Ok(())
        });

        let _ = std::fs::read_to_string(self.dht22_fs_humidity).and_then(|it| {
            let _ = it.trim_end().parse::<f32>().and_then(|parsed| {
                self.current_result.dht22_humidity = parsed / 1000.0;
                Ok(())
            });
            Ok(())
        });
    }

    pub fn get_weathervane_position(&mut self) {
        let n = self.weathervane_n_pin.is_high();
        let ne = self.weathervane_ne_pin.is_high();
        let e = self.weathervane_e_pin.is_high();
        let se = self.weathervane_se_pin.is_high();
        let s = self.weathervane_s_pin.is_high();
        let sw = self.weathervane_sw_pin.is_high();
        let w = self.weathervane_w_pin.is_high();
        let nw = self.weathervane_nw_pin.is_high();

        //Tylko jeden stan niski może wystąpić, jeśli jakimś przypadkiem jest więcej stanów niskich:
        //ustawia konkretny stan ignorując inny
        //jaki stan zostanie ostatecznie odczytany z zapisany w wynikach zależy od poniższego matcha
        //Program po kolei sprawdza warunki i gdy którykolwiek wystąpi, natychmiastowo zwraca ten wynik i ignoruje pozostałe
        let pos = match (n, ne, e, se, s, sw, w, nw) {
            (false, _, _, _, _, _, _, _) => WindDirection::North,
            (_, false, _, _, _, _, _, _) => WindDirection::NorthEast,
            (_, _, false, _, _, _, _, _) => WindDirection::East,
            (_, _, _, false, _, _, _, _) => WindDirection::SouthEast,
            (_, _, _, _, false, _, _, _) => WindDirection::South,
            (_, _, _, _, _, false, _, _) => WindDirection::SouthWest,
            (_, _, _, _, _, _, false, _) => WindDirection::West,
            (_, _, _, _, _, _, _, false) => WindDirection::NorthWest,
            _ => WindDirection::None, //gdy wszystkie poprzednie porównania nie pasują, zwróć None
        };
        
        //Ustaw kierunek do wyniku gdy nie jest None
        //Dzięki temu, gdy wiatrowskaz ustąpi pomędzy pinami, zostanie zapamiętany ostatnio minięty pin
        if pos != WindDirection::None {
            self.current_result.weathervane_position = pos;
        }
    }

    pub fn get_anemometer_speed(&mut self) {
        let speed = self.anemometer.get_speed().unwrap();
        self.current_result.anemometer_speed = speed;
    }

    pub fn get_rainfall(&mut self) {
        self.current_result.rainfall = self.rainfall.is_low();
    }

    pub fn button_is_pressed(&mut self) -> bool {
        self.button.is_low()
    }
}
