/* cairo - a vector graphics library with display and print output
 *
 * Copyright © 2002 University of Southern California
 * Copyright © 2005 Red Hat, Inc.
 *
 * This library is free software; you can redistribute it and/or
 * modify it either under the terms of the GNU Lesser General Public
 * License version 2.1 as published by the Free Software Foundation
 * (the "LGPL") or, at your option, under the terms of the Mozilla
 * Public License Version 1.1 (the "MPL"). If you do not alter this
 * notice, a recipient may use your version of this file under either
 * the MPL or the LGPL.
 *
 * You should have received a copy of the LGPL along with this library
 * in the file COPYING-LGPL-2.1; if not, write to the Free Software
 * Foundation, Inc., 51 Franklin Street, Suite 500, Boston, MA 02110-1335, USA
 * You should have received a copy of the MPL along with this library
 * in the file COPYING-MPL-1.1
 *
 * The contents of this file are subject to the Mozilla Public License
 * Version 1.1 (the "License"); you may not use this file except in
 * compliance with the License. You may obtain a copy of the License at
 * http://www.mozilla.org/MPL/
 *
 * This software is distributed on an "AS IS" basis, WITHOUT WARRANTY
 * OF ANY KIND, either express or implied. See the LGPL or the MPL for
 * the specific language governing rights and limitations.
 *
 * The Original Code is the cairo graphics library.
 *
 * The Initial Developer of the Original Code is University of Southern
 * California.
 *
 * Contributor(s):
 *	Carl D. Worth <cworth@cworth.org>
 */

#ifndef CAIRO_COMPILER_PRIVATE_H
#define CAIRO_COMPILER_PRIVATE_H

#include "cairo.h"

#include "config.h"

#include <stddef.h> /* size_t */
#include <stdint.h> /* SIZE_MAX */

/* Size in bytes of buffer to use off the stack per functions.
 * Mostly used by text functions.  For larger allocations, they'll
 * malloc(). */
#ifndef CAIRO_STACK_BUFFER_SIZE
#define CAIRO_STACK_BUFFER_SIZE (512 * sizeof (int))
#endif

#define CAIRO_STACK_ARRAY_LENGTH(T) (CAIRO_STACK_BUFFER_SIZE / sizeof(T))

/*
 * The goal of this block is to define the following macros for
 * providing faster linkage to functions in the public API for calls
 * from within cairo.
 *
 * slim_hidden_proto(f)
 * slim_hidden_proto_no_warn(f)
 *
 *   Declares `f' as a library internal function and hides the
 *   function from the global symbol table.  This macro must be
 *   expanded after `f' has been declared with a prototype but before
 *   any calls to the function are seen by the compiler.  The no_warn
 *   variant inhibits warnings about the return value being unused at
 *   call sites.  The macro works by renaming `f' to an internal name
 *   in the symbol table and hiding that.  As far as cairo internal
 *   calls are concerned they're calling a library internal function
 *   and thus don't need to bounce via the procedure linkage table (PLT).
 *
 * slim_hidden_def(f)
 *
 *   Exports `f' back to the global symbol table.  This macro must be
 *   expanded right after the function definition and only for symbols
 *   hidden previously with slim_hidden_proto().  The macro works by
 *   adding a global entry to the symbol table which points at the
 *   internal name of `f' created by slim_hidden_proto().
 *
 * Functions in the public API which aren't called by the library
 * don't need to be hidden and re-exported using the slim hidden
 * macros.
 */
#if __GNUC__ >= 3 && defined(__ELF__) && !defined(__sun)
# define slim_hidden_proto(name)		slim_hidden_proto1(name, slim_hidden_int_name(name)) cairo_private
# define slim_hidden_proto_no_warn(name)	slim_hidden_proto1(name, slim_hidden_int_name(name)) cairo_private_no_warn
# define slim_hidden_def(name)			slim_hidden_def1(name, slim_hidden_int_name(name))
# define slim_hidden_int_name(name) INT_##name
# define slim_hidden_proto1(name, internal)				\
  extern __typeof (name) name						\
	__asm__ (slim_hidden_asmname (internal))
# define slim_hidden_def1(name, internal)				\
  extern __typeof (name) EXT_##name __asm__(slim_hidden_asmname(name))	\
	__attribute__((__alias__(slim_hidden_asmname(internal))))
