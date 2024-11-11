//! # Application for algorithmic trading on the MOEX via the QUIK terminal.
use tracing_subscriber;
mod ema;
mod psql;
mod quik;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let path = r"c:\QUIK Junior\trans2quik.dll";
    let terminal = quik::Terminal::new(path)?;
    terminal.connect()?;
    terminal.is_dll_connected()?;
    terminal.is_quik_connected()?;
    terminal.set_connection_status_callback()?;
    terminal.set_transactions_reply_callback()?;
    let class_code = "QJSIM";
    let sec_code = "LKOH";
    terminal.subscribe_orders(class_code, sec_code)?;
    terminal.subscribe_trades(class_code, sec_code)?;
    terminal.start_orders();
    terminal.start_trades();
    let transaction_str = "ACCOUNT=NL0011100043; CLIENT_CODE=10077; TYPE=L; TRANS_ID=1; CLASSCODE=QJSIM; SECCODE=LKOH; ACTION=NEW_ORDER; OPERATION=B; PRICE=6957,0; QUANTITY=1;";
    terminal.send_async_transaction(transaction_str)?;
    terminal.disconnect()?;

    // let connection_str = "host=localhost user=postgres dbname=postgres password=password";
    // let database = psql::Db::new(connection_str).await?;
    // database.init().await?;

    // Параметры запроса
    // let instrument_code = "AAPL";
    // let lookback_interval_seconds: f64 = (5 * 3600 + 15 * 60) as f64; // 5 часов 15 минут в секундах
    // let period_length_seconds: f64 = (15 * 60) as f64; // 15 минут в секундах

    // let lookback_interval_seconds: f64 = 600 as f64;
    // let period_length_seconds: f64 = 10 as f64;
    // let data = database.get_data_for_ema(instrument_code, lookback_interval_seconds, period_length_seconds).await?;
    // println!("{:?}", d);
    // let e = ema::Ema::calculate_ema(data);
    // info!("ema value = {}", e);

    Ok(())
}
