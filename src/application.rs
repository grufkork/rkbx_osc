use crate::catch_panic;
use crate::catch_panic::ErrorInfo;
use crate::offsets::RekordboxOffsetCollection;
use crate::BeatKeeper;
use crate::RekordboxOffsets;
use rusty_link::{AblLink, SessionState};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::thread;

use iced::subscription;
use iced::widget::pick_list;
use iced::widget::{button, column, row, text, Checkbox};
use iced::Element;
use iced::Subscription;
use iced::Theme;
use std::sync::mpsc;

use crate::outputmodules::OutputModules;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const REPO: &str = "grufkork/rkbx_osc/rewrite";

#[derive(Debug, Clone)]
pub enum ToAppMessage {
    Beat(f32),
    ChangedUpdateCheckState(UpdateCheckState),
    Crash(String),
}

#[derive(Debug, Clone)]
struct ExeVersion(i32, i32, i32);

impl ExeVersion {
    fn from_string(s: &str) -> Option<ExeVersion> {
        let mut p = s.split(".").map(|x| x.parse::<i32>().unwrap());
        Some(ExeVersion(p.next()?, p.next()?, p.next()?))
    }
}

#[derive(Debug, Clone)]
pub enum AppToKeeperMessage {}

#[derive(Debug, Clone)]
enum UpdateCheckState {
    Checking,
    UpToDate,
    OffsetUpdateAvailable(i32),
    ExecutableUpdateAvailable(String),
    Failed(String),
}

enum AppState {
    Idling,
    UpdatingOffsets,
    Running,
}

pub struct App {
    beat: f32,
    offsets: Option<RekordboxOffsetCollection>,
    keeper_to_app_sender: std::sync::mpsc::Sender<ToAppMessage>,
    receiver: RefCell<Option<mpsc::Receiver<ToAppMessage>>>,
    state: AppState,
    versions: Vec<String>,
    selected_version: String,
    keeper: Option<BeatKeeper>,
    modules: Vec<(OutputModules, bool)>,
    app_to_keeper_sender: Option<mpsc::Sender<AppToKeeperMessage>>,
    update_check_state: UpdateCheckState,
    config: HashMap<String, HashMap<String, String>>,
    error_tx: mpsc::Sender<ErrorInfo>,
    log: Vec<String>,
}

