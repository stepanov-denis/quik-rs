//! # Application for algorithmic trading on the MOEX via the QUIK terminal.
use eframe::egui;
// use std::sync::{
//     atomic::{AtomicBool, Ordering},
//     Arc,
// };
use std::error::Error;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::RwLock;
use tracing::error;
mod app;
mod bot;
mod ema;
mod psql;
mod quik;
mod signal;

#[tokio::main]
async fn main() -> eframe::Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt::init();

    let (command_sender, command_receiver) = mpsc::unbounded_channel();

    let connection_str = "host=localhost user=postgres dbname=postgres password=password";
    let database = Arc::new(psql::Db::new(connection_str).await?);
    database.init().await?;

    let class_code = "QJSIM";
    let instrument_status = "торгуется";
    let instruments = Arc::new(RwLock::new(
        database
            .get_instruments(class_code, instrument_status)
            .await?,
    ));

    let database_clone = Arc::clone(&database);
    let instruments_clone = Arc::clone(&instruments);

    tokio::spawn(async move {
        if let Err(e) = bot::trade(command_receiver, database_clone, instruments_clone).await {
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
