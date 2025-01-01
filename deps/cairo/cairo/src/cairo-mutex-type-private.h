/* cairo - a vector graphics library with display and print output
 *
 * Copyright © 2002 University of Southern California
 * Copyright © 2005,2007 Red Hat, Inc.
 * Copyright © 2007 Mathias Hasselmann
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
 *	Mathias Hasselmann <mathias.hasselmann@gmx.de>
 *	Behdad Esfahbod <behdad@behdad.org>
 */

#ifndef CAIRO_MUTEX_TYPE_PRIVATE_H
#define CAIRO_MUTEX_TYPE_PRIVATE_H

#include "cairo-compiler-private.h"
#include "cairo-mutex-impl-private.h"

/* Only the following four are mandatory at this point */
#ifndef CAIRO_MUTEX_IMPL_LOCK
# error "CAIRO_MUTEX_IMPL_LOCK not defined.  Check cairo-mutex-impl-private.h."
#endif
#ifndef CAIRO_MUTEX_IMPL_TRY_LOCK
# error "CAIRO_MUTEX_IMPL_TRY_LOCK not defined.  Check cairo-mutex-impl-private.h."
#endif
#ifndef CAIRO_MUTEX_IMPL_UNLOCK
# error "CAIRO_MUTEX_IMPL_UNLOCK not defined.  Check cairo-mutex-impl-private.h."
#endif
#ifndef CAIRO_MUTEX_IMPL_NIL_INITIALIZER
# error "CAIRO_MUTEX_IMPL_NIL_INITIALIZER not defined.  Check cairo-mutex-impl-private.h."
#endif
#ifndef CAIRO_RECURSIVE_MUTEX_IMPL_INIT
# error "CAIRO_RECURSIVE_MUTEX_IMPL_INIT not defined.  Check cairo-mutex-impl-private.h."
#endif


/* make sure implementations don't fool us: we decide these ourself */
#undef _CAIRO_MUTEX_IMPL_USE_STATIC_INITIALIZER
#undef _CAIRO_MUTEX_IMPL_USE_STATIC_FINALIZER


#ifdef CAIRO_MUTEX_IMPL_INIT

/* If %CAIRO_MUTEX_IMPL_INIT is defined, we may need to initialize all
 * static mutex'es. */
# ifndef CAIRO_MUTEX_IMPL_INITIALIZE
#  define CAIRO_MUTEX_IMPL_INITIALIZE() do {	\
       if (!_cairo_mutex_initialized)	\
           _cairo_mutex_initialize ();	\
    } while(0)

/* and make sure we implement the above */
#  define _CAIRO_MUTEX_IMPL_USE_STATIC_INITIALIZER 1
# endif /* CAIRO_MUTEX_IMPL_INITIALIZE */

#else /* no CAIRO_MUTEX_IMPL_INIT */

/* Otherwise we probably don't need to initialize static mutex'es, */
# ifndef CAIRO_MUTEX_IMPL_INITIALIZE
#  define CAIRO_MUTEX_IMPL_INITIALIZE() CAIRO_MUTEX_IMPL_NOOP
# endif /* CAIRO_MUTEX_IMPL_INITIALIZE */

/* and dynamic ones can be initialized using the static initializer. */
# define CAIRO_MUTEX_IMPL_INIT(mutex) do {				\
      cairo_mutex_t _tmp_mutex = CAIRO_MUTEX_IMPL_NIL_INITIALIZER;	\
      memcpy (&(mutex), &_tmp_mutex, sizeof (_tmp_mutex));	\
  } while (0)

#endif /* CAIRO_MUTEX_IMPL_INIT */

#ifdef CAIRO_MUTEX_IMPL_FINI

/* If %CAIRO_MUTEX_IMPL_FINI is defined, we may need to finalize all
 * static mutex'es. */
# ifndef CAIRO_MUTEX_IMPL_FINALIZE
#  define CAIRO_MUTEX_IMPL_FINALIZE() do {	\
       if (_cairo_mutex_initialized)	\
           _cairo_mutex_finalize ();	\
    } while(0)

/* and make sure we implement the above */
#  define _CAIRO_MUTEX_IMPL_USE_STATIC_FINALIZER 1
# endif /* CAIRO_MUTEX_IMPL_FINALIZE */

#else /* no CAIRO_MUTEX_IMPL_FINI */

