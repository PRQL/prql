# Parser Migration to Chumsky 0.10

## Executive Summary

**Goal:** Migrate PRQL parser from Chumsky 0.9 to 0.10 (lexer already migrated)
**Scope:** ~1,500 lines across 6 parser modules
**Complexity:** High - custom error types, recursive parsers, operator precedence
**Test Coverage:** Excellent - ~30 inline tests + integration tests with snapshots
**Timeline:** 8-12 hours (1-2 focused days)
**Strategy:** All-at-once migration (parser modules exchange combinators, not data - cannot migrate incrementally)
**Safety net:** 101 parser tests + 100+ integration tests with snapshot testing

## Critical Findings from Analysis

### Lexer Migration Patterns (Reference)

The lexer migration (1,148 line diff) provides these key patterns:

1. **Type Setup:**
   ```rust
   use chumsky_0_10 as chumsky;
   use chumsky::extra;

   type ParserError<'a> = extra::Err<Rich<'a, TokenKind>>;
   type ParserInput<'a> = &'a [Token];  // TBD for parser
   ```

2. **All functions need `<'a>` lifetime:**
   ```rust
   fn parser<'a>() -> impl Parser<'a, ParserInput<'a>, Output, ParserError<'a>>
   ```

3. **API Changes:**
   - `parse_recovery()` → `parse().into_result()`
   - `.repeated()` requires `.collect()`
   - `.map_with_span()` → `.map_with()` using `extra.span()`
   - **Critical:** SimpleSpan uses **byte** offsets for `&str` - convert to char offsets

4. **Error Handling:**
   - Manual conversion from `Rich` errors to custom `Error` type
   - Handle at top level, not within parsers

### Parser Architecture

**Module Structure:**
- `perror.rs` (420 lines) - Custom error type implementing 0.9's Error trait
- `mod.rs` (212 lines) - Entry point, stream prep, utilities
- `types.rs` (119 lines) - Type expression parsing
- `interpolation.rs` (151 lines) - String interpolation (char-level)
- `expr.rs` (975 lines) - **Most complex** - expressions with 7-level precedence
- `stmt.rs` (604 lines) - Statement parsing

**Key Patterns:**
- 3 critical recursive parsers (expr, module contents, types)
- Circular dependency: expr ↔ types
- 7-level binary operator precedence using foldl/foldr
- 5 error recovery sites using `nested_delimiters`
- Stream preparation filters comments/line wraps

### Chumsky 0.10 Breaking Changes

1. **Parser trait redesigned:**
   - `Parser<I, O, Error = E>` → `Parser<'src, I, O, E>`
   - Four parameters, lifetime required

2. **Error types:**
   - `error::Simple` → `error::Rich`
   - Use `extra::Err<Rich<'src, T>>`

3. **Methods moved to IterParser:**
   - `foldr()` now on `IterParser` (returned by `.repeated()`)

4. **Removed combinators:**
   - `chain()` - use `.then()` + `.map()`
   - `then_with()` - use context-sensitive combinators
   - `take_until()` - removed

5. **New features:**
   - **Pratt parser** - perfect for operator precedence!
   - Zero-copy parsing
   - Better error recovery with `skip_parser()`
   - Context-sensitive parsing

## Staging Strategy: All-at-Once Migration

**Why incremental DOESN'T work:**
- ❌ Parser modules exchange **parser combinators**, not just data
- ❌ `stmt.rs` imports `expr()`, `ident()`, `pipeline()` - these are Chumsky 0.9 `Parser` traits
- ❌ `types.rs` imports `ident()` from expr.rs - also a parser combinator
- ❌ The `Parser` trait is different between 0.9 and 0.10 - **incompatible across versions**
- ❌ `recursive()` creates circular dependencies within and across modules

**Why lexer COULD migrate independently:**
- ✅ Lexer output is just **data**: `Vec<Token>`
- ✅ Interface to parser is `prepare_stream()` which takes data, returns Chumsky 0.9 Stream
- ✅ **No parser combinators cross the boundary**

