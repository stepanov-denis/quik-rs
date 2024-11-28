//! # bot work for algo trading
use crate::app::AppCommand;
use crate::ema;
use crate::psql;
use crate::quik::Terminal;
use crate::signal::Signal;
use chrono::{Datelike, Timelike, Utc, Weekday};
use rust_decimal::prelude::ToPrimitive;
use std::error::Error;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use tokio::sync::MutexGuard as TokioMutexGuard;
use tokio::time::{sleep, Duration};
use tracing::{error, info};

fn transaction_str(sec_code: &str, operation: &str) -> Result<String, &'static str> {
    if sec_code.is_empty() {
        return Err("SECCODE cannot be empty");
    }
    if operation.is_empty() {
        return Err("OPERATION cannot be empty");
    }

    let template = "ACCOUNT=NL0011100043; CLIENT_CODE=10677; TYPE=M; TRANS_ID=1; CLASSCODE=QJSIM; SECCODE=; ACTION=NEW_ORDER; OPERATION=B; QUANTITY=1;";
    let replaced_sec_code = template.replace("SECCODE=", &format!("SECCODE={};", sec_code));
    let transaction = replaced_sec_code.replace("OPERATION=", &format!("OPERATION={};", operation));

    Ok(transaction)
}

fn process_transaction(terminal_guard: TokioMutexGuard<'_, Terminal>, transaction_str: &str) {
    let result = terminal_guard.send_async_transaction(transaction_str);

    match result {
        Ok(_) => {
            info!("transaction successfully sent: {}", transaction_str);
        }
        Err(e) => {
            error!("failed to send transaction '{}': {}", transaction_str, e);
        }
    }
}

/// Checks whether the specified day is a weekday (Monday - Friday).
fn is_weekday(weekday: Weekday) -> bool {
    matches!(
        weekday,
        Weekday::Mon | Weekday::Tue | Weekday::Wed | Weekday::Thu | Weekday::Fri
    )
}

/// Checks whether the current time is after the start of trading (01:05).
fn is_after_start_time(hour: u32, minute: u32) -> bool {
    hour > 1 || (hour == 1 && minute >= 5)
}

/// Checks whether the current time is until the end of trading (23:00).
fn is_before_end_time(hour: u32) -> bool {
    hour < 23
}

/// Checks whether the specified time corresponds to the trading schedule.
/// QUIK Junior's work schedule at the Technical Center:
/// * 4:05 UTC+03 - start of the MOEX Stock Market trading session emulator (Main Market sector),
///   active operations are available (placing and withdrawing orders).
/// * 02:00 UTC+03 - stopping the emulator of the trading session of the Moscow Stock Exchange Stock Market (sector "Main Market"),
///   the end of the application acceptance and execution period. Automatic withdrawal of all unsatisfied applications.
fn is_trading_time() -> bool {
    let now = Utc::now();
    is_weekday(now.weekday())
        && is_after_start_time(now.hour(), now.minute())
        && is_before_end_time(now.hour())
}

pub async fn trade(
    mut command_receiver: mpsc::UnboundedReceiver<AppCommand>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Preparing to work with QUIK
    let path = r"c:\QUIK Junior\trans2quik.dll";
    let terminal = Terminal::new(path)?;
    let terminal = Arc::new(Mutex::new(terminal));
    {
        let terminal_guard = terminal.lock().await;
        terminal_guard.connect()?;
        terminal_guard.is_dll_connected()?;
        terminal_guard.is_quik_connected()?;
        terminal_guard.set_connection_status_callback()?;
        terminal_guard.set_transactions_reply_callback()?;
        let class_code = "";
        let sec_code = "";
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
    let mut instruments = database
        .get_instruments(class_code, instrument_status)
        .await?;

    // Preparing for trading
    let timeframe: i32 = 60; // minutes
    let short_number_of_candles: i32 = 8;
    let long_number_of_candles: i32 = 21;

    loop {
        tokio::select! {
            Some(command) = command_receiver.recv() => {
                match command {
                    AppCommand::Shutdown => {
                        info!("shutdown signal");
                        // Access to terminal via Mutex
                        let terminal_guard = terminal.lock().await;

                        if let Err(err) = terminal_guard.unsubscribe_orders() {
                            error!("error unsubscribing from orders: {}", err);
                        }
                        if let Err(err) = terminal_guard.unsubscribe_trades() {
                            error!("error unsubscribing from trades: {}", err);
                        }
                        if let Err(err) = terminal_guard.disconnect() {
                            error!("error disconnecting: {}", err);
                        }

                        info!("shutdown sequence completed");
                        break;
                    }
                }
            },
            result = async {
                if is_trading_time() {
                    for instrument in &mut instruments {
                        if instrument.sec_code == "SBER" {
                                                    // Get access to the terminal
                        let terminal_guard = terminal.lock().await;

                        // Calculate the short EMA
                        let short_ema_result = ema::Ema::calc(
                            &database,
                            &instrument.sec_code,
                            timeframe,
                            short_number_of_candles,
                        ).await;

                        let short_ema = match short_ema_result {
                            Ok(ema) => {
                                info!("short_ema: {}", ema);
                                ema
                            }
                            Err(e) => {
                                error!("{}", e);
                                continue;
                            }
                        };

                        // Calculate the long EMA
                        let long_ema_result = ema::Ema::calc(
                            &database,
                            &instrument.sec_code,
                            timeframe,
                            long_number_of_candles,
                        ).await;

                        let long_ema = match long_ema_result {
                            Ok(ema) => {
                                info!("long_ema: {}", ema);
                                ema
                            }
                            Err(e) => {
                                error!("{}", e);
                                continue;
                            }
                        };

                        // Updating the golden cross/death cross signal
                        if let Some(signal) = instrument.crossover_signal.update(short_ema, long_ema) {
                            match signal {
                                Signal::Buy => {
                                    info!("{} => {:?}", instrument.sec_code, signal);
                                    let operation = "B";
                                    let transaction_str = transaction_str(&instrument.sec_code, operation);
                                    match transaction_str {
                                        Ok(str) => {
                                            process_transaction(terminal_guard, &str);
                                        }
                                        Err(e) => error!("create transaction_str error: {}", e)
                                    }
                                }
                                Signal::Sell => {
                                    info!("{} => {:?}", instrument.sec_code, signal);
                                    let operation = "S";
                                    let transaction_str = transaction_str(&instrument.sec_code, operation);
                                    match transaction_str {
                                        Ok(str) => {
                                            process_transaction(terminal_guard, &str);
                                        }
                                        Err(e) => error!("create transaction_str error: {}", e)
                                    }
                                }
                            }
                        }
                        }

                    }
                } else {
                    info!("trading is paused, waiting for the next interval to check trading availability");
                }

                // Pause before the next iteration
                info!("sleep 60 seconds");
                sleep(Duration::from_secs(60)).await;

                Ok::<(), Box<dyn Error + Send + Sync>>(())
            } => {
                match result {
                    Ok(_) => {
                        // Everything went well, we continue the cycle
                    }
                    Err(err) => {
                        // Error Handling
                        error!("bot error: {}", err);
                        break;
                    }
                }
            },
        }
    }

    Ok(())
}
