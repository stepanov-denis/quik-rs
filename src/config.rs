use std::fs;
use std::error::Error;
use serde::Deserialize;

// Config from config.yaml
#[derive(Deserialize, Debug)]
pub struct Config {
    pub path_to_lib: String,
    pub path_to_quik: String,
    pub class_code: String,
    pub sec_code: String,
    pub instrument_status: String,
    pub timeframe: i32,
    pub short_num_of_candles: i32,
    pub long_num_of_candles: i32,
    pub hysteresis_percentage: f64,
    pub hysteresis_periods: u32,
    pub tg_token: String,
    pub psql_conn_str: String,
}

impl Config {
    pub fn new(path: &str) -> Result<Self, Box<dyn Error>> {
        // Чтение файла конфигурации
        let config_data = fs::read_to_string(path)?;
        let config: Config = serde_yaml::from_str(&config_data)?;

        Ok(config)
    }
}