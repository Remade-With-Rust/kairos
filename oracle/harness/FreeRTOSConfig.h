/*
 * Kairos oracle harness — FreeRTOSConfig.h for the C kernel on its Posix port.
 *
 * The values are the FreeRTOS/Demo/Posix_GCC demo's (the configuration the
 * standard demo tasks were written against), and the trace macros at the
 * bottom are the Kairos trace contract (umbrella ORACLES.md, "trace line
 * format"): every line this kernel prints, rusty_rtos_kernel must print too.
 *
 * Copy nothing from here into a firmware: this is the oracle's configuration.
 */

#ifndef FREERTOS_CONFIG_H
#define FREERTOS_CONFIG_H

#include "kairos_trace.h"

#define configUSE_PREEMPTION                       1
#define configUSE_TIME_SLICING                     1
#define configUSE_PORT_OPTIMISED_TASK_SELECTION    0
#define configUSE_IDLE_HOOK                        1
/* The interrupt half of the standard demos runs here: upstream's own Posix
 * demo sets this too, and several scenarios (QueueOverwrite, QueueSet,
 * IntSemTest, the stream buffers, the timers) are only exercised from an
 * interrupt at all. A scenario with no interrupt half leaves the hook
 * empty, and an empty hook takes no critical section, so it costs no sim
 * time and moves no trace. */
#define configUSE_TICK_HOOK                        1
#define configUSE_DAEMON_TASK_STARTUP_HOOK         0
#define configTICK_RATE_HZ                         ( 1000 )
#define configMINIMAL_STACK_SIZE                   ( 256 )  /* The Posix port ignores it; pthreads size their own stacks. */
#define configTOTAL_HEAP_SIZE                      ( ( size_t ) ( 65 * 1024 ) )
#define configMAX_TASK_NAME_LEN                    ( 12 )
#define configUSE_TRACE_FACILITY                   0
#define configTICK_TYPE_WIDTH_IN_BITS              TICK_TYPE_WIDTH_32_BITS
#define configIDLE_SHOULD_YIELD                    1
#define configUSE_MUTEXES                          1
#define configCHECK_FOR_STACK_OVERFLOW             0
#define configUSE_RECURSIVE_MUTEXES                1
#define configQUEUE_REGISTRY_SIZE                  20
#define configUSE_APPLICATION_TASK_TAG             1
#define configUSE_COUNTING_SEMAPHORES              1
#define configUSE_ALTERNATIVE_API                  0
#define configUSE_QUEUE_SETS                       1
#define configUSE_TASK_NOTIFICATIONS               1
#define configTASK_NOTIFICATION_ARRAY_ENTRIES      3
#define configSUPPORT_STATIC_ALLOCATION            0
#define configSUPPORT_DYNAMIC_ALLOCATION           1
#define configRECORD_STACK_HIGH_ADDRESS            1
#define configUSE_MALLOC_FAILED_HOOK               1
#define configUSE_EVENT_GROUPS                     1
#define configUSE_STREAM_BUFFERS                   1

#define configUSE_TIMERS                           1
#define configTIMER_TASK_PRIORITY                  ( configMAX_PRIORITIES - 1 )
#define configTIMER_QUEUE_LENGTH                   20
#define configTIMER_TASK_STACK_DEPTH               ( configMINIMAL_STACK_SIZE * 2 )
#define configMAX_PRIORITIES                       ( 7 )

#define configGENERATE_RUN_TIME_STATS              0
#define configUSE_CO_ROUTINES                      0
#define configMAX_CO_ROUTINE_PRIORITIES            ( 2 )
#define configUSE_STATS_FORMATTING_FUNCTIONS       0
#define configSTACK_DEPTH_TYPE                     uint32_t

#define INCLUDE_vTaskPrioritySet                   1
#define INCLUDE_uxTaskPriorityGet                  1
#define INCLUDE_vTaskDelete                        1
#define INCLUDE_vTaskCleanUpResources              0
#define INCLUDE_vTaskSuspend                       1
#define INCLUDE_vTaskDelayUntil                    1
#define INCLUDE_vTaskDelay                         1
#define INCLUDE_uxTaskGetStackHighWaterMark        1
#define INCLUDE_uxTaskGetStackHighWaterMark2       1
#define INCLUDE_xTaskGetSchedulerState             1
#define INCLUDE_xTimerGetTimerDaemonTaskHandle     1
#define INCLUDE_xTaskGetIdleTaskHandle             1
#define INCLUDE_xTaskGetHandle                     1
#define INCLUDE_eTaskGetState                      1
#define INCLUDE_xSemaphoreGetMutexHolder           1
#define INCLUDE_xTimerPendFunctionCall             1
#define INCLUDE_xTaskAbortDelay                    1
#define INCLUDE_xTaskGetCurrentTaskHandle          1

