use std::{collections::{HashMap, VecDeque}, sync::{mpsc::{channel, Receiver, Sender}, Arc, Mutex}};

struct Event {
    name: String,
    data: String,
}

impl Event {
    fn new(name: String, data: String) -> Self {
        Event { name, data }
    }
}

struct EventLoop {
    receiver: Receiver<Event>,
    sender: Sender<Event>,
    handlers: Arc<Mutex<HashMap<String, Box<dyn Fn(Event) -> ()>>>>
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
                    if let Ok(handlers) = self.handlers.try_lock() {
                        let event = handlers.get(&msg.name);
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

    pub fn on<F>(&mut self, name: String, func: F)
    where
        F : Fn(Event) -> () + 'static
    {
        if let Ok(mut handlers) = self.handlers.lock() {
            handlers.insert(
                name,
                Box::new(func)
            );
        }
    }

    pub fn dispatch(&mut self, event: Event) -> Result<(), String> {
        self.sender.send(event)
            .map_err(|_| "failed to lock mux".to_owned())?;

        Ok(())
    }
}
