/* -*- Mode: c; c-basic-offset: 4; indent-tabs-mode: t; tab-width: 8; -*- */
/*
 * Copyright © 2007 Red Hat, Inc.
 *
 * Permission to use, copy, modify, distribute, and sell this software
 * and its documentation for any purpose is hereby granted without
 * fee, provided that the above copyright notice appear in all copies
 * and that both that copyright notice and this permission notice
 * appear in supporting documentation, and that the name of
 * Red Hat, Inc. not be used in advertising or publicity pertaining to
 * distribution of the software without specific, written prior
 * permission. Red Hat, Inc. makes no representations about the
 * suitability of this software for any purpose.  It is provided "as
 * is" without express or implied warranty.
 *
 * RED HAT, INC. DISCLAIMS ALL WARRANTIES WITH REGARD TO THIS
 * SOFTWARE, INCLUDING ALL IMPLIED WARRANTIES OF MERCHANTABILITY AND
 * FITNESS, IN NO EVENT SHALL RED HAT, INC. BE LIABLE FOR ANY SPECIAL,
 * INDIRECT OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES WHATSOEVER
 * RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN ACTION
 * OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF OR
 * IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.
 *
 * Author: Behdad Esfahbod <behdad@behdad.org>
 */

/* A simple malloc wrapper that prints out statistics on termination */

#ifndef _GNU_SOURCE
#define _GNU_SOURCE
#endif

#include <stdlib.h>
#include <stdio.h>
#include <stdint.h>

/* caller-logging */

#include <string.h>

struct alloc_stat_t {
	unsigned int num;
	unsigned long long size;
};

struct alloc_stats_t {
	struct alloc_stat_t malloc, realloc, total;
};

struct func_stat_t {
	struct func_stat_t *next;

	const void *addr;
	const char *name;

	struct alloc_stats_t stat;
};

static struct alloc_stats_t total_allocations;
static struct func_stat_t *func_stats[31627];
static int func_stats_num;

#ifndef ARRAY_LENGTH
#define ARRAY_LENGTH(__array) ((int) (sizeof (__array) / sizeof (__array[0])))
#endif
static void
alloc_stats_add (struct alloc_stats_t *stats, int is_realloc, size_t size)
{
	struct alloc_stat_t *stat = is_realloc ? &stats->realloc : &stats->malloc;

	stats->total.num++;
	stats->total.size += size;

	stat->num++;
	stat->size += size;
}

#include <execinfo.h>

static void *
_perm_alloc (size_t size)
{
    static uint8_t *ptr;
    static size_t rem;

    void *ret;

#define SUPERBLOCK_SIZE (1<<23)
#define align(x, y) (((x) + ((y)-1)) & ~((y)-1))

    size = align (size, 2 * sizeof (void *));
    if (size > rem || rem == 0) {
	ptr = malloc (SUPERBLOCK_SIZE);
	if (ptr == NULL)
	    exit (1);
	rem = SUPERBLOCK_SIZE;
    }

#undef SUPERBLOCK_SIZE
#undef align

    ret = ptr;
    rem -= size;
    ptr += size;

    return ret;
}

static void
resolve_addrs (struct func_stat_t *func_stats, int num)
{
	int i;
	void **addrs;
	char **strings;

	addrs = malloc (num * sizeof (void *));
	for (i = 0; i < num; i++)
		addrs[i] = (void *) func_stats[i].addr;

	strings = backtrace_symbols (addrs, num);

	for (i = 0; i < num; i++) {
		char *p;
		char *name;
		int len;

		p = strchr (strings[i], '\t');
		if (p)
			p++;
		else
			p = strings[i];

		len = strlen (p) + 1;
		name = _perm_alloc (len);
		memcpy (name, p, len);
		func_stats[i].name = name;
	}

	free (strings);
	free (addrs);
}

static void
func_stats_add (const void *caller, int is_realloc, size_t size)
{
	int i;
	struct func_stat_t *elt;

	alloc_stats_add (&total_allocations, is_realloc, size);

	i = ((uintptr_t) caller ^ 1215497) % ARRAY_LENGTH (func_stats);
	for (elt = func_stats[i]; elt != NULL; elt = elt->next) {
		if (elt->addr == caller)
			break;
	}

	if (elt == NULL) {
		func_stats_num++;

		elt = _perm_alloc (sizeof (struct func_stat_t));
		elt->next = func_stats[i];
		func_stats[i] = elt;
		elt->addr = caller;
		elt->name = NULL;
		memset (&elt->stat, 0, sizeof (struct alloc_stats_t));
	}

	alloc_stats_add (&elt->stat, is_realloc, size);
}

/* wrapper stuff */

#include <dlfcn.h>

static void *(*old_malloc)(size_t);
static void *(*old_calloc)(size_t, size_t);
static void *(*old_realloc)(void *, size_t);
static int enable_hook = 0;

static void init(void);

