#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release
use lazy_static::lazy_static;
use std::error::Error;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;
use tracing::info;
use tracing_subscriber;
use crate::quik::Terminal;
use crate::signal;
use crate::psql;
use crate::ema;

use eframe::egui;

lazy_static! {
    static ref ORDER_CALLBACK_RECEIVED: Arc<(Mutex<bool>, Condvar)> =
        Arc::new((Mutex::new(false), Condvar::new()));
    static ref TRADE_CALLBACK_RECEIVED: Arc<(Mutex<bool>, Condvar)> =
        Arc::new((Mutex::new(false), Condvar::new()));
    static ref TRANSACTION_CALLBACK_RECEIVED: Arc<(Mutex<bool>, Condvar)> =
        Arc::new((Mutex::new(false), Condvar::new()));
}

pub async fn trade() -> Result<(), Box<dyn Error>> {
    let path = r"c:\QUIK Junior\trans2quik.dll";
    let terminal = Terminal::new(path)?;
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

    // Waiting for callback or timeout
    // {
    //     let order_received = {
    //         let (lock, cvar) = ORDER_CALLBACK_RECEIVED.as_ref();
    //         let received = lock.lock().unwrap();
    //         let timeout = Duration::from_secs(10);

    //         let (received, timeout_result) = cvar
    //             .wait_timeout_while(received, timeout, |received| !*received)
    //             .unwrap();

    //         if timeout_result.timed_out() {
    //             info!("Timed out waiting for order_status_callback");
    //         }

    //         *received
    //     };

    //     let trade_received = {
    //         let (lock, cvar) = TRADE_CALLBACK_RECEIVED.as_ref();
    //         let received = lock.lock().unwrap();
    //         let timeout = Duration::from_secs(10);

    //         let (received, timeout_result) = cvar
    //             .wait_timeout_while(received, timeout, |received| !*received)
    //             .unwrap();

    //         if timeout_result.timed_out() {
    //             info!("Timed out waiting for trade_status_callback");
    //         }

    //         *received
    //     };

    //     let transaction_received = {
    //         let (lock, cvar) = TRANSACTION_CALLBACK_RECEIVED.as_ref();
    //         let received = lock.lock().unwrap();
    //         let timeout = Duration::from_secs(10);

    //         let (received, timeout_result) = cvar
    //             .wait_timeout_while(received, timeout, |received| !*received)
    //             .unwrap();

    //         if timeout_result.timed_out() {
    //             info!("Timed out waiting for transaction_reply_callback");
    //         }

    //         *received
    //     };

    //     if !order_received && !trade_received && !transaction_received {
    //         info!("Did not receive all expected callbacks");
    //     }
    // }

    terminal.unsubscribe_orders()?;
    terminal.unsubscribe_trades()?;
    terminal.disconnect()?;

    let connection_str = "host=localhost user=postgres dbname=jym-Nut-setvam";
    let database = psql::Db::new(connection_str).await?;
    database.init().await?;

    let instrument_code = "LKOH";
    let short_period_quantity = 80 as usize;
    let short_period_len: f64 = (1 * 60) as f64;
    let short_interval: f64 = short_period_quantity as f64 * short_period_len as f64;

    let long_period_quantity = 210 as usize;
    let long_period_len: f64 = (1 * 60) as f64;
    let long_interval: f64 = long_period_quantity as f64 * long_period_len as f64;

    let hysteresis_percentage = 0.03; // 1% гистерезис
    let hysteresis_periods = 3; // 3 периода гистерезиса
    let mut crossover_signal =
        signal::CrossoverSignal::new(hysteresis_percentage, hysteresis_periods);

    loop {
        let short_ema = ema::Ema::calc(
            &database,
            &terminal,
            instrument_code,
            short_interval,
            short_period_len,
            short_period_quantity,
        )
        .await?;
        let long_ema = ema::Ema::calc(
            &database,
            &terminal,
            instrument_code,
            long_interval,
            long_period_len,
            long_period_quantity,
        )
        .await?;

        if let Some(signal) = crossover_signal.update(short_ema, long_ema) {
            match signal {
                signal::Signal::Buy => {
                    // Логика для сигнала на покупку
                    info!("Сигнал на покупку!");
                    // let transaction_str = "ACCOUNT=NL0011100043; CLIENT_CODE=10677; TYPE=L; TRANS_ID=1; CLASSCODE=QJSIM; SECCODE=LKOH; ACTION=NEW_ORDER; OPERATION=B; PRICE=6978,0; QUANTITY=1;";
                    let transaction_str = "ACCOUNT=NL0011100043; CLIENT_CODE=10677; TYPE=M; TRANS_ID=1; CLASSCODE=QJSIM; SECCODE=LKOH; ACTION=NEW_ORDER; OPERATION=B; QUANTITY=1;";
                    terminal.send_async_transaction(transaction_str)?;
                }
                signal::Signal::Sell => {
                    // Логика для сигнала на продажу
                    info!("Сигнал на продажу!");
                    let transaction_str = "ACCOUNT=NL0011100043; CLIENT_CODE=10677; TYPE=M; TRANS_ID=1; CLASSCODE=QJSIM; SECCODE=LKOH; ACTION=NEW_ORDER; OPERATION=S; QUANTITY=1;";
                    terminal.send_async_transaction(transaction_str)?;
                }
            }
        }

        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    Ok(())
}
