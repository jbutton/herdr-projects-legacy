//! Extra usage (`usage.extra`, docs/extra-usage.md): an agent that works on past its usage window, on paid extra usage
//! (Claude Code's usage credits, Codex's credits), where nothing fails and so nothing was seen. The readings and each
//! program's window are sw-core's (agents/read_window.rs, store/usage.rs); here what a task does with them: the mark,
//! the warnings, and under `ask` and `defer` the stops and the start held back. One area per file.
pub mod gate;
pub mod stop;
pub mod tick;
pub mod words;
