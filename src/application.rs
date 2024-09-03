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
use iced::widget::{button, column, row, text};
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

pub struct App {
    beat: f32,
    offsets: RekordboxOffsetCollection,
    keeper_to_app_sender: std::sync::mpsc::Sender<KeeperToAppMessage>,
    receiver: RefCell<Option<mpsc::Receiver<KeeperToAppMessage>>>,
    started: bool,
    versions: Vec<String>,
    selected_version: String,
    keeper: Option<BeatKeeper>,
    modules: Vec<(OutputModules, bool)>,
    app_to_keeper_sender: Option<mpsc::Sender<AppToKeeperMessage>>
}

#[derive(Debug, Clone)]
pub enum Msg {
    KeeperMessage(KeeperToAppMessage),
    Start,
    VersionSelected(String)
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
            started: false,
            selected_version: versions[0].clone(),
            versions,
            keeper: None,
            modules

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
                self.started = true;

                let (tx, rx) = std::sync::mpsc::channel::<AppToKeeperMessage>();
                
                self.keeper = Some(BeatKeeper::new(
                        self.offsets.get(&self.selected_version).unwrap().clone(),
                        self.modules.clone(),
                        rx,
                        self.keeper_to_app_sender.clone()));
                                



            },
            Msg::VersionSelected(version) => {
                self.selected_version = version;

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
        if self.started{
            text("Link started").into()
        }else{
            column!(
                text(format!("Beat: {}", self.beat)).size(16),
                button("Start").on_press(Msg::Start),
                pick_list([String::from("6.8.5"), String::from("7.0.3")], Some(String::from("6.8.5")), Msg::VersionSelected)
            ).into()
        }
    }

}
