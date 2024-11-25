//! # GUI for QUIK bot.
//! This module implements a graphical user interface for a bot working with the QUIK trading platform.
//! It leverages the eframe library for GUI creation and tokio for asynchronous command handling.
use tokio::sync::mpsc;
use tracing::{error, info};

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
    pub fn new(command_sender: mpsc::UnboundedSender<AppCommand>) -> Self {
        Self {
            show_confirmation_dialog: false,
            allowed_to_close: false,
            command_sender,
        }
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
        // Display the central panel with a heading.
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Make some money");
        });

        // Handle close request from the viewport.
        if ctx.input(|i| i.viewport().close_requested()) {
            if self.allowed_to_close {
                // Log that the GUI has been signaled to shut down.
                info!("GUI has been signaled to shut down");

                // Attempt to send a shutdown command through the channel.
                if let Err(err) = self.command_sender.send(AppCommand::Shutdown) {
                    error!("failed to send shutdown command: {}", err);
                }
            } else {
                // Cancel the close operation and show the confirmation dialog.
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
                        // Handle 'No' button click to dismiss the dialog.
                        if ui.button("No").clicked() {
                            self.show_confirmation_dialog = false;
                            self.allowed_to_close = false;
                        }

                        // Handle 'Yes' button click to allow closure.
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
