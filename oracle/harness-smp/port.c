/* The two-core sim port. See portmacro.h for the contract. */
#include <pthread.h>
#include <semaphore.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "FreeRTOS.h"
#include "task.h"

volatile int smp_core = 0;
volatile int smp_in_isr = 0;
UBaseType_t uxCriticalNestings[ 2 ] = { 0, 0 };

/* Reported in KAIROS_RESULT, as the one-core patch's counters are. */
unsigned long ulKairosYields = 0;
unsigned long ulKairosTicks = 0;
unsigned long ulKairosExits = 0;
unsigned long ulKairosTurns = 0;

/* How many turns per tick. Two: one per core, so a tick is a round. */
#define SMP_TURNS_PER_TICK    2

typedef struct Thread
{
    pthread_t xThread;
    sem_t xGo;
    TaskFunction_t pxCode;
    void * pvParams;
    /* API nesting depth, task context only. */
    int iDepth;
    /* ulKairosExits when the current top-level call began (or when this
     * thread was resumed inside one). */
    unsigned long ulApiBase;
    /* Resumed inside a call it was PREEMPTED in: that call's return is
     * not a turn boundary (the Rust side finished it in the earlier step). */
    int iTail;
    /* The top-level call in progress, for KAIROS_SMP_DEBUG. */
    const char * pcApi;
    /* The call in progress has blocked (a BLOCKING_* / *_BLOCK event). */
    int iBlocking;
} Thread_t;

static __thread Thread_t * pxSelf = NULL;
static sem_t xDriver;
static int xStarted = 0;
static int xInPort = 0;
static int xIrqOff[ 2 ] = { 0, 0 };
static int xSwitchPending[ 2 ] = { 0, 0 };

static Thread_t * prvThreadOf( TaskHandle_t xTask )
{
    /* tasks.c stores pxPortInitialiseStack's answer as the TCB's first
     * member, and that answer is the Thread_t. */
    return *( Thread_t ** ) xTask;
}

/* Give the token back to the driver and wait for it to come round again. */
static void prvEndTurn( const char * pcWhy )
{
    Thread_t * pxMe = pxSelf;

    if( getenv( "KAIROS_SMP_DEBUG" ) != NULL )
    {
        fprintf( stderr, "#   end %s depth=%d%s%s\n", pcWhy, pxMe->iDepth,
                 ( pxMe->pcApi != NULL && pcWhy[ 0 ] == 'c' ) ? " " : "",
                 ( pxMe->pcApi != NULL && pcWhy[ 0 ] == 'c' ) ? pxMe->pcApi : "" );
    }

    sem_post( &xDriver );
    sem_wait( &pxMe->xGo );
    if( getenv( "KAIROS_SMP_DEBUG" ) != NULL )
    {
        fprintf( stderr, "#   resume (parked at %s)\n", pcWhy );
    }
    /* A new turn. */
    pxMe->ulApiBase = ulKairosExits;
}

static void * prvThreadMain( void * pv )
{
    Thread_t * pxMe = ( Thread_t * ) pv;

    pxSelf = pxMe;
    sem_wait( &pxMe->xGo );
    pxMe->ulApiBase = ulKairosExits;
    pxMe->pxCode( pxMe->pvParams );
    fprintf( stderr, "a task returned\n" );
    exit( 3 );
}

StackType_t * pxPortInitialiseStack( StackType_t * pxTopOfStack,
                                     TaskFunction_t pxCode,
                                     void * pvParameters )
{
    /* At the top of the task's own stack, as the Posix port does: tasks.c
     * asserts that the answer lies inside the stack it allocated. */
    Thread_t * pxThread = ( Thread_t * ) ( pxTopOfStack + 1 ) - 1;
    pthread_attr_t xAttr;

    memset( pxThread, 0, sizeof( Thread_t ) );
    pxThread->pxCode = pxCode;
    pxThread->pvParams = pvParameters;
    sem_init( &pxThread->xGo, 0, 0 );
    pthread_attr_init( &xAttr );
    pthread_attr_setstacksize( &xAttr, 256 * 1024 );
    /* Inside a critical section, as the Posix port does: the Rust corpus
     * config says so (`PORT_STACK_INIT_CRITICAL`), so a task created after
     * the scheduler starts pays that exit on both sides. */
    portENTER_CRITICAL();
    if( pthread_create( &pxThread->xThread, &xAttr, prvThreadMain, pxThread ) != 0 )
    {
        fprintf( stderr, "pthread_create failed\n" );
        exit( 3 );
    }
    portEXIT_CRITICAL();
    return ( StackType_t * ) pxThread;
}

