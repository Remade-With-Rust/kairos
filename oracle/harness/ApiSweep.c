/*
 * ApiSweep.c -- KAIROS-authored. NOT an upstream demo.
 *
 * Every other scenario in this corpus is a FreeRTOS demo file compiled
 * verbatim, and that is the point of them: the C we diff against is code
 * nobody here wrote. This one is different and says so at the top.
 *
 * docs/HOLES.md H2 counts the public kernel APIs that no corpus scenario
 * reaches, so nobody has ever asked whether the Rust answers what the C
 * answers. Six of them have a direct C twin and NO upstream demo calls any
 * of them -- checked, not assumed:
 *
 *     pcTaskGetName              xTimerGetPeriod
 *     xQueueSendToFrontFromISR   xTimerGetExpiryTime
 *     xStreamBufferSetTriggerLevel  xTimerPendFunctionCall
 *
 * So there was nothing to port. The oracle does not have to be an upstream
 * DEMO, though -- it has to be the C KERNEL. This file drives those six
 * against real FreeRTOS, the Rust twin drives them against the Rust kernel,
 * and the traces are diffed like any other scenario.
 *
 * WHAT THE DIFFERENTIAL ACTUALLY CHECKS HERE
 * ==========================================
 * Most of these are queries that emit no trace event, so it would be easy to
 * think a trace cannot judge them. It can, twice over:
 *
 *   - Every one of them takes a critical section, and on the sim an
 *     outermost critical-section exit is a sixteenth of a tick. An API that
 *     takes a different NUMBER of sections than the C moves every event
 *     after it. The clock is the assertion.
 *   - The values are checked here, in C, and a wrong one latches
 *     xApiSweepStatus, which xAreApiSweepTasksStillRunning reports -- so a
 *     Rust kernel that answers differently fails the scenario's own check
 *     even where the trace would not move.
 *
 * The run is deliberately dull: one task, a fixed order, a fixed period.
 * Nothing here races, because what is under test is what each call ANSWERS
 * and what it COSTS, not how they interleave.
 */

#include <string.h>

#include "FreeRTOS.h"
#include "task.h"
#include "queue.h"
#include "timers.h"
#include "stream_buffer.h"

#include "ApiSweep.h"

/* The sweep task's own name, which pcTaskGetName must answer with. */
#define apiswpTASK_NAME             "ApiSweep"
#define apiswpPRIORITY              ( tskIDLE_PRIORITY + 1 )

/* The timer whose period and expiry time are read. A round number so the
 * expiry arithmetic below is exact rather than approximate. */
#define apiswpTIMER_PERIOD          ( ( TickType_t ) 50 )

/* The queue the interrupt writes to the FRONT of. Length 4 so the front/back
 * ordering is visible. */
#define apiswpQUEUE_LENGTH          ( ( UBaseType_t ) 4 )

/* The stream buffer whose trigger level is moved. Its size matters: a
 * trigger level ABOVE it must be refused, which is half of what
 * xStreamBufferSetTriggerLevel promises. */
#define apiswpBUFFER_BYTES          ( ( size_t ) 32 )
#define apiswpTRIGGER_OK            ( ( size_t ) 4 )
#define apiswpTRIGGER_TOO_BIG       ( apiswpBUFFER_BYTES + 1 )

/* One sweep per this many ticks. */
#define apiswpSWEEP_DELAY           ( ( TickType_t ) 20 )
/* The interrupt writes on every this-many'th tick. */
#define apiswpISR_PERIOD            ( 7UL )
/* The block time the pended call is posted with. */
#define apiswpPEND_BLOCK            ( ( TickType_t ) 0 )

/*-----------------------------------------------------------*/

static QueueHandle_t xSweepQueue = NULL;
static TimerHandle_t xSweepTimer = NULL;
static StreamBufferHandle_t xSweepBuffer = NULL;
static TaskHandle_t xSweepTask = NULL;

/* Incremented once per completed sweep; the check function wants it moving. */
static volatile uint32_t ulSweepCycles = 0UL;

