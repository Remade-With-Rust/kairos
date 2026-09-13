/* Declaration-only stub. clang has no bare-metal riscv32 sysroot on this box.
   Declarations emit no code, so object sizes are unaffected: calls to these
   remain external relocations exactly as with a real libc header. */
#ifndef _KAIROS_STUB_STDLIB_H
#define _KAIROS_STUB_STDLIB_H
#include <stddef.h>
void *malloc( size_t );
void free( void * );
void *calloc( size_t, size_t );
void abort( void );
void exit( int );
#endif
