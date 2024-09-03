use std::cell::RefCell;
use rusty_link::{AblLink, SessionState};
use crate::offsets::RekordboxOffsetCollection;
use crate::BeatKeeper;
use crate::RekordboxOffsets;
use std::collections::HashMap;

use iced::subscription;
use iced::widget::pick_list;
use iced::Command;
use iced::Subscription;
use iced::Element;
use iced::widget::{button, column, row, text, Checkbox};
use iced::Theme;
use std::sync::mpsc;

use crate::outputmodules::OutputModules;


pub struct Flag{
    pub offsets: RekordboxOffsetCollection
    

}

#[derive(Debug, Clone)]
pub enum KeeperToAppMessage {
    Beat(f32),
}

#[derive(Debug, Clone)]
pub enum AppToKeeperMessage {
}

enum UpdateCheckState{
    Checking,
    UpToDate,
    OffsetUpdateAvailable(String),
    ExecutableUpdateAvailable(String),
    Failed
}

enum AppState{
    Idling,
    UpdatingOffsets,
    Running
}

pub struct App {
    beat: f32,
    offsets: RekordboxOffsetCollection,
    keeper_to_app_sender: std::sync::mpsc::Sender<KeeperToAppMessage>,
    receiver: RefCell<Option<mpsc::Receiver<KeeperToAppMessage>>>,
    state: AppState,
    versions: Vec<String>,
    selected_version: String,
    keeper: Option<BeatKeeper>,
    modules: Vec<(OutputModules, bool)>,
    app_to_keeper_sender: Option<mpsc::Sender<AppToKeeperMessage>>,
    update_check_state: UpdateCheckState
}

#[derive(Debug, Clone)]
pub enum Msg {
    KeeperMessage(KeeperToAppMessage),
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

    fn new(_flags: ()) -> (App, Command<Msg>) {
        
        let offsets = RekordboxOffsets::from_file("offsets");
        let mut versions: Vec<String> = offsets.keys().map(|x| x.to_string()).collect();
        versions.sort();
        versions.reverse();

        let modules = [OutputModules::AbletonLink, OutputModules::OSC].iter().map(|x| (*x, false)).collect();




        let (tx, rx) = std::sync::mpsc::channel::<KeeperToAppMessage>();
        (App{
            keeper_to_app_sender: tx,
            app_to_keeper_sender: None,
            receiver: RefCell::new(Some(rx)),
            offsets,
            beat: 0.,
            state: AppState::Idling,
            selected_version: versions[0].clone(),
            versions,
            keeper: None,
            modules,
            update_check_state: UpdateCheckState::Checking

        }, Command::none())
    }

    fn title(&self) -> String {
        String::from("rkbxosc")
    }

    fn update(&mut self, message: Msg) -> iced::Command<Msg>{
        match message {
            Msg::KeeperMessage(msg) => {

            },
            Msg::Start => {
                self.state = AppState::Running;

                let (tx, rx) = std::sync::mpsc::channel::<AppToKeeperMessage>();

                BeatKeeper::start(
                    self.offsets.get(&self.selected_version).unwrap().clone(),
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

            }
        };
        Command::none()
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
                    button("Start").on_press(Msg::Start).width(100),
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
                                    UpdateCheckState::OffsetUpdateAvailable(version) => format!("Offset update available: {}", version.clone()),
                                    UpdateCheckState::ExecutableUpdateAvailable(version) => format!("Executable update available: {}. Download the latest version from github.com", version.clone()),
                                    UpdateCheckState::Failed => "Update check failed".to_string()
                                }).into()
                        ];

                        if let UpdateCheckState::OffsetUpdateAvailable(_) = self.update_check_state{
                            content.push(button("Update offsets").on_press(Msg::UpdateOffsets).into());
                        }

                        content
                    })
                ).into()

            },
            AppState::UpdatingOffsets => {
                text("Updating offsets").into()
            }
        }
    }

}
