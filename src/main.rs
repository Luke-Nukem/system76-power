#[macro_use]
extern crate clap;
extern crate dbus;
extern crate fern;
extern crate intel_pstate as pstate;
extern crate itertools;
extern crate libc;
#[macro_use]
extern crate log;
extern crate serde;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate smart_default;
extern crate toml;
extern crate xdg;
extern crate sysfs_class;

use log::LevelFilter;
use std::process;

use clap::{Arg, App, AppSettings, SubCommand};
mod client;
mod config;
mod daemon;
mod disks;
mod fan;
mod graphics;
mod hotplug;
mod kernel_parameters;
mod logging;
mod modprobe;
mod module;
mod pci;
mod snd;
mod util;
mod wifi;

include!(concat!(env!("OUT_DIR"), "/version.rs"));

pub static DBUS_NAME: &'static str = "com.system76.PowerDaemon";
pub static DBUS_PATH: &'static str = "/com/system76/PowerDaemon";
pub static DBUS_IFACE: &'static str = "com.system76.PowerDaemon";

pub trait Power {
    fn performance(&mut self) -> Result<(), String>;
    fn balanced(&mut self) -> Result<(), String>;
    fn battery(&mut self) -> Result<(), String>;

    fn get_profile(&self) -> Result<String, String>;
    fn get_profiles(&self) -> Result<String, String>;
    fn set_profile(&mut self, profile: &str) -> Result<(), String>;

    fn set_fan_curve(&mut self, profile: &str) -> Result<(), String>;

    fn get_graphics(&mut self) -> Result<String, String>;
    fn set_graphics(&mut self, vendor: &str) -> Result<(), String>;
    fn get_graphics_power(&mut self) -> Result<bool, String>;
    fn set_graphics_power(&mut self, power: bool) -> Result<(), String>;
    fn auto_graphics_power(&mut self) -> Result<(), String>;

    fn get_switchable(&mut self) -> Result<bool, String>;
}

// Helper function for errors
pub (crate) fn err_str<E: ::std::fmt::Display>(err: E) -> String {
    format!("{}", err)
}

fn main() {
    let matches = App::new("system76-power")
        .about("Utility for managing graphics and power profiles")
        .version(env!("CARGO_PKG_VERSION"))
        .global_setting(AppSettings::ColoredHelp)
        .global_setting(AppSettings::UnifiedHelpMessage)
        .global_setting(AppSettings::VersionlessSubcommands)
        .global_setting(AppSettings::InferSubcommands)
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .arg(Arg::with_name("quiet")
            .short("q")
            .long("quiet")
            .help("Set the verbosity of daemon logs to 'off' [default is 'info']")
            .global(true)
            .group("verbosity"))
        .arg(Arg::with_name("verbose")
            .short("v")
            .long("verbose")
            .help("Set the verbosity of daemon logs to 'debug' [default is 'info']")
            .global(true)
            .group("verbosity"))
        // TODO: Implement brightness controls.
        // .subcommand(SubCommand::with_name("brightness")
        //     .about("change brightness of the screen or keyboard")
        //     .arg(Arg::with_name("brightness").possible_values(&["keyboard", "screen"]))
        //     .arg(Arg::with_name("value").validator(valid_percent))
        //     .arg(Arg::with_name("min")
        //         .long("min")
        //         .help("sets the minimum brightness between the current and new value")
        //         .requires("value"))
        //     .arg(Arg::with_name("max")
        //         .long("max")
        //         .help("sets the maximum brightness between the current and new value")
        //         .requires("value"))
        // )
        .subcommand(SubCommand::with_name("config")
            .about("Display and verify the system76-power configuration"))
        .subcommand(SubCommand::with_name("daemon")
            .about("Runs the program in daemon mode")
            .long_about("Registers a new DBUS service and starts an event loop\
                to listen for, and respond to, DBUS events from clients")
            .arg(Arg::with_name("experimental")
                .long("experimental")
                .help("enables experimental power-saving features"))
        )
        .subcommand(SubCommand::with_name("fan-curve")
            .about("Set a fan curve profile. Default is 'standard'")
            .arg(Arg::with_name("profile").required(true))
        )
        .subcommand(SubCommand::with_name("profile")
            .about("Query or set the power profile")
            .long_about("Queries or sets the power profile.\n\n \
                - If an argument is not provided, the power profile will be queried\n \
                - Otherwise, that profile will be set, if it is a valid profile")
            .arg(Arg::with_name("profile")
                .help("set the power profile")
                .required(false))
            .arg(Arg::with_name("list")
                .help("list available power profiles")
                .long("list")
                .short("l"))
        )
        .subcommand(SubCommand::with_name("graphics")
            .about("Query or set the graphics mode")
            .long_about("Query or set the graphics mode.\n\n \
                - If an argument is not provided, the graphics profile will be queried\n \
                - Otherwise, that profile will be set, if it is a valid profile")
            .subcommand(SubCommand::with_name("intel")
                .about("Set the graphics mode to Intel"))
            .subcommand(SubCommand::with_name("nvidia")
                .about("Set the graphics mode to NVIDIA"))
            .subcommand(SubCommand::with_name("switchable")
                .about("Determines if the system has switchable graphics"))
            .subcommand(SubCommand::with_name("power")
                .about("Query or set the discrete graphics power state")
                .arg(Arg::with_name("state")
                    .help("Set whether discrete graphics should be on or off")
                    .possible_values(&["auto", "off", "on"]))
            )
        )
        .get_matches();

    if let Err(why) = logging::setup_logging(
        if matches.is_present("verbose") {
            LevelFilter::Debug
        } else if matches.is_present("quiet") {
            LevelFilter::Off
        } else {
            LevelFilter::Info
        }
    ) {
        eprintln!("failed to set up logging: {}", why);
        process::exit(1);
    }

    let res = match matches.subcommand() {
        ("daemon", Some(matches)) => {
            if unsafe { libc::geteuid() } == 0 {
                daemon::daemon(matches.is_present("experimental"))
            } else {
                Err("must be run as root".to_string())
            }
        }
        (subcommand, Some(matches)) => client::client(subcommand, matches),
        _ => unreachable!()
    };

    match res {
        Ok(()) => (),
        Err(err) => {
            error!("{}", err);
            process::exit(1);
        }
    }
}
