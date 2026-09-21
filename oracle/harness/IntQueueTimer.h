/*
 * IntQueueTimer.h -- the board half of IntQueue, for the Kairos sim harness.
 *
 * `IntQueue.c` is the only file in the corpus that includes a header no
 * demo directory supplies: every BOARD project writes its own
 * IntQueueTimer.c/h, because the demo needs two timer interrupts and only
 * the board knows what a timer is. That missing header is the whole reason
 * `IntQueue` was out of the corpus -- the file did not compile, so nothing
 * downstream of it could be judged.
 *
 * The contract is one function. `vStartInterruptQueueTasks` calls it once
 * after creating its tasks, and the board is then expected to call
 * `xFirstTimerHandler()` and `xSecondTimerHandler()` from two interrupts of
 * DIFFERENT priority, so that the second can nest inside the first.
 *
 * WHAT THIS HARNESS DOES INSTEAD, said plainly
 * ============================================
 *
 * This port has one interrupt source -- the tick -- and no nesting. So
 * `vInitialiseTimerForIntQueueTest` starts nothing, and the harness calls
 * both handlers from its tick hook, first then second, once per tick.
 *
 * That runs every queue access the demo makes, in a fixed order, and
 * `xAreIntQueueTasksStillRunning` is satisfied by it -- that function checks
 * only that all four tasks are still cycling and that no access logged an
 * error. It does NOT check that nesting occurred, and nothing else in the
 * file does either.
 *
 * So what this buys is real and it is bounded: the demo's queue accesses
 * from an interrupt, against tasks of three priorities, proven
 * trace-identical to the C kernel running the same way. What it does NOT
 * buy is the property the demo was written for -- that a high-priority
 * interrupt nesting inside a low-priority one still leaves the queues
 * coherent. That needs nested interrupts, which this port does not have,
 * and no arrangement of a single tick hook can stand in for it.
 *
 * Both halves of that are recorded in docs/LEDGER.md rather than left for a
 * reader to infer from a passing trace.
 */

#ifndef INT_QUEUE_TIMER_H
#define INT_QUEUE_TIMER_H

/*
 * On a board: start the two timers. Here: nothing, because the tick hook
 * drives both handlers. Kept as a real function rather than a macro so the
 * call still appears where the demo makes it.
 */
void vInitialiseTimerForIntQueueTest( void );

#endif /* INT_QUEUE_TIMER_H */
