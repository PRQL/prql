# Span Propagation Research: Fixing "Unknown name" Error Regression

## Problem Summary

After the Chumsky 0.10 migration, the "Unknown name" error lost its span
information:

**Before migration:**

```
Error:
   ╭─[ :3:21 ]
   │
 3 │     select lower f"{x}/{y}"
   │                     ┬
   │                     ╰── Unknown name `x`
───╯
```

**After migration:**

```
Error: Unknown name `x`
```

## Root Cause

The issue is in `prqlc/prqlc/src/semantic/resolver/expr.rs:78`:

```rust
let fq_ident = self.resolve_ident(&ident).with_span(node.span)?;
```

When `resolve_ident()` fails, it returns an error **without** a span. The
`.with_span(node.span)` is only applied to the success case, not the error case.

The error is generated in `prqlc/prqlc/src/semantic/resolver/names.rs:138-140`:

```rust
Err(Error::new_simple(
    format!("Unknown name `{}`", &ident).to_string(),
))
```

## Architecture Context

### Data Flow

1. **Parser AST** (`prqlc-parser`):
   - `Expr` has `span: Option<Span>` (at line 34 of `parser/pr/expr.rs`)
   - `ExprKind::Ident(Ident)` - the `Ident` itself has NO span
   - Span is stored at the `Expr` level, not the `Ident` level

2. **Semantic Resolution** (`prqlc/src/semantic/resolver`):
   - `resolve_expr()` receives `Expr` with span
   - Extracts `Ident` from `ExprKind::Ident`
   - Calls `resolve_ident(&ident)` - **loses span information**
   - Only attaches span on success via `.with_span(node.span)`

### Key Files

- `prqlc/prqlc-parser/src/parser/pr/ident.rs` - `Ident` struct (no span field)
- `prqlc/prqlc-parser/src/parser/pr/expr.rs` - `Expr` struct (has span field)
- `prqlc/prqlc/src/semantic/resolver/expr.rs:76-94` - Ident resolution call site
- `prqlc/prqlc/src/semantic/resolver/names.rs:15-143` - `resolve_ident()`
  function
- `prqlc/prqlc/src/semantic/resolver/names.rs:138-140` - Error creation site

## Options for Fix

### Option 1: Map Error at Call Site (SIMPLEST - RECOMMENDED)

**Approach:** Use `.map_err()` to attach span after `resolve_ident()` fails.

**Implementation:**

```rust
// In prqlc/prqlc/src/semantic/resolver/expr.rs:78
let fq_ident = self.resolve_ident(&ident)
    .map_err(|e| e.with_span(node.span))?;
```

**Pros:**

- ✅ Minimal change (1 line)
- ✅ No API changes
- ✅ No cascading modifications
- ✅ Fixes the immediate issue
- ✅ Easy to test
- ✅ Low risk

**Cons:**

- ⚠️ Same pattern may exist elsewhere (need to audit other call sites)
- ⚠️ Doesn't prevent future occurrences

**Impact:** Very low - single line change at call site

**Recommendation:** **START HERE**. This is the quickest fix that solves the
regression.

---

### Option 2: Thread Span Through resolve_ident

**Approach:** Add `span` parameter to `resolve_ident()` and attach it at error
creation.

**Implementation:**

```rust
// In names.rs
pub(super) fn resolve_ident(
    &mut self,
    ident: &Ident,
    span: Option<Span>  // NEW parameter
) -> Result<Ident, Error>

// At error site (line 138-140):
Err(Error::new_simple(
    format!("Unknown name `{}`", &ident)
).with_span(span))

// Update call sites:
// In expr.rs:78
let fq_ident = self.resolve_ident(&ident, node.span)?;
```

**Pros:**

- ✅ Centralized error creation with span
- ✅ More explicit API - caller must provide span
- ✅ Consistent span handling for all error paths

**Cons:**

- ⚠️ Requires updating ALL call sites (found 3 in grep results)
- ⚠️ API breaking change for internal function
- ⚠️ More test updates needed

**Impact:** Medium - requires updating 3+ call sites and their tests

**Recommendation:** Good for preventing recurrence, but more work than Option 1.

---

### Option 3: Add Span to Ident Struct

**Approach:** Add `span: Option<Span>` field to the `Ident` struct itself.

**Implementation:**

```rust
// In prqlc/prqlc-parser/src/parser/pr/ident.rs
pub struct Ident {
    pub path: Vec<String>,
    pub name: String,
    pub span: Option<Span>,  // NEW field
}
```

**Pros:**

- ✅ Span always available with Ident
- ✅ Most semantically correct solution
- ✅ Prevents all similar issues

**Cons:**

- ❌ **MAJOR** architectural change
- ❌ Touches parser AND semantic analyzer
- ❌ Requires updating:
  - Parser (ident creation ~20+ sites)
  - Semantic resolver (ident manipulation)
  - All tests that construct Idents
  - Serialization/deserialization
- ❌ Ident is cloned/manipulated frequently (`.prepend()`, `.pop()`, etc.)
- ❌ Need to decide span semantics for derived Idents
- ❌ High risk of bugs during migration

**Impact:** Very high - cascades through entire codebase

**Recommendation:** **AVOID**. Too invasive for fixing one error message.

---

### Option 4: Context Object with Span Stack

**Approach:** Maintain a stack of spans in the resolver context.

**Implementation:**

```rust
impl Resolver {
    fn with_span<T>(&mut self, span: Option<Span>, f: impl FnOnce(&mut Self) -> Result<T, Error>) -> Result<T, Error> {
        self.span_stack.push(span);
        let result = f(self).map_err(|e| e.with_span(span));
        self.span_stack.pop();
        result
    }
}

// Usage:
self.with_span(node.span, |r| r.resolve_ident(&ident))?
```