/* Switch THIS core now: a task's own yield, taken with interrupts enabled.
 * If another task was chosen, this thread parks until it is chosen again. */
static void prvSwitchHere( void )
{
    int c = smp_core;
    TaskHandle_t xOld, xNew;

    /* Every kernel call the port makes is inside `xInPort`: those calls
     * carry trace hooks too, and a hook reached from here would read as the
     * TASK making a call -- and end its turn before the switch. */
    xInPort = 1;
    xOld = xTaskGetCurrentTaskHandleForCore( c );
    xSwitchPending[ c ] = 0;
    vTaskSwitchContext( c );
    xNew = xTaskGetCurrentTaskHandleForCore( c );
    xInPort = 0;
    if( xNew != xOld )
    {
        Thread_t * pxMe = pxSelf;

        if( pxMe->iDepth > 0 )
        {
            /* Blocked: the call continues when the task runs again, and
             * the Rust side makes the same call again -- a fresh boundary.
             * Preempted: the Rust side already finished the call. */
            pxMe->iTail = pxMe->iBlocking ? 0 : 1;
        }
        pxMe->iBlocking = 0;
        prvEndTurn( "switch" );
    }
}

static int prvCanSwitchHere( void )
{
    int c = smp_core;

    return pxSelf != NULL && !smp_in_isr && !xInPort && xStarted &&
           uxCriticalNestings[ c ] == 0 && !xIrqOff[ c ];
}

void vPortYield( void )
{
    int iParked = 0;

    ulKairosYields++;
    xSwitchPending[ smp_core ] = 1;

    /* The Posix port's `vPortYield` is a critical section, and so is the Rust
     * kernel's `port_yield`: an outermost one leaves a critical section like
     * any call. (Found by GenQTest's `vTaskDelay( 0 )`, which is nothing but
     * this yield: without the exit the two sides disagreed about whether it
     * ended the turn.) */
    if( pxSelf != NULL && !smp_in_isr && !xInPort && xStarted && uxCriticalNestings[ smp_core ] == 0 )
    {
        ulKairosExits++;
    }
    if( prvCanSwitchHere() )
    {
        unsigned long ulTurnsBefore = ulKairosTurns;

        prvSwitchHere();
        iParked = ( ulKairosTurns != ulTurnsBefore );
    }
    /* A `taskYIELD()` the task makes itself, outside any kernel call, is a
     * call that left a critical section: under the eager rule it ends the
     * turn even when the same task is chosen again. */
    if( !iParked && pxSelf != NULL && pxSelf->iDepth == 0 && !smp_in_isr && !xInPort && xStarted &&
        uxCriticalNestings[ smp_core ] == 0 && !xIrqOff[ smp_core ] )
    {
        prvEndTurn( "yield" );
    }
    else if( getenv( "KAIROS_SMP_DEBUG" ) != NULL )
    {
        fprintf( stderr, "#   yield deferred: self=%d isr=%d port=%d nest=%lu irq=%d\n", pxSelf != NULL, smp_in_isr, xInPort,
                 ( unsigned long ) uxCriticalNestings[ smp_core ], xIrqOff[ smp_core ] );
    }
}

void vPortYieldFromISR( void )
{
    xSwitchPending[ smp_core ] = 1;
}

void vPortYieldCore( int xCoreID )
{
    xSwitchPending[ xCoreID ] = 1;
}

