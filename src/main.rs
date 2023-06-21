mod cli;
mod manager;

use clap::Parser;
use cli::Cli;
use manager::audiomanager::{AudioManager, ForecastData, Message};
use manager::manager::Manager;
use rppal::gpio::Gpio;
use std::sync::mpsc::TrySendError;
use std::time::{Duration, Instant};

fn main() {
    //Analizuj argumenty programu i utwórz strukturę Cli na jej podstawie
    let cli = Cli::parse();

    //Utwórz główny menedżer który będzie posiadał własności na Piny GPIO sensorów
    let mut manager = Manager::new().expect("can't get Manager");
    //Przygotowuje menedżer sensorów do działania, w tym przypadku, urchamia wątek który odczytuje stan anemometra
    manager.prepare();
    //Przygotowuje menedżer audio do działania i go następnie uruchamia
    let mut audio_manager =
        AudioManager::new(Gpio::new().unwrap().get(11).unwrap().into_output_high());
    audio_manager.start_thread();

    //Ustawia/resetuj mierniki
    let mut last_dht22 = Instant::now();
    let mut last_bmp280 = last_dht22;
    let mut last_print = last_dht22;
    let mut last_autoprint = last_print;

    //Pętla główna programu
    loop {
        if last_dht22.elapsed().as_secs() >= 5 { //sprawdzaj co 5 sekund
            last_dht22 = Instant::now();
            manager.get_dht22();
        }

        if last_bmp280.elapsed().as_secs() >= 10 { //sprawdzaj co 10 sekund
            last_bmp280 = Instant::now();
            manager.get_bmp280();
        }

        //  Zawsze sprawdzaj gdy tylko to możliwe
        manager.get_weathervane_position();
        //


        //Sprawdzenie czy wystąpiło jedno z wydarzeń: naciśnięcie przycisku | minął określony czas od ostatniego wydarzenia
        if (cli.auto_audio && (last_autoprint.elapsed().as_secs() >= cli.interval_audio))
            || ((last_print.elapsed().as_secs() >= 1) && manager.button_is_pressed())
        {
            last_print = Instant::now();

            //sprawdzaj wartości które nie wymagają ciągłego odczytu i mogą być natychmiastowo odczytane
            manager.get_anemometer_speed();
            manager.get_rainfall();

            //spróbuj przesłać wiadomość zawierającą wszystkie dane do wywołania syntetyzatora mowy do wątku audio
            let status = audio_manager
                .get_tx()
                .try_send(Message::SayForecast(ForecastData {
                    temperature: manager.current_result.bmp280_temp,
                    humidity: manager.current_result.dht22_humidity,
                    pressure: manager.current_result.bmp280_pressure,
                    wind_direction: manager.current_result.weathervane_position,
                    wind_speed: manager.current_result.anemometer_speed,
                    rainfall: manager.current_result.rainfall,
                }));

            //Sprawdź status próby wysłania wiadomości
            match status {
                Ok(_) => {
                    println!("{:?}", manager.current_result);
                    // resetuj czasomierz dla automatycznego działania
                    // tylko gdy wiadomość została prawidłowo przesłana
                    last_autoprint = Instant::now();
                }
                Err(TrySendError::Full(_)) => (), // gdy wątek audio dalej przetwarza poprzednią wiadomość
                Err(TrySendError::Disconnected(message)) => {
                    panic!("{}", TrySendError::Disconnected(message));
                }
            }
        }

        //Odczekaj chwilę zanim wykonasz kolejną iterację sprawdzającą aby zmniejszyć wykorzystanie mocy obliczeniowej
        std::thread::sleep(Duration::from_millis(100));
    }
}
