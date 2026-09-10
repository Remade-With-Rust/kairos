/*
 * Kairos oracle harness — the trace printer. See kairos_trace.h.
 *
 * Every function here is called from inside the kernel, often inside a
 * critical section, on whichever task's pthread is running. The Posix port
 * runs one task at a time, so the lines interleave deterministically; stdio's
 * own lock covers the rest. The tick comes from xTaskGetTickCount(), a plain
 * read of the kernel's counter.
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "FreeRTOS.h"
#include "task.h"

#include "kairos_trace.h"

#define KAIROS_MAX_OBJECTS    512

typedef struct
{
    char cKind;
    const void * pvObject;
} Ordinal_t;

static Ordinal_t xOrdinals[ KAIROS_MAX_OBJECTS ];
static unsigned long ulOrdinalCount[ 128 ]; /* per kind byte */
static unsigned long ulOrdinalsUsed = 0UL;
static unsigned long ulLines = 0UL;
static int xBufferSet = 0;
static int xShowExits = 0;

/* The patched Posix port's outermost-exit counter (sim contract v1, rule 3). */
extern unsigned long ulKairosExits;

void kairos_trace_debug_init( void )
{
    xShowExits = ( getenv( "KAIROS_TRACE_EXITS" ) != NULL ) ? 1 : 0;
}

/* End the line, with the debug column when it is switched on. */
static void prvTail( void )
{
    if( xShowExits != 0 )
    {
        fprintf( stderr, " #%lu", ulKairosExits );
    }

    fputc( '\n', stderr );
}

static void prvEnsureBuffer( void )
{
    static char cBuffer[ 1UL << 20 ];

    if( xBufferSet == 0 )
    {
        /* Fully buffered, one MiB: the run is short and the harness flushes on exit. */
        setvbuf( stderr, cBuffer, _IOFBF, sizeof( cBuffer ) );
        xBufferSet = 1;
    }
}

/* The creation ordinal of an object of one kind, 1-based, first-seen order. */
static unsigned long prvOrdinal( char cKind,
                                 const void * pvObject )
{
    unsigned long i;

    for( i = 0UL; i < ulOrdinalsUsed; i++ )
    {
        if( ( xOrdinals[ i ].cKind == cKind ) && ( xOrdinals[ i ].pvObject == pvObject ) )
        {
            return i + 1UL;
        }
    }

    if( ulOrdinalsUsed < KAIROS_MAX_OBJECTS )
    {
        xOrdinals[ ulOrdinalsUsed ].cKind = cKind;
        xOrdinals[ ulOrdinalsUsed ].pvObject = pvObject;
        ulOrdinalsUsed++;
        ulOrdinalCount[ ( unsigned char ) cKind & 0x7F ]++;
        return ulOrdinalsUsed;
    }

    return 0UL; /* Out of table: printed as <kind>0, which no real object gets. */
}

/* Ordinals are numbered per kind: the n-th queue is q<n> regardless of how
 * many event groups were created before it. */
static unsigned long prvOrdinalPerKind( char cKind,
                                        const void * pvObject )
{
    unsigned long i, ulSeen = 0UL;

    for( i = 0UL; i < ulOrdinalsUsed; i++ )
    {
        if( xOrdinals[ i ].cKind == cKind )
        {
            ulSeen++;

            if( xOrdinals[ i ].pvObject == pvObject )
            {
                return ulSeen;
            }
        }
    }

    ( void ) prvOrdinal( cKind, pvObject );
    return ulOrdinalCount[ ( unsigned char ) cKind & 0x7F ];
}

static unsigned long prvTick( void )
{
    return ( unsigned long ) xTaskGetTickCount();
}

void kairos_trace_ev( const char * pcEvent )
{
    prvEnsureBuffer();
    fprintf( stderr, "%lu %s", prvTick(), pcEvent );
    prvTail();
    ulLines++;
}

void kairos_trace_u( const char * pcEvent,
                     unsigned long ulArg )
{
    prvEnsureBuffer();
    fprintf( stderr, "%lu %s %lu", prvTick(), pcEvent, ulArg );
    prvTail();
    ulLines++;
}

