//! # bot work for algo trading
use crate::app::AppCommand;
use crate::ema;
use crate::psql::Db;
use crate::psql::Instrument;
use crate::psql::Operation;
use crate::quik::IsSell;
use crate::quik::Terminal;
use crate::quik::TradeInfo;
use crate::quik::TRADE_SENDER;
use crate::signal::Signal;
use chrono::{Datelike, Timelike, Utc, Weekday};
use std::error::Error;
use std::sync::Arc;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::sync::Mutex;
use tokio::sync::MutexGuard as TokioMutexGuard;
use tokio::sync::RwLock;
use tokio::time::{sleep, Duration};
use tracing::{error, info};

fn transaction_str(sec_code: &str, operation: &str) -> Result<String, &'static str> {
    if sec_code.is_empty() {
        return Err("SECCODE cannot be empty");
    }
    if operation.is_empty() {
        return Err("OPERATION cannot be empty");
    }

    let template = "ACCOUNT=NL0011100043; CLIENT_CODE=10677; TYPE=M; TRANS_ID=1; CLASSCODE=QJSIM; SECCODE=; ACTION=NEW_ORDER; OPERATION=; QUANTITY=1;";
    let replaced_sec_code = template.replace("SECCODE=", &format!("SECCODE={};", sec_code));
    let transaction = replaced_sec_code.replace("OPERATION=", &format!("OPERATION={};", operation));

    Ok(transaction)
}

fn process_transaction(terminal_guard: TokioMutexGuard<'_, Terminal>, transaction_str: &str) {
    let result = terminal_guard.send_async_transaction(transaction_str);

    match result {
        Ok(_) => info!("transaction successfully sent: {}", transaction_str),
        Err(e) => error!("failed to send transaction '{}': {}", transaction_str, e),
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
    database: Arc<Db>,
    instruments: Arc<RwLock<Vec<Instrument>>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Preparing to work with QUIK
    let path = r"c:\QUIK Junior\trans2quik.dll";
    let class_code = "";
    let sec_code = "";

    let terminal = Terminal::new(path)?;
    let terminal = Arc::new(Mutex::new(terminal));
    {
        let terminal_guard = terminal.lock().await;
        terminal_guard.connect()?;
        terminal_guard.is_dll_connected()?;
        terminal_guard.is_quik_connected()?;
        terminal_guard.set_connection_status_callback()?;
        terminal_guard.set_transactions_reply_callback()?;
        terminal_guard.subscribe_orders(class_code, sec_code)?;
        terminal_guard.subscribe_trades(class_code, sec_code)?;
        terminal_guard.start_orders();
        terminal_guard.start_trades();
    }

    // Preparing for trading
    let timeframe: i32 = 30;
    let short_number_of_candles: i32 = 8;
    let long_number_of_candles: i32 = 21;

    // Инициализируем канал
    let (sender, mut receiver): (UnboundedSender<TradeInfo>, UnboundedReceiver<TradeInfo>) =
        mpsc::unbounded_channel();

    // Инициализируем TRADE_SENDER
    {
        let mut trade_sender = TRADE_SENDER.lock().unwrap();
        *trade_sender = Some(sender);
    }

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
            Some(trade_info) = receiver.recv() => {
                info!("trade_status_callback received: {:?}", trade_info);

                // Calculate the short EMA
                let short_ema = match ema::Ema::calc(
                    &database,
                    &trade_info.sec_code,
                    timeframe,
                    short_number_of_candles,
                ).await {
                    Ok(short_ema) => {
                        info!("short_ema: {}", short_ema);
                        short_ema
                    }
                    Err(e) => {
                        error!("{}", e);
                        continue;
                    }
                };

                // Calculate the long EMA
                let long_ema = match ema::Ema::calc(
                    &database,
                    &trade_info.sec_code,
                    timeframe,
                    long_number_of_candles,
                ).await {
                    Ok(long_ema) => {
                        info!("long_ema: {}", long_ema);
                        long_ema
                    }
                    Err(e) => {
                        error!("{}", e);
                        continue;
                    }
                };

                let last_price = match  database.get_last_price(&trade_info.sec_code).await {
                    Ok(last_price) => {
                        info!("last_price: {}", last_price);
                        last_price
                    }
                    Err(e) => {
                        error!("{}", e);
                        continue;
                    }
                };

                let is_sell = IsSell::from(trade_info.is_sell);

                let operation = match is_sell {
                    IsSell::Buy => Operation::TradeBuy,
                    IsSell::Sell => Operation::TradeSell,
                };

                if let Err(e) = &database.insert_ema(&trade_info.sec_code, &short_ema, &long_ema, &last_price, operation).await {
                    error!("insert into ema error: {}", e);
                }
            },
            result = async {
                if is_trading_time() {
                    let mut instruments = instruments.write().await;
                    for instrument in instruments.iter_mut() {
                        // if instrument.sec_code == "SBER" {
                            // Get access to the terminal
                            let terminal_guard = terminal.lock().await;

                            // Calculate the short EMA
                            let short_ema = match ema::Ema::calc(
                                &database,
                                &instrument.sec_code,
                                timeframe,
                                short_number_of_candles,
                            ).await {
                                Ok(short_ema) => {
                                    info!("short_ema: {}", short_ema);
                                    short_ema
                                }
                                Err(e) => {
                                    error!("{}", e);
                                    continue;
                                }
                            };

                            // Calculate the long EMA
                            let long_ema = match ema::Ema::calc(
                                &database,
                                &instrument.sec_code,
                                timeframe,
                                long_number_of_candles,
                            ).await {
                                Ok(long_ema) => {
                                    info!("long_ema: {}", long_ema);
                                    long_ema
                                }
                                Err(e) => {
                                    error!("{}", e);
                                    continue;
                                }
                            };

                            let last_price = match  database.get_last_price(&instrument.sec_code).await {
                                Ok(last_price) => {
                                    info!("last_price: {}", last_price);
                                    last_price
                                }
                                Err(e) => {
                                    error!("{}", e);
                                    continue;
                                }
                            };

                            if let Err(e) = &database.insert_ema(&instrument.sec_code, &short_ema, &long_ema, &last_price, Operation::None).await {
                                error!("insert into ema error: {}", e);
                            }

                            // Updating the golden cross/death cross signal
                            if let Some(signal) = instrument.crossover_signal.update(short_ema, long_ema) {
                                let operation = match signal {
                                    Signal::Buy => "B",
                                    Signal::Sell => "S",
                                };
                                info!("{} => {:?}", instrument.sec_code, signal);

                                match transaction_str(&instrument.sec_code, operation) {
                                    Ok(transaction_str) => {
                                        process_transaction(terminal_guard, &transaction_str);
                                        let operation = match signal {
                                            Signal::Buy => Operation::SignalBuy,
                                            Signal::Sell => Operation::SignalSell,
                                        };

                                        if let Err(e) = database.insert_ema(&instrument.sec_code, &short_ema, &long_ema, &last_price, operation).await {
                                            error!("insert into ema error: {}", e);
                                        }
                                    }
                                    Err(e) => error!("create transaction_str error: {}", e),
                                }
                            }
                        // }

                    }
                } else {
                    info!("trading is paused, waiting for the next interval to check trading availability");
                }

                // Pause before the next iteration
                info!("sleep 5 seconds");
                sleep(Duration::from_secs(5)).await;

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
