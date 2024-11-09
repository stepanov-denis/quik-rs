#![allow(dead_code)]
use crate::psql::DataForEma;
use ta::indicators::ExponentialMovingAverage;
use ta::DataItem;
use ta::Next;

pub struct Ema {
    trend: ExponentialMovingAverage,
}

impl Ema {
    pub fn calculate_ema(data_for_ema: Vec<DataForEma>) -> f64 {
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
}