#define IRQLOG( what )    do { if( getenv( "KAIROS_SMP_IRQLOG" ) != NULL ) { fprintf( stderr, "#     irq %s core=%d -> %d from %p\n", what, smp_core, xIrqOff[ smp_core ], __builtin_return_address( 0 ) ); } } while( 0 )

void vPortDisableInterrupts( void )
{
    xIrqOff[ smp_core ] = 1;
    IRQLOG( "disable" );
}

void vPortEnableInterrupts( void )
{
    xIrqOff[ smp_core ] = 0;
    IRQLOG( "enable" );
    if( xSwitchPending[ smp_core ] && prvCanSwitchHere() )
    {
        prvSwitchHere();
    }
}

UBaseType_t uxPortSetInterruptMask( void )
{
    UBaseType_t x = ( UBaseType_t ) xIrqOff[ smp_core ];

    xIrqOff[ smp_core ] = 1;
    IRQLOG( "setmask" );
    return x;
}

void vPortClearInterruptMask( UBaseType_t x )
{
    if( x == 0 )
    {
        vPortEnableInterrupts();
    }
}

void vPortCriticalExited( BaseType_t xCoreID )
{
    if( uxCriticalNestings[ xCoreID ] == 0 && pxSelf != NULL && !smp_in_isr && xStarted )
    {
        ulKairosExits++;
    }
}

void vPortApiEnter( const char * pcName )
{
    Thread_t * pxMe = pxSelf;

    if( pxMe == NULL || smp_in_isr || xInPort || !xStarted )
    {
        return;
    }
    if( pxMe->iDepth == 0 )
    {
        pxMe->ulApiBase = ulKairosExits;
        pxMe->iBlocking = 0;
        pxMe->pcApi = pcName;
    }
    pxMe->iDepth++;
}

/* The calls contract v2 charges when they take no critical section: see
 * vPortApiReturn. */
static int prvIsBlindCall( const char * pcApi )
{
    return ( pcApi != NULL ) &&
           ( ( strcmp( pcApi, "xStreamBufferSend" ) == 0 ) ||
             ( strcmp( pcApi, "xStreamBufferReceive" ) == 0 ) );
}

void vPortApiReturn( void )
{
    Thread_t * pxMe = pxSelf;

    if( pxMe == NULL || smp_in_isr || xInPort || !xStarted )
    {
        return;
    }
    if( pxMe->iDepth > 0 )
    {
        pxMe->iDepth--;
    }
    if( pxMe->iDepth == 0 )
    {
        if( pxMe->iTail )
        {
            pxMe->iTail = 0;
        }
        else if( ( ulKairosExits == pxMe->ulApiBase ) && prvIsBlindCall( pxMe->pcApi ) )
        {
            /* Contract v2's blind call, as the one-core port's
             * vPortKairosApiReturn: a stream-buffer send or receive that
             * returned without a critical section costs one empty one -- a
             * non-blocking reader polling an empty buffer would otherwise
             * hold the token for ever. Port-internal, so the API hooks of
             * the section itself do not count; its exit does, and ends the
             * turn as any call's would. */
            xInPort = 1;
            taskENTER_CRITICAL();
            taskEXIT_CRITICAL();
            xInPort = 0;
            prvEndTurn( "call" );
        }
        else if( ulKairosExits > pxMe->ulApiBase )
        {
            /* EAGER: the turn ends as the call returns. The Rust side cannot
             * stop before a call it has not yet seen, so the statements that
             * follow a call wait for this task's next turn on both sides. */
            prvEndTurn( "call" );
        }
    }
}

void vPortBlocking( void )
{
    if( pxSelf != NULL )
    {
        pxSelf->iBlocking = 1;
    }
}

static int iTaskLockOwner = -1;
static int iTaskLockCount = 0;

