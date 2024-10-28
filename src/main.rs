use tracing_subscriber;
use tracing::{info, error};
mod connector;
mod trader;
mod psql;
mod ema;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    // TODO encoding to UTF8 if password is wrong
    // TODO create if not exists for function and trigger
    let connection_str = "host=localhost user=postgres dbname=postgres password=password";
    let database = psql::Db::new(connection_str).await?;
    database.init().await?;

    // Параметры запроса
    let instrument_code = "AAPL";
    let lookback_interval_seconds: f64 = (5 * 3600 + 15 * 60) as f64; // 5 часов 15 минут в секундах
    let period_length_seconds: f64 = (15 * 60) as f64; // 15 минут в секундах

    // let lookback_interval_seconds: f64 = 600 as f64;
    // let period_length_seconds: f64 = 10 as f64;
    let data = database.get_data_for_ema(instrument_code, lookback_interval_seconds, period_length_seconds).await?;
    // println!("{:?}", d);
    let e = ema::Ema::calculate_ema(data);
    info!("ema value = {}", e);

    Ok(())
}