//! # GUI for QUIK bot.
//! This module implements a graphical user interface for a bot working with the QUIK trading platform.
//! It leverages the eframe library for GUI creation and tokio for asynchronous command handling.
use crate::psql::Db;
use crate::psql::Ema;
use crate::psql::Instrument;
use crate::psql::Operation;
use bb8::RunError;
use chrono::DateTime;
use chrono::NaiveDateTime;
use eframe::egui;
use egui::Color32;
use egui_plot::GridMark;
use egui_plot::{Line, MarkerShape, Plot, PlotPoints, Points};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tokio::sync::RwLock;
use tracing::{error, info};

type EmaResult = Result<Vec<Ema>, RunError<bb8_postgres::tokio_postgres::Error>>;
type SharedEmaData = Arc<Mutex<Option<EmaResult>>>;

/// AppCommand represents the possible commands to control the QUIK Terminal state.
///
/// Variants:
/// - Shutdown: a command to initiate the shutdown process of the QUIk Terminal.
pub enum AppCommand {
    Shutdown,
}

/// MyApp is the main structure representing the application state and behavior.
///
/// Fields:
/// - show_confirmation_dialog: a boolean indicating if the confirmation dialog should be shown.
/// - allowed_to_close: a flag to mark if the application can be closed.
/// - command_sender: a channel sender used to transmit commands to other parts of the application.
pub struct MyApp {
    show_confirmation_dialog: bool,
    allowed_to_close: bool,
    command_sender: mpsc::UnboundedSender<AppCommand>,
    database: Arc<Db>,
    ema_data: SharedEmaData,
    plot_sec_codes: Arc<Mutex<Vec<String>>>,
    plot_sec_code: Arc<Mutex<String>>,
}

impl MyApp {
    /// Constructs a new MyApp instance.
    ///
    /// # Arguments
    ///
    /// * command_sender - The channel sender for passing AppCommand messages to the main loop.
    ///
    /// # Returns
    ///
    /// A new instance of MyApp with the initial dialog state set to not show and close permission denied.
    pub fn new(
        command_sender: mpsc::UnboundedSender<AppCommand>,
        database: Arc<Db>,
        instruments: Arc<RwLock<Vec<Instrument>>>,
    ) -> Self {
        let plot_sec_codes = Arc::new(Mutex::new(Vec::new()));
        let plot_sec_codes_cloned = Arc::clone(&plot_sec_codes);

        tokio::spawn(async move {
            let instruments = instruments.read().await;
            let mut plot_sec_codes = plot_sec_codes_cloned.lock().unwrap();
            for instrument in instruments.iter() {
                plot_sec_codes.push(instrument.sec_code.clone());
            }
        });

        let plot_sec_code = Arc::new(Mutex::new(String::from("SBER")));

        let app = Self {
            show_confirmation_dialog: false,
            allowed_to_close: false,
            command_sender,
            database,
            ema_data: Arc::new(Mutex::new(None)),
            plot_sec_codes,
            plot_sec_code,
        };

        app.start_periodic_fetch();
        app
    }

    /// Fetches EMA data asynchronously.
    fn fetch_ema_data(&self) {
        let database = Arc::clone(&self.database);
        let ema_data = Arc::clone(&self.ema_data);
        let plot_sec_code = Arc::clone(&self.plot_sec_code); // Clone the Arc<Mutex<String>>
        tokio::spawn(async move {
            info!("start fetch");
            let sec_code = plot_sec_code.lock().unwrap().clone(); // Lock the Mutex and clone the String
            let ema = database.get_ema(&sec_code).await; // Pass the &String as expected
            *ema_data.lock().unwrap() = Some(ema);
        });
    }

    /// Starts a periodic task to fetch EMA data.
    fn start_periodic_fetch(&self) {
        let database = Arc::clone(&self.database);
        let ema_data = Arc::clone(&self.ema_data);
        let plot_sec_code = Arc::clone(&self.plot_sec_code); // Clone the Arc<Mutex<String>>
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(5));
            loop {
                info!("start periodic fetch");
                interval.tick().await;
                let sec_code = plot_sec_code.lock().unwrap().clone(); // Lock the Mutex and clone the String
                let ema = database.get_ema(&sec_code).await; // Pass the &String as expected
                *ema_data.lock().unwrap() = Some(ema);
            }
        });
    }
}

impl eframe::App for MyApp {
    /// Updates the application state and renders the user interface.
    ///
    /// # Arguments
    ///
    /// * ctx - The egui context used for drawing the user interface.
    /// * _frame - The current frame, used for various frame operations.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // let mut fetch_required = false;

