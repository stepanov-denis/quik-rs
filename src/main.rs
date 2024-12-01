//! # Application for algorithmic trading on the MOEX via the QUIK terminal.
use eframe::egui;
// use std::sync::{
//     atomic::{AtomicBool, Ordering},
//     Arc,
// };
use std::error::Error;
use std::sync::Arc;
use tokio::sync::mpsc;
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
    let database_clone = Arc::clone(&database);

    // Spawn your asynchronous task using tokio::spawn
    // let loop_shutdown_signal = Arc::clone(&shutdown_signal);
    tokio::spawn(async move {
        if let Err(e) = bot::trade(command_receiver, database_clone).await {
            error!("something went wrong, bot error: {}", e);
        }
    });

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([320.0, 240.0]),
        ..Default::default()
    };

    eframe::run_native(
        "QUIK bot",
        options,
        Box::new(|_cc| {
            Ok(Box::new(app::MyApp::new(
                command_sender.clone(),
                Arc::clone(&database),
            )))
        }),
    )?;

    Ok(())
}
