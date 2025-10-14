use std::{
    collections::HashMap,
    sync::{
        mpsc::{Receiver, Sender, channel},
    },
};

#[derive(Debug, Hash, PartialEq, Eq)]
pub enum WifiEvent {
    Configure,
    PublishConnectRequest,
    Timeout,
    ConnAck,
    Publish,
    AckReceived,
    Close,
    Reset,
}

pub struct Event {
    event: WifiEvent,
    pub data: Option<String>,
}

impl Event {
    pub fn new(event: WifiEvent, data: Option<String>) -> Self {
        Event { event, data }
    }
}

pub struct EventLoop {
    receiver: Receiver<Event>,
    pub sender: Sender<Event>,
    handlers: HashMap<WifiEvent, Box<dyn FnMut(Event) -> ()>>,
}

impl EventLoop {
    pub fn new() -> Self {
        let (sender, receiver) = channel::<Event>();
        EventLoop {
            receiver,
            sender,
            handlers: HashMap::new(),
        }
    }

    pub fn start(&mut self) {
        loop {
            let msg = self.receiver.recv();
            match msg {
                Ok(msg) => {
                        let event = self.handlers.get_mut(&msg.event);
                        if let Some(ex) = event {
                            ex(msg);
                        }
                }
                Err(e) => {
                    println!("{}", e);
                }
            }
        }
    }

    pub fn on<F>(&mut self, event: WifiEvent, func: F)
    where
        F: FnMut(Event) -> () + Send + 'static,
    {
        self.handlers.insert(event, Box::new(func));
    }
}
