use std::process::Command;
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::{io, thread};

use rppal::gpio::OutputPin;

use super::{WindDirection, WindSpeed};

#[derive(Debug, Clone)]
pub enum Message {
    Echo(String),
    SayForecast(ForecastData),
    Break,
}

#[derive(Debug, Clone, Copy)]
pub struct ForecastData {
    pub temperature: f32,
    pub humidity: f32,
    pub pressure: f32,
    pub wind_direction: WindDirection,
    pub wind_speed: WindSpeed,
    pub rainfall: bool,
}

impl ForecastData {
    fn say_wind_direction(&self) -> &str {
        match self.wind_direction {
            WindDirection::North => "Pul/noc",
            WindDirection::NorthEast => "Pul/nocny wschud",
            WindDirection::East => "Wschud",
            WindDirection::SouthEast => "Pol/udniowy wschud",
            WindDirection::South => "Pol/udnie",
            WindDirection::SouthWest => "Pol/udniowy zachud",
            WindDirection::West => "Zachud",
            WindDirection::NorthWest => "Pul/nocno zachodni",
            WindDirection::None => "nieznany",
        }
    }
    fn say_rainfall(&self) -> &str {
        match self.rainfall {
            true => "wyste~puje",
            false => "nie wyste~puje",
        }
    }
}

pub struct AudioManager {
    pub tx: Option<SyncSender<Message>>,
    transmission_signal_pin: Arc<Mutex<OutputPin>>,
    thread_handler: Option<JoinHandle<()>>,
}

impl AudioManager {
    pub fn new(transmission_signal_pin: OutputPin) -> AudioManager {
        //przechowuj dostęp do Pina pod Arc<Mutex> aby był on wspóldzielony pomiędzy wątkami
        let transmission_signal_pin = Arc::new(Mutex::new(transmission_signal_pin));
        AudioManager {
            tx: None,
            thread_handler: None,
            transmission_signal_pin,
        }
    }

    pub fn start_thread(&mut self) {
        //utwórz kolejkowy kanał Multi-producer single-consumer z zerowym bufforem
        //dzięki temu wątek Audio będzie czekał na wiadomość gdy swoją pracę zakończy
        //a wątek główny będzie wysyłał tylko wtedy gdy wątek audio jest gotowy na odebranie wadomości
        let (tx, rx) = mpsc::sync_channel(0);
        let signal_pin = self.transmission_signal_pin.clone();

        let thread_handler = thread::spawn(move || thread_loop(rx, signal_pin));

        self.tx = Some(tx);
        self.thread_handler = Some(thread_handler);
    }

    pub fn get_tx(&mut self) -> SyncSender<Message> {
        self.tx.clone().unwrap()
    }
}

fn thread_loop(rx: Receiver<Message>, transmission_signal_pin: Arc<Mutex<OutputPin>>) {
    loop {
        //Oczekuj na wiadomość od wątku głównego
        let message: Message = rx.recv().unwrap();
        let _ = match message {
            Message::Break => break, //zakończ wątek gdy została odebrana ta wiadomość
            _ => (), //gdy wszystko inne, tu nic nie rób
        };

        { //zażądaj dostępu do pinu pod Mutexem i ustaw stan niski
            let mut pin = transmission_signal_pin.lock().unwrap();
            pin.set_low();
        }

        let res = run_local_command(&message);

        { //zażądaj dostępu do pinu pod Mutexem i ustaw stan wysoki
            let mut pin = transmission_signal_pin.lock().unwrap();
            pin.set_high();
        }

        match res {
            Err(_) => (),
            _ => (),
        }
    }
}

fn run_local_command(message: &Message) -> io::Result<()> {
    //polecenia dostępne w systemie
    const CMD_ECHO: &str = "echo";
    const CMD_SH: &str = "sh";
    const CMD_FESTIVAL: &str = "festival";

    let mut cmd: Command = match message {
        Message::Echo(msg) => {
            let mut cmd_build = Command::new(CMD_ECHO);
            cmd_build.arg(msg);
            cmd_build
        }
        Message::SayForecast(fc) => {
            let mut cmd_build = Command::new(CMD_SH);

            //Budowanie polecenia dla programu festival na podstawie danych z odebranej wiadomości
            let input_tts = format!(
                "(SayText \"Aktualny stan pogody\")
            (SayText \"temperatura\")(SayText \"{:.0} stopni Celsjusza\")
            (SayText \"Wilgotnos~c~\")(SayText \"{:.0} procent\")
            (SayText \"C~is~nienie\")(SayText \"{:.0} hektopaskali\")
            (SayText \"Kierunek wiatru\")(SayText \"{}\")
            (SayText \"Pre~dkos~c~ wiatru\")(SayText \"{:.0} metro~w na sekunde\")
            (SayText \"Opad atmosferyczny\")(SayText \"{}\")",
                fc.temperature,
                fc.humidity,
                fc.pressure,
                fc.say_wind_direction(),
                fc.wind_speed.meters_per_sec(),
                fc.say_rainfall()
            );

            println!("{}", input_tts);

            //budowanie końcowego polecenia które zostanie wywołanie w powłoce SH
            cmd_build
                .arg("-c")
                .arg(format!("echo '{input_tts}' | {CMD_FESTIVAL} -"));
            cmd_build
        }
        _ => unimplemented!(),
    };

    let status = cmd.status()?;

    if status.success() {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::Other,
            format!("failed to run: `{status:?}`, with message {message:?}"),
        ))
    }
}
