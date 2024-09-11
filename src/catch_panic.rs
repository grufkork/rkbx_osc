use std::{
    marker::{Send, Sync},
    panic::PanicHookInfo,
    sync::mpsc::{channel, Receiver, Sender},
    thread,
};

use iced::Application;

pub fn setup_panic_catcher(source: String, tx: Sender<ErrorInfo>) {
    std::panic::set_hook(Box::new(move |info| {
        tx.send(ErrorInfo::from_panic_hook_info(info)).unwrap();

        // error_window::run(iced::Settings::with_flags(error)).unwrap();
    }));
}

struct error_window {
    msg: String,
}

impl iced::Application for error_window {
    type Executor = iced::executor::Default;
    type Message = ();
    type Flags = String;
    type Theme = iced::Theme;

    fn new(flags: String) -> (error_window, iced::Command<()>) {
        println!("Start");
        (error_window { msg: flags }, iced::Command::none())
    }

    fn title(&self) -> String {
        String::from("Crashed")
    }

    fn update(&mut self, _message: Self::Message) -> iced::Command<()> {
        iced::Command::none()
    }

    fn view(&self) -> iced::Element<Self::Message> {
        println!("Error draw attempt");
        iced::widget::text("Hello!").size(50).into()
    }
}

pub struct ErrorInfo {
    pub payload: String,
    pub location: String,
}

pub fn payload_to_string(payload: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        s.to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.to_string()
    } else {
        "Unknown error type".to_string()
    }
}

impl ErrorInfo {
    fn from_panic_hook_info(info: &PanicHookInfo) -> Self {
        let payload = if let Some(s) = info.payload().downcast_ref::<&str>() {
            s
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s
        } else {
            "Unknown error type"
        };

        let location = info.location().unwrap();
        ErrorInfo {
            payload: payload.to_string(),
            location: format!(
                "{}:{}:{}",
                location.file(),
                location.line(),
                location.column()
            ),
        }
    }
}

pub fn start_panic_listener() -> Sender<ErrorInfo> {
    let (tx, rx) = channel::<ErrorInfo>();
    let txc = tx.clone();
    thread::spawn(move || match rx.recv() {
        Ok(err) => {
            println!("ERROR CAUGHT {}: {}", err.location, err.payload);
            crate::application::App::run(iced::settings::Settings::with_flags(txc)).unwrap();
            error_window::run(iced::Settings::with_flags(err.payload)).unwrap();
        }
        Err(e) => {
            println!("Error receiving error: {}", e);
        }
    });

    tx
}
