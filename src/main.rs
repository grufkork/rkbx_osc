use std::{fs, rc::Rc};
use std::path::Path;
use log::{Logger, ScopedLogger};
use outputmodules::{ModuleDefinition, OutputModules};
use beatkeeper::BeatKeeper;

mod offsets;
use offsets::RekordboxOffsets;

mod outputmodules;

mod config;
mod log;
mod beatkeeper;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const REPO: &str = "grufkork/rkbx_osc/rewrite";


const CHARS: [&str; 4] = ["|", "/", "-", "\\"];

fn main() {

    println!();
    println!("=====================");
    println!("Rekordbox Link v{}", VERSION);
    println!("Repo     https://github.com/grufkork/rkbx_osc");
    println!("Updates  [BUY/DOWNLOAD LINK HERE]");
    println!("=====================");
    println!();

    let logger = Rc::new(Logger::new(true));

    let mut config = config::Config::read(ScopedLogger::new(&logger, "Config"));

    let logger = Rc::new(Logger::new(config.get_or_default("app.debug", true)));
    config.logger = ScopedLogger::new(&logger, "Config");
    let applogger = ScopedLogger::new(&logger, "App");


    let modules = vec![
        ModuleDefinition::new("link", "Ableton Link", outputmodules::abletonlink::AbletonLink::create),
        ModuleDefinition::new("osc", "OSC", outputmodules::osc::Osc::create),
    ];


    let mut update = config.get_or_default("app.auto_update", true);
    if !Path::new("offsets").exists() {
        applogger.err("No offset file found, updating...");
        update = true;
    }

    if update{
        update_routine(ScopedLogger::new(&logger, "Update"));
    }

    let Ok(offsets) = RekordboxOffsets::from_file("offsets") else {
        unimplemented!("Handle offset file parse error");
    };

    let mut versions: Vec<String> = offsets
        .keys()
        .map(|x| x.to_string())
        .collect();
    versions.sort();
    versions.reverse();

    applogger.info(&format!("Rekordbox versions available: {versions:?}"));

    let selected_version = if let Some(version) = config.get("keeper.rekordbox_version") {
        version
    }else{
        applogger.warn("No version specified in config, using latest version");
        versions[0].clone()
    };

    applogger.info(&format!("Targeting Rekordbox version: {}", selected_version));

    let offset = if let Some(offset) = offsets.get(&selected_version) {
        offset
    }else{
        applogger.err(&format!("Offsets for Rekordbox version {} not available", selected_version));
        return;
    };

    BeatKeeper::start(
        offset.clone(),
        modules,
        config,
        ScopedLogger::new(&logger, "BeatKeeper"),
    );


}

fn update_routine(logger: ScopedLogger){
    logger.info("Checking for updates...");
    // Exe update
    let Ok(new_exe_version) = get_file("version_exe") else {
        logger.err("Failed to fetch new executable version from repository");
        return;
    };
    let new_exe_version = new_exe_version.trim();


    if new_exe_version == VERSION {
        logger.info(&format!("Program up to date (v{new_exe_version})"));
    } else {
        logger.warn(" ");
        logger.warn(&format!("   !! Executable update available: v{} !!", new_exe_version));
        logger.warn("Update the program to get the latest offset updates");
        logger.warn("");
        return;
    }

    // Offset update
    let Ok(new_offset_version) = get_file("version_offsets") else {
        logger.err("Failed to fetch new offset version from repository");
        return;
    };
    let Ok(new_offset_version) = new_offset_version.trim().parse::<i32>() else {
        logger.err(&format!("Failed to parse new offset version: {}", new_offset_version));
        return;
    };

    let mut update_offsets = false;
    if !Path::new("./version_offsets").exists(){
        logger.warn("Missing version_offsets file");
        update_offsets = true;
    }

    if Path::new("./offsets").exists(){
        if fs::read_to_string("./version_offsets").unwrap().trim().parse::<i32>().unwrap() < new_offset_version{
            logger.info("Offset update available");
            update_offsets = true;
        }else{
            logger.info(&format!("Offsets up to date (v{new_offset_version})"));
        }
    }else{
        logger.warn("Missing offsets file");
        update_offsets = true;
    }

    if update_offsets{
        // Offset update available
        logger.info("Downloading offsets...");
        if let Ok(offsets) = get_file("offsets") {
            std::fs::write("offsets", offsets).unwrap();
            std::fs::write("version_offsets", new_offset_version.to_string()).unwrap();
            logger.info("Offsets updated");
        }else{
            logger.err("Failed to fetch offsets from repository");
        }
    }
}

fn get_file(path: &str) -> Result<String, String> {
    let url = format!("https://raw.githubusercontent.com/{REPO}/{path}");
    let Ok(res) = reqwest::blocking::get(&url) else {
        return Err(format!("Get error: {}", &url));
    };
    if res.status().is_success() {
        Ok(res.text().unwrap())
    } else {
        Err(format!("Get error {}: {}", res.status(), &url))
    }
}

// !cargo r
