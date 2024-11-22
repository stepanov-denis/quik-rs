//! # Application for algorithmic trading on the MOEX via the QUIK terminal.
use eframe::egui;
// use std::sync::{
//     atomic::{AtomicBool, Ordering},
//     Arc,
// };
use tokio::sync::mpsc;
use tracing::error;
use tracing_subscriber;
mod app;
mod bot;
mod ema;
mod psql;
mod quik;
mod signal;

#[tokio::main]
async fn main() -> eframe::Result<()> {
    tracing_subscriber::fmt::init();

    // let shutdown_signal = Arc::new(AtomicBool::new(false));
    // let app_shutdown_signal = Arc::clone(&shutdown_signal);
    let (command_sender, command_receiver) = mpsc::unbounded_channel();
    // let mut command_receiver = command_receiver;

    // Spawn your asynchronous task using tokio::spawn
    // let loop_shutdown_signal = Arc::clone(&shutdown_signal);
    tokio::spawn(async move {
        let trade = bot::trade(command_receiver).await;
        match trade {
            Ok(_) => {}
            Err(e) => error!("Something went wrong, bot error: {}", e),
        }
    });

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([320.0, 240.0]),
        ..Default::default()
    };

    eframe::run_native(
        "QUIK bot",
        options,
        Box::new(|_cc| Ok(Box::new(app::MyApp::new(command_sender.clone())))),
    )?;

    // Signal shutdown after the application exits
    // shutdown_signal.store(true, Ordering::Relaxed);

    Ok(())
}