**The atomic migration unit:**
All parser modules in one commit (~1,500 lines):
- `perror.rs` (420 lines) - Error type
- `mod.rs` (212 lines) - Utilities & entry point
- `interpolation.rs` (151 lines) - String interpolation
- `types.rs` (119 lines) - Type expressions
- `expr.rs` (975 lines) - Expressions
- `stmt.rs` (604 lines) - Statements

**Migration phases:**
```
Phase 1 (2-3h)  → Research & prepare      [Understand blockers, prep tests]
Phase 2 (4-6h)  → Migrate all modules     [One big commit, no intermediate testing]
Phase 3 (1-2h)  → Fix compilation errors  [Get it to build]
Phase 4 (1-2h)  → Fix & review tests      [Run tests, review snapshots]
Phase 5 (30m)   → Cleanup                 [Remove dual deps]
```

## Test Preparation (DO FIRST)

### Changes We Can Make NOW (Before Migration)

These are compatible with Chumsky 0.9 and prepare for 0.10:

**File: `prqlc/prqlc-parser/src/test.rs`**

- [ ] Add type aliases for future compatibility:
  ```rust
  use chumsky::Parser;
  type ParseResult<T> = Result<T, Vec<Error>>;
  ```

- [ ] Improve documentation on `parse_with_parser`:
  ```rust
  /// Parse source with a specific parser.
  ///
  /// Steps:
  /// 1. Lex the source
  /// 2. Prepare token stream
  /// 3. Run parser with error recovery
  /// 4. Convert parser errors to Error type
  ```

- [ ] Standardize test helper naming (optional)

**File: `prqlc/prqlc-parser/src/parser/test.rs`**

- [ ] Review and document test patterns
- [ ] Add more comprehensive error tests (following lexer patterns)

### Changes That MUST Wait (After Migration)

**File: `prqlc/prqlc-parser/src/test.rs` - Update `parse_with_parser`:**

```rust
// AFTER migration:
pub(crate) fn parse_with_parser<O: Debug>(
    source: &str,
    parser: impl Parser<TokenKind, O, Error = PError>,
) -> Result<O, Vec<Error>> {
    let tokens = crate::lexer::lex_source(source)?;
    let stream = prepare_stream(tokens.0, 0);

    // NEW: Use .output() to extract result
    let parse_result = parser.parse(stream);
    let ast = parse_result.output();
    let parse_errors = parse_result.errors();

    if !parse_errors.is_empty() {
        return Err(parse_errors.into_iter().map(|e| e.into()).collect());
    }
    Ok(ast.unwrap())
}
```

**Key pattern from lexer:** Tests use `.output().unwrap()` instead of direct `.unwrap()`

### Test Files Affected

1. `/prqlc/prqlc-parser/src/test.rs` - Main helpers
2. `/prqlc/prqlc-parser/src/parser/test.rs` - Parser-specific
3. `/prqlc/prqlc-parser/src/parser/expr.rs` - Expr tests (~400 lines)
4. `/prqlc/prqlc-parser/src/parser/stmt.rs` - Stmt tests (~200 lines)
5. `/prqlc/prqlc-parser/src/parser/interpolation.rs` - Interpolation tests

**Minimal changes needed:** Most test structure remains the same; extraction pattern handled in helpers.

## Migration Phases

### Phase 1: Research & Preparation (2-3 hours)

#### 1.1 Research Critical APIs

- [ ] **Error trait API** in Chumsky 0.10:
  - Does `ChumError<T>` need to implement different trait?
  - Check trait bounds and method signatures
  - Pattern from lexer: Custom errors work, converted at top level

- [ ] **Stream API status:**
  - Does `Stream::from_iter()` exist in 0.10?
  - Lexer uses `parse(&str)` directly - tokens likely `parse(&[Token])`
  - Check `Parser::parse()` signature for input types

