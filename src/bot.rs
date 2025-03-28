//! # bot work for algo trading
use crate::app::AppCommand;
use crate::ema;
use crate::config::Config;
use crate::psql::Db;
use crate::psql::Instrument;
use crate::psql::Operation;
// use crate::quik::IsSell;
use crate::quik::OrderInfo;
use crate::quik::Terminal;
use crate::quik::TradeInfo;
use crate::quik::TransactionInfo;
use crate::quik::ORDER_STATUS_SENDER;
use crate::quik::TRADE_STATUS_SENDER;
use crate::quik::TRANSACTION_REPLY_SENDER;
use crate::signal::Signal;
use crate::tg::TgBot;
use chrono::{Datelike, NaiveDateTime, Timelike, Utc, Weekday};
use std::error::Error;
use std::sync::Arc;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::sync::Mutex;
use tokio::sync::MutexGuard as TokioMutexGuard;
use tokio::sync::RwLock;
use tokio::time::{sleep, Duration};
use tracing::{error, info};

fn _transaction_str(sec_code: &str, operation: &str) -> Result<String, &'static str> {
    if sec_code.is_empty() {
        return Err("SECCODE cannot be empty");
    }
    if operation.is_empty() {
        return Err("OPERATION cannot be empty");
    }

    let template = "ACCOUNT=NL0122200034; CLIENT_CODE=10127; TYPE=M; TRANS_ID=1; CLASSCODE=QJSIM; SECCODE=; ACTION=NEW_ORDER; OPERATION=; QUANTITY=1;";
    let replaced_sec_code = template.replace("SECCODE=", &format!("SECCODE={};", sec_code));
    let transaction = replaced_sec_code.replace("OPERATION=", &format!("OPERATION={};", operation));

    Ok(transaction)
}

