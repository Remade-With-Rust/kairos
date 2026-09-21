//! Umbrella-only: proof that the Kairos K7 packages COMPOSE.
//!
//! Each package is gated on its own against its own C oracle. That says
//! nothing about whether they fit together: two arms can each agree with
//! their own reference and still disagree at the seam between them.
//!
//! The tests here run one package over another for real. There is no
//! library surface; everything is in `tests/`.
