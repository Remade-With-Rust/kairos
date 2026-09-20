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
    unsigned long ulOrdinal;
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

/* ---- Object identity in the trace: CREATION ORDER, not address ---------
 *
 * A C `Queue_t` has no name and its address is not reproducible, so the
 * contract names unnamed objects by a per-kind ordinal: q1, g1, s1.
 *
 * This used to be "position among the same-kind pointers ever seen", which
 * made the ordinal a function of what MALLOC DID. A freed object left its
 * entry behind, so a successor got a new ordinal when it landed on a
 * different block and the OLD ordinal when it landed on the same one. The
 * Rust arena reuses a freed INDEX, so the two rules agreed only by luck --
 * and they disagreed in both directions at once:
 *
 *   EventGroupsDemo  deletes and recreates a same-sized group, malloc hands
 *                    back the same block, C said g2 every time.
 *   AbortDelay       deletes a binary semaphore and creates a 1-item queue,
 *                    malloc hands back a different block, C said q3 where
 *                    the arena's reused index said q2.
 *
 * No rule on ONE side can satisfy both, because the disagreement is about an
 * allocator the two kernels do not share. So the rule changed on BOTH sides
 * to a monotonic per-kind counter: the n-th object of a kind ever created is
 * <kind>n, and an ordinal is never reused.
 *
 * That is strictly better evidence, not a workaround. Identity now depends
 * only on CREATION ORDER -- which is a thing the differential already proves
 * identical, line by line -- instead of on a heap layout that was never
 * checking anything about the kernel under test.
 */

/* Give a newly created object the next ordinal of its kind. */
static unsigned long prvOrdinalNew( char cKind,
                                    const void * pvObject )
{
    unsigned long i, ulNext;

    ulNext = ++ulOrdinalCount[ ( unsigned char ) cKind & 0x7F ];

    /* A freed object's entry stays behind and malloc may hand its block
     * straight back, so REBIND a matching pointer to the new object rather
     * than appending -- otherwise a later lookup finds the dead entry. */
    for( i = 0UL; i < ulOrdinalsUsed; i++ )
    {
        if( ( xOrdinals[ i ].cKind == cKind ) && ( xOrdinals[ i ].pvObject == pvObject ) )
        {
            xOrdinals[ i ].ulOrdinal = ulNext;
            return ulNext;
        }
    }

    if( ulOrdinalsUsed < KAIROS_MAX_OBJECTS )
    {
        xOrdinals[ ulOrdinalsUsed ].cKind = cKind;
        xOrdinals[ ulOrdinalsUsed ].pvObject = pvObject;
        xOrdinals[ ulOrdinalsUsed ].ulOrdinal = ulNext;
        ulOrdinalsUsed++;
        return ulNext;
    }

    return 0UL; /* Out of table: printed as <kind>0, which no real object gets. */
}

/* The ordinal of an object that has already been announced. */
static unsigned long prvOrdinalPerKind( char cKind,
                                        const void * pvObject )
{
    unsigned long i;

    for( i = 0UL; i < ulOrdinalsUsed; i++ )
    {
        if( ( xOrdinals[ i ].cKind == cKind ) && ( xOrdinals[ i ].pvObject == pvObject ) )
        {
            return xOrdinals[ i ].ulOrdinal;
        }
    }

    /* Never announced. Every object the kernel makes fires its own _CREATE
     * first, so reaching here means a trace macro named an object nothing
     * created; register it so the line is at least stable. */
    return prvOrdinalNew( cKind, pvObject );
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

/* The _CREATE entry points. These ASSIGN the ordinal; everything else
 * looks one up. */
void kairos_trace_obj_create( const char * pcEvent,
                              char cKind,
                              const void * pvObject )
{
    prvEnsureBuffer();
    fprintf( stderr, "%lu %s %c%lu", prvTick(), pcEvent, cKind, prvOrdinalNew( cKind, pvObject ) );
    prvTail();
    ulLines++;
}

void kairos_trace_obj_create_u( const char * pcEvent,
                                char cKind,
                                const void * pvObject,
                                unsigned long ulArg )
{
    prvEnsureBuffer();
    fprintf( stderr, "%lu %s %c%lu %lu", prvTick(), pcEvent, cKind, prvOrdinalNew( cKind, pvObject ), ulArg );
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
