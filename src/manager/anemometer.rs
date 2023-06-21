use std::{
    self,
    sync::{Arc, Mutex, PoisonError, RwLock, RwLockReadGuard},
    time::{Duration, Instant}, thread::{self, JoinHandle},
};

use rppal::gpio::{InputPin, Level, Trigger};

use super::WindSpeed;

pub struct Anemometer {
    speed_pin: Arc<Mutex<InputPin>>,
    thread_handler: Option<JoinHandle<()>>,
    data: Arc<RwLock<AnemometerData>>,
}

pub struct AnemometerData {
    speed: f64, //rounds per second
}

impl Anemometer {
    pub fn new(speed_pin: InputPin) -> Anemometer {
        Anemometer {
            speed_pin: Arc::new(Mutex::new(speed_pin)),
            thread_handler: None,
            data: Arc::new(RwLock::new(AnemometerData { speed: 0.0 })),
        }
    }

    pub fn start_thread(&mut self) {
        if self.thread_handler.is_some() {
            return;
        }

        //skopiuj referencję do 'data' i 'speed_pin' aby póżniej je przekazać do nowo utworzonego wątku
        let data = self.data.clone();
        let speed_pin = self.speed_pin.clone();

        let thread_handle = thread::spawn(move || thread_loop(data, speed_pin));

        self.thread_handler = Some(thread_handle);
    }

    pub fn get_speed(&mut self) -> Result<WindSpeed, PoisonError<RwLockReadGuard<AnemometerData>>> {
        let data = self.data.read()?;
        Ok(WindSpeed::new(data.speed))
    }
}

fn thread_loop(data_arc: Arc<RwLock<AnemometerData>>, speed_pin_arc: Arc<Mutex<InputPin>>) {
    let mut speed_pin = speed_pin_arc.lock().unwrap();
    speed_pin.set_interrupt(Trigger::FallingEdge).unwrap(); //ustawia interrupcję na zbocze opadające

    const TARGET_COUNT: u8 = 2; //docelowa ilość zbocz opadających zanim zostanie zmierzony czas
    const SPIN: f64 = 0.5 * TARGET_COUNT as f64; //Jeden tik niski to pół obrotu.

    let mut count = 0; //Zwiększa się za każdym razem przed próbą odczytu zmiany stanu
    let mut lastcheck = Instant::now();

    //osobna funkcja która uzyskuje dostęp do danych, nadpisuje je i natychmiastowo zwalnia blokadę
    let set_speed_data = |round_per_sec: f64| {
        let mut data = data_arc.write().unwrap();
        data.speed = round_per_sec;
    };

    loop {
        count += 1;

        //czekanie na wcześniej ustawioną interrupcję
        let poll_result = speed_pin.poll_interrupt(true, Some(Duration::from_secs(10)));

        let _ = match poll_result {
            Ok(None) => { //przekroczenie limitu czasu przy odczycie
                set_speed_data(0.0); //zresetuj prędkość
                count = 0;
                lastcheck = Instant::now();
                continue;
            },
            Ok(Some(Level::High)) => { //ignoruj wysokie stany, nie resetuj prędkości
                count -= 1; //cofnij licznik niskich stanów który zwiększa się po continue
                continue
            },
            Ok(Some(Level::Low)) => (), //kontynuuj kod gdy pojawił się stan niski
            Err(_) => (), //martwy kod
        };


        if count < TARGET_COUNT {
            continue;
        }

        //czas w którym anenomentr wykonał obrót o `count` tików
        let span = lastcheck.elapsed();

        // obrót/s
        let round_per_sec: f64 = SPIN / span.as_secs_f64();
        set_speed_data(round_per_sec);

        //Restart
        count = 0;
        lastcheck = Instant::now();
    }
}
