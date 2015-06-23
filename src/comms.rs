//! Utilities for communication between the supervisor thread and services

use std::sync::mpsc::{channel, Sender, Receiver, RecvError, TryRecvError, SendError};
use std::thread;
use std::mem;

/// Commands sent from the supervisor thread to services
#[derive(Clone)]
pub enum CmdTo {
    /// Start the service
    Start,

    /// Stop the service (but keep the thread running)
    Stop,

    /// Stop the service and kill the thread
    Quit,

    /// Some data
    // TODO enum instead of str, somehow each service defines its own variants
    Data(String),
}

/// Commands sent from services up to the supervisor thread
#[derive(Clone)]
pub enum CmdFrom {
    /// Start another service
    Start(String, Sender<bool>),

    /// Stop another service
    Stop(String, Sender<bool>),

    /// Some data
    // TODO enum instead of str, somehow each service defines its own variants
    Data(String),

    /// Shut down everything
    Quit
}

/// A service that can be setup and torn down based on commands from a higher power.
pub trait Controllable {
    /// Setup the service.
    ///
    /// Should initialize any necessary libraries and devices. May be called more than once, but
    /// teardown() will be called in between.
    fn setup(Sender<CmdFrom>, Option<String>) -> Self;

    /// Run one "step".
    ///
    /// In the case of a device driver, this corresponds to gathering one frame or sample of data.
    ///
    /// Return true if we should wait for a command from the supervisor thread before calling
    /// step() again. Return false to call step() again right away (unless there is a pending
    /// command).
    fn step(&mut self, data: Option<String>) -> bool;

    /// Tear down the service.
    ///
    /// Should shut down any necessary libraries or services. Either the program is exiting, or the
    /// service is just being paused (setup() could be called again).
    fn teardown(&mut self);
}

/// Convenience macro for making an "RPC" call from a service up to the main thread. This is done
/// by generating a nonce channel, stuffing the sending end into a message that gets sent to the
/// main thread, and then waiting for the main thread to send back a reply.
/// 
/// This is a macro instead of a function because you need to pass in the _name_ of a CmdFrom
/// variant without constructing it (because a Sender is needed to construct it, but the macro is
/// creating the channel for you). Possibly a better design would be structs and a trait/generic.
///
/// * Inputs:
///     - tx: Sender\<CmdFrom> = Sender that the service uses to send commands to the main thread
///     - name = name of a CmdFrom variant that has a Sender<T> as the last parameter
///     - params = any other parameters for the CmdFrom variant
/// * Outputs:
///     - Result\<T, mpsc::RecvError> = the response received (or not) from the main thread
#[macro_export]
macro_rules! rpc {
    ($tx:expr, CmdFrom::$name:ident, $($param:expr),*) => {{
        let (msg_tx, msg_rx) = ::std::sync::mpsc::channel();
        $tx.send($crate::comms::CmdFrom::$name($($param),*, msg_tx));
        msg_rx.recv()
    }}
}

/// Convenience macro for defining a stub service that doesn't do anything (yet). Defines a
/// zero-sized struct and an impl that blocks between receiving messages from the main thread (so
/// it doesn't do anything, but it doesn't sit in a CPU-busy loop either).
///
/// * Inputs:
///     - t = name of the Controllable
/// * Items created:
///     - pub struct (named $t) and stub impl
macro_rules! stub {
    ($t:ident) => {
        pub struct $t;
        impl $crate::comms::Controllable for $t {
            fn setup(tx: ::std::sync::mpsc::Sender<$crate::comms::CmdFrom>, _: Option<String>) -> $t {
                $t
            }

            fn step(&mut self, _: Option<String>) -> bool {
                true
            }

            fn teardown(&mut self) {
            }
        }
    }
}

/// Service driving function
///
/// Runs in a loop receiving commands from the supervisor thread. Manages a Controllable instance,
/// calling its setup()/step()/teardown() methods as necessary.
pub fn go<C: Controllable>(rx: Receiver<CmdTo>, tx: Sender<CmdFrom>) {
    loop {
        let mut data = None;

        match rx.recv() {
            Ok(cmd) => match cmd {
                CmdTo::Start => {}, // let's go!
                CmdTo::Data(d) => data = Some(d),
                CmdTo::Stop | CmdTo::Quit => return, // didn't even get to start
            },
            Err(e) => return, // main thread exploded?
        }

        let mut c = C::setup(tx.clone(), data);
        let mut should_block = false;

        loop {
            data = None;

            if should_block {
                match rx.recv() {
                    Ok(cmd) => match cmd {
                        CmdTo::Start => {}, // already started
                        CmdTo::Stop => break, // shutdown command
                        CmdTo::Quit => { c.teardown(); return }, // real shutdown command
                        CmdTo::Data(d) => data = Some(d),
                    },
                    Err(_) => { c.teardown(); return }
                }
            } else {
                match rx.try_recv() {
                    Ok(cmd) => match cmd {
                        CmdTo::Start => {}, // already started
                        CmdTo::Stop => break, // shutdown command
                        CmdTo::Quit => { c.teardown(); return }, // real shutdown command
                        CmdTo::Data(d) => data = Some(d),
                    },
                    Err(e) => match e {
                        TryRecvError::Empty => {}, // continue
                        TryRecvError::Disconnected => { c.teardown(); return }, // main thread exploded?
                    },
                }
            }

            should_block = c.step(data);
        }

        c.teardown();
    }
}

/// Container for a thread that repeatedly performs some action in response to input. Can be
/// stopped and restarted.
pub struct RestartableThread<Data: Send + 'static> {
    /// Sending end of a channel used to send inputs to the thread.
    /// Wrapped in a Option so it can be dropped without moving self.
    tx: Option<Sender<Data>>,

    /// Handle to the running thread
    /// Wrapped in an option so it can be joined without moving self.
    thread: Option<thread::JoinHandle<()>>
}

impl<Data: Send + 'static> RestartableThread<Data> {
    /// Create a new RestartableThread which performs the given action in response to input.
    /// The thread will run (and wait for input) until RestartableThread::join() is called or the
    /// RestartableThread instance is dropped.
    /// To pass input, use RestartableThread::send().
    pub fn new<F>(f: F) -> RestartableThread<Data> where F: Send + 'static + Fn(Data)
    {
        let (tx, rx) = channel();
        RestartableThread {
            tx: Some(tx),
            thread: Some(thread::spawn(move || {
                while let Ok(x) = rx.recv() {
                    f(x);
                }
            }))
        }
    }

    /// Kill the thread. This shuts down the message queue, causing the thread to exit, and then
    /// waits for it to finish up any outstanding work. No deadlocks here!
    pub fn join(&mut self) {
        if self.thread.is_some() {
            self.tx = None; // this causes the Sender to be dropped

            let mut old_thread = None;
            mem::swap(&mut old_thread, &mut self.thread);
            old_thread.unwrap().join().unwrap(); // safe to join since we hung up the channel
        }
    }

    /// Send some input to the thread. Nonblocking.
    /// Returns a SendError if Sender::send() fails or if the private Sender has somehow
    /// disappeared (which is impossible).
    pub fn send(&self, d: Data) -> Result<(), SendError<Data>> {
        if let Some(ref s) = self.tx {
            s.send(d)
        } else {
            Err(SendError(d))
        }
    }
}

impl<Data: Send + 'static> Drop for RestartableThread<Data> {
    /// When the RestartableThread goes out of scope, kill the thread.
    fn drop(&mut self) {
        self.join();
    }
}