extern void vAssertCalled( const char * const pcFileName, unsigned long ulLine );
#define configASSERT( x )    if( ( x ) == 0 ) { vAssertCalled( __FILE__, __LINE__ ); }

/* ---------------------------------------------------------------------------
 * The Kairos SIM contract, version 2: the blind-call tick.
 *
 * `xStreamBufferSend` and `xStreamBufferReceive` are the only kernel calls
 * in this corpus that can return without taking a critical section -- the
 * zero-wait path reads the ring and leaves. Under contract v1 a task that
 * did nothing but that took no time at all and stopped the clock for every
 * other task (docs/HOLES.md, H9).
 *
 * These four hooks are FreeRTOS's own, no-ops by default. The port decides
 * what a blind call costs; see `vPortKairosApiReturn`.
 * ------------------------------------------------------------------------ */

extern void vPortKairosApiEnter( void );
extern void vPortKairosApiReturn( void );

#define traceENTER_xStreamBufferSend( xStreamBuffer, pvTxData, xDataLengthBytes, xTicksToWait )     vPortKairosApiEnter()
#define traceRETURN_xStreamBufferSend( xReturn )                  vPortKairosApiReturn()
#define traceENTER_xStreamBufferReceive( xStreamBuffer, pvRxData, xBufferLengthBytes, xTicksToWait )     vPortKairosApiEnter()
#define traceRETURN_xStreamBufferReceive( xReceivedLength )       vPortKairosApiReturn()

/* ---------------------------------------------------------------------------
 * The Kairos trace contract, version 1. One macro per event in the K1/K2 set;
 * rusty_rtos_core::trace::Event names every one of these. The ~470
 * traceENTER_* / traceRETURN_* pairs stay undefined (no-ops) on purpose.
 * ------------------------------------------------------------------------ */