fn _process_transaction(terminal_guard: TokioMutexGuard<'_, Terminal>, transaction_str: &str) {
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

/// Returns the trading start and end hours based on the day of the week.
fn trading_hours(weekday: Weekday) -> (u32, u32) {
    if is_weekday(weekday) {
        // For weekdays, trading starts after 3h and ends before 24h.
        (3, 24)
    } else {
        // For weekends, trading starts after 6h and ends before 16h.
        (6, 24)
    }
}

/// Checks whether the current time corresponds to the trading schedule.
fn is_trading_time() -> bool {
    let now = Utc::now();
    let current_hour = now.hour();
    let (start_hour, end_hour) = trading_hours(now.weekday());

    // Trading is active if time is after start_hour and before end_hour.
    current_hour > start_hour && current_hour < end_hour
}

pub async fn trade(
    mut command_receiver: mpsc::UnboundedReceiver<AppCommand>,
    database: Arc<Db>,
    instruments: Arc<RwLock<Vec<Instrument>>>,
    config: Config,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Создание нового экземпляра TgBot
    let tg_bot = TgBot::new(&config.tg_token);
    tg_bot.start_message_listener().await;

    // Preparing to work with QUIK
    let path_to_lib = &config.path_to_lib;
    let path_to_quik = &config.path_to_quik;
    let class_code = &config.class_code;
    let sec_code = &config.sec_code;

    let terminal = Terminal::new(path_to_lib, path_to_quik)?;
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
    let timeframe: i32 = config.timeframe;
    let short_num_of_candles: i32 = config.short_num_of_candles;
    let long_num_of_candles: i32 = config.long_num_of_candles;

    // Инициализируем канал
    let (transaction_sender, mut transaction_receiver): (
        UnboundedSender<TransactionInfo>,
        UnboundedReceiver<TransactionInfo>,
    ) = mpsc::unbounded_channel();

    // Инициализируем TRANSACTION_REPLY_SENDER
    {
        let mut transaction_reply_sender = TRANSACTION_REPLY_SENDER.lock().unwrap();
        *transaction_reply_sender = Some(transaction_sender);
    }

    // Инициализируем канал
    let (order_sender, mut order_receiver): (
        UnboundedSender<OrderInfo>,
        UnboundedReceiver<OrderInfo>,
    ) = mpsc::unbounded_channel();

    // Инициализируем ORDER_STATUS_SENDER
    {
        let mut order_status_sender = ORDER_STATUS_SENDER.lock().unwrap();
        *order_status_sender = Some(order_sender);
    }

    // Инициализируем канал
    let (trade_sender, mut trade_receiver): (
        UnboundedSender<TradeInfo>,
        UnboundedReceiver<TradeInfo>,
    ) = mpsc::unbounded_channel();

    // Инициализируем TRADE_SENDER
    {
        let mut trade_status_sender = TRADE_STATUS_SENDER.lock().unwrap();
        *trade_status_sender = Some(trade_sender);
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
            // Some(transaction_info) = transaction_receiver.recv() => {
            //     info!("transaction_reply_callback received: {:?}", transaction_info);
            //         // Calculate the short EMA
            //         let short_ema = match ema::Ema::calc(
            //             &database,
            //             &transaction_info.sec_code,
            //             timeframe,
            //             short_number_of_candles,
            //         ).await {
            //             Ok(short_ema) => {
            //                 info!("short_ema: {}", short_ema);
            //                 short_ema
            //             }
            //             Err(e) => {
            //                 error!("{}", e);
            //                 continue;
            //             }
            //         };

            //         // Calculate the long EMA
            //         let long_ema = match ema::Ema::calc(
            //             &database,
            //             &transaction_info.sec_code,
            //             timeframe,
            //             long_number_of_candles,
            //         ).await {
            //             Ok(long_ema) => {
            //                 info!("long_ema: {}", long_ema);
            //                 long_ema
            //             }
            //             Err(e) => {
            //                 error!("{}", e);
            //                 continue;
            //             }
            //         };

            //         let operation = Operation::TransactionReply;

            //         let update_timestamp: NaiveDateTime = Utc::now().naive_utc();

            //         if let Err(e) = &database.insert_ema(&transaction_info.sec_code, short_ema, long_ema, transaction_info.price, operation, update_timestamp).await {
            //             error!("insert into ema error: {}", e);
            //         }
            // },
            // Some(order_info) = order_receiver.recv() => {
            //     info!("order_status_callback received: {:?}", order_info);
            //     if order_info.is_valid() {
            //         // Calculate the short EMA
            //         let short_ema = match ema::Ema::calc(
            //             &database,
            //             &order_info.sec_code,
            //             timeframe,
            //             short_number_of_candles,
            //         ).await {
            //             Ok(short_ema) => {
            //                 info!("short_ema: {}", short_ema);
            //                 short_ema
            //             }
            //             Err(e) => {
            //                 error!("{}", e);
            //                 continue;
            //             }
            //         };

            //         // Calculate the long EMA
            //         let long_ema = match ema::Ema::calc(
            //             &database,
            //             &order_info.sec_code,
            //             timeframe,
            //             long_number_of_candles,
            //         ).await {
            //             Ok(long_ema) => {
            //                 info!("long_ema: {}", long_ema);
            //                 long_ema
            //             }
            //             Err(e) => {
            //                 error!("{}", e);
            //                 continue;
            //             }
            //         };

            //         let operation = match order_info.is_sell {
            //             IsSell::Buy => Operation::OrderBuy,
            //             IsSell::Sell => Operation::OrderSell,
            //         };

            //         let update_timestamp = NaiveDateTime::new(order_info.date, order_info.time);

            //         if let Err(e) = &database.insert_ema(&order_info.sec_code, short_ema, long_ema, order_info.price, operation, update_timestamp).await {
            //             error!("insert into ema error: {}", e);
            //         }
            //     } else {
            //         error!("order_info invalid");
            //     }
            // },
            // Some(trade_info) = trade_receiver.recv() => {
            //     info!("trade_status_callback received: {:?}", trade_info);
            //     if trade_info.is_valid() {
            //         // Calculate the short EMA
            //         let short_ema = match ema::Ema::calc(
            //             &database,
            //             &trade_info.sec_code,
            //             timeframe,
            //             short_number_of_candles,
            //         ).await {
            //             Ok(short_ema) => {
            //                 info!("short_ema: {}", short_ema);
            //                 short_ema
            //             }
            //             Err(e) => {
            //                 error!("{}", e);
            //                 continue;
            //             }
            //         };

            //         // Calculate the long EMA
            //         let long_ema = match ema::Ema::calc(
            //             &database,
            //             &trade_info.sec_code,
            //             timeframe,
            //             long_number_of_candles,
            //         ).await {
            //             Ok(long_ema) => {
            //                 info!("long_ema: {}", long_ema);
            //                 long_ema
            //             }
            //             Err(e) => {
            //                 error!("{}", e);
            //                 continue;
            //             }
            //         };

            //         let operation = match trade_info.is_sell {
            //             IsSell::Buy => Operation::TradeBuy,
            //             IsSell::Sell => Operation::TradeSell,
            //         };

            //         let update_timestamp = NaiveDateTime::new(trade_info.date, trade_info.time);

            //         if let Err(e) = &database.insert_ema(&trade_info.sec_code, short_ema, long_ema, trade_info.price, operation, update_timestamp).await {
            //             error!("insert into ema error: {}", e);
            //         }
            //     } else {
            //         error!("trade_info invalid");
            //     }
            // },
            result = async {
                if is_trading_time() {
                    let mut instruments = instruments.write().await;
                    for instrument in instruments.iter_mut() {
                            // Get access to the terminal
                            // let terminal_guard = terminal.lock().await;

                            // Calculate the short EMA
                            let short_ema = match ema::Ema::calc(
                                &database,
                                &instrument.sec_code,
                                timeframe,
                                short_num_of_candles,
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
                                long_num_of_candles,
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

                            // let operation = Operation::IsNone;

                            // let update_timestamp: NaiveDateTime = Utc::now().naive_utc();

                            // if let Err(e) = &database.insert_ema(&instrument.sec_code, short_ema, long_ema, last_price, operation, update_timestamp).await {
                            //     error!("insert into ema error: {}", e);
                            // }

                            // Updating the golden cross/death cross signal
                            if let Some(signal) = instrument.crossover_signal.update(short_ema, long_ema) {
                                // let operation = match signal {
                                //     Signal::Buy => "B",
                                //     Signal::Sell => "S",
                                // };
                                info!("{} => {:?}", instrument.sec_code, signal);

                                // match transaction_str(&instrument.sec_code, operation) {
                                //     Ok(transaction_str) => {
                                //         process_transaction(terminal_guard, &transaction_str);
                                //         let operation = match signal {
                                //             Signal::Buy => Operation::SignalBuy,
                                //             Signal::Sell => Operation::SignalSell,
                                //         };

                                //         let update_timestamp: NaiveDateTime = Utc::now().naive_utc();

                                //         if let Err(e) = database.insert_ema(&instrument.sec_code, short_ema, long_ema, last_price, operation, update_timestamp).await {
                                //             error!("insert into ema error: {}", e);
                                //         }

                                //         // Отправка сообщения всем подписчикам
                                //         let message = format!("Торговый сигнал для {}: {:?}, цена: {}", instrument.sec_code, signal, last_price);
                                //         tg_bot.broadcast(&message).await;
                                //     }
                                //     Err(e) => error!("create transaction_str error: {}", e),
                                // }

                                let operation = match signal {
                                    Signal::Buy => Operation::SignalBuy,
                                    Signal::Sell => Operation::SignalSell,
                                };

                                let update_timestamp: NaiveDateTime = Utc::now().naive_utc();

                                if let Err(e) = database.insert_ema(&instrument.sec_code, short_ema, long_ema, last_price, operation, update_timestamp).await {
                                    error!("insert into ema error: {}", e);
                                }

                                // Отправка сообщения всем подписчикам
                                let message = format!("Торговый сигнал для {}: {:?}, цена: {}", instrument.sec_code, signal, last_price);
                                tg_bot.broadcast(&message).await;
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
