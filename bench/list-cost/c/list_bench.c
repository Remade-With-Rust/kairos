/*
 * The C arm of the K1 arena-list cost row.
 *
 * It runs FreeRTOS's own list.c through the four operations the scheduler
 * actually performs on a ready list and a delayed list, `rounds` times, and
 * prints a checksum of what every operation returned. The Rust arm
 * (../rs/src/main.rs) does exactly the same work in the same order and must
 * print exactly the same checksum: that is the correctness gate, without
 * which the two instruction counts would be measuring different programs.
 *
 * Nothing here is timed. The run is counted under callgrind, twice, at
 * `rounds` and at `rounds / 2`; the difference divided by `rounds / 2` is
 * the marginal cost of one round, with process start-up, libc and the final
 * printf cancelled exactly rather than estimated.
 */

#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>

#include "FreeRTOS.h"
#include "list.h"

/* How many items are in a list at once: a plausible ready-list depth. */
#define BENCH_ITEMS    8

/* The pseudo-random values the ordered insert sorts by. A 32-bit xorshift,
 * spelled identically in both arms so both lists see the same sequence. */
static uint32_t prvNext( uint32_t * pulState )
{
    uint32_t ulX = *pulState;

    ulX ^= ulX << 13;
    ulX ^= ulX >> 17;
    ulX ^= ulX << 5;
    *pulState = ulX;
    return ulX;
}

static uint64_t prvMix( uint64_t ullSum, uint64_t ullValue )
{
    ullSum ^= ullValue;
    ullSum *= 0x100000001b3ULL;
    return ullSum;
}

int main( int argc, char ** argv )
{
    static List_t xReady, xDelayed;
    static ListItem_t xItems[ BENCH_ITEMS ];
    unsigned long ulRounds = 100000UL;
    uint64_t ullSum = 0xcbf29ce484222325ULL;
    uint32_t ulState = 0x12345678UL;
    unsigned long ulRound;
    int i;

    if( argc > 1 )
    {
        ulRounds = strtoul( argv[ 1 ], NULL, 10 );
    }

    vListInitialise( &xReady );
    vListInitialise( &xDelayed );

    for( i = 0; i < BENCH_ITEMS; i++ )
    {
        vListInitialiseItem( &xItems[ i ] );
        listSET_LIST_ITEM_OWNER( &xItems[ i ], ( void * ) ( uintptr_t ) i );
    }

    for( ulRound = 0; ulRound < ulRounds; ulRound++ )
    {
        /* prvAddTaskToReadyList: append to the end of a ready list. */
        for( i = 0; i < BENCH_ITEMS; i++ )
        {
            vListInsertEnd( &xReady, &xItems[ i ] );
        }

        /* taskSELECT_HIGHEST_PRIORITY_TASK: walk the round robin. */
        for( i = 0; i < BENCH_ITEMS; i++ )
        {
            void * pvOwner;
            listGET_OWNER_OF_NEXT_ENTRY( pvOwner, &xReady );
            ullSum = prvMix( ullSum, ( uint64_t ) ( uintptr_t ) pvOwner );
        }

        /* Leaving the ready list. */
        for( i = 0; i < BENCH_ITEMS; i++ )
        {
            ullSum = prvMix( ullSum, ( uint64_t ) uxListRemove( &xItems[ i ] ) );
        }

        /* prvAddCurrentTaskToDelayedList: an ordered insert by wake time. */
        for( i = 0; i < BENCH_ITEMS; i++ )
        {
            listSET_LIST_ITEM_VALUE( &xItems[ i ], ( TickType_t ) prvNext( &ulState ) );
            vListInsert( &xDelayed, &xItems[ i ] );
        }

        /* And out again, as the tick unblocks them. */
        for( i = 0; i < BENCH_ITEMS; i++ )
        {
            ullSum = prvMix( ullSum, ( uint64_t ) uxListRemove( &xItems[ i ] ) );
        }
    }

    printf( "rounds=%lu items=%d checksum=%016llx\n",
            ulRounds, BENCH_ITEMS, ( unsigned long long ) ullSum );
    return 0;
}

/* configASSERT's target; never reached in this driver. */
void vAssertCalled( const char * const pcFileName, unsigned long ulLine )
{
    fprintf( stderr, "assert %s:%lu\n", pcFileName, ulLine );
    abort();
}
