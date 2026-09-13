/*
 * The root set for the flash comparison: the kernel operations the Kairos
 * conformance corpus exercises, and therefore the ones proven byte-identical
 * between the two kernels.
 *
 * # Why this set and not the whole API
 *
 * `docs/API-MAP.md` lists 341 FreeRTOS entry points, nearly all still
 * `planned` on our side. Linking the C kernel against all of them and Kairos
 * against the forty it implements would price a feature gap, not a kernel.
 * The corpus set is the one place the two are known to do the SAME WORK --
 * eighteen scenarios, byte-identical traces, four architectures -- so it is
 * the only root set a footprint comparison can honestly use.
 *
 * # How the roots are held
 *
 * Each address is stored into a volatile array. `--gc-sections` keeps a
 * section only if something reachable references it, and a volatile store
 * cannot be elided, so this pins exactly these entry points and lets the
 * linker discard everything else the kernel can do. Calling them instead
 * would drag in whatever the call ARGUMENTS needed and measure this file.
 */

#include "FreeRTOS.h"
#include "task.h"
#include "queue.h"
#include "semphr.h"
#include "timers.h"
#include "event_groups.h"
#include "stream_buffer.h"

/* Volatile so the stores survive -O2 and the roots stay reachable. */
volatile void * kairos_roots[64];

void kairos_root_set( void );

void kairos_root_set( void )
{
    int i = 0;

    /* --- tasks: create, destroy, block, and the priority surface --- */
    kairos_roots[ i++ ] = ( void * ) xTaskCreate;
    kairos_roots[ i++ ] = ( void * ) vTaskDelete;
    kairos_roots[ i++ ] = ( void * ) vTaskDelay;
    kairos_roots[ i++ ] = ( void * ) xTaskDelayUntil;
    kairos_roots[ i++ ] = ( void * ) vTaskSuspend;
    kairos_roots[ i++ ] = ( void * ) vTaskResume;
    kairos_roots[ i++ ] = ( void * ) vTaskPrioritySet;
    kairos_roots[ i++ ] = ( void * ) uxTaskPriorityGet;
    kairos_roots[ i++ ] = ( void * ) eTaskGetState;
    kairos_roots[ i++ ] = ( void * ) xTaskGetHandle;
    kairos_roots[ i++ ] = ( void * ) xTaskAbortDelay;

    /* --- the scheduler itself --- */
    kairos_roots[ i++ ] = ( void * ) vTaskStartScheduler;
    kairos_roots[ i++ ] = ( void * ) vTaskSuspendAll;
    kairos_roots[ i++ ] = ( void * ) xTaskResumeAll;
    kairos_roots[ i++ ] = ( void * ) xTaskIncrementTick;
    kairos_roots[ i++ ] = ( void * ) vTaskSwitchContext;
    kairos_roots[ i++ ] = ( void * ) xTaskGetTickCount;

    /* --- queues, and the semaphores that are queues underneath --- */
    kairos_roots[ i++ ] = ( void * ) xQueueGenericCreate;
    kairos_roots[ i++ ] = ( void * ) xQueueGenericSend;
    kairos_roots[ i++ ] = ( void * ) xQueueReceive;
    kairos_roots[ i++ ] = ( void * ) xQueuePeek;
    kairos_roots[ i++ ] = ( void * ) uxQueueMessagesWaiting;
    kairos_roots[ i++ ] = ( void * ) xQueueSemaphoreTake;
    kairos_roots[ i++ ] = ( void * ) xQueueCreateMutex;
    kairos_roots[ i++ ] = ( void * ) xQueueCreateCountingSemaphore;
    kairos_roots[ i++ ] = ( void * ) xQueueGiveMutexRecursive;
    kairos_roots[ i++ ] = ( void * ) xQueueTakeMutexRecursive;
    kairos_roots[ i++ ] = ( void * ) xQueueGenericSendFromISR;
    kairos_roots[ i++ ] = ( void * ) xQueueReceiveFromISR;

    /* --- task notifications --- */
    kairos_roots[ i++ ] = ( void * ) xTaskGenericNotify;
    kairos_roots[ i++ ] = ( void * ) xTaskGenericNotifyWait;
    kairos_roots[ i++ ] = ( void * ) ulTaskGenericNotifyTake;
    kairos_roots[ i++ ] = ( void * ) xTaskGenericNotifyStateClear;
    kairos_roots[ i++ ] = ( void * ) xTaskGenericNotifyFromISR;

    /* --- software timers --- */
    kairos_roots[ i++ ] = ( void * ) xTimerCreate;
    kairos_roots[ i++ ] = ( void * ) xTimerGenericCommandFromTask;
    kairos_roots[ i++ ] = ( void * ) xTimerIsTimerActive;

    /* --- event groups --- */
    kairos_roots[ i++ ] = ( void * ) xEventGroupCreate;
    kairos_roots[ i++ ] = ( void * ) xEventGroupSetBits;
    kairos_roots[ i++ ] = ( void * ) xEventGroupWaitBits;
    kairos_roots[ i++ ] = ( void * ) xEventGroupClearBits;
    kairos_roots[ i++ ] = ( void * ) xEventGroupSync;

    /* --- stream buffers --- */
    kairos_roots[ i++ ] = ( void * ) xStreamBufferGenericCreate;
    kairos_roots[ i++ ] = ( void * ) xStreamBufferSend;
    kairos_roots[ i++ ] = ( void * ) xStreamBufferReceive;
    kairos_roots[ i++ ] = ( void * ) xStreamBufferSendFromISR;

    ( void ) i;
}