        // egui::SidePanel::left("my_left_panel")
        //     .resizable(false)
        //     .show(ctx, |ui| {
        //         let plot_sec_codes = self.plot_sec_codes.lock().unwrap();
        //         let mut plot_sec_code = self.plot_sec_code.lock().unwrap(); // Lock the plot_sec_code for modification

        //         egui::ComboBox::from_label("Select instrument")
        //             .selected_text(&*plot_sec_code)
        //             .show_ui(ui, |ui| {
        //                 for code in plot_sec_codes.iter() {
        //                     if ui
        //                         .selectable_value(&mut *plot_sec_code, code.clone(), code)
        //                         .clicked()
        //                     {
        //                         fetch_required = true;
        //                     }
        //                 }
        //             });
        //     });

        // if fetch_required {
        //     self.fetch_ema_data();
        // }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Make some money");
        });

        // Display the central panel with a heading.
        // egui::CentralPanel::default().show(ctx, |ui| {
        //     ui.heading("Make some money");

        //     // Trigger data fetching if not yet fetched
        //     if self.ema_data.lock().unwrap().is_none() {
        //         self.fetch_ema_data();
        //     }

        //     // Render the EMA plot if data is available
        //     if let Some(Ok(ema)) = &*self.ema_data.lock().unwrap() {
        //         let sec_code = if let Some(first_ema) = ema.first() {
        //             &first_ema.sec_code
        //         } else {
        //             "Unknown"
        //         };

        //         let short_ema_points: PlotPoints = ema
        //             .iter()
        //             .map(|e| {
        //                 let datetime: NaiveDateTime = e.timestamp;
        //                 [datetime.and_utc().timestamp_millis() as f64, e.short_ema]
        //             })
        //             .collect();

        //         let short_ema_line = Line::new(short_ema_points)
        //             .color(Color32::RED)
        //             .name("Short EMA");

        //         let long_ema_points: PlotPoints = ema
        //             .iter()
        //             .map(|e| {
        //                 let datetime: NaiveDateTime = e.timestamp;
        //                 [datetime.and_utc().timestamp_millis() as f64, e.long_ema]
        //             })
        //             .collect();

        //         let long_ema_line = Line::new(long_ema_points)
        //             .color(Color32::GREEN)
        //             .name("Long EMA");

        //         let last_price_points: PlotPoints = ema
        //             .iter()
        //             .map(|e| {
        //                 let datetime: NaiveDateTime = e.timestamp;
        //                 [datetime.and_utc().timestamp_millis() as f64, e.last_price]
        //             })
        //             .collect();

        //         let last_price_line = Line::new(last_price_points)
        //             .color(Color32::BLUE)
        //             .name(sec_code);

        //         let transaction_reply_points: PlotPoints = ema
        //             .iter()
        //             .filter(|e| e.operation == Operation::TransactionReply)
        //             .map(|e| {
        //                 let datetime: NaiveDateTime = e.timestamp;
        //                 [datetime.and_utc().timestamp_millis() as f64, e.last_price]
        //             })
        //             .collect();

        //         let transaction_reply_markers = Points::new(transaction_reply_points)
        //             .name("Transaction Reply")
        //             .color(Color32::ORANGE)
        //             .shape(MarkerShape::Asterisk)
        //             .filled(true)
        //             .radius(5.0);

        //         let signal_buy_points: PlotPoints = ema
        //             .iter()
        //             .filter(|e| e.operation == Operation::SignalBuy)
        //             .map(|e| {
        //                 let datetime: NaiveDateTime = e.timestamp;
        //                 [datetime.and_utc().timestamp_millis() as f64, e.last_price]
        //             })
        //             .collect();

        //         let signal_buy_markers = Points::new(signal_buy_points)
        //             .name("Signal Buy")
        //             .color(Color32::GREEN)
        //             .shape(MarkerShape::Circle)
        //             .filled(true)
        //             .radius(5.0);

        //         let signal_sell_points: PlotPoints = ema
        //             .iter()
        //             .filter(|e| e.operation == Operation::SignalSell)
        //             .map(|e| {
        //                 let datetime: NaiveDateTime = e.timestamp;
        //                 [datetime.and_utc().timestamp_millis() as f64, e.last_price]
        //             })
        //             .collect();

        //         let signal_sell_markers = Points::new(signal_sell_points)
        //             .name("Signal Sell")
        //             .color(Color32::RED)
        //             .shape(MarkerShape::Circle)
        //             .filled(true)
        //             .radius(5.0);

        //         let order_buy_points: PlotPoints = ema
        //             .iter()
        //             .filter(|e| e.operation == Operation::OrderBuy)
        //             .map(|e| {
        //                 let datetime: NaiveDateTime = e.timestamp;
        //                 [datetime.and_utc().timestamp_millis() as f64, e.last_price]
        //             })
        //             .collect();

        //         let order_buy_markers = Points::new(order_buy_points)
        //             .name("Order Buy")
        //             .color(Color32::GREEN)
        //             .shape(MarkerShape::Cross)
        //             .filled(true)
        //             .radius(5.0);

        //         let order_sell_points: PlotPoints = ema
        //             .iter()
        //             .filter(|e| e.operation == Operation::OrderSell)
        //             .map(|e| {
        //                 let datetime: NaiveDateTime = e.timestamp;
        //                 [datetime.and_utc().timestamp_millis() as f64, e.last_price]
        //             })
        //             .collect();

        //         let order_sell_markers = Points::new(order_sell_points)
        //             .name("Order Sell")
        //             .color(Color32::RED)
        //             .shape(MarkerShape::Cross)
        //             .filled(true)
        //             .radius(5.0);

        //         let trade_buy_points: PlotPoints = ema
        //             .iter()
        //             .filter(|e| e.operation == Operation::TradeBuy)
        //             .map(|e| {
        //                 let datetime: NaiveDateTime = e.timestamp;
        //                 [datetime.and_utc().timestamp_millis() as f64, e.last_price]
        //             })
        //             .collect();

        //         let trade_buy_markers = Points::new(trade_buy_points)
        //             .name("Trade Buy")
        //             .color(Color32::GREEN)
        //             .shape(MarkerShape::Up)
        //             .filled(true)
        //             .radius(5.0);

        //         let trade_sell_points: PlotPoints = ema
        //             .iter()
        //             .filter(|e| e.operation == Operation::TradeSell)
        //             .map(|e| {
        //                 let datetime: NaiveDateTime = e.timestamp;
        //                 [datetime.and_utc().timestamp_millis() as f64, e.last_price]
        //             })
        //             .collect();

        //         let trade_sell_markers = Points::new(trade_sell_points)
        //             .name("Trade Sell")
        //             .color(Color32::RED)
        //             .shape(MarkerShape::Down)
        //             .filled(true)
        //             .radius(5.0);

        //         Plot::new("EMA Plot")
        //             .view_aspect(2.0)
        //             .x_axis_formatter(|mark: GridMark, _range: &std::ops::RangeInclusive<f64>| {
        //                 let datetime = DateTime::from_timestamp_millis(mark.value as i64)
        //                     .expect("Invalid timestamp");
        //                 datetime.format("%Y-%m-%d %H:%M:%S").to_string()
        //             })
        //             .label_formatter(|name, value| {
        //                 let datetime = DateTime::from_timestamp_millis(value.x as i64)
        //                     .expect("Invalid timestamp");
        //                 format!(
        //                     "{}\n{:.4}\n{}",
        //                     name,
        //                     value.y,
        //                     datetime.format("%Y-%m-%d %H:%M:%S")
        //                 )
        //             })
        //             .show(ui, |plot_ui| {
        //                 plot_ui.line(short_ema_line);
        //                 plot_ui.line(long_ema_line);
        //                 plot_ui.line(last_price_line);
        //                 plot_ui.points(transaction_reply_markers);
        //                 plot_ui.points(signal_buy_markers);
        //                 plot_ui.points(signal_sell_markers);
        //                 plot_ui.points(order_buy_markers);
        //                 plot_ui.points(order_sell_markers);
        //                 plot_ui.points(trade_buy_markers);
        //                 plot_ui.points(trade_sell_markers);
        //             });
        //     }
        // });

        // Handle close request from the viewport.
        if ctx.input(|i| i.viewport().close_requested()) {
            if self.allowed_to_close {
                info!("GUI has been signaled to shut down");
                if let Err(err) = self.command_sender.send(AppCommand::Shutdown) {
                    error!("failed to send shutdown command: {}", err);
                }
            } else {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.show_confirmation_dialog = true;
            }
        }

        // Display confirmation dialog if requested.
        if self.show_confirmation_dialog {
            egui::Window::new("Do you want to quit?")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        if ui.button("No").clicked() {
                            self.show_confirmation_dialog = false;
                            self.allowed_to_close = false;
                        }
                        if ui.button("Yes").clicked() {
                            self.show_confirmation_dialog = false;
                            self.allowed_to_close = true;
                            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });
                });
        }
    }
}