/* Latched by any answer that was not the expected one. */
static volatile BaseType_t xApiSweepStatus = pdPASS;

/* Incremented by the function the timer daemon runs on our behalf, so
 * xTimerPendFunctionCall is proven to have actually DELIVERED rather than
 * merely returned pdPASS. */
static volatile uint32_t ulPendedCalls = 0UL;

/* The value the pended call was handed, so the ARGUMENTS are checked too. */
static volatile uint32_t ulLastPendedParameter = 0UL;

/* Set once the objects exist, so the interrupt does not touch a NULL. */
static volatile BaseType_t xSweepReady = pdFALSE;

static void prvSweepTask( void * pvParameters );
static void prvPendedFunction( void * pvParameter1,
                               uint32_t ulParameter2 );
static void prvSweepTimerCallback( TimerHandle_t xTimer );
static void prvFail( void );

/*-----------------------------------------------------------*/

static void prvFail( void )
{
    xApiSweepStatus = pdFAIL;
}
/*-----------------------------------------------------------*/

static void prvPendedFunction( void * pvParameter1,
                               uint32_t ulParameter2 )
{
    ( void ) pvParameter1;
    ulLastPendedParameter = ulParameter2;
    ulPendedCalls++;
}
/*-----------------------------------------------------------*/

static void prvSweepTimerCallback( TimerHandle_t xTimer )
{
    /* Nothing to do. The timer exists to be QUERIED -- its period and its
     * expiry time -- not to fire usefully. It is auto-reload so that
     * xTimerGetExpiryTime always has a next expiry to answer with. */
    ( void ) xTimer;
}
/*-----------------------------------------------------------*/

void vStartApiSweepTasks( void )
{
    xSweepQueue = xQueueCreate( apiswpQUEUE_LENGTH, ( UBaseType_t ) sizeof( uint32_t ) );
    configASSERT( xSweepQueue );

    xSweepBuffer = xStreamBufferCreate( apiswpBUFFER_BYTES, ( size_t ) 1 );
    configASSERT( xSweepBuffer );

    xSweepTimer = xTimerCreate( "SwpTmr",
                                apiswpTIMER_PERIOD,
                                pdTRUE,
                                NULL,
                                prvSweepTimerCallback );
    configASSERT( xSweepTimer );

    xTaskCreate( prvSweepTask,
                 apiswpTASK_NAME,
                 configMINIMAL_STACK_SIZE,
                 NULL,
                 apiswpPRIORITY,
                 &xSweepTask );
}
/*-----------------------------------------------------------*/

