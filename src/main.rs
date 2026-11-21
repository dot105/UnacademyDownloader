// =====================[ IMPORTS ]=====================
use clap::Parser; use uadl::Args;
// =====================[ MAIN ]=====================
fn main() { let args = Args::parse(); if let Err(e) = uadl::run_with_args(args) { eprintln!("\nerror: {e:#}"); std::process::exit(1) } }
