//! # Calculates the Exponential Moving Average (EMA), also known as an exponentially weighted moving average (EWMA).
use crate::psql::{DataForEma, Db};
use bb8::RunError;
use ta::indicators::ExponentialMovingAverage;
use ta::DataItem;
use ta::Next;

pub struct Ema {}

impl Ema {
    pub async fn calc(
        database: &Db,
        instrument_code: &str,
        interval: f64,
        period_len: f64,
        period_quantity: usize,
    ) -> Result<f64, RunError<bb8_postgres::tokio_postgres::Error>> {
        let data_for_ema = database
            .get_data_for_ema(instrument_code, interval, period_len)
            .await?;
        let ema_period = data_for_ema.len();
        if ema_period != period_quantity {
            panic!("Недостаточно данных для расчета!");
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
