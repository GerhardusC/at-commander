use std::{collections::HashMap, sync::{mpsc::{channel, Receiver, Sender}, Arc, Mutex}};

pub struct Event {
    name: String,
    data: String,
}

impl Event {
    pub fn new(name: String, data: String) -> Self {
        Event { name, data }
    }
}

impl From<Event> for String {
    fn from(value: Event) -> Self {
        format!("{}: {}", value.name, value.data)
    }
}

pub struct EventLoop {
    receiver: Receiver<Event>,
    pub sender: Sender<Event>,
    handlers: Arc<Mutex<HashMap<String, Box<dyn FnMut(Event) -> ()>>>>
}

impl EventLoop {
    pub fn new() -> Self {
        let (sender, receiver) = channel::<Event>();
        EventLoop { receiver, sender, handlers: Arc::new(Mutex::new(HashMap::new())) }
    }

    pub fn start(&self) {
        loop {
            let msg = self.receiver.recv();
            match msg {
                Ok(msg) => {
                    if let Ok(mut handlers) = self.handlers.try_lock() {
                        let event = handlers.get_mut(&msg.name);
                        if let Some(ex) = event {
                            ex(msg);
                        }
                    }
                },
                Err(e) => {
                    println!("{}", e);
                },
            }
        }
    }

    pub fn on<F>(&self, name: String, func: F)
    where
        F : FnMut(Event) -> () + 'static
    {
        if let Ok(mut handlers) = self.handlers.lock() {
            handlers.insert(
                name,
                Box::new(func)
            );
        }
    }
}
