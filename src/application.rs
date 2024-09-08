use std::cell::RefCell;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::thread;
use rusty_link::{AblLink, SessionState};
use crate::offsets::RekordboxOffsetCollection;
use crate::BeatKeeper;
use crate::RekordboxOffsets;
use std::collections::HashMap;

use iced::subscription;
use iced::widget::pick_list;
use iced::Subscription;
use iced::Element;
use iced::widget::{button, column, row, text, Checkbox};
use iced::Theme;
use std::sync::mpsc;

use crate::outputmodules::OutputModules;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const REPO: &str = "grufkork/rkbx_osc/rewrite";


#[derive(Debug, Clone)]
pub enum ToAppMessage {
    Beat(f32),
    ChangedUpdateCheckState(UpdateCheckState)
}

#[derive(Debug, Clone)]
struct ExeVersion(i32, i32, i32);

impl ExeVersion{
    fn from_string(s: &str) -> ExeVersion{
        let mut p = s.split(".").map(|x| x.parse::<i32>().unwrap());
        ExeVersion(p.next().unwrap(), p.next().unwrap(), p.next().unwrap())
    }
}

#[derive(Debug, Clone)]
pub enum AppToKeeperMessage {
}

#[derive(Debug, Clone)]
enum UpdateCheckState{
    Checking,
    UpToDate,
    OffsetUpdateAvailable(i32),
    ExecutableUpdateAvailable(String),
    Failed(String)
}

enum AppState{
    Idling,
    UpdatingOffsets,
    Running
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
    update_check_state: UpdateCheckState
}

impl App{
    fn reload_offsets(&mut self) -> Result<(), ()>{
        if !Path::new("offsets").exists(){
            return Err(());
        }
        self.offsets = Some(RekordboxOffsets::from_file("offsets"));
        let mut versions: Vec<String> = self.offsets.as_ref().unwrap().keys().map(|x| x.to_string()).collect();
        versions.sort();
        versions.reverse();
        self.versions = versions;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum Msg {
    KeeperMessage(ToAppMessage),
    Start,
    VersionSelected(String),
    ToggleModule(usize),
    UpdateOffsets
}

impl iced::Application for App {
    type Executor = iced::executor::Default;
    type Flags = ();
    type Message = Msg;
    type Theme = Theme;

    fn new(_flags: ()) -> (App, iced::Command<Msg>) {
        let modules = [OutputModules::AbletonLink, OutputModules::OSC].iter().map(|x| (*x, false)).collect();

        let (tx, rx) = std::sync::mpsc::channel::<ToAppMessage>();

        let txclone = tx.clone();
        thread::spawn(move ||{
            let Ok(new_exe_version) = get_file("version_exe") else {
                txclone.send(ToAppMessage::ChangedUpdateCheckState(UpdateCheckState::Failed("Failed to get exe version info".to_string()))).unwrap();
                return;
            };
            println!("Current: {:?}", VERSION);
            println!("New: {:?}", new_exe_version);

            if new_exe_version != VERSION{
                txclone.send(ToAppMessage::ChangedUpdateCheckState(UpdateCheckState::ExecutableUpdateAvailable(new_exe_version))).unwrap();
                return;
            }


            let Ok(new_offsets_version) = get_file("version_offsets") else {
                txclone.send(ToAppMessage::ChangedUpdateCheckState(UpdateCheckState::Failed("Failed to get offset version info".to_string()))).unwrap();
                return;
            };
            let new_offsets_version = new_offsets_version.parse::<i32>().unwrap();

            if !Path::new("./version_offsets").exists() || fs::read_to_string("./version_offsets").unwrap().parse::<i32>().unwrap() < new_offsets_version {
                txclone.send(ToAppMessage::ChangedUpdateCheckState(UpdateCheckState::OffsetUpdateAvailable(0))).unwrap();
                return;
            }

            txclone.send(ToAppMessage::ChangedUpdateCheckState(UpdateCheckState::UpToDate)).unwrap();

        });

        let versions = vec!["No offset file found".to_string()];
        let mut app = App{
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
            update_check_state: UpdateCheckState::Checking
        };

        app.reload_offsets();

        

        (app, iced::Command::none())
    }

    fn title(&self) -> String {
        String::from("rkbxosc")
    }

    fn update(&mut self, message: Msg) -> iced::Command<Msg>{
        match message {
            Msg::KeeperMessage(msg) => {
                match msg{
                    ToAppMessage::Beat(beat) => {
                        self.beat = beat;
                    },
                    ToAppMessage::ChangedUpdateCheckState(state) => {
                        self.update_check_state = state;
                    }
                }

            },
            Msg::Start => {
                self.state = AppState::Running;

                let (tx, rx) = std::sync::mpsc::channel::<AppToKeeperMessage>();

                BeatKeeper::start(
                    self.offsets.as_ref().unwrap().get(&self.selected_version).unwrap().clone(),
                    self.modules.clone(),
                    rx,
                    self.keeper_to_app_sender.clone());




            },
            Msg::VersionSelected(version) => {
                self.selected_version = version;

            },
            Msg::ToggleModule(idx )=> {
                self.modules[idx].1 = !self.modules[idx].1;
            },
            Msg::UpdateOffsets => {
                self.state = AppState::UpdatingOffsets;
                match download_offsets(){
                    Ok(_) => {
                        self.reload_offsets().unwrap();
                        self.update_check_state = UpdateCheckState::UpToDate;
                    },
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
        subscription::unfold("a", self.receiver.take(), 
            move |mut rx| async move {
                let val = rx.as_mut().unwrap().recv().unwrap();
                (Msg::KeeperMessage(val), rx)
            })
    }


    fn view(&self) -> Element<Msg> {
        match self.state{
            AppState::Running => {
                text("Link started").into()
            }
            AppState::Idling => {
                column!(
                    text(format!("Beat: {}", self.beat)).size(16),
                    

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

            },
            AppState::UpdatingOffsets => {
                text("Updating offsets").into()
            }
        }
    }
}

fn download_offsets() -> Result<(), String> {
    let offsets = get_file("offsets")?;
    std::fs::write("offsets", offsets).unwrap();
    let offsets = get_file("version")?;
    std::fs::write("version", format!("{VERSION} {}", offsets.split(' ').nth(1).unwrap())).unwrap();

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
    if res.status().is_success(){
        Ok(res.text().unwrap())

    }else{
        Err(format!("Get error {}: {}", res.status(), &url))
    }

}
