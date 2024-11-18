//! # Application for algorithmic trading on the MOEX via the QUIK terminal.
// #![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use lazy_static::lazy_static;
use std::error::Error;
// use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;
use tracing::info;
use tracing_subscriber;
use crate::signal::{Signal, CrossoverSignal};
mod ema;
mod psql;
mod quik;
mod signal;
mod app;
mod bot;
use eframe::egui;
use std::sync::mpsc::{channel, Sender};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> eframe::Result<()> {
    tracing_subscriber::fmt::init();

    let shutdown_signal = Arc::new(AtomicBool::new(false));
    let app_shutdown_signal = Arc::clone(&shutdown_signal);
    let (command_sender, command_receiver) = mpsc::unbounded_channel();
    let mut command_receiver = command_receiver;

    // Spawn your asynchronous task using tokio::spawn
    let loop_shutdown_signal = Arc::clone(&shutdown_signal);
    tokio::spawn(async move {
        bot::trade(loop_shutdown_signal, command_receiver).await;
    });

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([320.0, 240.0]),
        ..Default::default()
    };

    eframe::run_native(
        "QUIK bot",
        options,
        Box::new(|_cc| {
            Ok(Box::new(app::MyApp::new(app_shutdown_signal, command_sender.clone())))
        }),
    )?;

    // Signal shutdown after the application exits
    shutdown_signal.store(true, Ordering::Relaxed);

    Ok(())
}