static void prvSweepTask( void * pvParameters )
{
    const char * pcName;
    TickType_t xPeriod, xExpiry, xNow;
    uint32_t ulReceived, ulExpectedPended = 0UL;

    ( void ) pvParameters;

    /* The timer has to be running before its expiry time means anything:
     * xTimerGetExpiryTime on a dormant timer answers with whatever is left
     * in the field, which is not a promise either kernel makes. */
    if( xTimerStart( xSweepTimer, portMAX_DELAY ) != pdPASS )
    {
        prvFail();
    }

    /* Only now may the interrupt touch the queue. */
    xSweepReady = pdTRUE;

    for( ; ; )
    {
        /* --- pcTaskGetName, both spellings --------------------------- */

        /* NULL means "the calling task", which is this one. */
        pcName = pcTaskGetName( NULL );

        if( ( pcName == NULL ) || ( strcmp( pcName, apiswpTASK_NAME ) != 0 ) )
        {
            prvFail();
        }

        /* And by handle, which must answer the same. */
        pcName = pcTaskGetName( xSweepTask );

        if( ( pcName == NULL ) || ( strcmp( pcName, apiswpTASK_NAME ) != 0 ) )
        {
            prvFail();
        }

        /* --- xTimerGetPeriod ----------------------------------------- */

        xPeriod = xTimerGetPeriod( xSweepTimer );

        if( xPeriod != apiswpTIMER_PERIOD )
        {
            prvFail();
        }

        /* --- xTimerGetExpiryTime ------------------------------------- */

        /* Read the tick FIRST: between the two calls the timer may expire
         * and reload, and then the expiry is a period further out. Taking
         * the tick first makes the window one-sided, so the bound below is
         * a real bound and not a race. */
        xNow = xTaskGetTickCount();
        xExpiry = xTimerGetExpiryTime( xSweepTimer );

        /* An auto-reload timer's next expiry is always within one period of
         * now. Unsigned subtraction wraps the same way on both sides, which
         * is why this is written as a difference and not a comparison. */
        if( ( TickType_t ) ( xExpiry - xNow ) > apiswpTIMER_PERIOD )
        {
            prvFail();
        }

        /* --- xStreamBufferSetTriggerLevel ---------------------------- */

        /* A level the buffer can hold is accepted... */
        if( xStreamBufferSetTriggerLevel( xSweepBuffer, apiswpTRIGGER_OK ) != pdPASS )
        {
            prvFail();
        }

        /* ...and one larger than the buffer is refused. That refusal is
         * half of what the call promises, and it is the half a port is
         * likely to get wrong. */
        if( xStreamBufferSetTriggerLevel( xSweepBuffer, apiswpTRIGGER_TOO_BIG ) != pdFAIL )
        {
            prvFail();
        }

        /* --- xTimerPendFunctionCall ---------------------------------- */

        ulExpectedPended = ulSweepCycles;

        if( xTimerPendFunctionCall( prvPendedFunction,
                                    NULL,
                                    ulExpectedPended,
                                    apiswpPEND_BLOCK ) != pdPASS )
        {
            prvFail();
        }

        /* --- xQueueSendToFrontFromISR, read back --------------------- */

        /* Whatever the interrupt has put there. Not blocking: the point is
         * the SEND, which happened in the interrupt, and a blocking read
         * here would make the scenario's timing depend on it. */
        while( xQueueReceive( xSweepQueue, &ulReceived, ( TickType_t ) 0 ) == pdPASS )
        {
            /* The interrupt sends its own call count, which only ever
             * increases, so a zero means the queue handed back something
             * that was never sent. */
            if( ulReceived == 0UL )
            {
                prvFail();
            }
        }

        ulSweepCycles++;

        vTaskDelay( apiswpSWEEP_DELAY );

        /* The pended call must have been delivered by now: the daemon runs
         * at the top priority and this task has just slept. Checked AFTER
         * the delay so the daemon has had its chance. */
        if( ulPendedCalls == 0UL )
        {
            prvFail();
        }

        if( ulLastPendedParameter != ulExpectedPended )
        {
            prvFail();
        }
    }
}
/*-----------------------------------------------------------*/

void vApiSweepAccessFromISR( void )
{
    static uint32_t ulCallCount = 0UL;
    uint32_t ulValue;
    BaseType_t xHigherPriorityTaskWoken = pdFALSE;

    if( xSweepReady != pdTRUE )
    {
        return;
    }

    ulCallCount++;

    if( ( ulCallCount % apiswpISR_PERIOD ) == 0UL )
    {
        /* Never zero: the task checks for it. */
        ulValue = ulCallCount;

        /* To the FRONT. The queue is read without regard to order here --
         * what the differential judges is the event the send emits and the
         * sections it costs, which is where a front/back mix-up shows. */
        ( void ) xQueueSendToFrontFromISR( xSweepQueue,
                                           ( void * ) &ulValue,
                                           &xHigherPriorityTaskWoken );
    }
}
/*-----------------------------------------------------------*/

BaseType_t xAreApiSweepTasksStillRunning( void )
{
    static uint32_t ulLastSweepCycles = 0UL;
    BaseType_t xReturn = pdPASS;

    if( ulSweepCycles == ulLastSweepCycles )
    {
        /* The sweep task has stalled. */
        xReturn = pdFAIL;
    }

    ulLastSweepCycles = ulSweepCycles;

    if( xApiSweepStatus != pdPASS )
    {
        xReturn = pdFAIL;
    }

    return xReturn;
}
