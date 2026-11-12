//! The push of a task's branch to origin, one file each in tests/push/: what it may replace there and when it is not
//! needed (lease.rs), how long it may run (timeout.rs), and the tools the repo's hooks run with (tools.rs).
mod lease;
mod timeout;
mod tools;
