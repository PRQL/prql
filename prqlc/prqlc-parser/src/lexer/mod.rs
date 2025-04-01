#[cfg(not(feature = "chumsky-10"))]
mod chumsky_0_9;

#[cfg(feature = "chumsky-10")]
// Phase II in progress: Setting up combinator structure
mod chumsky_0_10;

pub mod lr;
#[cfg(test)]
mod test;

// Re-export the implementation based on the feature flag
#[cfg(not(feature = "chumsky-10"))]
pub use chumsky_0_9::*;

#[cfg(feature = "chumsky-10")]
pub use chumsky_0_10::*;