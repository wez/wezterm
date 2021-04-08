// Copyright 2013 The Servo Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]

use libc::*;

pub type FcChar8 = c_uchar;
pub type FcChar16 = c_ushort;
pub type FcChar32 = c_uint;
pub type FcBool = c_int;

pub type enum__FcType = c_uint;
pub const FcTypeVoid: u32 = 0_u32;
pub const FcTypeInteger: u32 = 1_u32;
pub const FcTypeDouble: u32 = 2_u32;
pub const FcTypeString: u32 = 3_u32;
pub const FcTypeBool: u32 = 4_u32;
pub const FcTypeMatrix: u32 = 5_u32;
pub const FcTypeCharSet: u32 = 6_u32;
pub const FcTypeFTFace: u32 = 7_u32;
pub const FcTypeLangSet: u32 = 8_u32;

pub type FcType = enum__FcType;

pub const FC_WEIGHT_THIN: c_int = 0;
pub const FC_WEIGHT_EXTRALIGHT: c_int = 40;
pub const FC_WEIGHT_ULTRALIGHT: c_int = FC_WEIGHT_EXTRALIGHT;
pub const FC_WEIGHT_LIGHT: c_int = 50;
pub const FC_WEIGHT_BOOK: c_int = 75;
pub const FC_WEIGHT_REGULAR: c_int = 80;
pub const FC_WEIGHT_NORMAL: c_int = FC_WEIGHT_REGULAR;
pub const FC_WEIGHT_MEDIUM: c_int = 100;
pub const FC_WEIGHT_DEMIBOLD: c_int = 180;
pub const FC_WEIGHT_SEMIBOLD: c_int = FC_WEIGHT_DEMIBOLD;
pub const FC_WEIGHT_BOLD: c_int = 200;
pub const FC_WEIGHT_EXTRABOLD: c_int = 205;
pub const FC_WEIGHT_ULTRABOLD: c_int = FC_WEIGHT_EXTRABOLD;
pub const FC_WEIGHT_BLACK: c_int = 210;
pub const FC_WEIGHT_HEAVY: c_int = FC_WEIGHT_BLACK;
pub const FC_WEIGHT_EXTRABLACK: c_int = 215;
pub const FC_WEIGHT_ULTRABLACK: c_int = FC_WEIGHT_EXTRABLACK;

pub const FC_SLANT_ROMAN: c_int = 0;
pub const FC_SLANT_ITALIC: c_int = 100;
pub const FC_SLANT_OBLIQUE: c_int = 110;

pub const FC_WIDTH_ULTRACONDENSED: c_int = 50;
pub const FC_WIDTH_EXTRACONDENSED: c_int = 63;
pub const FC_WIDTH_CONDENSED: c_int = 75;
pub const FC_WIDTH_SEMICONDENSED: c_int = 87;
pub const FC_WIDTH_NORMAL: c_int = 100;
pub const FC_WIDTH_SEMIEXPANDED: c_int = 113;
pub const FC_WIDTH_EXPANDED: c_int = 125;
pub const FC_WIDTH_EXTRAEXPANDED: c_int = 150;
pub const FC_WIDTH_ULTRAEXPANDED: c_int = 200;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct struct__FcMatrix {
    pub xx: c_double,
    pub xy: c_double,
    pub yx: c_double,
    pub yy: c_double,
}

pub type FcMatrix = struct__FcMatrix;

pub type struct__FcCharSet = c_void;

pub type FcCharSet = struct__FcCharSet;

#[repr(C)]
#[allow(missing_copy_implementations)]
pub struct struct__FcObjectType {
    pub object: *mut c_char,
    pub _type: FcType,
}

pub type FcObjectType = struct__FcObjectType;

#[repr(C)]
#[allow(missing_copy_implementations)]
pub struct struct__FcConstant {
    pub name: *mut FcChar8,
    pub object: *mut c_char,
    pub value: c_int,
}

pub type FcConstant = struct__FcConstant;

