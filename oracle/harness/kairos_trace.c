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
    fprintf( stderr, "%lu %s\n", prvTick(), pcEvent );
    ulLines++;
}

void kairos_trace_u( const char * pcEvent,
                     unsigned long ulArg )
{
    prvEnsureBuffer();
    fprintf( stderr, "%lu %s %lu\n", prvTick(), pcEvent, ulArg );
    ulLines++;
}

void kairos_trace_task( const char * pcEvent,
                        const char * pcName )
{
    prvEnsureBuffer();
    fprintf( stderr, "%lu %s %s\n", prvTick(), pcEvent, pcName );
    ulLines++;
}

void kairos_trace_task_u( const char * pcEvent,
                          const char * pcName,
                          unsigned long ulArg )
{
    prvEnsureBuffer();
    fprintf( stderr, "%lu %s %s %lu\n", prvTick(), pcEvent, pcName, ulArg );
    ulLines++;
}

void kairos_trace_task_iu( const char * pcEvent,
                           const char * pcName,
                           long lArg,
                           unsigned long ulArg )
{
    prvEnsureBuffer();
    fprintf( stderr, "%lu %s %s %ld %lu\n", prvTick(), pcEvent, pcName, lArg, ulArg );
    ulLines++;
}

void kairos_trace_obj( const char * pcEvent,
                       char cKind,
                       const void * pvObject )
{
    prvEnsureBuffer();
    fprintf( stderr, "%lu %s %c%lu\n", prvTick(), pcEvent, cKind, prvOrdinalPerKind( cKind, pvObject ) );
    ulLines++;
}

void kairos_trace_obj_u( const char * pcEvent,
                         char cKind,
                         const void * pvObject,
                         unsigned long ulArg )
{
    prvEnsureBuffer();
    fprintf( stderr, "%lu %s %c%lu %lu\n", prvTick(), pcEvent, cKind, prvOrdinalPerKind( cKind, pvObject ), ulArg );
    ulLines++;
}

void kairos_trace_obj_uu( const char * pcEvent,
                          char cKind,
                          const void * pvObject,
                          unsigned long ulArg1,
                          unsigned long ulArg2 )
{
    prvEnsureBuffer();
    fprintf( stderr, "%lu %s %c%lu %lu %lu\n", prvTick(), pcEvent, cKind, prvOrdinalPerKind( cKind, pvObject ), ulArg1, ulArg2 );
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
