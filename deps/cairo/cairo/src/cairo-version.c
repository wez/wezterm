/* -*- Mode: c; c-basic-offset: 4; indent-tabs-mode: t; tab-width: 8; -*- */
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

#include "cairoint.h"

/**
 * SECTION:cairo-version
 * @Title: Version Information
 * @Short_Description: Compile-time and run-time version checks.
 *
 * Cairo has a three-part version number scheme. In this scheme, we use
 * even vs. odd numbers to distinguish fixed points in the software
 * vs. in-progress development, (such as from git instead of a tar file,
 * or as a "snapshot" tar file as opposed to a "release" tar file).
 *
 * <informalexample><screen>
 *  _____ Major. Always 1, until we invent a new scheme.
 * /  ___ Minor. Even/Odd = Release/Snapshot (tar files) or Branch/Head (git)
 * | /  _ Micro. Even/Odd = Tar-file/git
 * | | /
 * 1.0.0
 * </screen></informalexample>
 *
 * Here are a few examples of versions that one might see.
 * <informalexample><screen>
 * Releases
 * --------
 * 1.0.0 - A major release
 * 1.0.2 - A subsequent maintenance release
 * 1.2.0 - Another major release
 * &nbsp;
 * Snapshots
 * ---------
 * 1.1.2 - A snapshot (working toward the 1.2.0 release)
 * &nbsp;
 * In-progress development (eg. from git)
 * --------------------------------------
 * 1.0.1 - Development on a maintenance branch (toward 1.0.2 release)
 * 1.1.1 - Development on head (toward 1.1.2 snapshot and 1.2.0 release)
 * </screen></informalexample>
 *
 * <refsect2>
 * <title>Compatibility</title>
 *
 * The API/ABI compatibility guarantees for various versions are as
 * follows. First, let's assume some cairo-using application code that is
 * successfully using the API/ABI "from" one version of cairo. Then let's
 * ask the question whether this same code can be moved "to" the API/ABI
 * of another version of cairo.
 *
 * Moving from a release to any later version (release, snapshot,
 * development) is always guaranteed to provide compatibility.
 *
 * Moving from a snapshot to any later version is not guaranteed to
 * provide compatibility, since snapshots may introduce new API that ends
 * up being removed before the next release.
 *
 * Moving from an in-development version (odd micro component) to any
 * later version is not guaranteed to provide compatibility. In fact,
 * there's not even a guarantee that the code will even continue to work
 * with the same in-development version number. This is because these
 * numbers don't correspond to any fixed state of the software, but
 * rather the many states between snapshots and releases.
 *
 * </refsect2>
 * <refsect2>
 * <title>Examining the version</title>
 *
 * Cairo provides the ability to examine the version at either
 * compile-time or run-time and in both a human-readable form as well as
 * an encoded form suitable for direct comparison. Cairo also provides the
 * macro CAIRO_VERSION_ENCODE() to perform the encoding.
 *
 * <informalexample><screen>
 * Compile-time
 * ------------
 * #CAIRO_VERSION_STRING   Human-readable
 * #CAIRO_VERSION          Encoded, suitable for comparison
 * &nbsp;
 * Run-time
 * --------
 * cairo_version_string()  Human-readable
 * cairo_version()         Encoded, suitable for comparison
 * </screen></informalexample>
 *
 * For example, checking that the cairo version is greater than or equal
 * to 1.0.0 could be achieved at compile-time or run-time as follows:
 *
 * <informalexample><programlisting>
 * ##if CAIRO_VERSION >= CAIRO_VERSION_ENCODE(1, 0, 0)
 * printf ("Compiling with suitable cairo version: %s\n", %CAIRO_VERSION_STRING);
 * ##endif
 *
 * if (cairo_version() >= CAIRO_VERSION_ENCODE(1, 0, 0))
 *     printf ("Running with suitable cairo version: %s\n", cairo_version_string ());
 * </programlisting></informalexample>
 *
 * </refsect2>
 *
 **/

/**
 * CAIRO_VERSION:
 *
 * The version of cairo available at compile-time, encoded using
 * CAIRO_VERSION_ENCODE().
 *
 * Since: 1.0
 **/

/**
 * CAIRO_VERSION_MAJOR:
 *
 * The major component of the version of cairo available at compile-time.
 *
 * Since: 1.0
 **/

/**
 * CAIRO_VERSION_MINOR:
 *
 * The minor component of the version of cairo available at compile-time.
 *
 * Since: 1.0
 **/

/**
 * CAIRO_VERSION_MICRO:
 *
 * The micro component of the version of cairo available at compile-time.
 *
 * Since: 1.0
 **/

/**
 * CAIRO_VERSION_STRING:
 *
 * A human-readable string literal containing the version of cairo available
 * at compile-time, in the form of "X.Y.Z".
 *
 * Since: 1.8
 **/

/**
 * CAIRO_VERSION_ENCODE:
 * @major: the major component of the version number
 * @minor: the minor component of the version number
 * @micro: the micro component of the version number
 *
 * This macro encodes the given cairo version into an integer.  The numbers
 * returned by %CAIRO_VERSION and cairo_version() are encoded using this macro.
 * Two encoded version numbers can be compared as integers.  The encoding ensures
 * that later versions compare greater than earlier versions.
 *
 * Returns: the encoded version.
 *
 * Since: 1.0
 **/

/**
 * CAIRO_VERSION_STRINGIZE:
 * @major: the major component of the version number
 * @minor: the minor component of the version number
 * @micro: the micro component of the version number
 *
 * This macro encodes the given cairo version into an string.  The numbers
 * returned by %CAIRO_VERSION_STRING and cairo_version_string() are encoded using this macro.
 * The parameters to this macro must expand to numerical literals.
 *
 * Returns: a string literal containing the version.
 *
 * Since: 1.8
 **/

/**
 * cairo_version:
 *
 * Returns the version of the cairo library encoded in a single
 * integer as per %CAIRO_VERSION_ENCODE. The encoding ensures that
 * later versions compare greater than earlier versions.
 *
 * A run-time comparison to check that cairo's version is greater than
 * or equal to version X.Y.Z could be performed as follows:
 *
 * <informalexample><programlisting>
 * if (cairo_version() >= CAIRO_VERSION_ENCODE(X,Y,Z)) {...}
 * </programlisting></informalexample>
 *
 * See also cairo_version_string() as well as the compile-time
 * equivalents %CAIRO_VERSION and %CAIRO_VERSION_STRING.
 *
 * Return value: the encoded version.
 *
 * Since: 1.0
 **/
int
cairo_version (void)
{
    return CAIRO_VERSION;
}

/**
 * cairo_version_string:
 *
 * Returns the version of the cairo library as a human-readable string
 * of the form "X.Y.Z".
 *
 * See also cairo_version() as well as the compile-time equivalents
 * %CAIRO_VERSION_STRING and %CAIRO_VERSION.
 *
 * Return value: a string containing the version.
 *
 * Since: 1.0
 **/
const char*
cairo_version_string (void)
{
    return CAIRO_VERSION_STRING;
}
slim_hidden_def (cairo_version_string);
