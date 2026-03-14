#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod geom;
mod gpu;
mod text;
mod ui;

fn main() {
    app::run();
}
