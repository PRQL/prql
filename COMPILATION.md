# Compilation Challenges with Chumsky 0.11.1

## The Core Problem

Parser combinators create deeply nested generic types. Each combinator adds another layer to the type signature:

```rust
// Simple example - types nest exponentially
let parser = just('a')           // Parser<char, ...>
    .then(just('b'))             // Then<Parser<char>, Parser<char>, ...>
    .or(just('c'))               // Or<Then<...>, Parser<char>, ...>
    .map(|x| x)                  // Map<Or<Then<...>, ...>, Closure, ...>
    .repeated();                 // Repeated<Map<Or<...>, ...>, ...>

// Real parsers in PRQL have 10-15+ levels of nesting
```

When Rust monomorphizes these types, it generates:
1. **Enormous symbol names** (thousands of characters)
2. **Deep template instantiations** (hundreds of levels)
3. **Large amounts of machine code** (each unique type gets its own code)

This hits hard limits in:
- **macOS ld64 linker**: Hard-coded symbol length limit (~10KB)
- **MSVC linker**: Template instantiation depth limits
- **Compilation time**: Each nested generic requires separate codegen

## Why Chumsky 0.11 Makes This Worse

Chumsky 0.11 introduced several changes that increase type complexity:

### 1. New Error Types (Rich)
```rust
// 0.9/0.10: Simple error type
Simple<char>

// 0.11: Rich error with more generic parameters
Rich<'src, Token, Span>
```

### 2. Input Abstraction
```rust
// 0.9/0.10: Direct slice access
&[Token]

// 0.11: Generic input trait with lifetime and span mapping
Input<'src, Token = lr::Token, Span = Span>
```

### 3. Extra Trait
```rust
// 0.11: Additional Extra trait for custom state
extra::Err<Rich<'src, Token, Span>>
```

Each additional generic parameter **multiplies** the symbol length because it appears at every nesting level.

## Current Mitigation Strategies

### 1. Strategic Boxing (Partial Solution)

**What it does**: Converts static generic types to trait objects:
```rust
// Before: Static type, contributes to symbol length
let parser = just('a').then(just('b')).or(just('c'));
// Type: Or<Then<Just<char>, Just<char>>, Just<char>>

// After: Boxed, flattens to trait object
let parser = just('a').then(just('b')).boxed().or(just('c')).boxed();
// Type: Or<Box<dyn Parser<...>>, Box<dyn Parser<...>>>
```

**Current usage**: 18 `.boxed()` calls across 7 files in prqlc-parser

**Effectiveness**:
- ✅ Reduces symbol length by breaking long type chains
- ✅ Helps compilation times slightly
- ⚠️ Doesn't eliminate the problem (still hit limits on macOS)
- ❌ Runtime cost: heap allocation + dynamic dispatch
- ❌ Makes code less readable (requires more explicit types)

**Where we box**:
```bash
prqlc-parser/src/parser/expr.rs:    .boxed()  # 4 times
prqlc-parser/src/parser/stmt.rs:    .boxed()  # 3 times
prqlc-parser/src/parser/pr.rs:      .boxed()  # 8 times
prqlc-parser/src/parser/common.rs:  .boxed()  # 2 times
prqlc-parser/src/parser/mod.rs:     .boxed()  # 1 time
```

**Why we can't box everything**:
1. **Performance**: Boxing every combinator would add significant heap allocation overhead
2. **Ergonomics**: Would require explicit type annotations everywhere
3. **Clone requirements**: Boxed parsers need `Clone`, which requires `dyn-clone` or `Arc`
4. **Doesn't solve the core issue**: Even with boxing, base types are still complex

### 2. Stacker (Not Solving Our Problem)

**What it does**: Increases stack size to prevent stack overflow in deeply recursive code

**Why it doesn't help us**:
- ❌ Our problem is **symbol length** and **type complexity**, not stack depth
- ❌ Stacker is for **runtime** stack overflow, not **compile-time** symbol generation
- ✅ We already have stacker in dependencies (for PRQL runtime recursion, not parsing)

**Evidence**: The linker errors are about symbol table entries, not stack:
```
ld: Assertion failed: (name.size() <= maxLength), function makeSymbolStringInPlace
```

