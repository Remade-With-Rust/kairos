/* K3's tick and switch WORK rows on rv32 -- the C arm.
 *
 * This is FreeRTOS V11.3.1 from the pinned `oracle/` checkout, UNMODIFIED,
 * running on QEMU `virt` with the oracle's own first-party RISC-V port. It
 * measures what one call to xTaskIncrementTick() and one call to
 * vTaskSwitchContext() cost, in RETIRED INSTRUCTIONS.
 *
 * WHY retired instructions and not cycles: QEMU is not a clock. Under
 * `-icount shift=0` the `minstret` CSR is exactly reproducible -- the sibling
 * cell `riscv32-qemu-switch` measured three runs at 199,902 / 199,902 /
 * 199,902 with it and a 20% spread without it. So this is a WORK row, the
 * same basis as `bench/switch-cost`, and it sits beside those rows rather
 * than pretending to be the silicon cycle rows.
 *
 * WHY a bracket and not a profiler: a host self-cost comparison between these
 * two kernels was built and REFUTED (see docs/LEDGER.md, 2026-09-21) --
 * FreeRTOS's tick self cost is 2.7% of its inclusive and ours is 13.1%, so
 * self-vs-callee is a factoring choice and not a measurement. Two csr reads
 * around the call define the boundary instead of the call graph, which is the
 * one thing that makes the arms comparable.
 *
 * NOTHING drives the tick but this program: configMTIME_BASE_ADDRESS is 0, so
 * the port never enables the machine timer and vPortSetupTimerInterrupt below
 * is empty. An interrupt landing inside a bracket would be counted as kernel
 * work.
 */

#include <stddef.h>
#include <stdint.h>

#include "FreeRTOS.h"
#include "queue.h"
#include "task.h"

#define SAMPLES    512

/* The POISON knob, and the reason this bench can be believed.
 *
 * A bracket that does not actually enclose the call reads a plausible number
 * and is worth nothing. So run.sh builds every arm twice -- once with the
 * measured call made ONCE per bracket and once with it made TWICE -- and
 * requires the row to move by one call's worth. A bracket measuring the loop
 * rather than the callee does not move, and the run fails. */
#ifndef KAIROS_REPEAT
    #define KAIROS_REPEAT    1
#endif

/* ------------------------------------------------- the QEMU virt machine -- */

#define UART0          ( ( volatile uint8_t * ) 0x10000000UL )
#define SIFIVE_TEST    ( ( volatile uint32_t * ) 0x00100000UL )

static void uart_putc( char c )
{
    while( ( UART0[ 5 ] & 0x20u ) == 0u )
    {
    }

    UART0[ 0 ] = ( uint8_t ) c;
}

static void prints( const char * s )
{
    while( *s != 0 )
    {
        uart_putc( *s++ );
    }
}

static void printu( uint32_t v )
{
    char buf[ 12 ];
    int n = 0;

    if( v == 0u )
    {
        uart_putc( 48 );
        return;
    }

    while( v != 0u )
    {
        buf[ n++ ] = ( char ) ( 48u + ( v % 10u ) );
        v /= 10u;
    }

    while( n > 0 )
    {
        uart_putc( buf[ --n ] );
    }
}

/* QEMU's `virt` test finisher: this is what makes the cell a GATE rather than
 * something a person reads. 0x5555 exits 0; anything else exits non-zero. */
static void qemu_exit( uint32_t code )
{
    *SIFIVE_TEST = ( code == 0u ) ? 0x5555u : ( 0x3333u | ( code << 16 ) );

    for( ; ; )
    {
    }
}

/* ------------------------------------------------------- the instrument -- */

static inline uint32_t rd_minstret( void )
{
    uint32_t v;

    __asm__ volatile ( "csrr %0, minstret" : "=r" ( v ) );
    return v;
}

/* ------------------------------------------------------------ the rows --- */

static uint32_t samples[ SAMPLES ];
static uint32_t tax;