**Pros:**

- ✅ Automatic span attachment for any errors
- ✅ No API changes to resolve_ident
- ✅ Works for deeply nested errors

**Cons:**

- ⚠️ More complex control flow
- ⚠️ Overhead of maintaining span stack
- ⚠️ May attach wrong span if nesting is incorrect
- ⚠️ Debugging becomes harder

**Impact:** Medium - adds complexity to resolver

**Recommendation:** Overengineered for this problem. Consider for future
refactoring.

---

### Option 5: Result Extension Trait

**Approach:** Add a trait extension for attaching spans to Results.

**Implementation:**

```rust
trait ResultExt<T, E> {
    fn with_span_on_err(self, span: Option<Span>) -> Result<T, Error>;
}

impl<T> ResultExt<T, Error> for Result<T, Error> {
    fn with_span_on_err(self, span: Option<Span>) -> Result<T, Error> {
        self.map_err(|e| e.with_span(span))
    }
}

// Usage:
let fq_ident = self.resolve_ident(&ident).with_span_on_err(node.span)?;
```

**Pros:**

- ✅ Ergonomic API
- ✅ Reusable pattern
- ✅ Self-documenting code
- ✅ Easy to apply consistently

**Cons:**

- ⚠️ Still requires updating call sites
- ⚠️ Adds new pattern to learn

**Impact:** Low-Medium - cleaner version of Option 1

**Recommendation:** Nice refinement if doing multiple fixes. Overkill for single
site.

---

## Audit: Other Call Sites

Found 3 call sites of `resolve_ident`:

1. **names.rs:37** - Resolving imports

   ```rust
   return self.resolve_ident(&target);
   ```

   - Already within error handling context
   - **Needs investigation**: Does this need span too?

2. **expr.rs:24** - Type checking context

   ```rust
   let fq_ident = self.resolve_ident(&ident)?;
   ```

   - **Needs investigation**: Check if this has span available

3. **expr.rs:78** - Main expression resolution (THE BUG)

   ```rust
   let fq_ident = self.resolve_ident(&ident).with_span(node.span)?;
   ```

   - **This is the reported bug**

**Action:** Need to check if lines 24 and 37 also lose span information.

---

## Recommendation: Phased Approach

### Phase 1: Quick Fix (Option 1)

**Time:** 15 minutes

1. Apply `.map_err(|e| e.with_span(node.span))` at expr.rs:78
2. Test with the failing case
3. Update snapshot
4. Commit

### Phase 2: Audit (30 minutes)

1. Check other 2 call sites (lines 24, 37)
2. Determine if they need span fixes
3. Apply same pattern if needed

### Phase 3: Consider Cleanup (Optional, 1-2 hours)

If multiple sites need fixing:

1. Implement Option 5 (trait extension)
2. Apply consistently across all sites
3. Document pattern in code comments

### Phase 4: Future Work (Track but don't implement now)

For next major refactor:

- Consider Option 2 (thread span through function)
- Better yet: Consider redesigning error handling to use a span context
- Document pattern in ARCHITECTURE.md

---

## Test Plan

### Minimal Test (Option 1)

```rust
#[test]
fn test_unknown_name_with_span() {
    let result = compile(r#"
        from foo
        select x
    "#);

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("Unknown name `x`"));
    assert!(err.to_string().contains("3:16")); // Check span present
}
```

### Integration Test

Use the existing `bad_error_messages.rs::select_with_extra_fstr` test - it
should show proper span after fix.

---

## Implementation Steps (Option 1 - Recommended)

1. **Apply fix:**

   ```bash
   # Edit prqlc/prqlc/src/semantic/resolver/expr.rs:78
   # Change:
   let fq_ident = self.resolve_ident(&ident).with_span(node.span)?;
   # To:
   let fq_ident = self.resolve_ident(&ident)
       .map_err(|e| e.with_span(node.span))?;
   ```

2. **Test:**

   ```bash
   cargo insta test --accept -p prqlc --test bad_error_messages
   ```

3. **Review snapshot:**
   - Should show structured error with span
   - Move test from `bad_error_messages.rs` to `error_messages.rs`

4. **Commit:**

   ```
   fix: Restore span information for "Unknown name" errors

   After Chumsky 0.10 migration, semantic resolution errors lost
   span information because .with_span() was only applied on success.

   Fix: Use .map_err() to attach span when resolve_ident() fails.
   ```

5. **Audit other sites:**
   - Check expr.rs:24 and names.rs:37
   - Apply same fix if needed

---

## Impact Assessment

**Option 1 (Recommended):**

- Files changed: 1
- Lines changed: 2
- Tests affected: 1 snapshot
- Risk: Very low
- Time: 15 minutes

**Option 2:**

- Files changed: 2-3
- Lines changed: 10-15
- Tests affected: Multiple
- Risk: Low-medium
- Time: 1-2 hours

**Option 3:**

- Files changed: 20+
- Lines changed: 100+
- Tests affected: Many
- Risk: High
- Time: 8+ hours

---

## Conclusion

**Recommended approach:** Start with **Option 1** (map_err at call site).

It's the simplest, lowest-risk fix that solves the immediate regression. If we
find multiple sites with the same issue during audit, we can refactor to Option
5 (trait extension) for consistency.

Option 3 (adding span to Ident) is the "right" architectural solution, but it's
too invasive for a bug fix. Consider it for a future major refactoring when
redesigning the semantic analyzer.