/* Otherwise we probably don't need to finalize static mutex'es, */
# ifndef CAIRO_MUTEX_IMPL_FINALIZE
#  define CAIRO_MUTEX_IMPL_FINALIZE() CAIRO_MUTEX_IMPL_NOOP
# endif /* CAIRO_MUTEX_IMPL_FINALIZE */

/* neither do the dynamic ones. */
# define CAIRO_MUTEX_IMPL_FINI(mutex)	CAIRO_MUTEX_IMPL_NOOP1(mutex)

#endif /* CAIRO_MUTEX_IMPL_FINI */


#ifndef _CAIRO_MUTEX_IMPL_USE_STATIC_INITIALIZER
#define _CAIRO_MUTEX_IMPL_USE_STATIC_INITIALIZER 0
#endif
#ifndef _CAIRO_MUTEX_IMPL_USE_STATIC_FINALIZER
#define _CAIRO_MUTEX_IMPL_USE_STATIC_FINALIZER 0
#endif


/* Make sure everything we want is defined */
#ifndef CAIRO_MUTEX_IMPL_INITIALIZE
# error "CAIRO_MUTEX_IMPL_INITIALIZE not defined"
#endif
#ifndef CAIRO_MUTEX_IMPL_FINALIZE
# error "CAIRO_MUTEX_IMPL_FINALIZE not defined"
#endif
#ifndef CAIRO_MUTEX_IMPL_LOCK
# error "CAIRO_MUTEX_IMPL_LOCK not defined"
#endif
#ifndef CAIRO_MUTEX_IMPL_TRY_LOCK
# error "CAIRO_MUTEX_IMPL_TRY_LOCK not defined"
#endif
#ifndef CAIRO_MUTEX_IMPL_UNLOCK
# error "CAIRO_MUTEX_IMPL_UNLOCK not defined"
#endif
#ifndef CAIRO_MUTEX_IMPL_INIT
# error "CAIRO_MUTEX_IMPL_INIT not defined"
#endif
#ifndef CAIRO_MUTEX_IMPL_FINI
# error "CAIRO_MUTEX_IMPL_FINI not defined"
#endif
#ifndef CAIRO_MUTEX_IMPL_NIL_INITIALIZER
# error "CAIRO_MUTEX_IMPL_NIL_INITIALIZER not defined"
#endif


/* Public interface. */

/* By default it simply uses the implementation provided.
 * But we can provide for debugging features by overriding them */

#ifndef CAIRO_MUTEX_DEBUG
typedef cairo_mutex_impl_t cairo_mutex_t;
typedef cairo_recursive_mutex_impl_t cairo_recursive_mutex_t;
#else
# define cairo_mutex_t			cairo_mutex_impl_t
#endif

#define CAIRO_MUTEX_INITIALIZE		CAIRO_MUTEX_IMPL_INITIALIZE
#define CAIRO_MUTEX_FINALIZE		CAIRO_MUTEX_IMPL_FINALIZE
#define CAIRO_MUTEX_LOCK		CAIRO_MUTEX_IMPL_LOCK
#define CAIRO_MUTEX_TRY_LOCK		CAIRO_MUTEX_IMPL_TRY_LOCK
#define CAIRO_MUTEX_UNLOCK		CAIRO_MUTEX_IMPL_UNLOCK
#define CAIRO_MUTEX_INIT		CAIRO_MUTEX_IMPL_INIT
#define CAIRO_MUTEX_FINI		CAIRO_MUTEX_IMPL_FINI
#define CAIRO_MUTEX_NIL_INITIALIZER	CAIRO_MUTEX_IMPL_NIL_INITIALIZER

#define CAIRO_RECURSIVE_MUTEX_INIT		CAIRO_RECURSIVE_MUTEX_IMPL_INIT
#define CAIRO_RECURSIVE_MUTEX_NIL_INITIALIZER	CAIRO_RECURSIVE_MUTEX_IMPL_NIL_INITIALIZER

#ifndef CAIRO_MUTEX_IS_LOCKED
# define CAIRO_MUTEX_IS_LOCKED(name) 1
#endif
#ifndef CAIRO_MUTEX_IS_UNLOCKED
# define CAIRO_MUTEX_IS_UNLOCKED(name) 1
#endif


/* Debugging support */

#ifdef CAIRO_MUTEX_DEBUG

/* TODO add mutex debugging facilities here (eg deadlock detection) */

#endif /* CAIRO_MUTEX_DEBUG */

#endif
