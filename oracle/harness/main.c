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
#include "BlockQ.h"
#include "EventGroupsDemo.h"
#if defined( KAIROS_AMP )
    #include "MessageBufferAMP.h"
#endif
#include "GenQTest.h"
#include "TaskNotify.h"
#include "AbortDelay.h"
#include "IntSemTest.h"
#include "StreamBufferDemo.h"
#include "MessageBufferDemo.h"
#include "StreamBufferInterrupt.h"
#include "TimerDemo.h"
#include "PollQ.h"
#include "QPeek.h"
#include "QueueOverwrite.h"
#include "QueueSetPolling.h"
#include "QueueSet.h"
#include "IntQueue.h"
#include "blocktim.h"
#include "countsem.h"
#include "death.h"
#include "dynamic.h"
#include "recmutex.h"
#include "semtest.h"

/* From the patched Posix port. */
extern void vPortKairosTick( void );
extern unsigned long ulKairosYields;
extern unsigned long ulKairosTicks;
extern unsigned long ulKairosExits;

#define harnessCHECK_TASK_PRIORITY    ( tskIDLE_PRIORITY + 5 )
#define harnessCHECK_PERIOD_TICKS     ( 100 )

/* The priority each scenario that takes one is started at. Written down
 * here because the Rust corpus has to use exactly these: a scenario started
 * at a different priority is a different scenario, and its trace would
 * rightly differ. */
#define harnessPOLLQ_PRIORITY         ( tskIDLE_PRIORITY + 2 )
#define harnessBLOCKQ_PRIORITY        ( tskIDLE_PRIORITY + 2 )
#define harnessSEMTEST_PRIORITY       ( tskIDLE_PRIORITY + 1 )
#define harnessGENQ_PRIORITY          ( tskIDLE_PRIORITY )
#define harnessQOVERWRITE_PRIORITY    ( tskIDLE_PRIORITY + 1 )
/* The upstream Posix demo's `mainCREATOR_TASK_PRIORITY`. */
#define harnessDEATH_PRIORITY         ( tskIDLE_PRIORITY + 3 )
#define harnessTIMER_BASE_PERIOD      ( 50 )

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

static void prvStartPollQ( void )
{
    vStartPolledQueueTasks( harnessPOLLQ_PRIORITY );
}

static void prvStartBlockQ( void )
{
    vStartBlockingQueueTasks( harnessBLOCKQ_PRIORITY );
}

static void prvStartSemTest( void )
{
    vStartSemaphoreTasks( harnessSEMTEST_PRIORITY );
}

static void prvStartCountSem( void )
{
    vStartCountingSemaphoreTasks();
}

static void prvStartRecMutex( void )
{
    vStartRecursiveMutexTasks();
}

static void prvStartBlockTim( void )
{
    vCreateBlockTimeTasks();
}

static void prvStartQPeek( void )
{
    vStartQueuePeekTasks();
}

static void prvStartGenQTest( void )
{
    vStartGenericQueueTasks( harnessGENQ_PRIORITY );
}

static void prvStartTaskNotify( void )
{
    vStartTaskNotifyTask();
}

static void prvStartAbortDelay( void )
{
    vCreateAbortDelayTasks();
}

static void prvStartDeath( void )
{
    vCreateSuicidalTasks( harnessDEATH_PRIORITY );
}

static void prvStartQueueOverwrite( void )
{
    vStartQueueOverwriteTask( harnessQOVERWRITE_PRIORITY );
}

static void prvStartQueueSetPolling( void )
{
    vStartQueueSetPollingTask();
}

static void prvStartQueueSet( void )
{
    vStartQueueSetTasks();
}

/* The board half of IntQueue. See oracle/harness/IntQueueTimer.h for what
 * this harness does instead of two nested timer interrupts, and for what
 * that does and does not buy. */
void vInitialiseTimerForIntQueueTest( void )
{
}

static void prvStartIntQueue( void )
{
    vStartInterruptQueueTasks();
}

static void prvStartIntSemTest( void )
{
    vStartInterruptSemaphoreTasks();
}

static void prvStartStreamBuffer( void )
{
    vStartStreamBufferTasks();
}

/* Unlike its stream-buffer twin this one takes the stack size, and
 * Posix_GCC/main_full.c passes configMINIMAL_STACK_SIZE. The value is not
 * cosmetic: the demo derives xBlockingStackSize from it. */
static void prvStartMessageBuffer( void )
{
    vStartMessageBufferTasks( configMINIMAL_STACK_SIZE );
}

static void prvStartStreamBufferInterrupt( void )
{
    vStartStreamBufferInterruptDemo();
}

static void prvStartEventGroups( void )
{
    vStartEventGroupTasks();
}

#if defined( KAIROS_AMP )
    static void prvStartMessageBufferAMP( void )
    {
        vStartMessageBufferAMPTasks( configMINIMAL_STACK_SIZE );
    }
#endif

static void prvStartTimerDemo( void )
{
    vStartTimerDemoTask( harnessTIMER_BASE_PERIOD );
}

