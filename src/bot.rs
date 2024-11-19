#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release
use lazy_static::lazy_static;
use std::error::Error;
// use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{info, error};
use tracing_subscriber;
use crate::quik::Terminal;
use crate::signal::{Signal, CrossoverSignal};
use crate::app::AppCommand;
use crate::psql;
use crate::ema;
use eframe::egui;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tokio::sync::Mutex;

pub async fn trade(shutdown_signal: Arc<AtomicBool>, mut command_receiver: mpsc::UnboundedReceiver<AppCommand>) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Preparing to work with QUIK
    let path = r"c:\QUIK Junior\trans2quik.dll";
    let terminal = Terminal::new(path)?;
    let terminal = Arc::new(Mutex::new(terminal));
    {
        let mut terminal_guard = terminal.lock().await;
        terminal_guard.connect()?;
        terminal_guard.is_dll_connected()?;
        terminal_guard.is_quik_connected()?;
        terminal_guard.set_connection_status_callback()?;
        terminal_guard.set_transactions_reply_callback()?;
        let class_code = "QJSIM";
        let sec_code = "LKOH";
        terminal_guard.subscribe_orders(class_code, sec_code)?;
        terminal_guard.subscribe_trades(class_code, sec_code)?;
        terminal_guard.start_orders();
        terminal_guard.start_trades();
    }

    // Preparing for work with PostgreSQL
    let connection_str = "host=localhost user=postgres dbname=postgres password=password";
    let database = psql::Db::new(connection_str).await?;
    database.init().await?;
    let class_code = "QJSIM";
    let instrument_status = "торгуется";
    let instruments = database.get_instruments(class_code, instrument_status).await?;

    // Preparing for trading
    let short_period_quantity = 8 as usize;
    let short_period_len: f64 = (1 * 60) as f64;
    let short_interval: f64 = short_period_quantity as f64 * short_period_len as f64;

    let long_period_quantity = 21 as usize;
    let long_period_len: f64 = (1 * 60) as f64;
    let long_interval: f64 = long_period_quantity as f64 * long_period_len as f64;

    let hysteresis_percentage = 0.03; // 1% гистерезис
    let hysteresis_periods = 3; // 3 периода гистерезиса
    let mut crossover_signal =
        CrossoverSignal::new(hysteresis_percentage, hysteresis_periods);
    
    loop {
        tokio::select! {
            Some(command) = command_receiver.recv() => {
                match command {
                    AppCommand::Shutdown => {
                        info!("Shutdown signal");
                        // Доступ к terminal через Mutex
                        let terminal_guard = terminal.lock().await;
                        // Выполняем необходимые асинхронные действия
                        if let Err(err) = terminal_guard.unsubscribe_orders() {
                            eprintln!("Error unsubscribing from orders: {}", err);
                        }
                        if let Err(err) = terminal_guard.unsubscribe_trades() {
                            eprintln!("Error unsubscribing from trades: {}", err);
                        }
                        if let Err(err) = terminal_guard.disconnect() {
                            eprintln!("Error disconnecting: {}", err);
                        }

                        info!("Shutdown sequence completed");
                        break;
                    }
                    // Обработка других команд
                }
            },
            result = async {
                for instrument in &instruments {
                    // Блок с торговой логикой, возвращающий Result
                    // Получаем доступ к terminal
                    let mut terminal_guard = terminal.lock().await;

                    // Вычисляем short EMA
                    let short_ema = ema::Ema::calc(
                        &database,
                        &terminal_guard,
                        &instrument.sec_code,
                        short_interval,
                        short_period_len,
                        short_period_quantity,
                    ).await?;

                    // Вычисляем long EMA
                    let long_ema = ema::Ema::calc(
                        &database,
                        &terminal_guard,
                        &instrument.sec_code,
                        long_interval,
                        long_period_len,
                        long_period_quantity,
                    ).await?;

                    // Обновляем сигнал пересечения
                    if let Some(signal) = crossover_signal.update(short_ema, long_ema) {
                        match signal {
                            Signal::Buy => {
                                info!("Сигнал на покупку!");
                                let transaction_str = "YOUR_TRANSACTION_STRING_FOR_BUY";
                                terminal_guard.send_async_transaction(transaction_str)?;
                            }
                            Signal::Sell => {
                                info!("Сигнал на продажу!");
                                let transaction_str = "YOUR_TRANSACTION_STRING_FOR_SELL";
                                terminal_guard.send_async_transaction(transaction_str)?;
                            }
                        }
                    }
                }
                // Пауза перед следующей итерацией
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;

                Ok::<(), Box<dyn Error + Send + Sync>>(())
            } => {
                match result {
                    Ok(_) => {
                        // Все прошло успешно, продолжаем цикл
                    }
                    Err(err) => {
                        // Обработка ошибки
                        error!("Ошибка в торговом алгоритме: {}", err);// Возможно, логично выйти из цикла при ошибке
                        break;
                    }
                }
            },
        }
    }

    Ok(())
}
