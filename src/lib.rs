use std::{collections::HashMap, sync::{mpsc::{channel, Receiver, Sender}, Arc, Mutex}};

#[derive(Debug, Hash, PartialEq, Eq)]
pub enum WifiEvent {
    PublishConnectRequest,
    Timeout,
    ConnAck,
    Publish,
    AckReceived,
    Close,
}

pub enum WifiState {
    Ready,
    WaitingConnectAck,
    Connected,
    WaitingPublishAck,
    Sent,
}

pub struct Event {
    event: WifiEvent,
    pub data: String,
}

impl Event {
    pub fn new(event: WifiEvent, data: String) -> Self {
        Event { event, data }
    }
}

impl From<Event> for String {
    fn from(value: Event) -> Self {
        format!("{:?}: {}", value.event, value.data)
    }
}

pub struct EventLoop {
    receiver: Receiver<Event>,
    pub sender: Sender<Event>,
    handlers: Arc<Mutex<HashMap<WifiEvent, Box<dyn FnMut(Event, &mut WifiState) -> ()>>>>
}

impl EventLoop {
    pub fn new() -> Self {
        let (sender, receiver) = channel::<Event>();
        EventLoop { receiver, sender, handlers: Arc::new(Mutex::new(HashMap::new())) }
    }

    pub fn start(&self, state: &mut WifiState) {
        loop {
            let msg = self.receiver.recv();
            match msg {
                Ok(msg) => {
                    if let Ok(mut handlers) = self.handlers.try_lock() {
                        let event = handlers.get_mut(&msg.event);
                        if let Some(ex) = event {
                            ex(msg, state);
                        }
                    }
                },
                Err(e) => {
                    println!("{}", e);
                },
            }
        }
    }

    pub fn on<F>(&self, event: WifiEvent, func: F)
    where
        F : FnMut(Event, &mut WifiState) -> () + 'static
    {
        if let Ok(mut handlers) = self.handlers.lock() {
            handlers.insert(
                event,
                Box::new(func)
            );
        }
    }
}