# define slim_hidden_ulp		slim_hidden_ulp1(__USER_LABEL_PREFIX__)
# define slim_hidden_ulp1(x)		slim_hidden_ulp2(x)
# define slim_hidden_ulp2(x)		#x
# define slim_hidden_asmname(name)	slim_hidden_asmname1(name)
# define slim_hidden_asmname1(name)	slim_hidden_ulp #name
#else
# define slim_hidden_proto(name)		int _cairo_dummy_prototype(void)
# define slim_hidden_proto_no_warn(name)	int _cairo_dummy_prototype(void)
# define slim_hidden_def(name)			int _cairo_dummy_prototype(void)
#endif

#if __GNUC__ > 2 || (__GNUC__ == 2 && __GNUC_MINOR__ > 4)
#ifdef __MINGW32__
#define CAIRO_PRINTF_FORMAT(fmt_index, va_index)                        \
	__attribute__((__format__(__MINGW_PRINTF_FORMAT, fmt_index, va_index)))
#else
#define CAIRO_PRINTF_FORMAT(fmt_index, va_index)                        \
	__attribute__((__format__(__printf__, fmt_index, va_index)))
#endif
#else
#define CAIRO_PRINTF_FORMAT(fmt_index, va_index)
#endif

/* slim_internal.h */
#define CAIRO_HAS_HIDDEN_SYMBOLS 1
#if (__GNUC__ > 3 || (__GNUC__ == 3 && __GNUC_MINOR__ >= 3)) && \
    (defined(__ELF__) || defined(__APPLE__)) &&			\
    !defined(__sun)
#define cairo_private_no_warn	__attribute__((__visibility__("hidden")))
#elif defined(__SUNPRO_C) && (__SUNPRO_C >= 0x550)
#define cairo_private_no_warn	__hidden
#else /* not gcc >= 3.3 and not Sun Studio >= 8 */
#define cairo_private_no_warn
#undef CAIRO_HAS_HIDDEN_SYMBOLS
#endif

#ifndef WARN_UNUSED_RESULT
#define WARN_UNUSED_RESULT
#endif
/* Add attribute(warn_unused_result) if supported */
#define cairo_warn	    WARN_UNUSED_RESULT
#define cairo_private	    cairo_private_no_warn cairo_warn