This is a **symbol table** limitation, not a runtime stack issue.

### 3. rust-lld Linker (Current Solution)

**What it does**: Uses LLVM's linker instead of system linker

**Effectiveness**:
- ✅ Works for Windows (MSVC linker has similar limits)
- ✅ Works for macOS non-Python builds (no hard symbol length limit)
- ❌ Doesn't work for macOS Python builds (maturin compatibility)

**Trade-offs**:
- ✅ No code changes required
- ✅ Faster linking (20-50% speedup)
- ⚠️ Requires build configuration (cargo config + env vars for Python)

### 4. v0 Symbol Mangling (Compression)

**What it does**: Uses newer mangling scheme with backreferences

**How it works**:
```
// Legacy mangling - repeats full paths
_ZN3std4iter5chain5Chain17hXXXXXXXXXXXE

// v0 mangling - uses backreferences
_RNvMs_NtNt3std4iter5chain5Chain
```

When types repeat (common in parser combinators), v0 can reference earlier occurrences.

**Effectiveness**:
- ⚠️ **Inconsistent**: Sometimes reduces, sometimes increases symbol length
- ⚠️ Benchmarks show v0 can be **larger** for some codebases (libstd.so: legacy 804KB, v0 858KB)
- ⚠️ Doesn't eliminate the fundamental problem
- ✅ Required for Python bindings on macOS (combined with system linker)

## What Would Actually Fix This

### Option 1: Reduce Parser Complexity (Major Refactoring)

**Strategy**: Break parsers into smaller, reusable pieces with explicit types

```rust
// Current: Deep nesting
pub fn expr() -> impl Parser<...> {
    binary_op()
        .or(unary_op())
        .or(literal())
        .or(ident())
        .or(paren_expr())
        .repeated()
        .map(...)
}

// Better: Explicit intermediate types
type ExprParser<'a> = Box<dyn Parser<'a, ...>>;

pub fn expr() -> ExprParser<'a> {
    let atom = atom_expr();  // Returns ExprParser
    let unary = unary_expr(atom.clone());
    let binary = binary_expr(unary);
    binary
}
```

**Pros**:
- ✅ Fundamentally reduces type complexity
- ✅ Improves compilation times
- ✅ Makes code more maintainable (explicit structure)
- ✅ Eliminates need for linker workarounds

**Cons**:
- ❌ **Major refactoring effort** (weeks of work)
- ❌ Runtime overhead from boxing (heap allocations)
- ❌ Less ergonomic than combinator style
- ❌ May still hit limits on very complex parsers

**Realistic assessment**: This would take 2-4 weeks of focused work to refactor the entire parser.

### Option 2: Use Pratt Parser for Binary Operators (Partially Done)

**What we did**: Replaced manual binary operator parsing with Chumsky's built-in Pratt parser

**Effectiveness**:
- ✅ Reduced type complexity for operator precedence parsing
- ✅ Cleaner code than manual precedence climbing
- ⚠️ Only helps for one part of the parser (binary ops)

**Still needed**: The rest of the parser (statements, types, etc.) still has deep nesting

### Option 3: Wait for Chumsky Improvements

