# A stop holds the whole agent

**A stopped agent stays stopped: helpers it left running in the background no longer keep it at work.** That holds for
your stop (`herdr-projects stop`, `Ctrl+X`) and for the board's own, when its program moved onto extra usage under
`usage.extra ask` or `defer` ([extra usage](extra-usage.md)) or a limit was reached ([limits](limits.md)).

A stop interrupts the agent as its program's own stop key does, and ends what it started in its terminal
([what `stop` ends](roles-and-commands.md)). Claude Code's background helpers (its subagents) are not ended by the
interrupt: they work on inside its program, and each one that comes back wakes the agent again. The board used to take
that waking for you typing in the agent's terminal: the task was at work again, reviewed when that turn ended, and,
after a stop for extra usage, given leave to run on it. Tasks worked on for twenty minutes after their stop, on paid
extra usage.

## A minute after the stop

- **The board watches the agent's saved conversation after every stop**, and each helper's beside it (Claude Code keeps
  them in `<conversation>/subagents/`). Half a minute after the interrupt it notes how far they are written; half a
  minute later it looks again.
- **Still written to a minute after the stop**, the agent's program is ended, and its helpers with it, as a hand-over
  ends one. A task you stopped says so: `stopped: its helpers worked on after the stop, so its program was ended`. A
  stop at a limit keeps its own line (`waiting for the usage window: resets 17:00`).
- **Nothing more written**, the agent is kept, as before, for a reply. While a helper it sent to the background has not
  come back (it may wait on a long command without a word), the board goes on looking every half minute.
- The program the board ends is the agent's own, in its terminal. Your other panes, and what runs elsewhere, are left as
  the stop leaves them.

## Woken by itself, it stays stopped

A stopped agent at work again with nobody's prompt since the stop (a helper came back and woke it) leaves the task
stopped: nothing is checked or reviewed when that turn ends, a stop for extra usage gives no leave to run on it, and a
task you stopped is never carried on at a usage window's reset. The board watches it as above, and ends its program if
it is still at work a minute on.

A prompt you type in the agent's own terminal after the stop is your turn, and carries the task on as before. The
board tells the two apart by the agent's saved conversation. Where it reads none (a program whose conversation it does
not know), any new turn counts as yours, as before.

## Carrying it on

- **A task you stopped waits for you**: `herdr-projects retry <id>`, `Ctrl+R`, or a reply carries it on. The agent is
  told that you stopped it and now say to carry on. Where its program was ended, it is started again in its own
  conversation, and told that the helpers it had out are gone, to start again any it still needs.
- **After a stop for extra usage**, the task carries on at the reset as before (or waits for you under `ask`), in its own
  conversation where its program was ended. The agent is told that the board stopped it, not you, and why.

## For contributors

- `crates/flow/src/stop_hold.rs`: the watch (`hold` on the task) and the pass that ends a program, run by whoever ticks;
- `crates/flow/src/stop_hold/woke.rs`: whether a stopped agent was given a prompt since its stop (hook.rs and
  finish/turn_ended.rs re-arm only then); a helper's `<task-notification>` begins no turn (agents/read_turn.rs);
- `crates/flow/src/stop_hold/yours.rs`: carrying on a task you stopped, and the words for a program that was ended;
- tests: `tests/lifecycle/stop_hold.rs`.