static void sort_samples( uint32_t * a, int n )
{
    int i, j;

    for( i = 1; i < n; i++ )
    {
        uint32_t key = a[ i ];

        for( j = i - 1; ( j >= 0 ) && ( a[ j ] > key ); j-- )
        {
            a[ j + 1 ] = a[ j ];
        }

        a[ j + 1 ] = key;
    }
}

static void report( const char * name )
{
    uint32_t med, lo, hi;

    sort_samples( samples, SAMPLES );
    med = samples[ SAMPLES / 2 ];
    lo = samples[ 0 ];
    hi = samples[ SAMPLES - 1 ];

    /* The tax comes off every row because it is in every row. Saturating, so
     * a row that cannot out-resolve the instrument reads 0 rather than
     * wrapping to four billion. */
    med = ( med > tax ) ? ( med - tax ) : 0u;
    lo = ( lo > tax ) ? ( lo - tax ) : 0u;
    hi = ( hi > tax ) ? ( hi - tax ) : 0u;

    prints( "ROW " );
    prints( name );
    prints( " median=" );
    printu( med );
    prints( " min=" );
    printu( lo );
    prints( " max=" );
    printu( hi );
    prints( "\n" );
}

/* ------------------------------------------------------------- the tasks - */

/* `mate` exists so the scheduler has a DECISION to make: a switch with one
 * ready task is not a switch. It shares measure's priority and hands control
 * straight back whenever it is given any. */
static void mate_task( void * pv )
{
    ( void ) pv;

    for( ; ; )
    {
        taskYIELD();
    }
}

/* `sleeper` puts something in the DELAYED list and keeps it there. It runs
 * once, at a higher priority, the moment it is created. */
static void sleeper_task( void * pv )
{
    ( void ) pv;

    for( ; ; )
    {
        vTaskDelay( 1000000U );
    }
}

static void measure_task( void * pv )
{
    int i, r;
    uint32_t a, b;

    ( void ) pv;

    prints( "\n=== FreeRTOS V11.3.1 (oracle, unmodified) tick/switch WORK rows, rv32 ===\n" );
    prints( "clock   minstret under -icount shift=0 (retired instructions)\n" );
    prints( "method  median of 512, bracket tax measured and subtracted\n\n" );

    /* The instrument measures itself first. */
    for( i = 0; i < SAMPLES; i++ )
    {
        a = rd_minstret();
        b = rd_minstret();
        samples[ i ] = b - a;
    }

    sort_samples( samples, SAMPLES );
    tax = samples[ SAMPLES / 2 ];
    prints( "TAX median=" );
    printu( tax );
    prints( "\n" );

    /* ROW 1 -- a tick with an EMPTY delayed list. Two tasks ready. */
    for( i = 0; i < SAMPLES; i++ )
    {
        a = rd_minstret();

        for( r = 0; r < KAIROS_REPEAT; r++ )
        {
            ( void ) xTaskIncrementTick();
        }

        b = rd_minstret();
        samples[ i ] = b - a;
    }

    report( "tick_idle" );

    /* Put a task in the delayed list. Creating it at a higher priority makes
     * the kernel switch to it immediately; it delays itself and blocks, and
     * control comes back here. This uses REAL switches, so pxCurrentTCB is
     * consistent afterwards. */
    if( xTaskCreate( sleeper_task, "sleep", configMINIMAL_STACK_SIZE,
                     NULL, 4, NULL ) != pdPASS )
    {
        prints( "RESULT: FAIL -- sleeper was refused\n" );
        qemu_exit( 1u );
    }

    /* ROW 2 -- the same tick with a NON-EMPTY delayed list. Quoting the idle
     * row alone would flatter both kernels equally, so both are reported. */
    for( i = 0; i < SAMPLES; i++ )
    {
        a = rd_minstret();

        for( r = 0; r < KAIROS_REPEAT; r++ )
        {
            ( void ) xTaskIncrementTick();
        }

        b = rd_minstret();
        samples[ i ] = b - a;
    }

    report( "tick_delayed" );

    /* ROW 3 -- the scheduler's selection, LAST.
     *
     * vTaskSwitchContext() moves pxCurrentTCB without saving or restoring any
     * registers -- that half is portASM.S, and `bench/switch-cost` already
     * prices it (30 against 83 cooperative, 74 against 83 preemptive). This
     * row is the OTHER half: choosing who runs next.
     *
     * It is last because it leaves pxCurrentTCB wherever the round robin
     * ends, and nothing below it calls a task API. With exactly two ready
     * tasks at this priority an even sample count returns it to measure. */
    for( i = 0; i < SAMPLES; i++ )
    {
        a = rd_minstret();

        for( r = 0; r < KAIROS_REPEAT; r++ )
        {
            vTaskSwitchContext();
        }

        b = rd_minstret();
        samples[ i ] = b - a;
    }

    report( "switch_select" );

    /* Work-parity anchors: the numbers that must match the Rust arm exactly,
     * or the two arms did different work and the comparison is void. */
    prints( "\nANCHOR samples=" );
    printu( SAMPLES );
    prints( " tick_calls=" );
    printu( 2u * SAMPLES * KAIROS_REPEAT );
    prints( " switch_calls=" );
    printu( SAMPLES * KAIROS_REPEAT );
    prints( " tick_count=" );
    printu( ( uint32_t ) xTaskGetTickCount() );
    prints( "\n" );

    prints( "RESULT: PASS\n" );
    qemu_exit( 0u );
}