/* ---- Timer names, looked up rather than dereferenced. ------------------
 *
 * `traceTIMER_COMMAND_SEND` fires AFTER `xQueueSendToBack` has queued the
 * command (timers.c:486). The daemon runs at a higher priority than most
 * tasks, so for `tmrCOMMAND_DELETE` it can preempt, process the delete and
 * `vPortFree` the `Timer_t` before the sending task resumes — at which
 * point the handle the macro is handed is dangling.
 *
 * That is not hypothetical and it is not ours: AddressSanitizer names it
 * exactly on `TaskNotify`, freed by the timer task in
 * `prvProcessReceivedCommands` and read back in
 * `xTimerGenericCommandFromTask`. Upstream never sees it because the
 * default `traceTIMER_COMMAND_SEND` ignores its arguments; ANY tracer that
 * dereferences the handle has a use-after-free on delete.
 *
 * So this harness never dereferences a timer handle outside creation. The
 * name is recorded at `traceTIMER_CREATE` and looked up by POINTER VALUE,
 * which stays a valid key after the object is freed. Output is unchanged,
 * which `TimerDemo`'s pinned trace proves. */
#define kairosMAX_TIMERS    64

static struct
{
    const void * pvHandle;
    const char * pcName;
} xTimerNames[ kairosMAX_TIMERS ];

static size_t xTimerNameCount = 0;

void kairos_trace_timer_create( const void * pvTimer,
                                const char * pcName )
{
    size_t i;

    /* A freed timer's address can be handed straight back by the
     * allocator, so a repeat handle overwrites rather than duplicates. */
    for( i = 0; i < xTimerNameCount; i++ )
    {
        if( xTimerNames[ i ].pvHandle == pvTimer )
        {
            xTimerNames[ i ].pcName = pcName;
            kairos_trace_task( "TIMER_CREATE", pcName );
            return;
        }
    }

    if( xTimerNameCount < kairosMAX_TIMERS )
    {
        xTimerNames[ xTimerNameCount ].pvHandle = pvTimer;
        xTimerNames[ xTimerNameCount ].pcName = pcName;
        xTimerNameCount++;
    }

    kairos_trace_task( "TIMER_CREATE", pcName );
}

/* The name recorded for `pvTimer`, or a marker that CANNOT be mistaken for
 * a name — a silently wrong name would corrupt a trace that is compared
 * byte for byte, so an overflow or an unknown handle has to be visible. */
static const char * prvTimerName( const void * pvTimer )
{
    size_t i;

    for( i = 0; i < xTimerNameCount; i++ )
    {
        if( xTimerNames[ i ].pvHandle == pvTimer )
        {
            return xTimerNames[ i ].pcName;
        }
    }

    return "<UNKNOWN-TIMER>";
}

void kairos_trace_timer_command( const void * pvTimer,
                                 long lCommand,
                                 unsigned long ulValue )
{
    kairos_trace_task_iu( "TIMER_COMMAND_SEND", prvTimerName( pvTimer ), lCommand, ulValue );
}

void kairos_trace_task( const char * pcEvent,
                        const char * pcName )
{
    prvEnsureBuffer();
    fprintf( stderr, "%lu %s %s", prvTick(), pcEvent, pcName );
    prvTail();
    ulLines++;
}

void kairos_trace_task_u( const char * pcEvent,
                          const char * pcName,
                          unsigned long ulArg )
{
    prvEnsureBuffer();
    fprintf( stderr, "%lu %s %s %lu", prvTick(), pcEvent, pcName, ulArg );
    prvTail();
    ulLines++;
}

void kairos_trace_task_iu( const char * pcEvent,
                           const char * pcName,
                           long lArg,
                           unsigned long ulArg )
{
    prvEnsureBuffer();
    fprintf( stderr, "%lu %s %s %ld %lu", prvTick(), pcEvent, pcName, lArg, ulArg );
    prvTail();
    ulLines++;
}

void kairos_trace_obj( const char * pcEvent,
                       char cKind,
                       const void * pvObject )
{
    prvEnsureBuffer();
    fprintf( stderr, "%lu %s %c%lu", prvTick(), pcEvent, cKind, prvOrdinalPerKind( cKind, pvObject ) );
    prvTail();
    ulLines++;
}

void kairos_trace_obj_u( const char * pcEvent,
                         char cKind,
                         const void * pvObject,
                         unsigned long ulArg )
{
    prvEnsureBuffer();
    fprintf( stderr, "%lu %s %c%lu %lu", prvTick(), pcEvent, cKind, prvOrdinalPerKind( cKind, pvObject ), ulArg );
    prvTail();
    ulLines++;
}

void kairos_trace_obj_uu( const char * pcEvent,
                          char cKind,
                          const void * pvObject,
                          unsigned long ulArg1,
                          unsigned long ulArg2 )
{
    prvEnsureBuffer();
    fprintf( stderr, "%lu %s %c%lu %lu %lu", prvTick(), pcEvent, cKind, prvOrdinalPerKind( cKind, pvObject ), ulArg1, ulArg2 );
    prvTail();
    ulLines++;
}

unsigned long kairos_trace_lines( void )
{
    return ulLines;
}

void kairos_trace_flush( void )
{
    fflush( stderr );
}