void vPortGetTaskLock( BaseType_t xCoreID )
{
    if( iTaskLockOwner != -1 && iTaskLockOwner != ( int ) xCoreID )
    {
        /* The turn rule skips a core while the other holds the lock, so a
         * core can only meet it held if the rule itself is broken. */
        fprintf( stderr, "KAIROS_RESULT ? fail task-lock-contended core=%ld owner=%d\n", ( long ) xCoreID, iTaskLockOwner );
        exit( 5 );
    }
    iTaskLockOwner = ( int ) xCoreID;
    iTaskLockCount++;
}

void vPortReleaseTaskLock( BaseType_t xCoreID )
{
    ( void ) xCoreID;
    if( iTaskLockCount > 0 && --iTaskLockCount == 0 )
    {
        iTaskLockOwner = -1;
    }
}

/* One pass of an idle task: the end of its turn. */
#if defined( portREMOVE_STATIC_QUALIFIER )
    extern List_t pxReadyTasksLists[ configMAX_PRIORITIES ];
#endif
static void prvIdlePass( void )
{
#if defined( portREMOVE_STATIC_QUALIFIER )
    if( getenv( "KAIROS_SMP_DEBUG" ) != NULL )
    {
        fprintf( stderr, "#   idle hook: ready[0]=%lu" "%c", ( unsigned long ) listCURRENT_LIST_LENGTH( &pxReadyTasksLists[ 0 ] ), 10 );
    }
#endif
    if( pxSelf != NULL && xStarted )
    {
        prvEndTurn( "idle" );
    }
}

void vApplicationIdleHook( void )
{
    prvIdlePass();
}

void vApplicationPassiveIdleHook( void )
{
    prvIdlePass();
}

static void prvTick( void )
{
    UBaseType_t uxSaved;

    smp_core = 0;
    smp_in_isr = 1;
    uxSaved = taskENTER_CRITICAL_FROM_ISR();
    ulKairosTicks++;
    if( xTaskIncrementTick() != pdFALSE )
    {
        xSwitchPending[ 0 ] = 1;
    }
    taskEXIT_CRITICAL_FROM_ISR( uxSaved );
    smp_in_isr = 0;
}

BaseType_t xPortStartScheduler( void )
{
    unsigned long ulTurn;

    sem_init( &xDriver, 0, 0 );
    /* `vTaskStartScheduler` disables interrupts so no tick lands before the
     * first task starts; starting it is where a port enables them. */
    xIrqOff[ 0 ] = 0;
    xIrqOff[ 1 ] = 0;
    xStarted = 1;
    for( ulTurn = 0; ; ulTurn++ )
    {
        int c = ( int ) ( ulTurn & 1UL );
        Thread_t * pxRun;

        ulKairosTurns++;
        smp_core = c;
        if( getenv( "KAIROS_SMP_DEBUG" ) != NULL )
        {
            fprintf( stderr, "# turn %lu core %d cur0=%s cur1=%s pend=%d%d lock=%d y=%lu\n", ulTurn, c,
                     pcTaskGetName( xTaskGetCurrentTaskHandleForCore( 0 ) ),
                     pcTaskGetName( xTaskGetCurrentTaskHandleForCore( 1 ) ),
                     xSwitchPending[ 0 ], xSwitchPending[ 1 ], iTaskLockOwner, ulKairosYields );
        }
        if( iTaskLockOwner != -1 && iTaskLockOwner != c )
        {
            /* The other core holds the scheduler suspended: this one would
             * spin at its next critical section, so its turn is skipped. */
        }
        else
        {
        if( xSwitchPending[ c ] )
        {
            xSwitchPending[ c ] = 0;
            xInPort = 1;
            vTaskSwitchContext( c );
            xInPort = 0;
        }
        pxRun = prvThreadOf( xTaskGetCurrentTaskHandleForCore( c ) );
        sem_post( &pxRun->xGo );
        sem_wait( &xDriver );
        }
        if( ( ulTurn + 1 ) % SMP_TURNS_PER_TICK == 0 )
        {
            prvTick();
        }
    }
    return pdTRUE;
}

void vPortEndScheduler( void )
{
}

