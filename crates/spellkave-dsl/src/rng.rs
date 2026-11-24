pub trait EvaluationRng {
    fn next_u32(&mut self) -> u32;

    /// Resolves a die's final face for a `count`-sides die. Default: draw raw
    /// entropy via `next_u32` and map it onto `1..=sides` (today's `roll()`
    /// formula, unchanged for every fresh-evaluation caller).
    fn roll_die(&mut self, sides: u32) -> u32 {
        (self.next_u32() % sides.max(1)) + 1
    }

    /// True once the most recent `next_u32`/`roll_die` call had no real draw
    /// left to return (ISS1627: a caller under-provisioned its replay stream
    /// relative to what the expression actually consumes). Default `false` —
    /// a live/unbounded RNG never runs dry.
    fn exhausted(&self) -> bool {
        false
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum DrawSource {
    /// `draws` holds raw entropy words; `roll_die` maps each through the
    /// sides modulo, same as a live `ctx.rng()` draw would.
    RawEntropy,
    /// `draws` holds already-resolved die faces (`StochasticDraw.value`,
    /// e.g. 20 for a natural 20) recorded by a prior evaluation; `roll_die`
    /// returns them verbatim instead of re-deriving via modulo.
    RecordedValues,
    /// `draws` holds already-resolved faces EXCEPT the one position at the
    /// contained index, which holds fresh raw entropy: the post-roll
    /// intervention replay (one die replaced, every other face preserved
    /// byte-identical). Feeding a resolved face back through the raw-entropy
    /// modulo corrupts it — `(face % sides) + 1` shifts every face but the
    /// maximum (#2059) — so the two sources must be split per index.
    RecordedValuesWithEntropyAt(usize),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeterministicRng {
    draws: Vec<u32>,
    next_index: usize,
    source: DrawSource,
    exhausted: bool,
}

impl DeterministicRng {
    pub fn new(draws: Vec<u32>) -> Self {
        Self {
            draws,
            next_index: 0,
            source: DrawSource::RawEntropy,
            exhausted: false,
        }
    }

    /// Replays draws whose `value` has already been resolved to a die face
    /// by an earlier evaluation (e.g. an item-source affordance predicate
    /// being revalidated from its recorded `StochasticDraw`s) rather than
    /// raw `ctx.rng()` entropy.
    pub fn from_recorded_values(values: Vec<u32>) -> Self {
        Self {
            draws: values,
            next_index: 0,
            source: DrawSource::RecordedValues,
            exhausted: false,
        }
    }

    /// Replay stream for a single-die replacement (#2059): every position
    /// holds a recorded face returned verbatim, EXCEPT `entropy_index`,
    /// which holds fresh raw entropy and resolves via the sides modulo at
    /// draw time (the die's side count is only known then).
    pub fn from_recorded_values_with_entropy_at(values: Vec<u32>, entropy_index: usize) -> Self {
        Self {
            draws: values,
            next_index: 0,
            source: DrawSource::RecordedValuesWithEntropyAt(entropy_index),
            exhausted: false,
        }
    }
}

impl EvaluationRng for DeterministicRng {
    fn next_u32(&mut self) -> u32 {
        let value = match self.draws.get(self.next_index).copied() {
            Some(value) => value,
            None => {
                self.exhausted = true;
                0
            }
        };
        self.next_index += 1;
        value
    }

    fn roll_die(&mut self, sides: u32) -> u32 {
        match self.source {
            DrawSource::RawEntropy => (self.next_u32() % sides.max(1)) + 1,
            DrawSource::RecordedValues => self.next_u32(),
            DrawSource::RecordedValuesWithEntropyAt(entropy_index) => {
                let is_entropy = self.next_index == entropy_index;
                let value = self.next_u32();
                if is_entropy {
                    (value % sides.max(1)) + 1
                } else {
                    value
                }
            }
        }
    }

    fn exhausted(&self) -> bool {
        self.exhausted
    }
}
