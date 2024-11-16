//! # Calculates the Exponential Moving Average (EMA), also known as an exponentially weighted moving average (EWMA).
// use core::fmt;
use std::fmt;
use std::error;
use crate::psql::{DataForEma, Db};
use crate::quik::Terminal;
use bb8::RunError;
use ta::indicators::ExponentialMovingAverage;
use ta::DataItem;
use ta::Next;
use tracing::error;


/// Composite error type for Ema.
#[derive(Debug)]
pub enum EmaError {
    Bb8(RunError<bb8_postgres::tokio_postgres::Error>),
    NoData

}

impl fmt::Display for EmaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EmaError::Bb8(err) => write!(f, "{}", err),
            EmaError::NoData => write!(f, "There is not enough data to calculate the EMA")
        }
    }
}

impl error::Error for EmaError {}

impl From<RunError<bb8_postgres::tokio_postgres::Error>> for EmaError {
    fn from(err: RunError<bb8_postgres::tokio_postgres::Error>) -> EmaError {
        EmaError::Bb8(err)
    }
}

pub struct Ema {}

impl Ema {
    pub async fn calc(
        database: &Db,
        terminal: &Terminal,
        instrument_code: &str,
        interval: f64,
        period_len: f64,
        period_quantity: usize,
    ) -> Result<f64, EmaError> {
        let data_for_ema = database
            .get_data_for_ema(instrument_code, interval, period_len)
            .await?;
        let ema_period = data_for_ema.len();
        if ema_period != period_quantity {
            terminal.unsubscribe_orders();
            terminal.unsubscribe_trades();
            terminal.disconnect();
            let err = EmaError::NoData;
            error!("{}", err);
            return Err(err)
        }
        let mut ema = ExponentialMovingAverage::new(ema_period).unwrap();
        println!("ema new with period = {}", ema);
        let mut ema_value = 0.0;
        for data in data_for_ema {
            let item = DataItem::builder()
                .open(data.open)
                .high(data.high)
                .low(data.low)
                .close(data.close)
                .volume(data.volume)
                .build()
                .unwrap();
            println!("item = {:?}", item);

            ema_value = ema.next(&item);
            println!("ema next = {}", ema_value);
        }
        println!("ema value: {}", ema);

        Ok(ema_value)
    }
}

fn calc_ema(data_for_ema: Vec<DataForEma>, _period_quantity: f64) -> f64 {
    let period = data_for_ema.len();
    let mut ema = ExponentialMovingAverage::new(period).unwrap();
    println!("ema new with period = {}", ema);
    let mut ema_value = 0.0;
    for data in data_for_ema {
        let item = DataItem::builder()
            .open(data.open)
            .high(data.high)
            .low(data.low)
            .close(data.close)
            .volume(data.volume)
            .build()
            .unwrap();
        println!("item = {:?}", item);

        ema_value = ema.next(&item);
        println!("ema next = {}", ema_value);
    }
    ema_value
}
