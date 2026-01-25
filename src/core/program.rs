use std::{
    io::{self, Write},
    sync::{Arc, Mutex, mpsc},
    thread,
    time::{Duration, Instant},
};

use crossterm::{
    cursor,
    event::{self, KeyEventKind},
    execute, queue,
    style::Print,
    terminal,
};

use crate::core::Model;

/// Internal message wrapper that includes system control messages
enum Internal<M> {
    User(M),
    Quit,
    Tick,
}

/// Builder for configuring a Program before running it.
///
/// ```
/// let program = ProgramBuilder::new(my_model)
///     .with_fps(60)
///     .with_alt_screen(true)
///     .build();
/// ```
pub struct ProgramBuilder<M: Model> {
    model: M,
    fps: u32,
    alt_screen: bool,
}

/// Runtime configuration for the event loop.
pub struct ConfigProgram {
    fps: u32,
    alt_screen: bool,
}

impl<M: Model + Send + Sync + 'static> ProgramBuilder<M> {
    /// Create a new program with your model.
    pub fn new(model: M) -> Self {
        Self {
            model,
            fps: 60,
            alt_screen: false,
        }
    }

    /// Set the max render rate. Default is 60.
    #[must_use]
    pub fn with_fps(mut self, fps: u32) -> Self {
        self.fps = fps;
        self
    }

    /// Use alternate screen buffer. Default is false.
    /// When true, your app runs in a separate screen that gets cleared on exit.
    #[must_use]
    pub fn with_alt_screen(mut self, alt_screen: bool) -> Self {
        self.alt_screen = alt_screen;
        self
    }

    /// Build the program. Call `.run()` on the result to start it.
    pub fn build(self) -> Program<M> {
        let config = ConfigProgram {
            fps: self.fps,
            alt_screen: self.alt_screen,
        };
        Program::new(self.model, config)
    }
}

/// The running application. Create with ProgramBuilder.
///
/// Manages the event loop, threading, and rendering.
pub struct Program<M: Model> {
    model: Arc<Mutex<M>>,
    tx: mpsc::Sender<Internal<M::Message>>,
    rx: mpsc::Receiver<Internal<M::Message>>,
    config: ConfigProgram,
}

impl<M: Model + Send + 'static> Program<M> {
    pub fn new(model: M, config: ConfigProgram) -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            model: Arc::new(Mutex::new(model)),
            tx,
            rx,
            config,
        }
    }

    /// Start the application. Blocks until the app exits.
    ///
    /// Sets up the terminal, spawns event handlers, and runs the main loop.
    /// Returns when a Command::quit() is issued or an error occurs.
    pub fn run(mut self) -> io::Result<()> {
        terminal::enable_raw_mode()?;
        let mut stdout = io::stdout();
        if self.config.alt_screen {
            execute!(stdout, terminal::EnterAlternateScreen, cursor::Hide)?;
        }

        self.init_event_handler();
        self.init_tick_handler();

        self.render()?;

        let result = self.event_loop();

        if self.config.alt_screen {
            execute!(stdout, cursor::Show, terminal::LeaveAlternateScreen)?;
        }
        terminal::disable_raw_mode()?;

        result
    }

    /// Get a handle to quit the app from anywhere (e.g., a background thread).
    #[must_use]
    pub fn quit_handle(&self) -> QuitHandle<M::Message> {
        QuitHandle {
            tx: self.tx.clone(),
        }
    }

    fn event_loop(&mut self) -> io::Result<()> {
        let frame_duration = Duration::from_secs_f64(1.0 / self.config.fps as f64);
        let mut last_render = Instant::now();
        let mut needs_render = false;

        loop {
            if let Ok(msg) = self.rx.recv_timeout(Duration::from_millis(16)) {
                match msg {
                    Internal::User(user_msg) => {
                        let mut model = self.model.lock().unwrap();

                        // Update the model and get the command
                        let command = model.update(user_msg);
                        drop(model); // Release lock before rendering

                        needs_render = true;

                        // Check if the command indicates we should quit
                        // This is completely transparent to the developer!
                        if command.is_quit() {
                            break;
                        }

                        // TODO: Handle other command types (Perform, Batch, etc.)
                    }
                    Internal::Tick => {
                        // Tick event for periodic updates
                        needs_render = true;
                    }
                    Internal::Quit => {
                        // Direct quit signal (from QuitHandle)
                        break;
                    }
                }
            }

            // Render only if needed and enough time has passed (frame limiting)
            if needs_render && last_render.elapsed() >= frame_duration {
                self.render()?;
                needs_render = false;
                last_render = Instant::now();
            }
        }
        Ok(())
    }

    fn init_event_handler(&self)
    where
        M: Send + 'static,
    {
        let tx = self.tx.clone();
        let model = Arc::clone(&self.model);
        thread::spawn(move || {
            loop {
                // Poll with shorter timeout to be more responsive
                if event::poll(Duration::from_millis(10)).unwrap_or(false) {
                    // There is an event available
                    if let Ok(ev) = event::read() {
                        // Filter out key repeat events to prevent duplication
                        if let crossterm::event::Event::Key(key_event) = ev {
                            // Only process Press events, ignore Repeat and Release
                            if key_event.kind != KeyEventKind::Press {
                                continue;
                            }
                        }

                        let model = model.lock().unwrap();
                        let msg = model.event(ev);
                        drop(model); // Release lock

                        if let Some(message) = msg {
                            // Send user message wrapped in internal message
                            if tx.send(Internal::User(message)).is_err() {
                                // Channel closed, exit thread
                                break;
                            }
                        }
                    } else {
                        break;
                    }
                }
            }
        });
    }

    fn init_tick_handler(&self)
    where
        M: Send + 'static,
    {
        let tx = self.tx.clone();
        let fps = self.config.fps;
        thread::spawn(move || {
            let tick_duration = Duration::from_secs_f64(1.0 / fps as f64);
            loop {
                thread::sleep(tick_duration);
                if tx.send(Internal::Tick).is_err() {
                    // Channel closed, exit thread
                    break;
                }
            }
        });
    }

    fn render(&self) -> io::Result<()> {
        let mut stdout = io::stdout();

        // if self.config.alt_screen {
        queue!(
            stdout,
            cursor::MoveTo(0, 0),
            terminal::Clear(terminal::ClearType::FromCursorDown)
        )?;
        // }

        let model = self.model.lock().unwrap();
        let view = model.view();
        drop(model); // Release lock before flushing

        queue!(stdout, Print(view))?;

        stdout.flush()?;

        Ok(())
    }
}

/// Quit the application from outside the event loop.
/// Clone this and pass it to background threads or callbacks.
pub struct QuitHandle<M: Clone + Send> {
    tx: mpsc::Sender<Internal<M>>,
}

impl<M: Clone + Send> QuitHandle<M> {
    /// Signal the app to exit.
    pub fn quit(&self) {
        let _ = self.tx.send(Internal::Quit);
    }
}

impl<M: Clone + Send> Clone for QuitHandle<M> {
    fn clone(&self) -> Self {
        QuitHandle {
            tx: self.tx.clone(),
        }
    }
}
