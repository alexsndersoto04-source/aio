use crate::{Value, VmError};
use std::collections::HashSet;
use std::sync::mpsc::{self, Receiver, Sender};
use titan_codegen::SourceLocation;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Breakpoint { Instruction { function: usize, instruction: usize }, Line { function: usize, line: usize }, SourceLine { source_file: String, line: usize } }
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugMode { Continue, StepIn, StepOver { depth: usize }, StepOut { depth: usize } }
#[derive(Debug, Clone, PartialEq)]
pub struct DebugFrame { pub function_id: usize, pub function_name: String, pub source_file: Option<String>, pub instruction: usize, pub depth: usize, pub location: Option<SourceLocation>, pub locals: Vec<Value>, pub stack: Vec<Value> }
#[derive(Debug, Clone, PartialEq)]
pub enum DebugEvent { Stopped(DebugFrame), Terminated { error: Option<String> } }
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DebugCommand { Continue, StepIn, StepOver, StepOut, Terminate, AddBreakpoint(Breakpoint), RemoveBreakpoint(Breakpoint) }

pub trait DebugHook {
    fn before_instruction(&mut self, frame: &DebugFrame) -> Result<(), VmError>;
    fn terminated(&mut self, _error: Option<&VmError>) {}
}

pub struct DebugController { commands: Sender<DebugCommand>, events: Receiver<DebugEvent> }
impl DebugController {
    pub fn command(&self, command: DebugCommand) -> Result<(), String> { self.commands.send(command).map_err(|_| "debug session has ended".into()) }
    pub fn recv(&self) -> Result<DebugEvent, String> { self.events.recv().map_err(|_| "debug session has ended".into()) }
    pub fn try_recv(&self) -> Result<Option<DebugEvent>, String> { match self.events.try_recv() { Ok(event) => Ok(Some(event)), Err(mpsc::TryRecvError::Empty) => Ok(None), Err(mpsc::TryRecvError::Disconnected) => Err("debug session has ended".into()) } }
}

pub struct Debugger { breakpoints: HashSet<Breakpoint>, mode: DebugMode, commands: Receiver<DebugCommand>, events: Sender<DebugEvent> }
impl Debugger {
    pub fn channel(breakpoints: impl IntoIterator<Item = Breakpoint>) -> (DebugController, Self) {
        let (command_tx, command_rx) = mpsc::channel(); let (event_tx, event_rx) = mpsc::channel();
        (DebugController { commands: command_tx, events: event_rx }, Self { breakpoints: breakpoints.into_iter().collect(), mode: DebugMode::Continue, commands: command_rx, events: event_tx })
    }
    fn matches_breakpoint(&self, frame: &DebugFrame) -> bool { self.breakpoints.iter().any(|breakpoint| match breakpoint { Breakpoint::Instruction { function, instruction } => *function == frame.function_id && *instruction == frame.instruction, Breakpoint::Line { function, line } => *function == frame.function_id && frame.location.is_some_and(|location| location.line == *line), Breakpoint::SourceLine { source_file, line } => frame.source_file.as_ref() == Some(source_file) && frame.location.is_some_and(|location| location.line == *line) }) }
    fn should_stop(&self, frame: &DebugFrame) -> bool { match self.mode { DebugMode::Continue => self.matches_breakpoint(frame), DebugMode::StepIn => true, DebugMode::StepOver { depth } => frame.depth <= depth, DebugMode::StepOut { depth } => frame.depth < depth } }
}
impl DebugHook for Debugger {
    fn before_instruction(&mut self, frame: &DebugFrame) -> Result<(), VmError> {
        while let Ok(command) = self.commands.try_recv() { match command { DebugCommand::AddBreakpoint(value) => { self.breakpoints.insert(value); } DebugCommand::RemoveBreakpoint(value) => { self.breakpoints.remove(&value); } DebugCommand::Terminate => return Err(VmError::DebugTerminated), command => { self.mode = command_mode(command, frame.depth); } } }
        if !self.should_stop(frame) { return Ok(()); }
        self.events.send(DebugEvent::Stopped(frame.clone())).map_err(|_| VmError::DebugTerminated)?;
        loop { match self.commands.recv().map_err(|_| VmError::DebugTerminated)? { DebugCommand::Continue => { self.mode = DebugMode::Continue; return Ok(()); } DebugCommand::StepIn => { self.mode = DebugMode::StepIn; return Ok(()); } DebugCommand::StepOver => { self.mode = DebugMode::StepOver { depth: frame.depth }; return Ok(()); } DebugCommand::StepOut => { self.mode = DebugMode::StepOut { depth: frame.depth }; return Ok(()); } DebugCommand::Terminate => return Err(VmError::DebugTerminated), DebugCommand::AddBreakpoint(value) => { self.breakpoints.insert(value); } DebugCommand::RemoveBreakpoint(value) => { self.breakpoints.remove(&value); } } }
    }
    fn terminated(&mut self, error: Option<&VmError>) { let _ = self.events.send(DebugEvent::Terminated { error: error.map(ToString::to_string) }); }
}
fn command_mode(command: DebugCommand, depth: usize) -> DebugMode { match command { DebugCommand::Continue => DebugMode::Continue, DebugCommand::StepIn => DebugMode::StepIn, DebugCommand::StepOver => DebugMode::StepOver { depth }, DebugCommand::StepOut => DebugMode::StepOut { depth }, _ => DebugMode::Continue } }
