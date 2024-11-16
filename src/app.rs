use std::sync::mpsc::Sender;

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

pub struct MyApp {
    show_confirmation_dialog: bool,
    allowed_to_close: bool,
    shutdown_signal: Arc<AtomicBool>,
}

impl MyApp {
    pub fn new(shutdown_signal: Arc<AtomicBool>) -> Self {
        Self {
            show_confirmation_dialog: false,
            allowed_to_close: false,
            shutdown_signal,
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Try to close the window");
        });

        if ctx.input(|i| i.viewport().close_requested()) {
            if self.allowed_to_close {
                // Send the shutdown signal for QUIK to main
                self.shutdown_signal.store(true, Ordering::Relaxed);
            } else {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.show_confirmation_dialog = true;
            }
        }

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