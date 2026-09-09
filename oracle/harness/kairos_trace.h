/*
 * Kairos oracle harness — the trace printer the FreeRTOSConfig.h trace macros
 * call. One line per event on stderr, in the contract's format:
 *
 *     <tick> <EVENT> [<subject>] [<arg>...]
 *
 * Tasks and timers are named by their own names; queues, event groups and
 * stream buffers by a creation ordinal per kind (q1, q2, ... g1, ... s1, ...),
 * assigned in first-seen order, so the line carries no address and two runs
 * (and the Rust kernel) print the same thing.
 */

#ifndef KAIROS_TRACE_H
#define KAIROS_TRACE_H

void kairos_trace_ev( const char * pcEvent );
void kairos_trace_u( const char * pcEvent, unsigned long ulArg );
void kairos_trace_task( const char * pcEvent, const char * pcName );
void kairos_trace_task_u( const char * pcEvent, const char * pcName, unsigned long ulArg );
void kairos_trace_task_iu( const char * pcEvent, const char * pcName, long lArg, unsigned long ulArg );
void kairos_trace_obj( const char * pcEvent, char cKind, const void * pvObject );
void kairos_trace_obj_u( const char * pcEvent, char cKind, const void * pvObject, unsigned long ulArg );
void kairos_trace_obj_uu( const char * pcEvent, char cKind, const void * pvObject, unsigned long ulArg1, unsigned long ulArg2 );

/* How many lines have been printed; the harness reports it at the end. */
unsigned long kairos_trace_lines( void );

/* With KAIROS_TRACE_EXITS set in the environment, every line gains a final
 * " #<outermost critical-section exits>" column. It is the only way to see
 * where sim time is passing, and a Rust-side divergence is almost always a
 * disagreement about this number rather than about the event itself. Off by
 * default: the contract's line format has no such column. */
void kairos_trace_debug_init( void );

/* Flush the trace (the harness exits with _exit so the buffer must be flushed by hand). */
void kairos_trace_flush( void );

#endif /* KAIROS_TRACE_H */
