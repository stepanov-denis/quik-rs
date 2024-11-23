use crate::app::AppCommand;
use crate::ema;
use crate::psql;
use crate::quik::Terminal;
use crate::signal::Signal;
// use egui::mutex::MutexGuard;
use std::error::Error;
use std::sync::{
    // atomic::AtomicBool,
    Arc,
};
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use tokio::sync::MutexGuard as TokioMutexGuard;
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
    let short_period_quantity = 8 as usize;
    let short_period_len: f64 = (1 * 60) as f64;
    let short_interval: f64 = short_period_quantity as f64 * short_period_len as f64;

    let long_period_quantity = 21 as usize;
    let long_period_len: f64 = (1 * 60) as f64;
    let long_interval: f64 = long_period_quantity as f64 * long_period_len as f64;

    loop {
        tokio::select! {
            Some(command) = command_receiver.recv() => {
                match command {
                    AppCommand::Shutdown => {
                        info!("shutdown signal");
                        // Access to terminal via Mutex
                        let terminal_guard = terminal.lock().await;

                        if let Err(err) = terminal_guard.unsubscribe_orders() {
                            eprintln!("error unsubscribing from orders: {}", err);
                        }
                        if let Err(err) = terminal_guard.unsubscribe_trades() {
                            eprintln!("error unsubscribing from trades: {}", err);
                        }
                        if let Err(err) = terminal_guard.disconnect() {
                            eprintln!("error disconnecting: {}", err);
                        }

                        info!("shutdown sequence completed");
                        break;
                    }
                }
            },
            result = async {
                for instrument in &mut instruments {
                    // Get access to the terminal
                    let terminal_guard = terminal.lock().await;

                    // Calculate the short EMA
                    let short_ema_result = ema::Ema::calc(
                        &database,
                        &instrument.sec_code,
                        short_interval,
                        short_period_len,
                        short_period_quantity,
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
                        long_interval,
                        long_period_len,
                        long_period_quantity,
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
                // Pause before the next iteration
                info!("sleep");
                tokio::time::sleep(std::time::Duration::from_secs(60)).await;

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
