use std::rc::Rc;


#[derive(PartialEq)]
pub enum LogLevel {
    Debug,
    Good,
    Info,
    Warning,
    Error,
}

#[derive(Clone)]
pub struct Logger {
    pub debug_enabled: bool,
}

impl Logger{
    pub fn new(debug: bool) -> Self{
        Logger{debug_enabled: debug}
    }

    pub fn log(&self, source: &str, message: &str, level: LogLevel){
        if !self.debug_enabled && level == LogLevel::Debug{
            return;
        }

        let esc ="\x1b[";
        let rst = "\x1b[0m";

        let format = match level{
            LogLevel::Debug => format!("{esc}34m"),
            LogLevel::Good => format!("{esc}32m"),
            LogLevel::Info => format!("{esc}37m"),
            LogLevel::Warning => format!("{esc}33m"),
            LogLevel::Error => format!("{esc}31m"),
        };
        println!("{}[{}]  {}{rst}", format, source, message);
    }

    pub fn debug(&self, source: &str, message: &str){
        self.log(source, message, LogLevel::Debug);
    }

    pub fn good(&self, source: &str, message: &str){
        self.log(source, message, LogLevel::Good);
    }

    pub fn info(&self, source: &str, message: &str){
        self.log(source, message, LogLevel::Info);
    }

    pub fn warning(&self, source: &str, message: &str){
        self.log(source, message, LogLevel::Warning);
    }

    pub fn error(&self, source: &str, message: &str){
        self.log(source, message, LogLevel::Error);
    }
}

#[derive(Clone)]
pub struct ScopedLogger{
    pub logger: Rc<Logger>,
    source: String
}

impl ScopedLogger{
    pub fn new(logger: &Rc<Logger>, source: &str) -> Self{
        ScopedLogger{logger: logger.clone(), source: source.to_string()}
    }

    pub fn debug(&self, message: &str){
        self.logger.debug(&self.source, message);
    }

    pub fn good(&self, message: &str){
        self.logger.good(&self.source, message);
    }

    pub fn info(&self, message: &str){
        self.logger.info(&self.source, message);
    }

    pub fn warn(&self, message: &str){
        self.logger.warning(&self.source, message);
    }

    pub fn err(&self, message: &str){
        self.logger.error(&self.source, message);
    }
}