static BaseType_t prvTimerDemoStillRunning( void )
{
    return xAreTimerDemoTasksStillRunning( harnessCHECK_PERIOD_TICKS );
}

static const Scenario_t xScenarios[] =
{
    { "dynamic",  prvStartDynamic,  xAreDynamicPriorityTasksStillRunning   },
    { "PollQ",    prvStartPollQ,    xArePollingQueuesStillRunning          },
    { "BlockQ",   prvStartBlockQ,   xAreBlockingQueuesStillRunning         },
    { "semtest",  prvStartSemTest,  xAreSemaphoreTasksStillRunning         },
    { "countsem", prvStartCountSem, xAreCountingSemaphoreTasksStillRunning },
    { "recmutex", prvStartRecMutex, xAreRecursiveMutexTasksStillRunning    },
    { "blocktim", prvStartBlockTim, xAreBlockTimeTestTasksStillRunning     },
    { "QPeek",    prvStartQPeek,    xAreQueuePeekTasksStillRunning         },
    { "GenQTest", prvStartGenQTest, xAreGenericQueueTasksStillRunning      },
    { "TaskNotify", prvStartTaskNotify, xAreTaskNotificationTasksStillRunning },
    { "AbortDelay", prvStartAbortDelay, xAreAbortDelayTestTasksStillRunning },
    { "QueueOverwrite", prvStartQueueOverwrite, xIsQueueOverwriteTaskStillRunning },
    { "QueueSetPolling", prvStartQueueSetPolling, xAreQueueSetPollTasksStillRunning },
    { "QueueSet", prvStartQueueSet, xAreQueueSetTasksStillRunning },
    { "IntQueue", prvStartIntQueue, xAreIntQueueTasksStillRunning },
    { "IntSemTest", prvStartIntSemTest, xAreInterruptSemaphoreTasksStillRunning },
    { "StreamBufferDemo", prvStartStreamBuffer, xAreStreamBufferTasksStillRunning },
    { "MessageBufferDemo", prvStartMessageBuffer, xAreMessageBufferTasksStillRunning },
    { "StreamBufferInterrupt", prvStartStreamBufferInterrupt, xIsInterruptStreamBufferDemoStillRunning },
    { "TimerDemo", prvStartTimerDemo, prvTimerDemoStillRunning },
    { "EventGroupsDemo", prvStartEventGroups, xAreEventGroupTasksStillRunning },
    { "death",    prvStartDeath,    xIsCreateTaskStillRunning              },
#if defined( KAIROS_AMP )
    /* Only in the AMP binary: the `sbSEND_COMPLETED` override above is
     * global, so this scenario and the other fifteen cannot share one. */
    { "MessageBufferAMP", prvStartMessageBufferAMP, xAreMessageBufferAMPTasksStillRunning },
#endif
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

    kairos_trace_debug_init();
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

/* The interrupt half of whichever scenario is running.
 *
 * Upstream's own Posix demo runs every scenario at once and calls all of
 * their periodic ISR functions from here (vFullDemoTickHookFunction in
 * Demo/Posix_GCC/main_full.c). This harness runs one scenario per trace, so
 * it dispatches to that one and no other: a trace has to be attributable to
 * the scenario named on the command line. rusty_rtos_demo's TickIsr does the
 * same, dispatching on the same name. */
void vApplicationTickHook( void )
{
    if( pxScenario == NULL )
    {
        return;
    }

    if( strcmp( pxScenario->pcName, "QueueOverwrite" ) == 0 )
    {
        vQueueOverwritePeriodicISRDemo();
    }
    else if( strcmp( pxScenario->pcName, "QueueSetPolling" ) == 0 )
    {
        vQueueSetPollingInterruptAccess();
    }
    else if( strcmp( pxScenario->pcName, "QueueSet" ) == 0 )
    {
        vQueueSetAccessQueueSetFromISR();
    }
    else if( strcmp( pxScenario->pcName, "IntQueue" ) == 0 )
    {
        /* On a board these are two interrupts of different priority, so
         * the second can nest inside the first. Here they are one tick,
         * first then second, and the ORDER is the contract: the Rust side
         * calls them the same way round. */
        ( void ) xFirstTimerHandler();
        ( void ) xSecondTimerHandler();
    }
    else if( strcmp( pxScenario->pcName, "IntSemTest" ) == 0 )
    {
        vInterruptSemaphorePeriodicTest();
    }
    else if( strcmp( pxScenario->pcName, "StreamBufferDemo" ) == 0 )
    {
        vPeriodicStreamBufferProcessing();
    }
    else if( strcmp( pxScenario->pcName, "StreamBufferInterrupt" ) == 0 )
    {
        vBasicStreamBufferSendFromISR();
    }
    else if( strcmp( pxScenario->pcName, "TaskNotify" ) == 0 )
    {
        xNotifyTaskFromISR();
    }
    else if( strcmp( pxScenario->pcName, "TimerDemo" ) == 0 )
    {
        vTimerPeriodicISRTests();
    }
    else if( strcmp( pxScenario->pcName, "EventGroupsDemo" ) == 0 )
    {
        vPeriodicEventGroupsProcessing();
    }
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
