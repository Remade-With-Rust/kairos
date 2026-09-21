/*
 * ApiSweep.h -- KAIROS-authored, in the shape of a demo header.
 *
 * See ApiSweep.c for what this scenario is and why it is not a port.
 */

#ifndef API_SWEEP_H
#define API_SWEEP_H

void vStartApiSweepTasks( void );
BaseType_t xAreApiSweepTasksStillRunning( void );

/* Called from the tick hook, as every other interrupt half in this corpus
 * is. Writes to the FRONT of the sweep queue. */
void vApiSweepAccessFromISR( void );

#endif /* API_SWEEP_H */