pub type enum__FcResult = c_uint;
pub const FcResultMatch: u32 = 0_u32;
pub const FcResultNoMatch: u32 = 1_u32;
pub const FcResultTypeMismatch: u32 = 2_u32;
pub const FcResultNoId: u32 = 3_u32;
pub const FcResultOutOfMemory: u32 = 4_u32;

pub type FcResult = enum__FcResult;

pub type struct__FcPattern = c_void;

pub type FcPattern = struct__FcPattern;

pub type struct__FcLangSet = c_void;

pub type FcLangSet = struct__FcLangSet;

#[repr(C)]
#[allow(missing_copy_implementations)]
pub struct struct__FcValue {
    pub _type: FcType,
    pub u: union_unnamed1,
}

pub type FcValue = struct__FcValue;

#[repr(C)]
#[allow(missing_copy_implementations)]
pub struct struct__FcFontSet {
    pub nfont: c_int,
    pub sfont: c_int,
    pub fonts: *mut *mut FcPattern,
}

pub type FcFontSet = struct__FcFontSet;

#[repr(C)]
#[allow(missing_copy_implementations)]
pub struct struct__FcObjectSet {
    pub nobject: c_int,
    pub sobject: c_int,
    pub objects: *mut *mut c_char,
}

pub type FcObjectSet = struct__FcObjectSet;

pub type enum__FcMatchKind = c_uint;
pub const FcMatchPattern: u32 = 0_u32;
pub const FcMatchFont: u32 = 1_u32;
pub const FcMatchScan: u32 = 2_u32;

pub type FcMatchKind = enum__FcMatchKind;

pub type enum__FcLangResult = c_uint;
pub const FcLangEqual: u32 = 0_u32;
pub const FcLangDifferentCountry: u32 = 1_u32;
pub const FcLangDifferentTerritory: u32 = 1_u32;
pub const FcLangDifferentLang: u32 = 2_u32;

pub type FcLangResult = enum__FcLangResult;

pub type enum__FcSetName = c_uint;
pub const FcSetSystem: u32 = 0_u32;
pub const FcSetApplication: u32 = 1_u32;

pub type FcSetName = enum__FcSetName;

pub type struct__FcAtomic = c_void;

pub type FcAtomic = struct__FcAtomic;

pub type FcEndian = c_uint;
pub const FcEndianBig: u32 = 0_u32;
pub const FcEndianLittle: u32 = 1_u32;

pub type struct__FcConfig = c_void;

pub type FcConfig = struct__FcConfig;

pub type struct__FcGlobalCache = c_void;

pub type FcFileCache = struct__FcGlobalCache;

pub type struct__FcBlanks = c_void;

pub type FcBlanks = struct__FcBlanks;

pub type struct__FcStrList = c_void;

pub type FcStrList = struct__FcStrList;

pub type struct__FcStrSet = c_void;

pub type FcStrSet = struct__FcStrSet;

pub type struct__FcCache = c_void;

pub type FcCache = struct__FcCache;

pub type union_unnamed1 = c_void;

