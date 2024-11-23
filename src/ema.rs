//! # Calculates the Exponential Moving Average (EMA), also known as an exponentially weighted moving average (EWMA).
use crate::psql::Db;
use bb8::RunError;
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
        interval: f64,
        period_len: f64,
        period_quantity: usize,
    ) -> Result<f64, EmaError> {
        info!("start calculate EMA");
        info!(
            "instrument_code: {}, interval: {}, period_len: {}, period_quantity: {}",
            sec_code, interval, period_len, period_quantity
        );
        let data_for_ema = database
            .get_data_for_ema(sec_code, interval, period_len)
            .await?;
        // info!("data_for_ema: {:?}", data_for_ema);
        let ema_period = data_for_ema.len();
        info!(
            "ema_period: {}, period_quantity: {}",
            ema_period, period_quantity
        );
        if ema_period != period_quantity {
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
            // info!("item = {:?}", item);

            ema_value = ema.next(&item);
            // info!("ema next = {}", ema_value);
        }
        // info!("ema value: {}", ema);

        Ok(ema_value)
    }
}