void *
malloc(size_t size)
{
    if (!old_malloc)
      init ();

    if (enable_hook) {
	enable_hook = 0;
	void *caller = __builtin_return_address(0);
	func_stats_add (caller, 0, size);
	enable_hook = 1;
    }

    return old_malloc (size);
}

void *
calloc(size_t nmemb, size_t size)
{
    if (!old_calloc)
      init ();

    if (enable_hook) {
	enable_hook = 0;
	void *caller = __builtin_return_address(0);
	func_stats_add (caller, 0, nmemb * size);
	enable_hook = 1;
    }

    return old_calloc (nmemb, size);
}

void *
realloc(void *ptr, size_t size)
{
    if (!old_malloc)
      init ();

    if (enable_hook) {
	enable_hook = 0;
	void *caller = __builtin_return_address(0);
	func_stats_add (caller, 1, size);
	enable_hook = 1;
    }

    return old_realloc (ptr, size);
}

static void
init(void)
{
    old_malloc = dlsym(RTLD_NEXT, "malloc");
    if (!old_malloc) {
	fprintf(stderr, "%s\n", dlerror());
	exit(1);
    }
    old_calloc = dlsym(RTLD_NEXT, "calloc");
    if (!old_calloc) {
	fprintf(stderr, "%s\n", dlerror());
	exit(1);
    }
    old_realloc = dlsym(RTLD_NEXT, "realloc");
    if (!old_realloc) {
	fprintf(stderr, "%s\n", dlerror());
	exit(1);
    }
    enable_hook = 1;
}

/* reporting */

#include <locale.h>

static void
add_alloc_stats (struct alloc_stats_t *a, struct alloc_stats_t *b)
{
	a->total.num += b->total.num;
	a->total.size += b->total.size;
	a->malloc.num += b->malloc.num;
	a->malloc.size += b->malloc.size;
	a->realloc.num += b->realloc.num;
	a->realloc.size += b->realloc.size;
}

static void
dump_alloc_stats (struct alloc_stats_t *stats, const char *name)
{
	printf ("%8u %'11llu %8u %'11llu %8u %'11llu %s\n",
		stats->total.num, stats->total.size,
		stats->malloc.num, stats->malloc.size,
		stats->realloc.num, stats->realloc.size,
		name);
}

static int
compare_func_stats_name (const void *pa, const void *pb)
{
	const struct func_stat_t *a = pa, *b = pb;
	int i;

	i = strcmp (a->name, b->name);
	if (i)
		return i;

	return ((char *) a->addr - (char *) b->addr);
}

static int
compare_func_stats (const void *pa, const void *pb)
{
	const struct func_stat_t *a = pa, *b = pb;

	if (a->stat.total.num != b->stat.total.num)
		return (a->stat.total.num - b->stat.total.num);

	if (a->stat.total.size != b->stat.total.size)
		return (a->stat.total.size - b->stat.total.size);

	return compare_func_stats_name (pa, pb);
}

static int
merge_similar_entries (struct func_stat_t *func_stats, int num)
{
	int i, j;

	j = 0;
	for (i = 1; i < num; i++) {
		if (i != j && 0 == strcmp (func_stats[i].name, func_stats[j].name)) {
			add_alloc_stats (&func_stats[j].stat, &func_stats[i].stat);
		} else {
			j++;
			if (i != j)
				func_stats[j] = func_stats[i];
		}
	}
	j++;

	return j;
}

__attribute__ ((destructor))
static void
malloc_stats (void)
{
	unsigned int i, j;
	struct func_stat_t *sorted_func_stats;

	enable_hook = 0;

	if (! func_stats_num)
		return;

	sorted_func_stats = malloc (sizeof (struct func_stat_t) * (func_stats_num + 1));
	if (sorted_func_stats == NULL)
		return;

	j = 0;
	for (i = 0; i < ARRAY_LENGTH (func_stats); i++) {
		struct func_stat_t *elt;
		for (elt = func_stats[i]; elt != NULL; elt = elt->next)
			sorted_func_stats[j++] = *elt;
	}

	resolve_addrs (sorted_func_stats, j);

	/* merge entries with same name */
	qsort (sorted_func_stats, j,
	       sizeof (struct func_stat_t), compare_func_stats_name);
	j = merge_similar_entries (sorted_func_stats, j);

	qsort (sorted_func_stats, j,
	       sizeof (struct func_stat_t), compare_func_stats);

	/* add total */
	sorted_func_stats[j].next = NULL;
	sorted_func_stats[j].addr = (void *) -1;
	sorted_func_stats[j].name = "(total)";
	sorted_func_stats[j].stat = total_allocations;
	j++;

	setlocale (LC_ALL, "");

	printf ("          TOTAL                MALLOC              REALLOC\n");
	printf ("     num        size      num        size      num        size\n");

	for (i = 0; i < j; i++) {
		dump_alloc_stats (&sorted_func_stats[i].stat,
				  sorted_func_stats[i].name);
	}

	/* XXX free other stuff? */

	free (sorted_func_stats);
}
