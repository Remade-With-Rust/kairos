/* The Kairos TWO-CORE sim port: FreeRTOS SMP on one host thread at a time.
 *
 * Each task is a pthread, as on the Posix port, but only the thread holding
 * the token runs. The driver (`xPortStartScheduler`, port.c) hands the token
 * to core 0's current task, then core 1's, and so on: one TURN each. That is
 * the whole of the two-core sim contract (ORACLES.md, "SMP contract"), and
 * rusty_rtos_demo's two-core runner implements the same rule.
 *
 * A turn ends at the first of:
 *   - the return of a top-level kernel call that left at least one critical
 *     section (the statements after it run in the task's next turn);
 *   - a context switch of this core (`portYIELD`, taken as soon as
 *     interrupts are enabled, as PendSV would be);
 *   - one pass of an idle task (its hook).
 * Ticks are delivered on core 0 between turns, never inside a call; a yield
 * for the OTHER core is taken at the start of that core's next turn. */
#ifndef PORTMACRO_H
#define PORTMACRO_H

#include <limits.h>
#include <stdint.h>

#define portCHAR          char
#define portFLOAT         float
#define portDOUBLE        double
#define portLONG          long
#define portSHORT         short
#define portSTACK_TYPE    unsigned long
#define portBASE_TYPE     long

typedef portSTACK_TYPE StackType_t;
typedef long BaseType_t;
typedef unsigned long UBaseType_t;
/* As the Posix port: the Rust corpus config (`PosixDemoConfig`) is 64-bit. */
typedef unsigned long TickType_t;
#define portMAX_DELAY              ( ( TickType_t ) ULONG_MAX )
#define portTICK_TYPE_IS_ATOMIC    1

#define portSTACK_GROWTH          ( -1 )
#define portTICK_PERIOD_MS        ( ( TickType_t ) 1000 / configTICK_RATE_HZ )
#define portBYTE_ALIGNMENT        8
#define portPOINTER_SIZE_TYPE     uintptr_t
#define portDONT_DISCARD          __attribute__( ( used ) )

extern volatile int smp_core;
extern volatile int smp_in_isr;
extern UBaseType_t uxCriticalNestings[ 2 ];

#define portMAX_CORE_COUNT        2
#define portGET_CORE_ID()         ( ( BaseType_t ) smp_core )
void vPortYield( void );
void vPortYieldCore( int xCoreID );
#define portYIELD()               vPortYield()
#define portYIELD_CORE( a )       vPortYieldCore( a )
#define portYIELD_FROM_ISR( x )   do { if( x ) { vPortYieldFromISR(); } } while( 0 )
void vPortYieldFromISR( void );
#define portEND_SWITCHING_ISR( x ) portYIELD_FROM_ISR( x )
#define portCHECK_IF_IN_ISR()     ( smp_in_isr )

void vPortCriticalExited( BaseType_t xCoreID );
#define portCRITICAL_NESTING_IN_TCB    0
#define portGET_CRITICAL_NESTING_COUNT( xCoreID )          ( uxCriticalNestings[ ( xCoreID ) ] )
#define portSET_CRITICAL_NESTING_COUNT( xCoreID, x )       ( uxCriticalNestings[ ( xCoreID ) ] = ( x ) )
#define portINCREMENT_CRITICAL_NESTING_COUNT( xCoreID )    ( uxCriticalNestings[ ( xCoreID ) ]++ )
#define portDECREMENT_CRITICAL_NESTING_COUNT( xCoreID ) \
    do { uxCriticalNestings[ ( xCoreID ) ]--; vPortCriticalExited( xCoreID ); } while( 0 )

UBaseType_t uxPortSetInterruptMask( void );
void vPortClearInterruptMask( UBaseType_t x );
void vPortDisableInterrupts( void );
void vPortEnableInterrupts( void );
#define portSET_INTERRUPT_MASK()                  uxPortSetInterruptMask()
#define portCLEAR_INTERRUPT_MASK( x )             vPortClearInterruptMask( x )
#define portSET_INTERRUPT_MASK_FROM_ISR()         uxPortSetInterruptMask()
#define portCLEAR_INTERRUPT_MASK_FROM_ISR( x )    vPortClearInterruptMask( x )
#define portDISABLE_INTERRUPTS()                  vPortDisableInterrupts()
#define portENABLE_INTERRUPTS()                   vPortEnableInterrupts()

void vTaskEnterCritical( void );
void vTaskExitCritical( void );
UBaseType_t vTaskEnterCriticalFromISR( void );
void vTaskExitCriticalFromISR( UBaseType_t uxSavedInterruptStatus );
#define portENTER_CRITICAL()               vTaskEnterCritical()
#define portEXIT_CRITICAL()                vTaskExitCritical()
#define portENTER_CRITICAL_FROM_ISR()      vTaskEnterCriticalFromISR()
#define portEXIT_CRITICAL_FROM_ISR( x )    vTaskExitCriticalFromISR( x )

/* One thread holds the token, so the ISR lock has nothing to exclude. The
 * TASK lock is modelled, because it is held across turns: `vTaskSuspendAll`
 * takes it and `xTaskResumeAll` gives it back, and on real silicon the other
 * core spins at its next critical section meanwhile. The contract's version
 * of that spin: while one core holds the task lock, the other core's turns
 * are skipped. */
void vPortGetTaskLock( BaseType_t xCoreID );
void vPortReleaseTaskLock( BaseType_t xCoreID );
#define portGET_ISR_LOCK( xCoreID )
#define portRELEASE_ISR_LOCK( xCoreID )
#define portGET_TASK_LOCK( xCoreID )        vPortGetTaskLock( xCoreID )
#define portRELEASE_TASK_LOCK( xCoreID )    vPortReleaseTaskLock( xCoreID )

#define portTASK_FUNCTION_PROTO( vFunction, pvParameters )    void vFunction( void * pvParameters )
#define portTASK_FUNCTION( vFunction, pvParameters )          void vFunction( void * pvParameters )
#define portNOP()
#define portMEMORY_BARRIER()    __sync_synchronize()

/* The API-boundary hooks every traceENTER_* / traceRETURN_* expands to
 * (api_hooks.h, generated), and the block marker. */
void vPortApiEnter( void );
void vPortApiReturn( void );
void vPortBlocking( void );

#endif
