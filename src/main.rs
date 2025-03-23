//! # Application for algorithmic trading on the MOEX via the QUIK terminal.
// use eframe::egui;
// use std::sync::{
//     atomic::{AtomicBool, Ordering},
//     Arc,
// };
use std::error::Error;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::RwLock;
use tracing::error;
use crate::config::Config;
mod app;
mod bot;
mod config;
mod ema;
mod psql;
mod quik;
mod signal;
mod tg;


#[tokio::main]
async fn main() -> eframe::Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt::init();

    let config = Config::new("config.yaml")?;

    let (command_sender, command_receiver) = mpsc::unbounded_channel();

    let database = Arc::new(psql::Db::new(&config.psql_conn_str).await?);
    database.init().await?;

    let instruments = Arc::new(RwLock::new(
        database
            .get_instruments(&config.class_code, &config.instrument_status, config.hysteresis_percentage, config.hysteresis_periods)
            .await?,
    ));

    let database_clone = Arc::clone(&database);
    let instruments_clone = Arc::clone(&instruments);

    tokio::spawn(async move {
        if let Err(e) = bot::trade(command_receiver, database_clone, instruments_clone, config).await {
            error!("something went wrong, bot error: {}", e);
        }
    });

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([640.0, 480.0]),
        ..Default::default()
    };

    let database_clone = Arc::clone(&database);
    let instruments_clone = Arc::clone(&instruments);

    eframe::run_native(
        "QUIK bot",
        options,
        Box::new(|_cc| {
            Ok(Box::new(app::MyApp::new(
                command_sender,
                database_clone,
                instruments_clone,
            )))
        }),
    )?;

    Ok(())
}