- [ ] **`recursive()` combinator:**
  - Signature changes in 0.10?
  - Lifetime handling?
  - Look at lexer patterns (though lexer doesn't use recursion)

- [ ] **Error recovery:**
  - Does `nested_delimiters` exist in 0.10?
  - Alternative: `skip_parser()` combinator
  - Check recovery strategies in docs

- [ ] **DON'T research Pratt parser yet** - stick with foldl/foldr for first migration

#### 1.2 Prepare Development Environment

- [ ] Create migration branch:
  ```bash
  git switch -c parser-chumsky-10-migration
  git commit --allow-empty -m "Start parser migration to Chumsky 0.10"
  ```

- [ ] Make test preparation changes (see "Test Preparation" section above)

- [ ] Document findings from research in TODO.md

#### 1.3 Create Migration Checklist

Based on research, create concrete checklist:
- [ ] List all `.map_with_span()` → `.map_with()` changes needed
- [ ] List all `.repeated()` that need `.collect()`
- [ ] Identify `Stream::from_iter()` replacement pattern
- [ ] Document `recursive()` changes needed

### Phase 2: Migrate All Parser Modules (4-6 hours)

**CRITICAL: This is ONE atomic commit - all modules together**

Change imports in ALL files simultaneously:

**Files to update:**
- [ ] `prqlc/prqlc-parser/src/parser/perror.rs`
- [ ] `prqlc/prqlc-parser/src/parser/mod.rs`
- [ ] `prqlc/prqlc-parser/src/parser/expr.rs`
- [ ] `prqlc/prqlc-parser/src/parser/stmt.rs`
- [ ] `prqlc/prqlc-parser/src/parser/types.rs`
- [ ] `prqlc/prqlc-parser/src/parser/interpolation.rs`

**Change:**
```rust
// OLD:
use chumsky::prelude::*;

// NEW:
use chumsky_0_10 as chumsky;
use chumsky::prelude::*;
```

**Expected result:** Everything broken, won't compile yet

#### 2.2 Update `perror.rs` - Error Type (1h)

- [ ] Update `ChumError<T>` to implement Chumsky 0.10's `Error` trait
  - Check trait signature from research
  - Update trait bounds
  - Update method implementations

- [ ] Update `From<PError> for Error` if needed

**Don't test yet** - won't compile until all modules updated

#### 2.3 Update `mod.rs` - Utilities & Stream (1h)

- [ ] Add `<'a>` lifetime to all utility functions:
  - `ident_part()`, `keyword()`, `new_line()`, `ctrl()`
  - `sequence()`, `pipe()`, `with_doc_comment()`

- [ ] **CRITICAL: Update `prepare_stream()`**
  - Based on research: likely parse from slice directly
  - Pattern from lexer: `parse(&str)` → tokens: `parse(&[Token])`
  - Update return type

- [ ] Update `parse_lr_to_pr()` entry point:
  - `.parse_recovery()` → `.parse().into_result()`
  - Match pattern from lexer migration

**Don't test yet** - still won't compile

#### 2.4 Update Simple Parsers (1h)

**`interpolation.rs`:**
- [ ] Add `<'a>` to all functions
- [ ] Update `recursive()` usage
- [ ] Fix `.map_with_span()` → `.map_with()`
- [ ] Apply lexer patterns (char-level parsing)

**`types.rs`:**
- [ ] Add `<'a>` to all functions
- [ ] Update `recursive()` usage
- [ ] Fix `.map_with_span()` → `.map_with()`
- [ ] Add `.collect()` to `.repeated()` where needed

**Don't test yet**

#### 2.5 Update `expr.rs` - The Big One (2-3h)

- [ ] Add `<'a>` to all function signatures (~20 functions)
- [ ] Update main `expr()` recursive parser
- [ ] Fix ALL `.map_with_span()` → `.map_with()` (7 instances)
- [ ] Update operator precedence:
  - **Keep foldl/foldr for now** (Pratt is future optimization)
  - `.foldl()` now on IterParser (after `.repeated()`)
  - Update `.foldr()` transformation
- [ ] Update helper parsers:
  - `tuple()`, `array()`, `case()`, `func_call()`, `lambda_func()`
  - `pipeline()`, `range()`, `interpolation()`
- [ ] Error recovery: Update `nested_delimiters` or replace with `skip_parser()`

**Don't test yet**

#### 2.6 Update `stmt.rs` (1h)

- [ ] Add `<'a>` to all function signatures
- [ ] Update `source()` and `module_contents()` recursive parsers
- [ ] Fix `.map_with_span()` → `.map_with()` if present
- [ ] Update all statement types

**Now it should compile!**

### Phase 3: Fix Compilation Errors (1-2 hours)

- [ ] Run `cargo build -p prqlc-parser` and fix errors iteratively
- [ ] Common issues to expect:
  - Lifetime errors in `recursive()` closures
  - Type mismatches in parser combinators
  - Missing `extra` parameter in `.map_with()`
  - Missing `.collect()` after `.repeated()`
  - Stream API issues

- [ ] Work through errors systematically:
  - Start with perror.rs errors (foundation)
  - Then mod.rs (utilities used everywhere)
  - Then simple parsers (types, interpolation)
  - Finally complex parsers (expr, stmt)

**Don't test until it compiles!**

**Checkpoint:** Once `cargo build -p prqlc-parser` succeeds, commit:
```bash
git add -A
git commit -m "refactor: Migrate parser to Chumsky 0.10 (builds but tests not fixed)"
```

### Phase 4: Fix & Review Tests (1-2 hours)

#### 4.1 Run Tests and Fix Failures

- [ ] Run parser unit tests:
  ```bash
  cargo insta test -p prqlc-parser --accept
  ```
  - Review ALL snapshot changes carefully
  - Check for unexpected behavior changes
  - Fix test-specific issues (e.g., `.output()` extraction)

- [ ] Run integration tests:
  ```bash
  cargo insta test -p prqlc --test integration --accept
  cargo insta test -p prqlc --test error_messages --accept
  cargo insta test -p prqlc --test bad_error_messages --accept
  ```

- [ ] Review snapshot diffs:
  - Error message changes (acceptable if clearer)
  - Span position changes (must be accurate)
  - AST structure changes (should be identical)

#### 4.2 Manual Validation

- [ ] Verify error messages are helpful:
  - Check a few failed parse examples
  - Ensure span positions are correct
  - Error recovery still works

- [ ] Run full test suite:
  ```bash
  task test-all
  ```

**Commit:** ✅ `refactor: Complete parser migration to Chumsky 0.10`

### Phase 5: Cleanup (30 minutes)

#### 5.1 Remove Dual Dependency

- [ ] Update `prqlc/prqlc-parser/Cargo.toml`:
  ```toml
  # Remove both old entries:
  # chumsky = { version = "0.9.2" }
  # chumsky_0_10 = { version = "0.10.1", package = "chumsky" }

  # Replace with single dependency:
  chumsky = { version = "0.10.1" }
  ```

- [ ] Remove `use chumsky_0_10 as chumsky;` from all files:
  - [ ] `prqlc/prqlc-parser/src/lexer/mod.rs`
  - [ ] `prqlc/prqlc-parser/src/parser/perror.rs`
  - [ ] `prqlc/prqlc-parser/src/parser/mod.rs`
  - [ ] `prqlc/prqlc-parser/src/parser/expr.rs`
  - [ ] `prqlc/prqlc-parser/src/parser/stmt.rs`
  - [ ] `prqlc/prqlc-parser/src/parser/types.rs`
  - [ ] `prqlc/prqlc-parser/src/parser/interpolation.rs`

- [ ] Replace with: `use chumsky::prelude::*;` (or specific imports)

#### 5.2 Final Validation

- [ ] Run full test suite:
  ```bash
  task test-all
  ```

- [ ] Run lints:
  ```bash
  task test-lint
  ```

- [ ] Check for any remaining 0.9 references:
  ```bash
  rg "chumsky.*0\.9" --type rust
  rg "chumsky_0_10" --type rust
  ```

#### 5.3 Performance & Documentation

- [ ] Benchmark against baseline (if available)
- [ ] Target: <10% regression, ideally improvement
- [ ] Update CLAUDE.md if needed (migration complete)
- [ ] Add learnings to this TODO.md for future reference

**Final Commit:** ✅ `refactor: Remove Chumsky 0.9 dependency, complete migration`

## High-Risk Areas & Mitigation

### Risk 1: Custom Error Type (PError)
**Issue:** Implements 0.9's Error trait which likely changed significantly

**Mitigation:**
- Research Error trait API in 0.10 first
- Consider using Rich errors directly if trait too different
- Create wrapper type if needed
- Keep integration with existing Error type for upstream

**Contingency:** Simplify to wrapper around Rich instead of trait implementation

### Risk 2: Stream API Changes
**Issue:** `Stream::from_iter()` may not exist or work differently

**Mitigation:**
- Research Stream API in 0.10 docs
- May parse token slice `&[Token]` directly instead
- Consider zero-copy benefits

**Contingency:** Refactor to slice-based parsing, move filtering to parser

### Risk 3: Recursive Parser Lifetimes
**Issue:** Circular dependencies (expr ↔ types) may cause lifetime/borrowing issues

**Mitigation:**
- Use `lazy()` wrapper if needed
- Add explicit type annotations
- Test circular case early

**Contingency:** Break circular dependency (move ident to separate module)

### Risk 4: Error Recovery
**Issue:** `nested_delimiters` may not exist, recovery may work differently

**Mitigation:**
- Research 0.10 recovery strategies early
- Use `skip_parser()` combinator
- Test error recovery explicitly

**Contingency:** Simplify recovery, prioritize correctness

### Risk 5: Operator Precedence Performance
**Issue:** Pratt parser may have different performance

**Mitigation:**
- Benchmark before/after
- Can keep foldl/foldr if needed
- Profile with large expressions

**Contingency:** Stick with foldl/foldr if Pratt regresses

## Key Decision Points

### Decision 1: Pratt Parser vs Manual Precedence

**Question:** Migrate to Pratt parser for expressions?

**Pratt Pros:**
- Cleaner code (~100 lines eliminated)
- Built-in precedence handling
- Likely better performance
- Eliminates complex foldr transformation

**Pratt Cons:**
- New API to learn
- May behave differently
- Migration risk

**Recommendation:** **Try Pratt** - designed exactly for this use case

### Decision 2: Stream vs Slice Input

**Question:** Keep Stream or parse token slices directly?

**Research needed:**
- Does Stream exist in 0.10?
- Performance implications?
- Zero-copy requirements?

**Recommendation:** Research first, decide in Phase 1

### Decision 3: Error Recovery Strategy

**Question:** How to replace `nested_delimiters`?

**Research needed:**
- Available recovery strategies in 0.10
- Can we achieve similar behavior?

**Recommendation:** Research in Phase 1, decide early in Phase 2

## Success Criteria

✅ **All tests passing:**
- Inline parser tests (~30)
- Integration tests
- Error message tests

✅ **No performance regression:**
- Benchmark against 0.9
- Target: <10% regression, ideally improvement

✅ **Error quality maintained:**
- Helpful error messages
- Accurate spans
- Recovery works for IDE use

✅ **Code quality:**
- Cleaner than before (leverage new APIs)
- Well-documented
- No TODO comments

## Key Learnings from Lexer (Reference)

1. **Byte vs char offsets:** SimpleSpan uses byte offsets for `&str` - must convert
2. **Collect required:** `.repeated()` needs `.collect()`
3. **Manual error conversion:** Convert Rich to custom Error manually
4. **Span access:** Use `extra.span()` in `.map_with()`
5. **Recovery at top level:** Not within parsers
6. **Test output:** Use `.output().unwrap()` + dereference

## Quick Reference: Common Migrations

```rust
// Lifetime parameter
fn parser() -> impl Parser<TokenKind, Output, Error = PError>
fn parser<'a>() -> impl Parser<'a, ParserInput<'a>, Output, ParserError<'a>>

// Parse and error handling
parser.parse_recovery(input)
parser.parse(input).into_result()

// Repeated with collect
parser.repeated()
parser.repeated().collect::<Vec<_>>()

// Map with span
.map_with_span(|v, span| ...)
.map_with(|v, extra| { let span = extra.span(); ... })

// Select (may need update)
select! { TokenKind::Ident(id) => id }
// Verify syntax in 0.10

// Foldr (now on IterParser)
parser.repeated().foldr(...)
// Works on IterParser directly
```

## Implementation Summary

### Why All-at-Once Is Required

**Cannot migrate incrementally because:**

1. **Parser modules exchange combinators, not data:** `stmt.rs` imports `expr()`, `ident()`, `pipeline()` - these return Chumsky 0.9 `Parser` traits
2. **Trait incompatibility:** The `Parser` trait in 0.9 vs 0.10 are different types - cannot mix
3. **Circular dependencies:** `recursive()` creates tight coupling within and across modules
4. **Lexer was different:** Lexer could migrate independently because it outputs **data** (`Vec<Token>`), not parser combinators

**Why this approach works despite being all-at-once:**

1. **Strong test coverage:** 101 parser tests + 100+ integration tests catch regressions
2. **Snapshot testing:** `insta` tests show exact behavioral changes
3. **Proven patterns:** Lexer migration provides migration template
4. **Clear interfaces:** Only `TokenKind` crosses module boundaries as data
5. **Fast iteration:** 8-12 hours total (vs weeks of architectural redesign for incremental)

### Critical Success Factors

**Do first:**
- ✅ Prepare tests (add helpers, update docs) - compatible with 0.9
- ✅ Research Phase 0 (Error trait, Stream API, recursion)
- ✅ Create migration branch with empty commit

**During migration:**
- ✅ One module = one commit
- ✅ Run tests after EVERY module (no batching)
- ✅ Manually review ALL snapshot changes (don't blindly accept)
- ✅ Check error message quality (common regression point)

**Don't do:**
- ❌ Skip test validation between modules
- ❌ Batch multiple modules before testing
- ❌ Accept snapshots without review
- ❌ Continue if Stream API is unclear (resolve in Phase 1)

### Migration Checklist (Quick Reference)

**Phase 1 (Research & Prep):** 2-3h
- [ ] Research Error trait, Stream API, recursive(), recovery strategies
- [ ] Create migration branch
- [ ] Prepare test infrastructure

**Phase 2 (Migrate All):** 4-6h
- [ ] Update imports in ALL 6 files simultaneously
- [ ] Update perror.rs, mod.rs, interpolation.rs, types.rs
- [ ] Update expr.rs (the big one - keep foldl/foldr, no Pratt yet)
- [ ] Update stmt.rs

**Phase 3 (Compile):** 1-2h
- [ ] Fix compilation errors iteratively
- [ ] Commit when it builds

**Phase 4 (Test):** 1-2h
- [ ] Run tests, review snapshots
- [ ] Fix test failures
- [ ] Validate error messages

**Phase 5 (Cleanup):** 30m
- [ ] Remove dual dependency
- [ ] Final validation

**Total: 8-12 hours (1-2 focused days)**

### Open Questions (Resolve in Phase 0)

1. ❓ Error trait API in Chumsky 0.10 - does `ChumError<T>` need changes?
2. ❓ Stream API - does `Stream::from_iter()` exist, or do we parse slices?
3. ❓ `recursive()` signature - any lifetime changes?
4. ❓ `nested_delimiters` recovery - exists in 0.10 or use `skip_parser()`?
5. ❓ Pratt parser - API and integration approach?

### Next Immediate Steps

**Right now:**
1. Make test preparation changes (Phase 0 section above)
2. Create migration branch: `git switch -c parser-chumsky-10-migration`
3. Review Chumsky 0.10 documentation for answers to open questions

**Then:**
1. Start Phase 1.1: Migrate perror.rs
2. Test thoroughly
3. Commit and move to Phase 1.2

**Remember:** AD FONTES - reproduce and verify at each step. The test suite is our safety net.