/* This macro allow us to deprecate a function by providing an alias
   for the old function name to the new function name. With this
   macro, binary compatibility is preserved. The macro only works on
   some platforms --- tough.

   Meanwhile, new definitions in the public header file break the
   source code so that it will no longer link against the old
   symbols. Instead it will give a descriptive error message
   indicating that the old function has been deprecated by the new
   function.
*/
#if __GNUC__ >= 2 && defined(__ELF__)
# define CAIRO_FUNCTION_ALIAS(old, new)		\
	extern __typeof (new) old		\
	__asm__ ("" #old)			\
	__attribute__((__alias__("" #new)))
#else
# define CAIRO_FUNCTION_ALIAS(old, new)
#endif

/*
 * Cairo uses the following function attributes in order to improve the
 * generated code (effectively by manual inter-procedural analysis).
 *
 *   'cairo_pure': The function is only allowed to read from its arguments
 *                 and global memory (i.e. following a pointer argument or
 *                 accessing a shared variable). The return value should
 *                 only depend on its arguments, and for an identical set of
 *                 arguments should return the same value.
 *
 *   'cairo_const': The function is only allowed to read from its arguments.
 *                  It is not allowed to access global memory. The return
 *                  value should only depend its arguments, and for an
 *                  identical set of arguments should return the same value.
 *                  This is currently the most strict function attribute.
 *
 * Both these function attributes allow gcc to perform CSE and
 * constant-folding, with 'cairo_const 'also guaranteeing that pointer contents
 * do not change across the function call.
 */
#if __GNUC__ >= 3
#define cairo_pure __attribute__((pure))
#define cairo_const __attribute__((const))
#define cairo_always_inline inline __attribute__((always_inline))
#else
#define cairo_pure
#define cairo_const
#define cairo_always_inline inline
#endif

#if defined(__GNUC__) && (__GNUC__ > 2) && defined(__OPTIMIZE__)
#define likely(expr) (__builtin_expect (!!(expr), 1))
#define unlikely(expr) (__builtin_expect (!!(expr), 0))
#else
#define likely(expr) (expr)
#define unlikely(expr) (expr)
#endif

#ifndef __GNUC__
#undef __attribute__
#define __attribute__(x)
#endif

#if (defined(__WIN32__) && !defined(__WINE__)) || defined(_MSC_VER)
#define access _access
#ifndef R_OK
#define R_OK 4
#endif
#define fdopen _fdopen
#define hypot _hypot
#define pclose _pclose
#define popen _popen
#define strdup _strdup
#define unlink _unlink
#if _MSC_VER < 1900
  #define vsnprintf _vsnprintf
  #define snprintf _snprintf
#endif
#endif

#ifdef _MSC_VER
#ifndef __cplusplus
#undef inline
#define inline __inline
#endif
#endif

#if defined(_MSC_VER) && defined(_M_IX86)
/* When compiling with /Gy and /OPT:ICF identical functions will be folded in together.
   The CAIRO_ENSURE_UNIQUE macro ensures that a function is always unique and
   will never be folded into another one. Something like this might eventually
   be needed for GCC but it seems fine for now. */
#define CAIRO_ENSURE_UNIQUE                       \
    do {                                          \
	char file[] = __FILE__;                   \
	__asm {                                   \
	    __asm jmp __internal_skip_line_no     \
	    __asm _emit (__COUNTER__ & 0xff)      \
	    __asm _emit ((__COUNTER__>>8) & 0xff) \
	    __asm _emit ((__COUNTER__>>16) & 0xff)\
	    __asm _emit ((__COUNTER__>>24) & 0xff)\
	    __asm lea eax, dword ptr file         \
	    __asm __internal_skip_line_no:        \
	};                                        \
    } while (0)
#else
#define CAIRO_ENSURE_UNIQUE    do { } while (0)
#endif

#ifdef __STRICT_ANSI__
#undef inline
#define inline __inline__
#endif

/* size_t add/multiply with overflow check.
 *
 * These _cairo_fallback_*_size_t_overflow() functions are always defined
 * to allow them to be tested in the test suite.  They are used
 * if no compiler builtin is available.
 */
static cairo_always_inline cairo_bool_t
_cairo_fallback_add_size_t_overflow(size_t a, size_t b, size_t *c)
{
    if (b > SIZE_MAX - a)
        return 1;

    *c = a + b;
    return 0;
}

static cairo_always_inline cairo_bool_t
_cairo_fallback_mul_size_t_overflow(size_t a, size_t b, size_t *c)
{
    if (b != 0 && a > SIZE_MAX / b)
        return 1;

    *c = a * b;
    return 0;
}

/* Clang defines __GNUC__ so check clang builtins before gcc.
 * MSVC does not support feature macros so hide the __has_builtin inside the #if __clang__ block
 */
#ifdef __clang__
#if defined(__has_builtin) && __has_builtin(__builtin_add_overflow)
#define _cairo_add_size_t_overflow(a, b, c)  __builtin_add_overflow((size_t)(a), (size_t)(b), (size_t*)(c))
#define _cairo_mul_size_t_overflow(a, b, c)  __builtin_mul_overflow((size_t)(a), (size_t)(b), (size_t*)(c))
#endif
#elif __GNUC__ >= 8 || (__GNUC__ >= 5 && (INTPTR_MAX == INT64_MAX))
/* Overflow builtins are available in gcc 5 but the 32-bit version is broken on gcc < 8.
 *   https://gcc.gnu.org/bugzilla/show_bug.cgi?id=82274
 */
#define _cairo_add_size_t_overflow(a, b, c)  __builtin_add_overflow((size_t)(a), (size_t)(b), (size_t*)(c))
#define _cairo_mul_size_t_overflow(a, b, c)  __builtin_mul_overflow((size_t)(a), (size_t)(b), (size_t*)(c))
#elif defined(_MSC_VER) && defined(HAVE_INTSAFE_H)
#include <intsafe.h>
#define _cairo_add_size_t_overflow(a,b,c) (SizeTAdd((size_t)(a), (size_t)(b), (size_t*)(c)) != S_OK)
#define _cairo_mul_size_t_overflow(a,b,c) (SizeTMult((size_t)(a), (size_t)(b), (size_t*)(c)) != S_OK)
#endif

#ifndef _cairo_add_size_t_overflow
#define _cairo_add_size_t_overflow _cairo_fallback_add_size_t_overflow
#define _cairo_mul_size_t_overflow _cairo_fallback_mul_size_t_overflow
#endif

#endif
