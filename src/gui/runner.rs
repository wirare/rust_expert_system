//! Background inference task, isolated from the UI and the solver internals.

use std::any::Any;
use std::sync::mpsc::{self, Receiver, TryRecvError};

use eframe::egui;

use crate::engine::{RunReport, SolveMode, run_source_with_facts};

pub enum RunState
{
    Idle,
    Running,
    Completed(RunReport),
    Failed(String),
}

pub struct RunController
{
    pub state: RunState,
    receiver: Option<Receiver<Result<RunReport, String>>>,
}

impl Default for RunController
{
    fn default() -> Self
    {
        Self { state: RunState::Idle, receiver: None }
    }
}

fn panic_description(payload: &(dyn Any + Send)) -> String
{
    if let Some(message) = payload.downcast_ref::<String>()
    {
        format!("The inference engine panicked: {message}")
    }
    else if let Some(message) = payload.downcast_ref::<&str>()
    {
        format!("The inference engine panicked: {message}")
    }
    else
    {
        "The inference engine terminated unexpectedly".to_owned()
    }
}

impl RunController
{
    pub fn is_running(&self) -> bool
    {
        matches!(self.state, RunState::Running)
    }

    pub fn clear(&mut self)
    {
        if !self.is_running()
        {
            self.state = RunState::Idle;
            self.receiver = None;
        }
    }

    /// Spawn a worker that owns the source snapshot. No borrowed AST or
    /// EvaluationCtx ever crosses this thread boundary.
    pub fn start(
        &mut self,
        source: String,
        mode: SolveMode,
        active_facts: [bool; 26],
        context: egui::Context,
    )
    {
        if self.is_running()
        {
            return;
        }

        let (sender, receiver) = mpsc::channel();
        self.receiver = Some(receiver);
        self.state = RunState::Running;

        std::thread::spawn(move ||
        {
            let result = std::panic::catch_unwind(||
            {
                run_source_with_facts(&source, mode, active_facts)
            });

            let result = match result
            {
                Ok(report) => report,
                Err(panic) => Err(panic_description(panic.as_ref())),
            };

            let _ = sender.send(result);
            context.request_repaint();
        });
    }

    /// Nonblocking completion polling; call once per UI frame.
    pub fn poll(&mut self)
    {
        let status = self.receiver.as_ref().map(|receiver| receiver.try_recv());

        match status
        {
            Some(Ok(Ok(report))) =>
            {
                self.state = RunState::Completed(report);
                self.receiver = None;
            }
            Some(Ok(Err(error))) =>
            {
                self.state = RunState::Failed(error);
                self.receiver = None;
            }
            Some(Err(TryRecvError::Disconnected)) =>
            {
                self.state = RunState::Failed("Inference worker disconnected unexpectedly".to_owned());
                self.receiver = None;
            }
            Some(Err(TryRecvError::Empty)) | None => {}
        }
    }
}