/* ---------------------------------------- what the port asks the app for -- */

/* configMTIME_BASE_ADDRESS is 0, so port.c leaves this to the application.
 * Empty ON PURPOSE -- see the header comment. */
void vPortSetupTimerInterrupt( void )
{
}

void vApplicationStackOverflowHook( TaskHandle_t xTask,
                                    char * pcTaskName );
void vApplicationStackOverflowHook( TaskHandle_t xTask,
                                    char * pcTaskName )
{
    ( void ) xTask;
    prints( "RESULT: FAIL -- stack overflow in " );
    prints( pcTaskName );
    prints( "\n" );
    qemu_exit( 2u );
}

/* ------------------------------------------------------------ freestanding */
/* No libc is linked. FreeRTOS needs exactly these three. */

size_t strlen( const char * s )
{
    const char * p = s;

    while( *p != 0 )
    {
        p++;
    }

    return ( size_t ) ( p - s );
}

void * memset( void * d, int c, size_t n )
{
    uint8_t * p = ( uint8_t * ) d;

    while( n-- != 0u )
    {
        *p++ = ( uint8_t ) c;
    }

    return d;
}

void * memcpy( void * d, const void * s, size_t n )
{
    uint8_t * p = ( uint8_t * ) d;
    const uint8_t * q = ( const uint8_t * ) s;

    while( n-- != 0u )
    {
        *p++ = *q++;
    }

    return d;
}

int main( void )
{
    /* mate first, measure second: prvAddNewTaskToReadyList makes the most
     * recently created task of the highest priority the current one, so
     * measure is the task the scheduler enters. */
    prints( "M0\n" );

    if( xTaskCreate( mate_task, "mate", configMINIMAL_STACK_SIZE, NULL, 3, NULL ) != pdPASS )
    {
        prints( "RESULT: FAIL -- mate was refused\n" );
        qemu_exit( 1u );
    }

    if( xTaskCreate( measure_task, "meas", configMINIMAL_STACK_SIZE * 2, NULL, 3, NULL ) != pdPASS )
    {
        prints( "RESULT: FAIL -- measure was refused\n" );
        qemu_exit( 1u );
    }

    prints( "M2\n" );

    vTaskStartScheduler();

    prints( "RESULT: FAIL -- the scheduler returned\n" );
    qemu_exit( 3u );
    return 0;
}
