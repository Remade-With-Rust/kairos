# API map — the FreeRTOS public surface and its Kairos twin

Generated 2026-09-09 from FreeRTOS-Kernel **V11.3.1** (`3a22924e`) by `kairos`'s K0 script over the seven public headers: every FreeRTOS-named function or API macro (`xFoo(`, `vFoo(`, `uxFoo(`, `ulFoo(`, `pcFoo(`, `pvFoo(`, `eFoo(`) the header declares or expands to. The Rust column is the planned twin (mission plan §3.1: the name minus the Hungarian prefix, on the object it belongs to); the status column is the contract CI will check against the facade's exports from K2 on.

Status: `planned` (not yet built) · `done` (exported by the facade and gated) · `never` (with the reason). Nothing is `done` at K0. Rows marked `(K8)` belong to the MPU and SMP packages.

| header | C symbol | Rust twin (planned) | status |
|---|---|---|---|
| task.h | `eTaskConfirmSleepModeStatus` | `TaskConfirmSleepModeStatus` | planned |
| task.h | `eTaskGetState` | `TaskGetState` | planned |
| task.h | `pcTaskGetName` | `TaskGetName` | planned |
| task.h | `pvTaskGetThreadLocalStoragePointer` | `TaskGetThreadLocalStoragePointer` | planned |
| task.h | `pvTaskIncrementMutexHeldCount` | `TaskIncrementMutexHeldCount` | planned |
| task.h | `ulTaskGenericNotifyTake` | `TaskGenericNotifyTake` | planned |
| task.h | `ulTaskGenericNotifyValueClear` | `TaskGenericNotifyValueClear` | planned |
| task.h | `ulTaskGetIdleRunTimeCounter` | `TaskGetIdleRunTimeCounter` | planned |
| task.h | `ulTaskGetIdleRunTimePercent` | `TaskGetIdleRunTimePercent` | planned |
| task.h | `ulTaskGetRunTimeCounter` | `TaskGetRunTimeCounter` | planned |
| task.h | `ulTaskGetRunTimePercent` | `TaskGetRunTimePercent` | planned |
| task.h | `ulTaskNotifyTake` | `TaskNotifyTake` | planned |
| task.h | `ulTaskNotifyTakeIndexed` | `TaskNotifyTakeIndexed` | planned |
| task.h | `ulTaskNotifyValueClear` | `TaskNotifyValueClear` | planned |
| task.h | `ulTaskNotifyValueClearIndexed` | `TaskNotifyValueClearIndexed` | planned |
| task.h | `uxTaskBasePriorityGet` | `TaskBasePriorityGet` | planned |
| task.h | `uxTaskBasePriorityGetFromISR` | `TaskBasePriorityGetFromISR (with an `Isr` token)` | planned |
| task.h | `uxTaskCallForEachTask` | `TaskCallForEachTask` | planned |
| task.h | `uxTaskGetNumberOfTasks` | `TaskGetNumberOfTasks` | planned |
| task.h | `uxTaskGetStackHighWaterMark` | `TaskGetStackHighWaterMark` | planned |
| task.h | `uxTaskGetStackHighWaterMark2` | `TaskGetStackHighWaterMark2` | planned |
| task.h | `uxTaskGetSystemState` | `TaskGetSystemState` | planned |
| task.h | `uxTaskGetTaskNumber` | `TaskGetTaskNumber` | planned |
| task.h | `uxTaskPriorityGet` | `TaskPriorityGet` | planned |
| task.h | `uxTaskPriorityGetFromISR` | `TaskPriorityGetFromISR (with an `Isr` token)` | planned |
| task.h | `uxTaskResetEventItemValue` | `TaskResetEventItemValue` | planned |
| task.h | `vApplicationGetIdleTaskMemory` | `ApplicationGetIdleTaskMemory` | planned |
| task.h | `vApplicationGetPassiveIdleTaskMemory` | `ApplicationGetPassiveIdleTaskMemory` | planned |
| task.h | `vApplicationIdleHook` | `ApplicationIdleHook` | planned |
| task.h | `vApplicationStackOverflowHook` | `ApplicationStackOverflowHook` | planned |
| task.h | `vApplicationTickHook` | `ApplicationTickHook` | planned |
| task.h | `vGrantAccessToKernelObject` | `GrantAccessToKernelObject` | planned |
| task.h | `vPortGrantAccessToKernelObject` | `PortGrantAccessToKernelObject` | planned |
| task.h | `vPortRevokeAccessToKernelObject` | `PortRevokeAccessToKernelObject` | planned |
| task.h | `vRevokeAccessToKernelObject` | `RevokeAccessToKernelObject` | planned |
| task.h | `vTaskAllocateMPURegions` | `TaskAllocateMPURegions` | planned (K8, rusty_rtos_mpu) |
| task.h | `vTaskCoreAffinityGet` | `TaskCoreAffinityGet` | planned (K8, smp) |
| task.h | `vTaskCoreAffinitySet` | `TaskCoreAffinitySet` | planned (K8, smp) |
| task.h | `vTaskDelay` | `TaskDelay` | planned |
| task.h | `vTaskDelayUntil` | `TaskDelayUntil` | planned |
| task.h | `vTaskDelete` | `TaskDelete` | planned |
| task.h | `vTaskEndScheduler` | `TaskEndScheduler` | planned |
| task.h | `vTaskEnterCritical` | `TaskEnterCritical` | planned |
| task.h | `vTaskEnterCriticalFromISR` | `TaskEnterCriticalFromISR (with an `Isr` token)` | planned |
| task.h | `vTaskExitCritical` | `TaskExitCritical` | planned |
| task.h | `vTaskExitCriticalFromISR` | `TaskExitCriticalFromISR (with an `Isr` token)` | planned |
| task.h | `vTaskGenericNotifyGiveFromISR` | `TaskGenericNotifyGiveFromISR (with an `Isr` token)` | planned |
| task.h | `vTaskGetInfo` | `TaskGetInfo` | planned |
| task.h | `vTaskGetRunTimeStatistics` | `TaskGetRunTimeStatistics` | planned |
| task.h | `vTaskGetRunTimeStats` | `TaskGetRunTimeStats` | planned |
| task.h | `vTaskInternalSetTimeOutState` | `TaskInternalSetTimeOutState` | planned |
| task.h | `vTaskList` | `TaskList` | planned |
| task.h | `vTaskListTasks` | `TaskListTasks` | planned |
| task.h | `vTaskMissedYield` | `TaskMissedYield` | planned |
| task.h | `vTaskNotifyGiveFromISR` | `TaskNotifyGiveFromISR (with an `Isr` token)` | planned |
| task.h | `vTaskNotifyGiveIndexedFromISR` | `TaskNotifyGiveIndexedFromISR (with an `Isr` token)` | planned |
| task.h | `vTaskPlaceOnEventList` | `TaskPlaceOnEventList` | planned |
| task.h | `vTaskPlaceOnEventListRestricted` | `TaskPlaceOnEventListRestricted` | planned (K8, rusty_rtos_mpu) |
| task.h | `vTaskPlaceOnUnorderedEventList` | `TaskPlaceOnUnorderedEventList` | planned |
| task.h | `vTaskPreemptionDisable` | `TaskPreemptionDisable` | planned (K8, smp) |
| task.h | `vTaskPreemptionEnable` | `TaskPreemptionEnable` | planned (K8, smp) |
| task.h | `vTaskPriorityDisinheritAfterTimeout` | `TaskPriorityDisinheritAfterTimeout` | planned |
| task.h | `vTaskPrioritySet` | `TaskPrioritySet` | planned |
| task.h | `vTaskRemoveFromUnorderedEventList` | `TaskRemoveFromUnorderedEventList` | planned |
| task.h | `vTaskResetState` | `TaskResetState` | planned |
| task.h | `vTaskResume` | `TaskResume` | planned |
| task.h | `vTaskSetApplicationTaskTag` | `TaskSetApplicationTaskTag` | planned |
| task.h | `vTaskSetTaskNumber` | `TaskSetTaskNumber` | planned |
| task.h | `vTaskSetThreadLocalStoragePointer` | `TaskSetThreadLocalStoragePointer` | planned |
| task.h | `vTaskSetTimeOutState` | `TaskSetTimeOutState` | planned |
| task.h | `vTaskStartScheduler` | `TaskStartScheduler` | planned |
| task.h | `vTaskStepTick` | `TaskStepTick` | planned |
| task.h | `vTaskSuspend` | `TaskSuspend` | planned |
| task.h | `vTaskSuspendAll` | `TaskSuspendAll` | planned |
| task.h | `vTaskSwitchContext` | `TaskSwitchContext` | planned |
| task.h | `vTaskYieldWithinAPI` | `TaskYieldWithinAPI` | planned |
| task.h | `xTaskAbortDelay` | `TaskAbortDelay` | planned |
| task.h | `xTaskCallApplicationTaskHook` | `TaskCallApplicationTaskHook` | planned |
| task.h | `xTaskCatchUpTicks` | `TaskCatchUpTicks` | planned |
| task.h | `xTaskCheckForTimeOut` | `TaskCheckForTimeOut` | planned |
| task.h | `xTaskCreate` | `TaskCreate` | planned |
| task.h | `xTaskCreateAffinitySet` | `TaskCreateAffinitySet` | planned (K8, smp) |
| task.h | `xTaskCreateRestricted` | `TaskCreateRestricted` | planned (K8, rusty_rtos_mpu) |
| task.h | `xTaskCreateRestrictedAffinitySet` | `TaskCreateRestrictedAffinitySet` | planned (K8, rusty_rtos_mpu) |
| task.h | `xTaskCreateRestrictedStatic` | `TaskCreateRestrictedStatic` | planned (K8, rusty_rtos_mpu) |
| task.h | `xTaskCreateRestrictedStaticAffinitySet` | `TaskCreateRestrictedStaticAffinitySet` | planned (K8, rusty_rtos_mpu) |
| task.h | `xTaskCreateStatic` | `TaskCreateStatic` | planned |
| task.h | `xTaskCreateStaticAffinitySet` | `TaskCreateStaticAffinitySet` | planned (K8, smp) |
| task.h | `xTaskDelayUntil` | `TaskDelayUntil` | planned |
| task.h | `xTaskGenericNotify` | `TaskGenericNotify` | planned |
| task.h | `xTaskGenericNotifyFromISR` | `TaskGenericNotifyFromISR (with an `Isr` token)` | planned |
| task.h | `xTaskGenericNotifyStateClear` | `TaskGenericNotifyStateClear` | planned |
| task.h | `xTaskGenericNotifyWait` | `TaskGenericNotifyWait` | planned |
| task.h | `xTaskGetApplicationTaskTag` | `TaskGetApplicationTaskTag` | planned |
| task.h | `xTaskGetApplicationTaskTagFromISR` | `TaskGetApplicationTaskTagFromISR (with an `Isr` token)` | planned |
| task.h | `xTaskGetCurrentTaskHandle` | `TaskGetCurrentTaskHandle` | planned |
| task.h | `xTaskGetCurrentTaskHandleForCore` | `TaskGetCurrentTaskHandleForCore` | planned (K8, smp) |
| task.h | `xTaskGetHandle` | `TaskGetHandle` | planned |
| task.h | `xTaskGetIdleTaskHandle` | `TaskGetIdleTaskHandle` | planned |
| task.h | `xTaskGetIdleTaskHandleForCore` | `TaskGetIdleTaskHandleForCore` | planned (K8, smp) |
| task.h | `xTaskGetMPUSettings` | `TaskGetMPUSettings` | planned (K8, rusty_rtos_mpu) |
| task.h | `xTaskGetSchedulerState` | `TaskGetSchedulerState` | planned |
| task.h | `xTaskGetStaticBuffers` | `TaskGetStaticBuffers` | planned |
| task.h | `xTaskGetTickCount` | `TaskGetTickCount` | planned |
| task.h | `xTaskGetTickCountFromISR` | `TaskGetTickCountFromISR (with an `Isr` token)` | planned |
| task.h | `xTaskIncrementTick` | `TaskIncrementTick` | planned |
| task.h | `xTaskNotify` | `TaskNotify` | planned |
| task.h | `xTaskNotifyAndQuery` | `TaskNotifyAndQuery` | planned |
| task.h | `xTaskNotifyAndQueryFromISR` | `TaskNotifyAndQueryFromISR (with an `Isr` token)` | planned |
| task.h | `xTaskNotifyAndQueryIndexed` | `TaskNotifyAndQueryIndexed` | planned |
| task.h | `xTaskNotifyAndQueryIndexedFromISR` | `TaskNotifyAndQueryIndexedFromISR (with an `Isr` token)` | planned |
| task.h | `xTaskNotifyFromISR` | `TaskNotifyFromISR (with an `Isr` token)` | planned |
| task.h | `xTaskNotifyGive` | `TaskNotifyGive` | planned |
| task.h | `xTaskNotifyGiveIndexed` | `TaskNotifyGiveIndexed` | planned |
| task.h | `xTaskNotifyIndexed` | `TaskNotifyIndexed` | planned |
| task.h | `xTaskNotifyIndexedFromISR` | `TaskNotifyIndexedFromISR (with an `Isr` token)` | planned |
| task.h | `xTaskNotifyStateClear` | `TaskNotifyStateClear` | planned |
| task.h | `xTaskNotifyStateClearIndexed` | `TaskNotifyStateClearIndexed` | planned |
| task.h | `xTaskNotifyWait` | `TaskNotifyWait` | planned |
| task.h | `xTaskNotifyWaitIndexed` | `TaskNotifyWaitIndexed` | planned |
| task.h | `xTaskPeriodicDelay` | `TaskPeriodicDelay` | planned |
| task.h | `xTaskPriorityDisinherit` | `TaskPriorityDisinherit` | planned |
| task.h | `xTaskPriorityInherit` | `TaskPriorityInherit` | planned |
| task.h | `xTaskRemoveFromEventList` | `TaskRemoveFromEventList` | planned |
| task.h | `xTaskResumeAll` | `TaskResumeAll` | planned |
| task.h | `xTaskResumeFromISR` | `TaskResumeFromISR (with an `Isr` token)` | planned |
| queue.h | `pcQueueGetName` | `QueueGetName` | planned |
| queue.h | `ucQueueGetQueueType` | `QueueGetQueueType` | planned |
| queue.h | `uxQueueGetQueueItemSize` | `QueueGetQueueItemSize` | planned |
| queue.h | `uxQueueGetQueueLength` | `QueueGetQueueLength` | planned |
| queue.h | `uxQueueGetQueueNumber` | `QueueGetQueueNumber` | planned |
| queue.h | `uxQueueMessagesWaiting` | `QueueMessagesWaiting` | planned |
| queue.h | `uxQueueMessagesWaitingFromISR` | `QueueMessagesWaitingFromISR (with an `Isr` token)` | planned |
| queue.h | `uxQueueSpacesAvailable` | `QueueSpacesAvailable` | planned |
| queue.h | `vQueueAddToRegistry` | `QueueAddToRegistry` | planned |
| queue.h | `vQueueDelete` | `QueueDelete` | planned |
| queue.h | `vQueueSetQueueNumber` | `QueueSetQueueNumber` | planned |
| queue.h | `vQueueUnregisterQueue` | `QueueUnregisterQueue` | planned |
| queue.h | `vQueueWaitForMessageRestricted` | `QueueWaitForMessageRestricted` | planned (K8, rusty_rtos_mpu) |
| queue.h | `xQueueAddToSet` | `QueueAddToSet` | planned |
| queue.h | `xQueueCRReceive` | `QueueCRReceive` | never — co-routines are deprecated upstream |
| queue.h | `xQueueCRReceiveFromISR` | `QueueCRReceiveFromISR` | never — co-routines are deprecated upstream |
| queue.h | `xQueueCRSend` | `QueueCRSend` | never — co-routines are deprecated upstream |
| queue.h | `xQueueCRSendFromISR` | `QueueCRSendFromISR` | never — co-routines are deprecated upstream |
| queue.h | `xQueueCreate` | `QueueCreate` | planned |
| queue.h | `xQueueCreateCountingSemaphore` | `QueueCreateCountingSemaphore` | planned |
| queue.h | `xQueueCreateCountingSemaphoreStatic` | `QueueCreateCountingSemaphoreStatic` | planned |
| queue.h | `xQueueCreateMutex` | `QueueCreateMutex` | planned |
| queue.h | `xQueueCreateMutexStatic` | `QueueCreateMutexStatic` | planned |
| queue.h | `xQueueCreateSet` | `QueueCreateSet` | planned |
| queue.h | `xQueueCreateSetStatic` | `QueueCreateSetStatic` | planned |
| queue.h | `xQueueCreateStatic` | `QueueCreateStatic` | planned |
| queue.h | `xQueueGenericCreate` | `QueueGenericCreate` | planned |
| queue.h | `xQueueGenericCreateStatic` | `QueueGenericCreateStatic` | planned |
| queue.h | `xQueueGenericGetStaticBuffers` | `QueueGenericGetStaticBuffers` | planned |
| queue.h | `xQueueGenericReset` | `QueueGenericReset` | planned |
| queue.h | `xQueueGenericSend` | `QueueGenericSend` | planned |
| queue.h | `xQueueGenericSendFromISR` | `QueueGenericSendFromISR (with an `Isr` token)` | planned |
| queue.h | `xQueueGetMutexHolder` | `QueueGetMutexHolder` | planned |
| queue.h | `xQueueGetMutexHolderFromISR` | `QueueGetMutexHolderFromISR (with an `Isr` token)` | planned |
| queue.h | `xQueueGetStaticBuffers` | `QueueGetStaticBuffers` | planned |
| queue.h | `xQueueGiveFromISR` | `QueueGiveFromISR (with an `Isr` token)` | planned |
| queue.h | `xQueueGiveMutexRecursive` | `QueueGiveMutexRecursive` | planned |
| queue.h | `xQueueIsQueueEmptyFromISR` | `QueueIsQueueEmptyFromISR (with an `Isr` token)` | planned |
| queue.h | `xQueueIsQueueFullFromISR` | `QueueIsQueueFullFromISR (with an `Isr` token)` | planned |
| queue.h | `xQueueOverwrite` | `QueueOverwrite` | planned |
| queue.h | `xQueueOverwriteFromISR` | `QueueOverwriteFromISR (with an `Isr` token)` | planned |
| queue.h | `xQueuePeek` | `QueuePeek` | planned |
| queue.h | `xQueuePeekFromISR` | `QueuePeekFromISR (with an `Isr` token)` | planned |
| queue.h | `xQueueReceive` | `QueueReceive` | planned |
| queue.h | `xQueueReceiveFromISR` | `QueueReceiveFromISR (with an `Isr` token)` | planned |
| queue.h | `xQueueRemoveFromSet` | `QueueRemoveFromSet` | planned |
| queue.h | `xQueueReset` | `QueueReset` | planned |
| queue.h | `xQueueSelectFromSet` | `QueueSelectFromSet` | planned |
| queue.h | `xQueueSelectFromSetFromISR` | `QueueSelectFromSetFromISR (with an `Isr` token)` | planned |
| queue.h | `xQueueSemaphoreTake` | `QueueSemaphoreTake` | planned |
| queue.h | `xQueueSend` | `QueueSend` | planned |
| queue.h | `xQueueSendFromISR` | `QueueSendFromISR (with an `Isr` token)` | planned |
| queue.h | `xQueueSendToBack` | `QueueSendToBack` | planned |
| queue.h | `xQueueSendToBackFromISR` | `QueueSendToBackFromISR (with an `Isr` token)` | planned |
| queue.h | `xQueueSendToFront` | `QueueSendToFront` | planned |
| queue.h | `xQueueSendToFrontFromISR` | `QueueSendToFrontFromISR (with an `Isr` token)` | planned |
| queue.h | `xQueueTakeMutexRecursive` | `QueueTakeMutexRecursive` | planned |
| semphr.h | `uxQueueMessagesWaiting` | `QueueMessagesWaiting` | planned |
| semphr.h | `uxQueueMessagesWaitingFromISR` | `QueueMessagesWaitingFromISR (with an `Isr` token)` | planned |
| semphr.h | `uxSemaphoreGetCount` | `SemaphoreGetCount` | planned |
| semphr.h | `uxSemaphoreGetCountFromISR` | `SemaphoreGetCountFromISR (with an `Isr` token)` | planned |
| semphr.h | `vQueueDelete` | `QueueDelete` | planned |
| semphr.h | `vSemaphoreCreateBinary` | `SemaphoreCreateBinary` | planned |
| semphr.h | `vSemaphoreDelete` | `SemaphoreDelete` | planned |
| semphr.h | `xQueueCreateCountingSemaphore` | `QueueCreateCountingSemaphore` | planned |
| semphr.h | `xQueueCreateCountingSemaphoreStatic` | `QueueCreateCountingSemaphoreStatic` | planned |
| semphr.h | `xQueueCreateMutex` | `QueueCreateMutex` | planned |
| semphr.h | `xQueueCreateMutexStatic` | `QueueCreateMutexStatic` | planned |
| semphr.h | `xQueueGenericCreate` | `QueueGenericCreate` | planned |
| semphr.h | `xQueueGenericCreateStatic` | `QueueGenericCreateStatic` | planned |
| semphr.h | `xQueueGenericGetStaticBuffers` | `QueueGenericGetStaticBuffers` | planned |
| semphr.h | `xQueueGenericSend` | `QueueGenericSend` | planned |
| semphr.h | `xQueueGetMutexHolder` | `QueueGetMutexHolder` | planned |
| semphr.h | `xQueueGetMutexHolderFromISR` | `QueueGetMutexHolderFromISR (with an `Isr` token)` | planned |
| semphr.h | `xQueueGiveFromISR` | `QueueGiveFromISR (with an `Isr` token)` | planned |
| semphr.h | `xQueueGiveMutexRecursive` | `QueueGiveMutexRecursive` | planned |
| semphr.h | `xQueueReceiveFromISR` | `QueueReceiveFromISR (with an `Isr` token)` | planned |
| semphr.h | `xQueueSemaphoreTake` | `QueueSemaphoreTake` | planned |
| semphr.h | `xQueueTakeMutexRecursive` | `QueueTakeMutexRecursive` | planned |
| semphr.h | `xSemaphoreCreateBinary` | `SemaphoreCreateBinary` | planned |
| semphr.h | `xSemaphoreCreateBinaryStatic` | `SemaphoreCreateBinaryStatic` | planned |
| semphr.h | `xSemaphoreCreateCounting` | `SemaphoreCreateCounting` | planned |
| semphr.h | `xSemaphoreCreateCountingStatic` | `SemaphoreCreateCountingStatic` | planned |
| semphr.h | `xSemaphoreCreateMutex` | `SemaphoreCreateMutex` | planned |
| semphr.h | `xSemaphoreCreateMutexStatic` | `SemaphoreCreateMutexStatic` | planned |
| semphr.h | `xSemaphoreCreateRecursiveMutex` | `SemaphoreCreateRecursiveMutex` | planned |
| semphr.h | `xSemaphoreCreateRecursiveMutexStatic` | `SemaphoreCreateRecursiveMutexStatic` | planned |
| semphr.h | `xSemaphoreGetMutexHolder` | `SemaphoreGetMutexHolder` | planned |
| semphr.h | `xSemaphoreGetMutexHolderFromISR` | `SemaphoreGetMutexHolderFromISR (with an `Isr` token)` | planned |
| semphr.h | `xSemaphoreGetStaticBuffer` | `SemaphoreGetStaticBuffer` | planned |
| semphr.h | `xSemaphoreGive` | `SemaphoreGive` | planned |
| semphr.h | `xSemaphoreGiveFromISR` | `SemaphoreGiveFromISR (with an `Isr` token)` | planned |
| semphr.h | `xSemaphoreGiveRecursive` | `SemaphoreGiveRecursive` | planned |
| semphr.h | `xSemaphoreTake` | `SemaphoreTake` | planned |
| semphr.h | `xSemaphoreTakeFromISR` | `SemaphoreTakeFromISR (with an `Isr` token)` | planned |
| semphr.h | `xSemaphoreTakeRecursive` | `SemaphoreTakeRecursive` | planned |
| timers.h | `pcTimerGetName` | `TimerGetName` | planned |
| timers.h | `pvTimerGetTimerID` | `TimerGetTimerID` | planned |
| timers.h | `uxTimerGetReloadMode` | `TimerGetReloadMode` | planned |
| timers.h | `uxTimerGetTimerNumber` | `TimerGetTimerNumber` | planned |
| timers.h | `vApplicationDaemonTaskStartupHook` | `ApplicationDaemonTaskStartupHook` | planned |
| timers.h | `vApplicationGetTimerTaskMemory` | `ApplicationGetTimerTaskMemory` | planned |
| timers.h | `vTimerResetState` | `TimerResetState` | planned |
| timers.h | `vTimerSetReloadMode` | `TimerSetReloadMode` | planned |
| timers.h | `vTimerSetTimerID` | `TimerSetTimerID` | planned |
| timers.h | `vTimerSetTimerNumber` | `TimerSetTimerNumber` | planned |
| timers.h | `xTaskGetTickCount` | `TaskGetTickCount` | planned |
| timers.h | `xTaskGetTickCountFromISR` | `TaskGetTickCountFromISR (with an `Isr` token)` | planned |
| timers.h | `xTimerChangePeriod` | `TimerChangePeriod` | planned |
| timers.h | `xTimerChangePeriodFromISR` | `TimerChangePeriodFromISR (with an `Isr` token)` | planned |
| timers.h | `xTimerCreate` | `TimerCreate` | planned |
| timers.h | `xTimerCreateStatic` | `TimerCreateStatic` | planned |
| timers.h | `xTimerCreateTimerTask` | `TimerCreateTimerTask` | planned |
| timers.h | `xTimerDelete` | `TimerDelete` | planned |
| timers.h | `xTimerGenericCommand` | `TimerGenericCommand` | planned |
| timers.h | `xTimerGenericCommandFromISR` | `TimerGenericCommandFromISR (with an `Isr` token)` | planned |
| timers.h | `xTimerGenericCommandFromTask` | `TimerGenericCommandFromTask` | planned |
| timers.h | `xTimerGetExpiryTime` | `TimerGetExpiryTime` | planned |
| timers.h | `xTimerGetPeriod` | `TimerGetPeriod` | planned |
| timers.h | `xTimerGetReloadMode` | `TimerGetReloadMode` | planned |
| timers.h | `xTimerGetStaticBuffer` | `TimerGetStaticBuffer` | planned |
| timers.h | `xTimerGetTimerDaemonTaskHandle` | `TimerGetTimerDaemonTaskHandle` | planned |
| timers.h | `xTimerIsTimerActive` | `TimerIsTimerActive` | planned |
| timers.h | `xTimerPendFunctionCall` | `TimerPendFunctionCall` | planned |
| timers.h | `xTimerPendFunctionCallFromISR` | `TimerPendFunctionCallFromISR (with an `Isr` token)` | planned |
| timers.h | `xTimerReset` | `TimerReset` | planned |
| timers.h | `xTimerResetFromISR` | `TimerResetFromISR (with an `Isr` token)` | planned |
| timers.h | `xTimerStart` | `TimerStart` | planned |
| timers.h | `xTimerStartFromISR` | `TimerStartFromISR (with an `Isr` token)` | planned |
| timers.h | `xTimerStop` | `TimerStop` | planned |
| timers.h | `xTimerStopFromISR` | `TimerStopFromISR (with an `Isr` token)` | planned |
| event_groups.h | `uxEventGroupGetNumber` | `EventGroupGetNumber` | planned |
| event_groups.h | `vEventGroupClearBitsCallback` | `EventGroupClearBitsCallback` | planned |
| event_groups.h | `vEventGroupDelete` | `EventGroupDelete` | planned |
| event_groups.h | `vEventGroupSetBitsCallback` | `EventGroupSetBitsCallback` | planned |
| event_groups.h | `vEventGroupSetNumber` | `EventGroupSetNumber` | planned |
| event_groups.h | `xEventGroupClearBits` | `EventGroupClearBits` | planned |
| event_groups.h | `xEventGroupClearBitsFromISR` | `EventGroupClearBitsFromISR (with an `Isr` token)` | planned |
| event_groups.h | `xEventGroupCreate` | `EventGroupCreate` | planned |
| event_groups.h | `xEventGroupCreateStatic` | `EventGroupCreateStatic` | planned |
| event_groups.h | `xEventGroupGetBits` | `EventGroupGetBits` | planned |
| event_groups.h | `xEventGroupGetBitsFromISR` | `EventGroupGetBitsFromISR (with an `Isr` token)` | planned |
| event_groups.h | `xEventGroupGetStaticBuffer` | `EventGroupGetStaticBuffer` | planned |
| event_groups.h | `xEventGroupSetBits` | `EventGroupSetBits` | planned |
| event_groups.h | `xEventGroupSetBitsFromISR` | `EventGroupSetBitsFromISR (with an `Isr` token)` | planned |
| event_groups.h | `xEventGroupSync` | `EventGroupSync` | planned |
| event_groups.h | `xEventGroupWaitBits` | `EventGroupWaitBits` | planned |
| stream_buffer.h | `ucStreamBufferGetStreamBufferType` | `StreamBufferGetStreamBufferType` | planned |
| stream_buffer.h | `uxStreamBufferGetStreamBufferNotificationIndex` | `StreamBufferGetStreamBufferNotificationIndex` | planned |
| stream_buffer.h | `uxStreamBufferGetStreamBufferNumber` | `StreamBufferGetStreamBufferNumber` | planned |
| stream_buffer.h | `vStreamBufferDelete` | `StreamBufferDelete` | planned |
| stream_buffer.h | `vStreamBufferSetStreamBufferNotificationIndex` | `StreamBufferSetStreamBufferNotificationIndex` | planned |
| stream_buffer.h | `vStreamBufferSetStreamBufferNumber` | `StreamBufferSetStreamBufferNumber` | planned |
| stream_buffer.h | `xStreamBatchingBufferCreate` | `StreamBatchingBufferCreate` | planned |
| stream_buffer.h | `xStreamBatchingBufferCreateStatic` | `StreamBatchingBufferCreateStatic` | planned |
| stream_buffer.h | `xStreamBatchingBufferCreateStaticWithCallback` | `StreamBatchingBufferCreateStaticWithCallback` | planned |
| stream_buffer.h | `xStreamBatchingBufferCreateWithCallback` | `StreamBatchingBufferCreateWithCallback` | planned |
| stream_buffer.h | `xStreamBufferBytesAvailable` | `StreamBufferBytesAvailable` | planned |
| stream_buffer.h | `xStreamBufferCreate` | `StreamBufferCreate` | planned |
| stream_buffer.h | `xStreamBufferCreateStatic` | `StreamBufferCreateStatic` | planned |
| stream_buffer.h | `xStreamBufferCreateStaticWithCallback` | `StreamBufferCreateStaticWithCallback` | planned |
| stream_buffer.h | `xStreamBufferCreateWithCallback` | `StreamBufferCreateWithCallback` | planned |
| stream_buffer.h | `xStreamBufferGenericCreate` | `StreamBufferGenericCreate` | planned |
| stream_buffer.h | `xStreamBufferGenericCreateStatic` | `StreamBufferGenericCreateStatic` | planned |
| stream_buffer.h | `xStreamBufferGetStaticBuffers` | `StreamBufferGetStaticBuffers` | planned |
| stream_buffer.h | `xStreamBufferIsEmpty` | `StreamBufferIsEmpty` | planned |
| stream_buffer.h | `xStreamBufferIsFull` | `StreamBufferIsFull` | planned |
| stream_buffer.h | `xStreamBufferNextMessageLengthBytes` | `StreamBufferNextMessageLengthBytes` | planned |
| stream_buffer.h | `xStreamBufferReceive` | `StreamBufferReceive` | planned |
| stream_buffer.h | `xStreamBufferReceiveCompletedFromISR` | `StreamBufferReceiveCompletedFromISR (with an `Isr` token)` | planned |
| stream_buffer.h | `xStreamBufferReceiveFromISR` | `StreamBufferReceiveFromISR (with an `Isr` token)` | planned |
| stream_buffer.h | `xStreamBufferReset` | `StreamBufferReset` | planned |
| stream_buffer.h | `xStreamBufferResetFromISR` | `StreamBufferResetFromISR (with an `Isr` token)` | planned |
| stream_buffer.h | `xStreamBufferSend` | `StreamBufferSend` | planned |
| stream_buffer.h | `xStreamBufferSendCompletedFromISR` | `StreamBufferSendCompletedFromISR (with an `Isr` token)` | planned |
| stream_buffer.h | `xStreamBufferSendFromISR` | `StreamBufferSendFromISR (with an `Isr` token)` | planned |
| stream_buffer.h | `xStreamBufferSetTriggerLevel` | `StreamBufferSetTriggerLevel` | planned |
| stream_buffer.h | `xStreamBufferSpacesAvailable` | `StreamBufferSpacesAvailable` | planned |
| message_buffer.h | `vMessageBufferDelete` | `MessageBufferDelete` | planned |
| message_buffer.h | `vStreamBufferDelete` | `StreamBufferDelete` | planned |
| message_buffer.h | `xMessageBufferCreate` | `MessageBufferCreate` | planned |
| message_buffer.h | `xMessageBufferCreateStatic` | `MessageBufferCreateStatic` | planned |
| message_buffer.h | `xMessageBufferCreateStaticWithCallback` | `MessageBufferCreateStaticWithCallback` | planned |
| message_buffer.h | `xMessageBufferCreateWithCallback` | `MessageBufferCreateWithCallback` | planned |
| message_buffer.h | `xMessageBufferGetStaticBuffers` | `MessageBufferGetStaticBuffers` | planned |
| message_buffer.h | `xMessageBufferIsEmpty` | `MessageBufferIsEmpty` | planned |
| message_buffer.h | `xMessageBufferIsFull` | `MessageBufferIsFull` | planned |
| message_buffer.h | `xMessageBufferNextLengthBytes` | `MessageBufferNextLengthBytes` | planned |
| message_buffer.h | `xMessageBufferReceive` | `MessageBufferReceive` | planned |
| message_buffer.h | `xMessageBufferReceiveCompletedFromISR` | `MessageBufferReceiveCompletedFromISR (with an `Isr` token)` | planned |
| message_buffer.h | `xMessageBufferReceiveFromISR` | `MessageBufferReceiveFromISR (with an `Isr` token)` | planned |
| message_buffer.h | `xMessageBufferReset` | `MessageBufferReset` | planned |
| message_buffer.h | `xMessageBufferResetFromISR` | `MessageBufferResetFromISR (with an `Isr` token)` | planned |
| message_buffer.h | `xMessageBufferSend` | `MessageBufferSend` | planned |
| message_buffer.h | `xMessageBufferSendCompletedFromISR` | `MessageBufferSendCompletedFromISR (with an `Isr` token)` | planned |
| message_buffer.h | `xMessageBufferSendFromISR` | `MessageBufferSendFromISR (with an `Isr` token)` | planned |
| message_buffer.h | `xMessageBufferSpaceAvailable` | `MessageBufferSpaceAvailable` | planned |
| message_buffer.h | `xMessageBufferSpacesAvailable` | `MessageBufferSpacesAvailable` | planned |
| message_buffer.h | `xStreamBufferGenericCreate` | `StreamBufferGenericCreate` | planned |
| message_buffer.h | `xStreamBufferGenericCreateStatic` | `StreamBufferGenericCreateStatic` | planned |
| message_buffer.h | `xStreamBufferGetStaticBuffers` | `StreamBufferGetStaticBuffers` | planned |
| message_buffer.h | `xStreamBufferIsEmpty` | `StreamBufferIsEmpty` | planned |
| message_buffer.h | `xStreamBufferIsFull` | `StreamBufferIsFull` | planned |
| message_buffer.h | `xStreamBufferNextMessageLengthBytes` | `StreamBufferNextMessageLengthBytes` | planned |
| message_buffer.h | `xStreamBufferReceive` | `StreamBufferReceive` | planned |
| message_buffer.h | `xStreamBufferReceiveCompletedFromISR` | `StreamBufferReceiveCompletedFromISR (with an `Isr` token)` | planned |
| message_buffer.h | `xStreamBufferReceiveFromISR` | `StreamBufferReceiveFromISR (with an `Isr` token)` | planned |
| message_buffer.h | `xStreamBufferReset` | `StreamBufferReset` | planned |
| message_buffer.h | `xStreamBufferResetFromISR` | `StreamBufferResetFromISR (with an `Isr` token)` | planned |
| message_buffer.h | `xStreamBufferSend` | `StreamBufferSend` | planned |
| message_buffer.h | `xStreamBufferSendCompletedFromISR` | `StreamBufferSendCompletedFromISR (with an `Isr` token)` | planned |
| message_buffer.h | `xStreamBufferSendFromISR` | `StreamBufferSendFromISR (with an `Isr` token)` | planned |
| message_buffer.h | `xStreamBufferSpacesAvailable` | `StreamBufferSpacesAvailable` | planned |
