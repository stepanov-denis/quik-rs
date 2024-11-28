//! # Calculates the Exponential Moving Average (EMA), also known as an exponentially weighted moving average (EWMA).
use crate::psql::Db;
use bb8::RunError;
use rust_decimal::prelude::ToPrimitive;
use std::error;
use std::fmt;
use ta::indicators::ExponentialMovingAverage;
use ta::DataItem;
use ta::Next;
use tracing::info;

/// Composite error type for Ema.
#[derive(Debug)]
pub enum EmaError {
    Bb8(RunError<bb8_postgres::tokio_postgres::Error>),
    NoData,
}

impl fmt::Display for EmaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EmaError::Bb8(err) => write!(f, "{}", err),
            EmaError::NoData => write!(f, "there is not enough data to calculate the EMA"),
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
        sec_code: &str,
        timeframe: i32,
        number_of_candles: i32,
    ) -> Result<f64, EmaError> {
        info!("start calculate EMA");
        info!(
            "instrument_code: {}, timeframe: {} minutes, number_of_candles: {}",
            sec_code, timeframe, number_of_candles
        );
        let data_for_ema = database
            .get_candles(sec_code, timeframe, number_of_candles)
            .await?;
        let ema_period= data_for_ema.len();
        info!(
            "ema_period: {}, number_of_candles: {}",
            ema_period, number_of_candles
        );
        if ema_period as i32 != number_of_candles{
            return Err(EmaError::NoData);
        }
        let mut ema = ExponentialMovingAverage::new(ema_period).unwrap();
        info!("ema new with period = {}", ema);
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

            ema_value = ema.next(&item);
        }

        Ok(ema_value)
    }
}