**Chumsky maintainers are aware** of this issue (see Chumsky issues #13, #387)

**Potential improvements**:
1. **Type erasure internally**: Chumsky could box internally where it matters
2. **Simpler trait bounds**: Reduce generic parameter count
3. **Better error types**: Rich errors could be simplified

**Timeline**: Unknown - Chumsky 1.0 is in development but no ETA

### Option 4: Alternative Parser Generator

**Consider switching to**:
- **winnow**: Lower-level, less abstraction, smaller types
- **nom**: Similar to winnow, well-tested
- **hand-written recursive descent**: Ultimate control, no generic explosion

**Trade-offs**:
- ✅ Would eliminate type complexity issues
- ❌ **Massive rewrite** (2-3 months of work)
- ❌ Lose parser combinator ergonomics
- ❌ More code to maintain manually

**Realistic assessment**: Not worth it unless Chumsky becomes unmaintainable

## Why We Can't Just "Box Everything"

The question often arises: "Why not just `.boxed()` every parser?"

### 1. Performance Cost

Each boxed parser requires:
- Heap allocation (small but adds up)
- Dynamic dispatch (prevents inlining, ~5-10% overhead)
- Reference indirection (cache unfriendly)

For a parser that's called millions of times (like identifier parsing), this matters.

### 2. Ergonomics Cost

```rust
// Without boxing - inferred types work
let parser = just("let").then(ident()).then(just("="));

// With boxing - explicit types required
let parser: BoxedParser<'_, _, _> = just("let")
    .boxed()
    .then(ident().boxed())
    .boxed()
    .then(just("=").boxed())
    .boxed();
```

The code becomes significantly harder to read and maintain.

### 3. Still Doesn't Eliminate the Problem

Even with boxing, the **base types** are still complex:
```rust
Box<dyn Parser<'a, Input<'a, Token = lr::Token, Span = Span>,
                Output = Expr,
                Error = extra::Err<Rich<'a, lr::Token, Span>>>>
```

This signature itself contributes to symbol length. Boxing reduces but doesn't eliminate the issue.

### 4. Clone Complexity

Parsers need to be `Clone` (Chumsky's design requirement). Boxing requires:
```rust
Box<dyn Parser + Clone>  // Doesn't work - object safety

// Need special handling:
Arc<dyn Parser>  // Slower, thread-safe overhead
dyn-clone crate  // Additional dependency, boilerplate
```

## Current Strategy: Minimal Configuration

**What we're doing**:
1. **rust-lld globally** - Eliminates symbol length limits for most builds
2. **Strategic boxing** - 18 carefully chosen locations where it helps most
3. **Python-specific workaround** - System linker + v0 mangling only for Python bindings

**Why this is pragmatic**:
- ✅ Minimal code changes (no major refactoring)
- ✅ Minimal performance impact (only 18 boxes)
- ✅ Minimal configuration (cargo config + 2 workflow env vars)
- ✅ Maintainable (well-documented, clear rationale)

## The Real Question: Should We Do More?

### If Compilation Fails on New Platforms

**Symptoms**:
- New linker errors on different architectures
- Symbol length issues on Linux (mold, gold)
- Windows MSVC template depth errors

**Response**:
1. Add rust-lld for that platform in `.cargo/config.toml`
2. If rust-lld doesn't work, add strategic boxing in hot paths
3. Document in ISSUE.md

### If Compilation Times Become Unacceptable

**Current status**: prqlc-parser compiles in ~30s on modern hardware

**If this doubles** (>60s):
1. Profile with `cargo build --timings`
2. Identify which functions cause monomorphization bloat
3. Add boxing to those specific functions
4. Consider splitting into multiple crates

### If Type Inference Breaks

**Symptoms**:
- "Type annotations needed" errors
- Compilation failures after refactoring

**Response**:
- Add explicit type aliases for complex parsers
- Use helper functions to break up chains
- Consider more aggressive boxing in that module

## Recommendations

### Short Term (Current Approach)
✅ Keep using rust-lld + minimal boxing
✅ Document the configuration clearly
✅ Monitor compilation times

### Medium Term (If Needed)
⚠️ Add more strategic boxing if new platforms fail
⚠️ Consider type aliases for commonly used parser types
⚠️ Profile and optimize hot parsing paths

### Long Term (If Critical)
❌ Don't do a major refactoring unless absolutely necessary
❌ Don't switch parser generators without strong evidence
✅ Monitor Chumsky development for built-in solutions

## Conclusion

**The 7x performance improvement from Chumsky 0.11 is worth the compilation complexity.**

The symbol length issue is a **toolchain limitation**, not a fundamental problem with our code. We've solved it with minimal configuration (rust-lld) and strategic boxing (18 locations).

**We should NOT**:
- Box everything (performance cost, ergonomics cost, doesn't fully solve it)
- Rely on stacker (wrong problem - it's for runtime, not compile-time)
- Refactor the entire parser (weeks of work, uncertain benefit)

**We SHOULD**:
- Keep the current minimal configuration
- Monitor for issues on new platforms
- Add boxing tactically if specific functions cause problems
- Document everything clearly (this file!)

The goal is **pragmatic engineering**: solve the problem with minimal cost, monitor, iterate as needed.
