use std::{
    io::{self, Write},
    sync::mpsc,
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
}

/// Builder for configuring a Program before running it.
///
/// ```
/// # use nobubbles::core::{Model, ProgramBuilder};
/// struct MyModel;
/// impl Model for MyModel {
///     type Message = ();
///     type Output = String;
///     fn event(&self, _ev: crossterm::event::Event) -> Option<Self::Message> { None }
///     fn update(&mut self, _msg: Self::Message) -> nobubbles::core::Command<Self::Message> {
///         nobubbles::core::Command::none()
///     }
///     fn view(&self) -> Self::Output { "Hello, World!".to_string() }
/// }
/// let program = ProgramBuilder::new(MyModel)
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

impl<M: Model> ProgramBuilder<M> {
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

/// The running application. Create with [`ProgramBuilder`].
///
/// Manages the event loop, threading, and rendering.
pub struct Program<M: Model> {
    model: M,
    tx: mpsc::Sender<Internal<M::Message>>,
    rx: mpsc::Receiver<Internal<M::Message>>,
    config: ConfigProgram,
}

impl<M: Model> Program<M> {
    pub fn new(model: M, config: ConfigProgram) -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            model,
            tx,
            rx,
            config,
        }
    }

    /// Start the application. Blocks until the app exits.
    ///
    /// Sets up the terminal, spawns event handlers, and runs the main loop.
    /// Returns when a [`Command::quit()`] is issued or an error occurs.
    ///
    /// # Errors
    /// It can trigger an error if the terminal cannot be set to raw mode, or if there is an issue with rendering or event handling.
    pub fn run(mut self) -> io::Result<()> {
        terminal::enable_raw_mode()?;
        let mut stdout = io::stdout();
        if self.config.alt_screen {
            execute!(stdout, terminal::EnterAlternateScreen, cursor::Hide)?;
        }

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

    /// The main event loop of the application. Polls for events, processes messages, and renders the view.
    fn event_loop(&mut self) -> io::Result<()> {
        const IDLE: Duration = Duration::from_millis(250);
        let frame = Duration::from_secs_f64(1.0 / f64::from(self.config.fps));
        let mut last_render = Instant::now();
        let mut dirty = false;

        loop {
            let timeout = if dirty {
                frame.saturating_sub(last_render.elapsed())
            } else {
                IDLE
            };

            if event::poll(timeout)? {
                let ev = event::read()?;
                if matches!(ev, event::Event::Key(k) if k.kind != KeyEventKind::Press) {
                    continue; // Ignore key releases
                }
                if let Some(msg) = self.model.event(ev) {
                    if self.model.update(msg).is_quit() {
                        break;
                    }
                    dirty = true;
                }
            }

            while let Ok(internal) = self.rx.try_recv() {
                match internal {
                    Internal::Quit => return Ok(()),
                    Internal::User(msg) => {
                        if self.model.update(msg).is_quit() {
                            return Ok(());
                        }
                        dirty = true;
                    }
                }
            }
            if dirty && last_render.elapsed() >= frame {
                self.render()?;
                dirty = false;
                last_render = Instant::now();
            }
        }

        Ok(())
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

        let model = &self.model;
        let view = model.view();
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