impl App {
    fn reload_offsets(&mut self) -> Result<(), String> {
        if !Path::new("offsets").exists() {
            return Err("No offset file found".to_string());
        }

        match RekordboxOffsets::from_file("offsets") {
            Ok(offsets) => {
                self.offsets = Some(offsets);
            }
            Err(e) => {
                return Err(e);
            }
        }

        let mut versions: Vec<String> = self
            .offsets
            .as_ref()
            .unwrap()
            .keys()
            .map(|x| x.to_string())
            .collect();
        versions.sort();
        versions.reverse();
        self.versions = versions;
        self.selected_version = self.versions[0].clone();
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum Msg {
    KeeperMessage(ToAppMessage),
    Start,
    VersionSelected(String),
    ToggleModule(usize),
    UpdateOffsets,
}

impl iced::Application for App {
    type Executor = iced::executor::Default;
    type Flags = mpsc::Sender<ErrorInfo>;
    type Message = Msg;
    type Theme = Theme;

    fn new(error_tx: mpsc::Sender<ErrorInfo>) -> (App, iced::Command<Msg>) {
        let modules = [OutputModules::AbletonLink, OutputModules::Osc]
            .iter()
            .map(|x| (*x, false))
            .collect();

        let (tx, rx) = std::sync::mpsc::channel::<ToAppMessage>();

        let txclone = tx.clone();
        let error_tx_clone = error_tx.clone();
        thread::spawn(move || {
            // Update routine
            let Ok(new_exe_version) = get_file("version_exe") else {
                txclone
                    .send(ToAppMessage::ChangedUpdateCheckState(
                        UpdateCheckState::Failed("Failed to get exe version info".to_string()),
                    ))
                    .unwrap();
                return;
            };
            let new_exe_version = new_exe_version.trim();

            println!("Current: {:?}", VERSION);
            println!("New: {:?}", new_exe_version);

            if new_exe_version != VERSION {
                txclone
                    .send(ToAppMessage::ChangedUpdateCheckState(
                        UpdateCheckState::ExecutableUpdateAvailable(new_exe_version.to_string()),
                    ))
                    .unwrap();
                return;
            }

            let Ok(new_offsets_version) = get_file("version_offsets") else {
                txclone
                    .send(ToAppMessage::ChangedUpdateCheckState(
                        UpdateCheckState::Failed("Failed to get offset version info".to_string()),
                    ))
                    .unwrap();
                return;
            };
            let Ok(new_offsets_version) = new_offsets_version.trim().parse::<i32>() else {
                txclone
                    .send(ToAppMessage::ChangedUpdateCheckState(
                        UpdateCheckState::Failed("Failed to parse offset version info".to_string()),
                    ))
                    .unwrap();
                return;
            };

            if !Path::new("./version_offsets").exists()
                || !Path::new("./offsets").exists()
                || fs::read_to_string("./version_offsets")
                    .unwrap()
                    .trim()
                    .parse::<i32>()
                    .unwrap()
                    < new_offsets_version
            {
                txclone
                    .send(ToAppMessage::ChangedUpdateCheckState(
                        UpdateCheckState::OffsetUpdateAvailable(0),
                    ))
                    .unwrap();
                return;
            }

            txclone
                .send(ToAppMessage::ChangedUpdateCheckState(
                    UpdateCheckState::UpToDate,
                ))
                .unwrap();
        });

        let mut config = HashMap::new();
        let config_src = fs::read_to_string("config").unwrap_or_default();
        let config_lines = config_src.lines();
        for line in config_lines {
            let Some(split_index) = line.find(" ") else {
                continue;
            };
            let path = &line[..split_index];
            let mut split = path.split(".");
            let Some(component) = split.next() else {
                continue;
            };
            let Some(key) = split.next() else {
                continue;
            };

            if !config.contains_key(component) {
                config.insert(component.to_string(), HashMap::new());
            }

            config
                .get_mut(component)
                .unwrap()
                .insert(key.to_string(), line[split_index + 1..].to_string());
        }
        println!("{:?}", config);

        let versions = vec!["No offset file found".to_string()];
        let mut app = App {
            keeper_to_app_sender: tx,
            app_to_keeper_sender: None,
            receiver: RefCell::new(Some(rx)),
            offsets: None,
            beat: 0.,
            state: AppState::Idling,
            selected_version: versions[0].clone(),
            versions,
            keeper: None,
            modules,
            update_check_state: UpdateCheckState::Checking,
            config,
            error_tx: error_tx.clone(),
            log: vec![],
        };

        app.reload_offsets();

        (app, iced::Command::none())
    }

    fn title(&self) -> String {
        String::from("rkbxosc")
    }

    fn update(&mut self, message: Msg) -> iced::Command<Msg> {
        match message {
            Msg::KeeperMessage(msg) => match msg {
                ToAppMessage::Beat(beat) => {
                    self.beat = beat;
                }
                ToAppMessage::ChangedUpdateCheckState(state) => {
                    self.update_check_state = state;
                }
                ToAppMessage::Crash(e) => {
                    self.log.push(format!("BeatKeeper crashed: {}", e));
                }
            },
            Msg::Start => {
                self.state = AppState::Running;

                let (tx, rx) = std::sync::mpsc::channel::<AppToKeeperMessage>();

                BeatKeeper::start(
                    self.offsets
                        .as_ref()
                        .unwrap()
                        .get(&self.selected_version)
                        .unwrap()
                        .clone(),
                    self.modules.clone(),
                    self.config.clone(),
                    rx,
                    self.keeper_to_app_sender.clone(),
                    self.error_tx.clone(),
                );
            }
            Msg::VersionSelected(version) => {
                self.selected_version = version;
            }
            Msg::ToggleModule(idx) => {
                self.modules[idx].1 = !self.modules[idx].1;
            }
            Msg::UpdateOffsets => {
                self.state = AppState::UpdatingOffsets;
                match download_offsets() {
                    Ok(_) => {
                        self.reload_offsets().unwrap();
                        self.update_check_state = UpdateCheckState::UpToDate;
                    }
                    Err(e) => {
                        println!("Error: {}", e);
                        self.update_check_state = UpdateCheckState::Failed(e);
                    }
                }
                self.state = AppState::Idling;
            }
        };
        iced::Command::none()
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        subscription::unfold("a", self.receiver.take(), move |mut rx| async move {
            let val = rx.as_mut().unwrap().recv().unwrap();
            (Msg::KeeperMessage(val), rx)
        })
    }

    fn view(&self) -> Element<Msg> {
        println!("View uodate");
        column([match self.state {
            AppState::Running => text("Link started").into(),
            AppState::Idling => {
                column!(
                        if self.offsets.is_some() {
                            button("Start").on_press(Msg::Start)
                        }else{
                            button("No offsets downloaded!")
                        }.width(100),
                    pick_list(self.versions.clone(), Some(self.selected_version.clone()), Msg::VersionSelected),
                    column(self.modules.iter().enumerate().map(|(i, (module, enabled))| {
                        row([
                            Checkbox::new("", *enabled).on_toggle(move |_|  {Msg::ToggleModule(i)}).into(),
                            // button(["Off", "On"][*enabled as usize]).on_press(Msg::ToggleModule(i)).into(),

                            text(format!("{}", module)).into()
                        ]).into()
                    })),
                    row({
                        let mut content = vec![
                            text(
                                match &self.update_check_state{
                                    UpdateCheckState::Checking => "Checking for updates...".to_string(),
                                    UpdateCheckState::UpToDate => "Up to date!".to_string(),
                                    UpdateCheckState::OffsetUpdateAvailable(version) => format!("Offset update available: v{}", version.clone()),
                                    UpdateCheckState::ExecutableUpdateAvailable(version) => format!("Executable update available: v{}. Download the latest version from https://github.com/grufkork/rkbx_osc to update memory offsets", version.clone()),
                                    UpdateCheckState::Failed(e) => format!("Update failed: {e}").to_string()
                                }).into()
                        ];

                        if let UpdateCheckState::OffsetUpdateAvailable(_) = self.update_check_state{
                            content.push(button("Update offsets").on_press(Msg::UpdateOffsets).into());
                        }

                        content
                    }),
                    text(format!("Version: {}", VERSION))
                ).into()
            }
            AppState::UpdatingOffsets => text("Updating offsets").into(),
        },
        text(self.log.join("\n")).size(20).into()
        ]).into()
    }
}

fn download_offsets() -> Result<(), String> {
    std::fs::write("offsets", get_file("offsets")?).unwrap();
    std::fs::write("version_offsets", get_file("version_offsets")?).unwrap();

    /*match Command::new("curl")
        .args([
            "-o",
            "offsets",
            &format!("https://raw.githubusercontent.com/{REPO}/offsets"),
        ])
        .output()
        {
            Ok(output) => {
                println!("{}", String::from_utf8(output.stdout).unwrap());
                let stderr = String::from_utf8(output.stderr).unwrap();
                if !stderr.is_empty(){
                    return Err(stderr);
                }
            }
            Err(error) => println!("{}", error),
        }
    match Command::new("curl")
        .args([
            "-o",
            "offsets",
            &format!("https://raw.githubusercontent.com/{REPO}/version"),
        ])
        .output()
        {
            Ok(output) => {
                println!("{}", String::from_utf8(output.stdout).unwrap());
                let stderr = String::from_utf8(output.stderr).unwrap();
                if !stderr.is_empty(){
                    return Err(stderr);
                }
            }
            Err(error) => println!("{}", error),
        }*/
    Ok(())
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