extern "C" {

    pub fn FcBlanksCreate() -> *mut FcBlanks;

    pub fn FcBlanksDestroy(b: *mut FcBlanks);

    pub fn FcBlanksAdd(b: *mut FcBlanks, ucs4: FcChar32) -> FcBool;

    pub fn FcBlanksIsMember(b: *mut FcBlanks, ucs4: FcChar32) -> FcBool;

    pub fn FcCacheDir(c: *mut FcCache) -> *const FcChar8;

    pub fn FcCacheCopySet(c: *const FcCache) -> *mut FcFontSet;

    pub fn FcCacheSubdir(c: *const FcCache, i: c_int) -> *const FcChar8;

    pub fn FcCacheNumSubdir(c: *const FcCache) -> c_int;

    pub fn FcCacheNumFont(c: *const FcCache) -> c_int;

    pub fn FcDirCacheUnlink(dir: *const FcChar8, config: *mut FcConfig) -> FcBool;

    pub fn FcDirCacheValid(cache_file: *const FcChar8) -> FcBool;

    pub fn FcConfigHome() -> *mut FcChar8;

    pub fn FcConfigEnableHome(enable: FcBool) -> FcBool;

    pub fn FcConfigFilename(url: *const FcChar8) -> *mut FcChar8;

    pub fn FcConfigCreate() -> *mut FcConfig;

    pub fn FcConfigReference(config: *mut FcConfig) -> *mut FcConfig;

    pub fn FcConfigDestroy(config: *mut FcConfig);

    pub fn FcConfigSetCurrent(config: *mut FcConfig) -> FcBool;

    pub fn FcConfigGetCurrent() -> *mut FcConfig;

    pub fn FcConfigUptoDate(config: *mut FcConfig) -> FcBool;

    pub fn FcConfigBuildFonts(config: *mut FcConfig) -> FcBool;

    pub fn FcConfigGetFontDirs(config: *mut FcConfig) -> *mut FcStrList;

    pub fn FcConfigGetConfigDirs(config: *mut FcConfig) -> *mut FcStrList;

    pub fn FcConfigGetConfigFiles(config: *mut FcConfig) -> *mut FcStrList;

    pub fn FcConfigGetCache(config: *mut FcConfig) -> *mut FcChar8;

    pub fn FcConfigGetBlanks(config: *mut FcConfig) -> *mut FcBlanks;

    pub fn FcConfigGetCacheDirs(config: *const FcConfig) -> *mut FcStrList;

    pub fn FcConfigGetRescanInterval(config: *mut FcConfig) -> c_int;

    pub fn FcConfigSetRescanInterval(config: *mut FcConfig, rescanInterval: c_int) -> FcBool;

    pub fn FcConfigGetFonts(config: *mut FcConfig, set: FcSetName) -> *mut FcFontSet;

    pub fn FcConfigAppFontAddFile(config: *mut FcConfig, file: *const FcChar8) -> FcBool;

    pub fn FcConfigAppFontAddDir(config: *mut FcConfig, dir: *const FcChar8) -> FcBool;

    pub fn FcConfigAppFontClear(config: *mut FcConfig);

    pub fn FcConfigSubstituteWithPat(
        config: *mut FcConfig,
        p: *mut FcPattern,
        p_pat: *mut FcPattern,
        kind: FcMatchKind,
    ) -> FcBool;

    pub fn FcConfigSubstitute(
        config: *mut FcConfig,
        p: *mut FcPattern,
        kind: FcMatchKind,
    ) -> FcBool;

    pub fn FcCharSetCreate() -> *mut FcCharSet;

    pub fn FcCharSetNew() -> *mut FcCharSet;

    pub fn FcCharSetDestroy(fcs: *mut FcCharSet);

    pub fn FcCharSetAddChar(fcs: *mut FcCharSet, ucs4: FcChar32) -> FcBool;

    pub fn FcCharSetCopy(src: *mut FcCharSet) -> *mut FcCharSet;

    pub fn FcCharSetEqual(a: *const FcCharSet, b: *const FcCharSet) -> FcBool;

    pub fn FcCharSetIntersect(a: *const FcCharSet, b: *const FcCharSet) -> *mut FcCharSet;

    pub fn FcCharSetUnion(a: *const FcCharSet, b: *const FcCharSet) -> *mut FcCharSet;

    pub fn FcCharSetSubtract(a: *const FcCharSet, b: *const FcCharSet) -> *mut FcCharSet;

    pub fn FcCharSetMerge(a: *mut FcCharSet, b: *const FcCharSet, changed: *mut FcBool) -> FcBool;

    pub fn FcCharSetHasChar(fcs: *const FcCharSet, ucs4: FcChar32) -> FcBool;

    pub fn FcCharSetCount(a: *const FcCharSet) -> FcChar32;

    pub fn FcCharSetIntersectCount(a: *const FcCharSet, b: *const FcCharSet) -> FcChar32;

    pub fn FcCharSetSubtractCount(a: *const FcCharSet, b: *const FcCharSet) -> FcChar32;

    pub fn FcCharSetIsSubset(a: *const FcCharSet, bi: *const FcCharSet) -> FcBool;

    pub fn FcCharSetFirstPage(
        a: *const FcCharSet,
        map: *mut FcChar32,
        next: *mut FcChar32,
    ) -> FcChar32;

    pub fn FcCharSetNextPage(
        a: *const FcCharSet,
        map: *mut FcChar32,
        next: *mut FcChar32,
    ) -> FcChar32;

    pub fn FcCharSetCoverage(
        a: *const FcCharSet,
        page: FcChar32,
        result: *mut FcChar32,
    ) -> FcChar32;

    pub fn FcValuePrint(v: FcValue);

    pub fn FcPatternPrint(p: *const FcPattern);

    pub fn FcFontSetPrint(s: *mut FcFontSet);

    pub fn FcDefaultSubstitute(pattern: *mut FcPattern);

    pub fn FcFileIsDir(file: *const FcChar8) -> FcBool;

    pub fn FcFileScan(
        set: *mut FcFontSet,
        dirs: *mut FcStrSet,
        cache: *mut FcFileCache,
        blanks: *mut FcBlanks,
        file: *const FcChar8,
        force: FcBool,
    ) -> FcBool;

    pub fn FcDirScan(
        set: *mut FcFontSet,
        dirs: *mut FcStrSet,
        cache: *mut FcFileCache,
        blanks: *mut FcBlanks,
        dir: *const FcChar8,
        force: FcBool,
    ) -> FcBool;

    pub fn FcDirSave(set: *mut FcFontSet, dirs: *const FcStrSet, dir: *mut FcChar8) -> FcBool;

    pub fn FcDirCacheLoad(
        dir: *const FcChar8,
        config: *mut FcConfig,
        cache_file: *mut *mut FcChar8,
    ) -> *mut FcCache;

    pub fn FcDirCacheRead(
        dir: *const FcChar8,
        force: FcBool,
        config: *mut FcConfig,
    ) -> *mut FcCache;

    //pub fn FcDirCacheLoadFile(cache_file: *mut FcChar8, file_stat: *mut struct_stat) -> *mut FcCache;

    pub fn FcDirCacheUnload(cache: *mut FcCache);

    pub fn FcFreeTypeQuery(
        file: *const FcChar8,
        id: c_int,
        blanks: *mut FcBlanks,
        count: *mut c_int,
    ) -> *mut FcPattern;

    pub fn FcFontSetCreate() -> *mut FcFontSet;

    pub fn FcFontSetDestroy(s: *mut FcFontSet);

    pub fn FcFontSetAdd(s: *mut FcFontSet, font: *mut FcPattern) -> FcBool;

    pub fn FcInitLoadConfig() -> *mut FcConfig;

    pub fn FcInitLoadConfigAndFonts() -> *mut FcConfig;

    pub fn FcInit() -> FcBool;

    pub fn FcFini();

    pub fn FcGetVersion() -> c_int;

    pub fn FcInitReinitialize() -> FcBool;

    pub fn FcInitBringUptoDate() -> FcBool;

    pub fn FcGetLangs() -> *mut FcStrSet;

    pub fn FcLangGetCharSet(lang: *const FcChar8) -> *mut FcCharSet;

    pub fn FcLangSetCreate() -> *mut FcLangSet;

    pub fn FcLangSetDestroy(ls: *mut FcLangSet);

    pub fn FcLangSetCopy(ls: *const FcLangSet) -> *mut FcLangSet;

    pub fn FcLangSetAdd(ls: *mut FcLangSet, lang: *const FcChar8) -> FcBool;

    pub fn FcLangSetHasLang(ls: *const FcLangSet, lang: *const FcChar8) -> FcLangResult;

    pub fn FcLangSetCompare(lsa: *const FcLangSet, lsb: *const FcLangSet) -> FcLangResult;

    pub fn FcLangSetContains(lsa: *const FcLangSet, lsb: *const FcLangSet) -> FcBool;

    pub fn FcLangSetEqual(lsa: *const FcLangSet, lsb: *const FcLangSet) -> FcBool;

    pub fn FcLangSetHash(ls: *const FcLangSet) -> FcChar32;

    pub fn FcLangSetGetLangs(ls: *const FcLangSet) -> *mut FcStrSet;

    pub fn FcObjectSetCreate() -> *mut FcObjectSet;

    pub fn FcObjectSetAdd(os: *mut FcObjectSet, object: *const c_char) -> FcBool;

    pub fn FcObjectSetDestroy(os: *mut FcObjectSet);

    //pub fn FcObjectSetVaBuild(first: *mut c_char, va: *mut __va_list_tag) -> *mut FcObjectSet;

    pub fn FcObjectSetBuild(first: *mut c_char, ...) -> *mut FcObjectSet;

    pub fn FcFontSetList(
        config: *mut FcConfig,
        sets: *mut *mut FcFontSet,
        nsets: c_int,
        p: *mut FcPattern,
        os: *mut FcObjectSet,
    ) -> *mut FcFontSet;

    pub fn FcFontList(
        config: *mut FcConfig,
        p: *mut FcPattern,
        os: *mut FcObjectSet,
    ) -> *mut FcFontSet;

    pub fn FcAtomicCreate(file: *const FcChar8) -> *mut FcAtomic;

    pub fn FcAtomicLock(atomic: *mut FcAtomic) -> FcBool;

    pub fn FcAtomicNewFile(atomic: *mut FcAtomic) -> *mut FcChar8;

    pub fn FcAtomicOrigFile(atomic: *mut FcAtomic) -> *mut FcChar8;

    pub fn FcAtomicReplaceOrig(atomic: *mut FcAtomic) -> FcBool;

    pub fn FcAtomicDeleteNew(atomic: *mut FcAtomic);

    pub fn FcAtomicUnlock(atomic: *mut FcAtomic);

    pub fn FcAtomicDestroy(atomic: *mut FcAtomic);

    pub fn FcFontSetMatch(
        config: *mut FcConfig,
        sets: *mut *mut FcFontSet,
        nsets: c_int,
        p: *mut FcPattern,
        result: *mut FcResult,
    ) -> *mut FcPattern;

    pub fn FcFontMatch(
        config: *mut FcConfig,
        p: *mut FcPattern,
        result: *mut FcResult,
    ) -> *mut FcPattern;

    pub fn FcFontRenderPrepare(
        config: *mut FcConfig,
        pat: *mut FcPattern,
        font: *mut FcPattern,
    ) -> *mut FcPattern;

    pub fn FcFontSetSort(
        config: *mut FcConfig,
        sets: *mut *mut FcFontSet,
        nsets: c_int,
        p: *mut FcPattern,
        trim: FcBool,
        csp: *mut *mut FcCharSet,
        result: *mut FcResult,
    ) -> *mut FcFontSet;

    pub fn FcFontSort(
        config: *mut FcConfig,
        p: *mut FcPattern,
        trim: FcBool,
        csp: *mut *mut FcCharSet,
        result: *mut FcResult,
    ) -> *mut FcFontSet;

    pub fn FcFontSetSortDestroy(fs: *mut FcFontSet);

    pub fn FcMatrixCopy(mat: *const FcMatrix) -> *mut FcMatrix;

    pub fn FcMatrixEqual(mat1: *const FcMatrix, mat2: *const FcMatrix) -> FcBool;

    pub fn FcMatrixMultiply(result: *mut FcMatrix, a: *const FcMatrix, b: *const FcMatrix);

    pub fn FcMatrixRotate(m: *mut FcMatrix, c: c_double, s: c_double);

    pub fn FcMatrixScale(m: *mut FcMatrix, sx: c_double, sy: c_double);

    pub fn FcMatrixShear(m: *mut FcMatrix, sh: c_double, sv: c_double);

    pub fn FcNameRegisterObjectTypes(types: *const FcObjectType, ntype: c_int) -> FcBool;

    pub fn FcNameUnregisterObjectTypes(types: *const FcObjectType, ntype: c_int) -> FcBool;

    pub fn FcNameGetObjectType(object: *const c_char) -> *const FcObjectType;

    pub fn FcNameRegisterConstants(consts: *const FcConstant, nconsts: c_int) -> FcBool;

    pub fn FcNameUnregisterConstants(consts: *const FcConstant, nconsts: c_int) -> FcBool;

    pub fn FcNameGetConstant(string: *mut FcChar8) -> *const FcConstant;

    pub fn FcNameConstant(string: *mut FcChar8, result: *mut c_int) -> FcBool;

    pub fn FcNameParse(name: *const FcChar8) -> *mut FcPattern;

    pub fn FcNameUnparse(pat: *mut FcPattern) -> *mut FcChar8;

    pub fn FcPatternCreate() -> *mut FcPattern;

    pub fn FcPatternDuplicate(p: *const FcPattern) -> *mut FcPattern;

    pub fn FcPatternReference(p: *mut FcPattern);

    pub fn FcPatternFilter(p: *mut FcPattern, os: *const FcObjectSet) -> *mut FcPattern;

    pub fn FcValueDestroy(v: FcValue);

    pub fn FcValueEqual(va: FcValue, vb: FcValue) -> FcBool;

    pub fn FcValueSave(v: FcValue) -> FcValue;

    pub fn FcPatternDestroy(p: *mut FcPattern);

    pub fn FcPatternEqual(pa: *const FcPattern, pb: *const FcPattern) -> FcBool;

    pub fn FcPatternEqualSubset(
        pa: *const FcPattern,
        pb: *const FcPattern,
        os: *const FcObjectSet,
    ) -> FcBool;

    pub fn FcPatternHash(p: *const FcPattern) -> FcChar32;

    pub fn FcPatternAdd(
        p: *mut FcPattern,
        object: *const c_char,
        value: FcValue,
        append: FcBool,
    ) -> FcBool;

    pub fn FcPatternAddWeak(
        p: *mut FcPattern,
        object: *const c_char,
        value: FcValue,
        append: FcBool,
    ) -> FcBool;

    pub fn FcPatternGet(
        p: *mut FcPattern,
        object: *const c_char,
        id: c_int,
        v: *mut FcValue,
    ) -> FcResult;

    pub fn FcPatternDel(p: *mut FcPattern, object: *const c_char) -> FcBool;

    pub fn FcPatternRemove(p: *mut FcPattern, object: *const c_char, id: c_int) -> FcBool;

    pub fn FcPatternAddInteger(p: *mut FcPattern, object: *const c_char, i: c_int) -> FcBool;

    pub fn FcPatternAddDouble(p: *mut FcPattern, object: *const c_char, d: c_double) -> FcBool;

    pub fn FcPatternAddString(
        p: *mut FcPattern,
        object: *const c_char,
        s: *const FcChar8,
    ) -> FcBool;

    pub fn FcPatternAddMatrix(
        p: *mut FcPattern,
        object: *const c_char,
        s: *const FcMatrix,
    ) -> FcBool;

    pub fn FcPatternAddCharSet(
        p: *mut FcPattern,
        object: *const c_char,
        c: *const FcCharSet,
    ) -> FcBool;

    pub fn FcPatternAddBool(p: *mut FcPattern, object: *const c_char, b: FcBool) -> FcBool;

    pub fn FcPatternAddLangSet(
        p: *mut FcPattern,
        object: *const c_char,
        ls: *const FcLangSet,
    ) -> FcBool;

    pub fn FcPatternGetInteger(
        p: *mut FcPattern,
        object: *const c_char,
        n: c_int,
        i: *mut c_int,
    ) -> FcResult;

    pub fn FcPatternGetDouble(
        p: *mut FcPattern,
        object: *const c_char,
        n: c_int,
        d: *mut c_double,
    ) -> FcResult;

    pub fn FcPatternGetString(
        p: *mut FcPattern,
        object: *const c_char,
        n: c_int,
        s: *mut *mut FcChar8,
    ) -> FcResult;

    pub fn FcPatternGetMatrix(
        p: *mut FcPattern,
        object: *const c_char,
        n: c_int,
        s: *mut *mut FcMatrix,
    ) -> FcResult;

    pub fn FcPatternGetCharSet(
        p: *mut FcPattern,
        object: *const c_char,
        n: c_int,
        c: *mut *mut FcCharSet,
    ) -> FcResult;

    pub fn FcPatternGetBool(
        p: *mut FcPattern,
        object: *const c_char,
        n: c_int,
        b: *mut FcBool,
    ) -> FcResult;

    pub fn FcPatternGetLangSet(
        p: *mut FcPattern,
        object: *const c_char,
        n: c_int,
        ls: *mut *mut FcLangSet,
    ) -> FcResult;

    //pub fn FcPatternVaBuild(p: *mut FcPattern, va: *mut __va_list_tag) -> *mut FcPattern;

    pub fn FcPatternBuild(p: *mut FcPattern, ...) -> *mut FcPattern;

    pub fn FcPatternFormat(pat: *mut FcPattern, format: *const FcChar8) -> *mut FcChar8;

    pub fn FcStrCopy(s: *const FcChar8) -> *mut FcChar8;

    pub fn FcStrCopyFilename(s: *const FcChar8) -> *mut FcChar8;

    pub fn FcStrPlus(s1: *const FcChar8, s2: *const FcChar8) -> *mut FcChar8;

    pub fn FcStrFree(s: *mut FcChar8);

    pub fn FcStrDowncase(s: *const FcChar8) -> *mut FcChar8;

    pub fn FcStrCmpIgnoreCase(s1: *const FcChar8, s2: *const FcChar8) -> c_int;

    pub fn FcStrCmp(s1: *const FcChar8, s2: *const FcChar8) -> c_int;

    pub fn FcStrStrIgnoreCase(s1: *const FcChar8, s2: *const FcChar8) -> *mut FcChar8;

    pub fn FcStrStr(s1: *const FcChar8, s2: *const FcChar8) -> *mut FcChar8;

    pub fn FcUtf8ToUcs4(src_orig: *mut FcChar8, dst: *mut FcChar32, len: c_int) -> c_int;

    pub fn FcUtf8Len(
        string: *mut FcChar8,
        len: c_int,
        nchar: *mut c_int,
        wchar: *mut c_int,
    ) -> FcBool;

    pub fn FcUcs4ToUtf8(ucs4: FcChar32, dest: *mut FcChar8) -> c_int;

    pub fn FcUtf16ToUcs4(
        src_orig: *mut FcChar8,
        endian: FcEndian,
        dst: *mut FcChar32,
        len: c_int,
    ) -> c_int;

    pub fn FcUtf16Len(
        string: *mut FcChar8,
        endian: FcEndian,
        len: c_int,
        nchar: *mut c_int,
        wchar: *mut c_int,
    ) -> FcBool;

    pub fn FcStrDirname(file: *const FcChar8) -> *mut FcChar8;

    pub fn FcStrBasename(file: *const FcChar8) -> *mut FcChar8;

    pub fn FcStrSetCreate() -> *mut FcStrSet;

    pub fn FcStrSetMember(set: *mut FcStrSet, s: *const FcChar8) -> FcBool;

    pub fn FcStrSetEqual(sa: *mut FcStrSet, sb: *mut FcStrSet) -> FcBool;

    pub fn FcStrSetAdd(set: *mut FcStrSet, s: *const FcChar8) -> FcBool;

    pub fn FcStrSetAddFilename(set: *mut FcStrSet, s: *const FcChar8) -> FcBool;

    pub fn FcStrSetDel(set: *mut FcStrSet, s: *const FcChar8) -> FcBool;

    pub fn FcStrSetDestroy(set: *mut FcStrSet);

    pub fn FcStrListCreate(set: *mut FcStrSet) -> *mut FcStrList;

    pub fn FcStrListNext(list: *mut FcStrList) -> *mut FcChar8;

    pub fn FcStrListDone(list: *mut FcStrList);

    pub fn FcConfigParseAndLoad(
        config: *mut FcConfig,
        file: *const FcChar8,
        complain: FcBool,
    ) -> FcBool;

}
