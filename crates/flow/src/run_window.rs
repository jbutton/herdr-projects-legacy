//! Run windows (`run.window`, `run.windowEnd`, docs/run-window.md): a task's agent runs only within a window of this
//! machine's time, such as nights or weekends. The value and when a window is open are sw-core's
//! (store/settings/run_window.rs, window_times.rs); here what a task does with them, on the same waits PRJ-26 built for
//! extra usage: a start held in the queue (gate.rs, beside usage/gate.rs), and under `pause` the limits' own stop with
//! its carry-on by itself (pause.rs). The words: words.rs. One area per file.
pub mod gate;
pub mod words;
