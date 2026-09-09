/*
 * Kairos oracle harness — runs one standard demo scenario on the C kernel
 * (FreeRTOS-Kernel V11.3.1, Posix port, deterministic tick) and stops after a
 * given number of ticks, reporting the scenario's own verdict.
 *
 *     ./<scenario> <max_ticks>
 *
 * The trace goes to stderr (kairos_trace.c); the verdict line
 *     KAIROS_RESULT <scenario> pass|fail ticks=<n> yields=<n> exits=<n> lines=<n>
 * is the last line of stderr, and the exit code is 0 on pass.
 *
 * The sim contract (umbrella ORACLES.md): no timer thread; the idle hook
 * delivers one tick per pass; every 16th outermost critical-section exit
 * delivers one (the port patch: where a hardware tick that arrived during
 * the section would fire); a CHECK task at priority 5 wakes every 100 ticks
 * and ends the run at max_ticks. rusty_rtos_demo's runner does the same.
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#include "FreeRTOS.h"
#include "task.h"

#include "kairos_trace.h"

/* The standard demo tasks (FreeRTOS/Demo/Common/include). */
#include "dynamic.h"

/* From the patched Posix port. */
extern void vPortKairosTick( void );
extern unsigned long ulKairosYields;
extern unsigned long ulKairosTicks;
extern unsigned long ulKairosExits;

#define harnessCHECK_TASK_PRIORITY    ( tskIDLE_PRIORITY + 5 )
#define harnessCHECK_PERIOD_TICKS     ( 100 )

typedef struct
{
    const char * pcName;
    void ( * pfnStart )( void );
    BaseType_t ( * pfnStillRunning )( void );
} Scenario_t;

static void prvStartDynamic( void )
{
    vStartDynamicPriorityTasks();
}

static const Scenario_t xScenarios[] =
{
    { "dynamic", prvStartDynamic, xAreDynamicPriorityTasksStillRunning },
};

static const Scenario_t * pxScenario = NULL;
static unsigned long ulMaxTicks = 2000UL;

static void prvFinish( BaseType_t xPass )
{
    fprintf( stderr, "KAIROS_RESULT %s %s ticks=%lu yields=%lu exits=%lu lines=%lu\n",
             pxScenario->pcName,
             xPass == pdTRUE ? "pass" : "fail",
             ( unsigned long ) xTaskGetTickCount(),
             ulKairosYields,
             ulKairosExits,
             kairos_trace_lines() );
    kairos_trace_flush();
    _exit( xPass == pdTRUE ? 0 : 1 );
}

static void prvCheckTask( void * pvParameters )
{
    ( void ) pvParameters;

    for( ; ; )
    {
        vTaskDelay( harnessCHECK_PERIOD_TICKS );

        if( ( unsigned long ) xTaskGetTickCount() >= ulMaxTicks )
        {
            prvFinish( pxScenario->pfnStillRunning() );
        }
    }
}

int main( int argc,
          char ** argv )
{
    size_t i;

    if( argc < 2 )
    {
        fprintf( stderr, "usage: %s <scenario> [max_ticks]\n", argv[ 0 ] );
        return 2;
    }

    for( i = 0; i < sizeof( xScenarios ) / sizeof( xScenarios[ 0 ] ); i++ )
    {
        if( strcmp( argv[ 1 ], xScenarios[ i ].pcName ) == 0 )
        {
            pxScenario = &xScenarios[ i ];
        }
    }

    if( pxScenario == NULL )
    {
        fprintf( stderr, "unknown scenario %s\n", argv[ 1 ] );
        return 2;
    }

    if( argc > 2 )
    {
        ulMaxTicks = strtoul( argv[ 2 ], NULL, 10 );
    }

    pxScenario->pfnStart();
    xTaskCreate( prvCheckTask, "CHECK", configMINIMAL_STACK_SIZE, NULL, harnessCHECK_TASK_PRIORITY, NULL );
    vTaskStartScheduler();

    /* vTaskStartScheduler only returns when the kernel could not start. */
    fprintf( stderr, "KAIROS_RESULT %s fail scheduler-did-not-start\n", pxScenario->pcName );
    kairos_trace_flush();
    return 1;
}

/* --------------------------------------------------------------- hooks --- */

void vApplicationIdleHook( void )
{
    /* The sim contract: one tick per idle pass. */
    vPortKairosTick();
}

void vApplicationMallocFailedHook( void )
{
    fprintf( stderr, "KAIROS_RESULT %s fail malloc-failed\n", pxScenario ? pxScenario->pcName : "?" );
    kairos_trace_flush();
    _exit( 3 );
}

void vApplicationStackOverflowHook( TaskHandle_t pxTask,
                                    char * pcTaskName )
{
    ( void ) pxTask;
    fprintf( stderr, "KAIROS_RESULT %s fail stack-overflow %s\n", pxScenario ? pxScenario->pcName : "?", pcTaskName );
    kairos_trace_flush();
    _exit( 3 );
}

void vAssertCalled( const char * const pcFileName,
                    unsigned long ulLine )
{
    fprintf( stderr, "KAIROS_RESULT %s fail assert %s:%lu\n", pxScenario ? pxScenario->pcName : "?", pcFileName, ulLine );
    kairos_trace_flush();
    _exit( 4 );
}