#define traceSTARTING_SCHEDULER( xIdleTaskHandles )                    kairos_trace_ev( "STARTING_SCHEDULER" )
#define traceTASK_CREATE( pxNewTCB )                                   kairos_trace_task_u( "TASK_CREATE", ( pxNewTCB )->pcTaskName, ( unsigned long ) ( pxNewTCB )->uxPriority )
#define traceTASK_CREATE_FAILED()                                      kairos_trace_ev( "TASK_CREATE_FAILED" )
#define traceTASK_DELETE( pxTaskToDelete )                             kairos_trace_task( "TASK_DELETE", ( pxTaskToDelete )->pcTaskName )
#define traceTASK_SWITCHED_IN()                                        kairos_trace_task( "TASK_SWITCHED_IN", pxCurrentTCB->pcTaskName )
#define traceTASK_SWITCHED_OUT()                                       kairos_trace_task( "TASK_SWITCHED_OUT", pxCurrentTCB->pcTaskName )
#define traceTASK_DELAY()                                              kairos_trace_task_u( "TASK_DELAY", pxCurrentTCB->pcTaskName, ( unsigned long ) xTicksToDelay )
#define traceTASK_DELAY_UNTIL( x )                                     kairos_trace_task_u( "TASK_DELAY_UNTIL", pxCurrentTCB->pcTaskName, ( unsigned long ) ( x ) )
#define traceTASK_SUSPEND( pxTaskToSuspend )                           kairos_trace_task( "TASK_SUSPEND", ( pxTaskToSuspend )->pcTaskName )
#define traceTASK_RESUME( pxTaskToResume )                             kairos_trace_task( "TASK_RESUME", ( pxTaskToResume )->pcTaskName )
#define traceTASK_RESUME_FROM_ISR( pxTaskToResume )                    kairos_trace_task( "TASK_RESUME_FROM_ISR", ( pxTaskToResume )->pcTaskName )
#define traceTASK_PRIORITY_SET( pxTask, uxNewPriority )                kairos_trace_task_u( "TASK_PRIORITY_SET", ( pxTask )->pcTaskName, ( unsigned long ) ( uxNewPriority ) )
#define traceTASK_PRIORITY_INHERIT( pxTCBOfMutexHolder, uxInheritedPriority )    kairos_trace_task_u( "TASK_PRIORITY_INHERIT", ( pxTCBOfMutexHolder )->pcTaskName, ( unsigned long ) ( uxInheritedPriority ) )
#define traceTASK_PRIORITY_DISINHERIT( pxTCBOfMutexHolder, uxOriginalPriority )  kairos_trace_task_u( "TASK_PRIORITY_DISINHERIT", ( pxTCBOfMutexHolder )->pcTaskName, ( unsigned long ) ( uxOriginalPriority ) )
#define traceTASK_INCREMENT_TICK( xTickCount )                         kairos_trace_u( "TASK_INCREMENT_TICK", ( unsigned long ) ( xTickCount ) )
#define traceMOVED_TASK_TO_READY_STATE( pxTCB )                        kairos_trace_task( "MOVED_TASK_TO_READY_STATE", ( pxTCB )->pcTaskName )
#define traceMOVED_TASK_TO_DELAYED_LIST()                              kairos_trace_task( "MOVED_TASK_TO_DELAYED_LIST", pxCurrentTCB->pcTaskName )
#define traceMOVED_TASK_TO_OVERFLOW_DELAYED_LIST()                     kairos_trace_task( "MOVED_TASK_TO_OVERFLOW_DELAYED_LIST", pxCurrentTCB->pcTaskName )
#define traceTASK_NOTIFY( uxIndexToNotify )                            kairos_trace_task_u( "TASK_NOTIFY", pxTCB->pcTaskName, ( unsigned long ) ( uxIndexToNotify ) )
#define traceTASK_NOTIFY_WAIT( uxIndexToWaitOn )                       kairos_trace_task_u( "TASK_NOTIFY_WAIT", pxCurrentTCB->pcTaskName, ( unsigned long ) ( uxIndexToWaitOn ) )
#define traceTASK_NOTIFY_WAIT_BLOCK( uxIndexToWaitOn )                 kairos_trace_task_u( "TASK_NOTIFY_WAIT_BLOCK", pxCurrentTCB->pcTaskName, ( unsigned long ) ( uxIndexToWaitOn ) )
#define traceTASK_NOTIFY_TAKE( uxIndexToWaitOn )                       kairos_trace_task_u( "TASK_NOTIFY_TAKE", pxCurrentTCB->pcTaskName, ( unsigned long ) ( uxIndexToWaitOn ) )
#define traceTASK_NOTIFY_TAKE_BLOCK( uxIndexToWaitOn )                 kairos_trace_task_u( "TASK_NOTIFY_TAKE_BLOCK", pxCurrentTCB->pcTaskName, ( unsigned long ) ( uxIndexToWaitOn ) )
#define traceQUEUE_CREATE( pxNewQueue )                                kairos_trace_obj_create_u( "QUEUE_CREATE", 'q', ( const void * ) ( pxNewQueue ), ( unsigned long ) ( pxNewQueue )->uxLength )
#define traceQUEUE_SEND( pxQueue )                                     kairos_trace_obj( "QUEUE_SEND", 'q', ( const void * ) ( pxQueue ) )
#define traceQUEUE_SEND_FAILED( pxQueue )                              kairos_trace_obj( "QUEUE_SEND_FAILED", 'q', ( const void * ) ( pxQueue ) )
#define traceQUEUE_SEND_FROM_ISR( pxQueue )                            kairos_trace_obj( "QUEUE_SEND_FROM_ISR", 'q', ( const void * ) ( pxQueue ) )
#define traceQUEUE_RECEIVE( pxQueue )                                  kairos_trace_obj( "QUEUE_RECEIVE", 'q', ( const void * ) ( pxQueue ) )
#define traceQUEUE_RECEIVE_FAILED( pxQueue )                           kairos_trace_obj( "QUEUE_RECEIVE_FAILED", 'q', ( const void * ) ( pxQueue ) )
#define traceQUEUE_RECEIVE_FROM_ISR( pxQueue )                         kairos_trace_obj( "QUEUE_RECEIVE_FROM_ISR", 'q', ( const void * ) ( pxQueue ) )
#define traceQUEUE_PEEK( pxQueue )                                     kairos_trace_obj( "QUEUE_PEEK", 'q', ( const void * ) ( pxQueue ) )
#define traceBLOCKING_ON_QUEUE_SEND( pxQueue )                         kairos_trace_obj( "BLOCKING_ON_QUEUE_SEND", 'q', ( const void * ) ( pxQueue ) )
#define traceBLOCKING_ON_QUEUE_RECEIVE( pxQueue )                      kairos_trace_obj( "BLOCKING_ON_QUEUE_RECEIVE", 'q', ( const void * ) ( pxQueue ) )
#define traceBLOCKING_ON_QUEUE_PEEK( pxQueue )                         kairos_trace_obj( "BLOCKING_ON_QUEUE_PEEK", 'q', ( const void * ) ( pxQueue ) )
#define traceTIMER_CREATE( pxNewTimer )                                kairos_trace_timer_create( ( const void * ) ( pxNewTimer ), ( pxNewTimer )->pcTimerName )
/* NOT `( xTimer )->pcTimerName`: this hook fires after the command is
 * queued, so a delete may already have freed the timer. See kairos_trace.c. */
#define traceTIMER_COMMAND_SEND( xTimer, xMessageID, xMessageValueValue, xReturn )    kairos_trace_timer_command( ( const void * ) ( xTimer ), ( long ) ( xMessageID ), ( unsigned long ) ( xMessageValueValue ) )
#define traceTIMER_EXPIRED( pxTimer )                                  kairos_trace_task( "TIMER_EXPIRED", ( pxTimer )->pcTimerName )
#define traceEVENT_GROUP_CREATE( pxEventGroup )                        kairos_trace_obj_create( "EVENT_GROUP_CREATE", 'g', ( const void * ) ( pxEventGroup ) )
#define traceEVENT_GROUP_SET_BITS( xEventGroup, uxBitsToSet )          kairos_trace_obj_u( "EVENT_GROUP_SET_BITS", 'g', ( const void * ) ( xEventGroup ), ( unsigned long ) ( uxBitsToSet ) )
#define traceEVENT_GROUP_WAIT_BITS_BLOCK( xEventGroup, uxBitsToWaitFor )   kairos_trace_obj_u( "EVENT_GROUP_WAIT_BITS_BLOCK", 'g', ( const void * ) ( xEventGroup ), ( unsigned long ) ( uxBitsToWaitFor ) )
#define traceEVENT_GROUP_WAIT_BITS_END( xEventGroup, uxBitsToWaitFor, xTimeoutOccurred )    kairos_trace_obj_uu( "EVENT_GROUP_WAIT_BITS_END", 'g', ( const void * ) ( xEventGroup ), ( unsigned long ) ( uxBitsToWaitFor ), ( unsigned long ) ( xTimeoutOccurred ) )
#define traceSTREAM_BUFFER_CREATE( pxStreamBuffer, xIsMessageBuffer )  kairos_trace_obj_create_u( "STREAM_BUFFER_CREATE", 's', ( const void * ) ( pxStreamBuffer ), ( unsigned long ) ( xIsMessageBuffer ) )
#define traceSTREAM_BUFFER_SEND( xStreamBuffer, xBytesSent )           kairos_trace_obj_u( "STREAM_BUFFER_SEND", 's', ( const void * ) ( xStreamBuffer ), ( unsigned long ) ( xBytesSent ) )
#define traceSTREAM_BUFFER_RECEIVE( xStreamBuffer, xReceivedLength )   kairos_trace_obj_u( "STREAM_BUFFER_RECEIVE", 's', ( const void * ) ( xStreamBuffer ), ( unsigned long ) ( xReceivedLength ) )
#define traceLOW_POWER_IDLE_BEGIN()                                    kairos_trace_ev( "LOW_POWER_IDLE_BEGIN" )
#define traceLOW_POWER_IDLE_END()                                      kairos_trace_ev( "LOW_POWER_IDLE_END" )

/* MessageBufferAMP pretends to be two cores by replacing the notification a
 * completed send would have made with a message on a control buffer and a
 * call to what stands in for the other core's interrupt handler. The
 * replacement is this macro, and it is global: with it defined, every other
 * stream-buffer scenario would behave differently, and `vGenerateCoreBInterrupt`
 * would reach a control buffer that only exists once the AMP demo has
 * started. So the AMP demo gets its own binary, built with -DKAIROS_AMP=1,
 * and this is the only thing that differs between the two. */
#if defined( KAIROS_AMP )
    void vGenerateCoreBInterrupt( void * xUpdatedMessageBuffer );
    #define sbSEND_COMPLETED( pxStreamBuffer )                         vGenerateCoreBInterrupt( ( void * ) ( pxStreamBuffer ) )
#endif

#endif /* FREERTOS_CONFIG_H */
